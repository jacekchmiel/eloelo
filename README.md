# EloElo

Create balanced teams.

## Dev guide

Prerequisites:

- Rust (<https://rustup.rs/>)
- Node/Npm (e.g. with manager <https://github.com/Schniz/fnm>)

```shell
git clone git@github.com:jacekchmiel/eloelo.git
# Build UI
cd ui && npm install && npm run build
# Run server
cd .. && cargo run
```

## DotA agent

Supplements main EloElo app with automated screenshot analysis to determine who's in lobby.