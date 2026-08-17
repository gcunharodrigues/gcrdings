#!/usr/bin/env bash
# Lane entry: atomically claim the next unblocked `hotfix` ticket (FIFO by id).
# Exit 1 = lane idle; the run simply ends with nothing touched.
exec python3 ~/.agents/adw/board.py claim-next hotfix
