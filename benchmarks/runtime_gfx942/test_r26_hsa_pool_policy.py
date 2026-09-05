#!/usr/bin/env python3

"""Host-only tests for the R26 HSA pool policy."""

from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


DIRECTORY = pathlib.Path(__file__).resolve().parent
SOURCE = DIRECTORY / "r26_hsa_pool_policy_test.cpp"


class R26HsaPoolPolicyTests(unittest.TestCase):
    def test_pool_policy(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            executable = pathlib.Path(temporary_directory) / "r26-hsa-pool-policy-test"
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
