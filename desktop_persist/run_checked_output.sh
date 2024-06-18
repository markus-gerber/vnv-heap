if [ -n "$1" ]; then
    python3 test_persist.py $1 | python3 check_output.py
else
    python3 test_persist.py | python3 check_output.py
fi

