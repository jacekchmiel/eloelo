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

## DotA agent

Supplements main EloElo app with automated screenshot analysis to determine who's in lobby.