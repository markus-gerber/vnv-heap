#!/bin/sh

echo "Execute ./run_checked_output.sh for $1"
timeout $1 ./run_checked_output.sh

EXITCODE=$?
if [ $EXITCODE -ne 124 ]
then
    echo "Command failed!"
    exit 1
fi
exit 0
