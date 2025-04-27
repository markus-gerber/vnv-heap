#!/usr/bin/env bash

if [ -z "$DOCKER" ]; then
    echo "Note: Not running inside docker container. Manually make sure all required packages are installed."
fi

SCRIPT_DIR=$(realpath $(dirname ${BASH_SOURCE[0]}))

jupyter nbconvert --to notebook --inplace --execute ${SCRIPT_DIR}/../evaluation/get_ref.ipynb
jupyter nbconvert --to notebook --inplace --execute ${SCRIPT_DIR}/../evaluation/kvs.ipynb
jupyter nbconvert --to notebook --inplace --execute ${SCRIPT_DIR}/../evaluation/persist.ipynb
jupyter nbconvert --to notebook --inplace --execute ${SCRIPT_DIR}/../evaluation/queue.ipynb

PLOT_DIR=$(realpath ${SCRIPT_DIR}/../evaluation/figures)

echo ""
echo "All plot notebooks executed successfully!"
echo "The plots were saved to ${PLOT_DIR}!"
echo "Tip: You also can access them in the vnv_heap repository of the host machine."
