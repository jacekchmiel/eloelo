#!/bin/bash

cargo watch -w src -w Cargo.toml -w crates -w "$HOME"/.local/share/eloelo/config.yaml -x check -x build -s 'touch .trigger'

