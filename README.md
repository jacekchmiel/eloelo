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
discordTestChannelName: bot-testing # used while the connected main server is in test mode
discordTestModePlayers: [player1] # the only players getting DMs in test mode
```

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

Hero bans/allowlists (`discord_bot_state.yaml`) live next to the bot, so move the file to the bot
server's data dir when switching to remote mode.

## DotA agent

Supplements main EloElo app with automated screenshot analysis to determine who's in lobby.