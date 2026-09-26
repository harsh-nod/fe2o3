#!/usr/bin/env python3
"""Independent full-byte oracle using an exact modular geometric sum."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent
MODULUS = 1 << 32
MULTIPLIER = 1664525
INCREMENT = 1013904223
ITERATIONS = {"short": 257, "long": 33554433}
SIZE = 384


def advance(value, count):
    # Reduce modulo (a-1)*M before exact division, not modulo M: a-1 has no
    # multiplicative inverse modulo 2^32. This differs from Rust's affine power.
    residue = pow(MULTIPLIER, count, (MULTIPLIER - 1) * MODULUS)
    geometric_sum = (residue - 1) // (MULTIPLIER - 1)
    return (residue * value + INCREMENT * geometric_sum) % MODULUS


def initial():
    words = [(0xd15ea5ed ^ (i * 0x01010101)) % MODULUS for i in range(96)]
    words[16:80] = [((i * 0x045d9f3b) ^ 0xa5a55a5a) % MODULUS for i in range(64)]
    return struct.pack("<96I", *words)


def expected(variant, count=None):
    words = list(struct.unpack("<96I", initial()))
    steps = ITERATIONS[variant] if count is None else count
    words[16:80] = [advance(word, steps) for word in words[16:80]]
    return struct.pack("<96I", *words)


def require(value, message):
    if not value:
        raise ValueError(message)


def validate_observed(variant, observed):
    return len(observed) == SIZE and observed == expected(variant)


def read_image(path):
    with path.open("rb") as source:
        return source.read(SIZE + 1)


def self_test():
    for count in (0, 1, 2, 31, 32, 255, 256, 257, 1025):
        for seed in (0, 1, MODULUS - 1, 0xa5a55a5a, 0x01234567):
            direct = seed
            for _ in range(count):
                direct = (MULTIPLIER * direct + INCREMENT) % MODULUS
            require(direct == advance(seed, count), "independent short recurrence")
    for variant, count in ITERATIONS.items():
        output = expected(variant)
        require(output != initial(), "work changes the payload")
        require(output != expected(variant, count - 1) and output != expected(variant, count + 1), "exact work count")
        require(output[:64] == initial()[:64] and output[320:] == initial()[320:], "unchanged guards")
        require(output != expected("long" if variant == "short" else "short"), "distinct profiles")
        require(validate_observed(variant, output), "accept exact output")
        for bad in (initial(), output[:-1], output + b"\0", expected("long" if variant == "short" else "short")):
            require(not validate_observed(variant, bad), "reject wrong image or extent")
        for index in range(SIZE):
            bad = bytearray(output)
            bad[index] ^= 1
            require(not validate_observed(variant, bytes(bad)), "every byte participates")


def main():
    require(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("emit", "check", "validate"))
    parser.add_argument("--variant", choices=tuple(ITERATIONS))
    parser.add_argument("--observed", type=Path)
    args = parser.parse_args()
    self_test()
    images = {"initial.bin": initial(), **{name + ".expected.bin": expected(name) for name in ITERATIONS}}
    if args.mode == "validate":
        require(args.variant is not None and args.observed is not None, "variant and observation required")
        require(validate_observed(args.variant, read_image(args.observed)), "exact extent, complete observed output and guards")
    else:
        require(args.variant is None and args.observed is None, "no observation for fixture generation/check")
        for name, data in images.items():
            path = HERE / name
            if args.mode == "emit":
                with path.open("xb") as output:
                    output.write(data)
            require(read_image(path) == data, "independently recomputed image: " + name)
    print(json.dumps({"status": "PASS", "mode": args.mode, "bytes_per_image": SIZE,
                      "images": {name: hashlib.sha256(data).hexdigest() for name, data in images.items()}}, sort_keys=True))


if __name__ == "__main__":
    main()
