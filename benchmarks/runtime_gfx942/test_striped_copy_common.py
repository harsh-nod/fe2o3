#!/usr/bin/env python3

from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


DIRECTORY = pathlib.Path(__file__).resolve().parent
SOURCE = DIRECTORY / "striped_copy_benchmark_common_test.cpp"


class StripedCopyCommonTests(unittest.TestCase):
    def test_host_only_shape_and_assignment_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            executable = pathlib.Path(temporary) / "striped-copy-common-test"
            subprocess.run(
                [
                    "/usr/bin/g++",
                    "-std=c++17",
                    "-O2",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-pedantic",
                    "-I",
                    str(DIRECTORY),
                    str(SOURCE),
                    "-o",
                    str(executable),
                ],
                check=True,
            )
            subprocess.run([str(executable)], check=True)


if __name__ == "__main__":
    unittest.main()
