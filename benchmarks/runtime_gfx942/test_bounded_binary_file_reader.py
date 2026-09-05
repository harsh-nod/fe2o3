#!/usr/bin/env python3

from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


HERE = pathlib.Path(__file__).parent
MAXIMUM_BYTES = 64 * 1024 * 1024
HARNESS_SOURCE = r"""
#include <cstdlib>
#include <iostream>
#include <vector>

#include "bounded_binary_file_reader.hpp"

int main(int argc, char **argv) {
  if (argc != 3 && argc != 4)
    return 64;
  const auto maximum_byte_count =
      static_cast<std::streamoff>(std::strtoll(argv[2], nullptr, 10));
  std::vector<char> bytes;
  const auto status = fe2o3::r26::read_bounded_binary_file(
      argv[1], maximum_byte_count, &bytes);
  std::cout << static_cast<int>(status) << ' ' << bytes.size() << '\n';
  if (status == fe2o3::r26::BoundedBinaryFileReadStatus::Success && argc == 4)
    std::cout.write(bytes.data(), static_cast<std::streamsize>(bytes.size()));
  return 0;
}
"""


class BoundedBinaryFileReaderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temporary = tempfile.TemporaryDirectory()
        temporary = pathlib.Path(cls.temporary.name)
        source = temporary / "reader_harness.cpp"
        cls.harness = temporary / "reader-harness"
        source.write_text(HARNESS_SOURCE, encoding="utf-8")
        subprocess.run(
            [
                "/usr/bin/g++",
                "-std=c++17",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Werror",
                f"-I{HERE}",
                str(source),
                "-o",
                str(cls.harness),
            ],
            check=True,
        )

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temporary.cleanup()

    def read(
        self, path: pathlib.Path, *, emit_bytes: bool = False
    ) -> tuple[int, int, bytes]:
        arguments = [str(self.harness), str(path), str(MAXIMUM_BYTES)]
        if emit_bytes:
            arguments.append("emit")
        completed = subprocess.run(arguments, check=True, capture_output=True)
        header, separator, payload = completed.stdout.partition(b"\n")
        self.assertEqual(separator, b"\n")
        status, byte_count = (int(field) for field in header.split())
        return status, byte_count, payload

    def test_missing_file_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            status, byte_count, payload = self.read(
                pathlib.Path(directory) / "missing.hsaco"
            )
        self.assertEqual(
            status,
            1,
        )
        self.assertEqual((byte_count, payload), (0, b""))

    def test_empty_file_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "empty.hsaco"
            path.touch()
            status, byte_count, payload = self.read(path)
        self.assertEqual(status, 2)
        self.assertEqual((byte_count, payload), (0, b""))

    def test_valid_binary_file_is_returned_exactly(self) -> None:
        expected = bytes(range(256)) * 17 + b"\x00\xffHSACO\x00"
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "valid.hsaco"
            path.write_bytes(expected)
            status, byte_count, payload = self.read(path, emit_bytes=True)
        self.assertEqual(status, 0)
        self.assertEqual(byte_count, len(expected))
        self.assertEqual(payload, expected)

    def test_exact_limit_file_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "exact-limit.hsaco"
            with path.open("wb") as output:
                output.truncate(MAXIMUM_BYTES)
            status, byte_count, payload = self.read(path)
        self.assertEqual(status, 0)
        self.assertEqual((byte_count, payload), (MAXIMUM_BYTES, b""))

    def test_over_limit_file_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "over-limit.hsaco"
            with path.open("wb") as output:
                output.truncate(MAXIMUM_BYTES + 1)
            status, byte_count, payload = self.read(path)
        self.assertEqual(status, 2)
        self.assertEqual((byte_count, payload), (0, b""))


if __name__ == "__main__":
    unittest.main()
