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

## Discord bot

By default the Discord bot runs inside the main server. It can also run as a standalone server
(`elodisco` binary) that stays up all the time; the main server then connects to it over a WebSocket.

Main server `config.yaml`:
```yaml
discordBot:
  mode: remote # embedded (default), remote or disabled
  url: ws://bot-host:3001
bridgeSecret: some-long-random-string
```

Bot server `config.yaml` (Discord settings as usual, plus):
```yaml
bridgeSecret: some-long-random-string # same as above
bridgeListenAddr: 0.0.0.0:3001 # default
```

```shell
cargo run --release --bin elodisco
```

The main server keeps reconnecting while the bot server is unreachable; the UI app bar shows
"Discord bot offline" until the connection is back.

Hero bans/allowlists (`discord_bot_state.yaml`) live next to the bot, so move the file to the bot
server's data dir when switching to remote mode.

## DotA agent

Supplements main EloElo app with automated screenshot analysis to determine who's in lobby.