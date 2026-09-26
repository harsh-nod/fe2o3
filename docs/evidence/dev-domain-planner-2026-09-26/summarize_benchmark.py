#!/usr/bin/env python3
"""Summarize the alternating CPU planner measurements, not native performance."""
import csv
import math
from pathlib import Path
import statistics
import sys

rows = {}
with Path(sys.argv[1]).open() as source:
    for row in csv.reader(source):
        if len(row) != 5 or not row[0].isdigit():
            continue
        depth, count, iteration = map(int, row[:3])
        old, new = map(float, row[3:])
        assert all(math.isfinite(value) and value > 0 for value in (old, new))
        key = depth, count, iteration
        assert key not in rows
        rows[key] = old, new
assert set(rows) == {(depth, count, iteration) for depth in range(1, 5)
                     for count in (1, 2, 16, 1024, 65536) for iteration in range(9)}
print('depth\tcount\told_median_ns\tnew_median_ns\tpaired_median_ratio\tpaired_min_ratio\tpaired_max_ratio')
for depth in range(1, 5):
    for count in (1, 2, 16, 1024, 65536):
        samples = [rows[depth, count, iteration] for iteration in range(9)]
        ratios = [old / new for old, new in samples]
        print(f'{depth}\t{count}\t{statistics.median(old for old, _ in samples):.2f}\t'
              f'{statistics.median(new for _, new in samples):.2f}\t{statistics.median(ratios):.3f}\t'
              f'{min(ratios):.3f}\t{max(ratios):.3f}')
