# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Keep this file current: when a change makes something here wrong (commands, config keys, message flow, module
roles), update this file in the same change.

EloElo creates balanced teams for a group of players (mostly DotA 2), tracks match history and computes ELO
from it. A Rust server (axum) serves a React/MUI UI and optionally runs a Discord bot.

## Commands

Backend (Cargo workspace: root crate `eloelo` + `crates/eloelo_model`, `crates/spawelo`):

```shell
cargo run                                  # main server (default-run = eloelo), UI at http://localhost:3000/ui
cargo run --release --bin elodisco         # standalone Discord bot server
cargo test --workspace                     # what CI runs
cargo test -p spawelo <test_name_substr>   # single test / module, e.g. `cargo test -p eloelo hero_assignment_strategy`
cargo fmt --check                          # part of ./pre-commit
bacon run-long                             # backend watch mode (bacon `c` = clippy-all, `r` = run-long)
```

UI (the `package.json`, `vite.config.ts` and `biome.json` are at the repo root, sources in `ui/src`, build output in
`ui/dist`, which the server serves as static files). Pre-commit and CI run these from `ui/`:

```shell
npm install && npm run build   # tsc + vite build -> ui/dist; must be rebuilt for the server to pick up UI changes
npm run watch                  # dev-mode build in watch mode (use with bacon run-long)
npm run check                  # biome lint + format check (check:fix to apply)
npx vitest run ui/src/Duration.test.ts   # single UI test
```

`./pre-commit` = `cargo fmt --check` + `npm run check`. CI (`.github/workflows`) runs `cargo check`/`cargo test --workspace`
on Rust changes and `npm run check`/`test`/`build` on `ui/` changes.

## Architecture

### Message bus

Everything in the server talks over a single `tokio::broadcast` `MessageBus` (`src/eloelo/message_bus.rs`). A
`Message` is one of `UiCommand` (requests to change state), `UiUpdate` (state pushed to the UI) or `Event` (things
that happened: match start/finish, heroes assigned, Discord bot status, …). Components subscribe and filter:

- `api.rs` — axum HTTP API under `/ui/api/v1`. Each POST endpoint only turns its body into a `UiCommand` on the bus;
  results reach the browser through the `/ui_stream` WebSocket, which forwards `UiUpdate`s. `/api/v1/dota_screenshot`
  receives screenshots from `dota-agent`.
- `EloElo` (`src/eloelo.rs`) — the core state machine: players, teams, lobby, game state, history. `dispatch_messages`
  handles `UiCommand`s, persists state and publishes `UiState`. Team balancing and ELO come from the `spawelo` crate
  (`ml_elo` recomputes ELO from the whole history).
- `elodisco` (`src/eloelo/elodisco/`) — Discord bot (poise/serenity): notifications/DMs to players, hero assignment and
  rerolls (`dota_bot.rs`, `hero_assignment_strategy/`), slash commands in `elodisco.rs`. Its own state (hero bans,
  allowlists) is `discord_bot_state.yaml`.
- `ocr.rs` — runs `dota-ocr-engine` (Python, `uv run dota-ocr-engine.py`, needs tesseract) on screenshots to detect
  who is in the lobby. `fosiaudio.rs` calls an external "autogrzybke" HTTP service to announce missing players.

Subscribe before sending startup messages (see `main.rs`) — a broadcast receiver created later misses them.

### Discord bot modes

`config.yaml` `discordBot.mode`: `embedded` (bot runs in-process on the same bus), `remote`, or `disabled`. In remote
mode `bridge.rs` connects the local bus to the bus of a standalone `elodisco` server (`src/bin/elodisco.rs`) over a
WebSocket authenticated with the secret in the `bridge_secret` file. Messages cross unchanged as JSON; which ones cross is decided only by the
`to_bot`/`to_eloelo` filters in `bridge.rs`, which must not overlap or messages bounce between buses. The bot server
serves one eloelo at a time, follows the test mode the client sends in the handshake, and only accepts clients once the
bot is ready. The UI shows bridge health via `DiscordBotStatus`. See README for config and `./deploy-elodisco.sh`.

eloelo owns the players. It sends its `PlayerDb` (`Event::PlayerDbUpdated`) whenever the bot becomes reachable
(`DiscordBotStatus` `Connected`/`Embedded`) and when players change; the bot keeps the latest copy (also from
`MatchStart`) and persists it in `discord_bot_state.yaml`, so player id <-> Discord username mapping works while eloelo
is offline.

### Test mode

`testMode` in eloelo's `config.yaml` makes the app safe to try out: the shared history repo is never touched, and the
bot uses `discordTestChannelName` / DMs only `discordTestModePlayers`. It is shown in the UI. Keep new side effects
(Discord messages, history writes, external calls) gated on it. It doesn't affect the eloelo and elodisco storages:
config, players, options and players' bot data are real either way.

### Data and persistence

All runtime data lives in the platform data dir (`directories::ProjectDirs`, on Linux `~/.local/share/eloelo/`, or
`$XDG_DATA_HOME/eloelo`); `store.rs` loads and saves it:

- `eloelo/` — eloelo's `config.yaml`, `players.yaml`, `options.yaml`. Pushed on exit.
- `elodisco/` — the bot's `config.yaml` (`EloDiscoConfig`) and `discord_bot_state.yaml`. Pushed hourly and on exit.
- `history/` — `<game>.history.json`. Committed and pushed on every new match (not opened in test mode).
- `discord_bot_token`, `bridge_secret` — secrets, one per file, never in git.
- `state.yaml` (current teams/lobby), `logs/` — not backed up.

Each of the three directories is a `GitStorage` (`git_storage.rs`): a clone of its own backup repo. Opening it pulls
(`--ff-only`), commits changes left on disk and pushes; any git failure stops the app. `store()` only writes files,
`push()` commits and pushes. A directory without its own `.git` works as plain files with a warning — the escape hatch
for running without backups. Broken config or data files stop the app instead of falling back to defaults that would
overwrite them. `local_config/`, if present, is a gitignored symlink to the data dir for quick access — it may not
exist on every checkout, so never rely on it from code or scripts.

Config structs use `#[serde(rename_all = "camelCase", deny_unknown_fields)]` with defaults (`config.rs`,
`elodisco/config.rs`); add a default for every new field so existing config files keep loading, and put each field in
the config of the app that uses it.

### Shared model and options UI

`crates/eloelo_model` holds the types shared by the server and `spawelo` (players, history, `Decimal`, options).
User-tunable options (`EloEloOptions`, `SpaweloOptions`) describe themselves via `to_described_options_group_vec`, and
`ui/src/views/OptionsView.tsx` renders the settings form generically from those descriptions, so adding an option
needs no UI code. UI types matching the Rust messages are in `ui/src/model.tsx`, parsing in `ui/src/parse.tsx`.

### Python helpers

`dota-agent/` runs on one player's PC: it watches the DotA screenshots folder and POSTs new lobby screenshots to
eloelo, which marks the players found there as present in the lobby. `./deploy-dota-agent.sh` packs it into
`ui/dist/dota-agent.zip`, served by eloelo for download. `dota-ocr-engine/` is invoked by the server to parse the
screenshots. Both use `uv`.
