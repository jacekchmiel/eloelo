#!/bin/bash

touch .trigger
cargo watch --no-vcs-ignores -w .trigger -x run

