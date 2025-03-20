# Copyright (C) 2025  Markus Elias Gerber
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

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
