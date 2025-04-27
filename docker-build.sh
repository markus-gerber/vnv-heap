#!/usr/bin/env bash
set -euo pipefail
bash -n "$(command -v "$0")"
shopt -s nullglob
set -x

PROJECT_DIR=$(realpath $(dirname ${BASH_SOURCE[0]}))
RUST_TOOLCHAIN=$(cat ${PROJECT_DIR}/rust-toolchain)

docker build -t $1 \
    --build-arg USER=$USER --build-arg UID=$(id -u $USER) \
    --build-arg HOME=$HOME \
    --build-arg RUST_TOOLCHAIN=$RUST_TOOLCHAIN \
    $PROJECT_DIR
