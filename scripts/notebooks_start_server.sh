#!/usr/bin/env bash

if [ -z "$DOCKER" ]; then
    echo "Note: Not running inside docker container. Manually make sure all required packages are installed."
fi

SCRIPT_DIR=$(realpath $(dirname ${BASH_SOURCE[0]}))

jupyter lab --notebook-dir=${SCRIPT_DIR}/../evaluation/
