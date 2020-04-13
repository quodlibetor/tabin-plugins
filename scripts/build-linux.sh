#!/usr/bin/env bash

docker run --rm --user "$(id -u)":"$(id -g)" -it -v "$PWD":/build -w /build rust:1 cargo "$@"
