#!/bin/bash

cargo watch -x check -x build -s 'touch .trigger'

