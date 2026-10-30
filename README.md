# EloElo

Create balanced teams.

## Dev guide

### Prerequisites:

- Rust (<https://rustup.rs/>)
- Node/Npm (e.g. with manager <https://github.com/Schniz/fnm>)

#### Nice to haves:

- Bacon (<https://dystroy.org/bacon>): `cargo install bacon`

### How to run (simple)
```shell
git clone git@github.com:jacekchmiel/eloelo.git
# Build UI
cd ui && npm install && npm run build
# Run server
cd .. && cargo run
```

### How to run in watch mode

```shell
bacon run-long # backend
```

```shell
npm run watch # ui
```

## Data

Everything lives in the data dir, `~/.local/share/eloelo/` on Linux (`$XDG_DATA_HOME/eloelo`):

| Path | Contents | Backup |
|------|----------|--------|
| `eloelo/` | `config.yaml`, `players.yaml`, `options.yaml` | git, pushed on exit |
| `elodisco/` | Discord bot `config.yaml`, `discord_bot_state.yaml` | git, pushed every hour and on exit |
| `history/` | match history (`<game>.history.json`) | git, pushed after every match, untouched in test mode |
| `discord_bot_token`, `bridge_secret` | secrets, one per file | none |
| `state.yaml`, `logs/` | current teams and lobby, logs | none |

Each git-backed directory is a clone of its own backup repository. On start the app pulls it
(fast-forward only), commits changes left on disk and pushes, and stops if any of that fails. To
run without backups, e.g. without access to the repository, move the directory's `.git` away: a
directory that isn't a git repository is used as plain files, with a warning.

eloelo only uses `elodisco/` and `discord_bot_token` with an embedded bot, and `bridge_secret` with
a remote one.

## Discord bot

By default the Discord bot runs inside the main server. It can also run as a standalone server
(`elodisco` binary) that stays up all the time; the main server then connects to it over a WebSocket.

Main server `eloelo/config.yaml`:
```yaml
discordBot:
  mode: remote # embedded (default), remote or disabled
  url: ws://bot-host:3001
```

Bot server `elodisco/config.yaml` (Discord settings as usual, plus):
```yaml
bridgeListenAddr: 0.0.0.0:3001 # default
discordTestChannelName: bot-testing # used while the connected main server is in test mode
discordTestModePlayers: [player1] # the only players getting DMs in test mode
```

Both sides need the same secret in the `bridge_secret` file, and the bot needs its token in
`discord_bot_token` (see [Data](#data)).

The bot follows the `testMode` of the main server, which sends it when connecting. The bot server's
own `testMode` only sets the mode until a main server connects.

```shell
cargo run --release --bin elodisco
```

To deploy it to a host running it as the `elodisco` systemd service (binary in
`/var/www/elodisco/`), built locally:

```shell
./deploy-elodisco.sh HOST           # build, back up the current binary, install, restart
./deploy-elodisco.sh HOST rollback  # restore the binary from before the last deploy
```

The main server keeps reconnecting while the bot server is unreachable; the Discord icon in the UI
app bar is red until the connection is back (hover it for the error).

The bot server serves one main server at a time. Another one trying to connect meanwhile is
rejected ("Another eloelo is already connected") and keeps retrying until the first disconnects.

The bot's config and its players' data (hero bans, allowlists) live in `elodisco/` where the bot
runs, so move that directory to the bot server when switching to remote mode.

## DotA agent

Supplements main EloElo app with automated screenshot analysis to determine who's in lobby.