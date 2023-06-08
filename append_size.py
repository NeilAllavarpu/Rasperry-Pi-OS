import sys
import os

def to_u32(x):
    for i in range(4):
        yield x % 256
        x //= 256

with open(sys.argv[1], "ab") as outfile:
    size = os.stat(sys.argv[2]).st_size
    with open(sys.argv[2], "rb") as infile:
        outfile.write(bytes(to_u32(size)))
        outfile.write(infile.read())
