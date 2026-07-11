#!/usr/bin/env python3
"""Extract the data_table array from banner.c and write as binary."""

import re
import sys
import os

# Read the C source
c_path = os.path.join(os.path.dirname(__file__), '..', 'games', 'banner', 'banner.c')
with open(c_path, 'r') as f:
    content = f.read()

# Find the data_table array
# Pattern: static const unsigned char data_table[NBYTES] = { ... };
match = re.search(r'static const unsigned char data_table\[NBYTES\]\s*=\s*\{(.*?)\};', content, re.DOTALL)
if not match:
    print("ERROR: Could not find data_table in banner.c", file=sys.stderr)
    sys.exit(1)

array_text = match.group(1)

# Extract all numbers
numbers = []
for m in re.finditer(r'(\d+)', array_text):
    val = int(m.group(1))
    if 0 <= val <= 255:
        numbers.append(val)

print(f"Found {len(numbers)} bytes in data_table")

if len(numbers) != 9271:
    # Some values may be >255 in the C code (like 129, 227 etc.) - these ARE bytes
    # The C array is unsigned char, so values up to 255 are fine
    # 129, 227 etc are just decimal byte values
    print(f"WARNING: Expected 9271 bytes, got {len(numbers)}")

# Write as binary
out_dir = os.path.join(os.path.dirname(__file__), '..', 'rust', 'gertoys', 'data')
os.makedirs(out_dir, exist_ok=True)

out_path = os.path.join(out_dir, 'banner_table.bin')
with open(out_path, 'wb') as f:
    f.write(bytes(numbers))

print(f"Written {len(numbers)} bytes to {out_path}")
