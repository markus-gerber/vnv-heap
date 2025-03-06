import sys

rec_counter = 0
queued_counter = 0
state = 0

try:
    for line in sys.stdin:
        print(line.rstrip("\n"))

        if "persist was triggered" in line:
            if state == 2:
                continue

            if state != 0:
                raise "invalid state"
    
            state = 1
            rec_counter += 1

        if "persist was queued! persist now..." in line:
            if state != 1:
                raise "invalid state"

            state = 2
            queued_counter += 1

        if "finished clearing buffer" in line:
            if state != 2 and state != 1:
                raise "invalid state"

            state = 3

        if "restore finished" in line:
            if state != 3:
                raise "invalid state"

            state = 0

except KeyboardInterrupt:
    pass

print()
print("########## STATISTICS ##########")
print(f"Total Recoveries Executed: {rec_counter}")
print(f"Recoveries Queued: {queued_counter}")
print("################################")
