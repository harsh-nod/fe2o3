#!/usr/bin/env python3
"""Calibration of complete selected-roster and outcome parsing."""

import unittest
import verify


def harness():
    return ("running 32 tests\n" + "".join(f"test {name} ... ok\n" for name in sorted(verify.EXPECTED))
            + "\ntest result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 1398 filtered out; finished in 1.00s\n\n")


class Calibration(unittest.TestCase):
    def rejects(self, function, text):
        with self.assertRaises((AssertionError, ValueError, IndexError)):
            function(text)

    def test_complete_harness(self):
        self.assertEqual(verify.parse_harness(harness()), dict.fromkeys(verify.EXPECTED, "ok"))

    def test_malformed_harnesses(self):
        text = harness()
        first = sorted(verify.EXPECTED)[0]
        for mutated in [
            text[:text.index("test result:")],
            text + "trailing\n",
            text.replace(" ... ok", " ... FAILED", 1),
            text.replace(" ... ok", " ... ignored", 1),
            text.replace(first, "wrong::test", 1),
            text.replace(first, sorted(verify.EXPECTED)[1], 1),
            text.replace("1398 filtered", "1397 filtered"),
            text.replace("32 passed", "31 passed"),
            text[:text.index("test result:")] + "\0" * 256,
        ]:
            with self.subTest(mutated=mutated):
                self.rejects(verify.parse_harness, mutated)

    def test_complete_roster(self):
        text = "".join(f"{name}: test\n" for name in sorted(verify.EXPECTED)) + "\n32 tests, 0 benchmarks\n"
        verify.parse_roster(text)

    def test_malformed_rosters(self):
        names = sorted(verify.EXPECTED)
        text = "".join(f"{name}: test\n" for name in names) + "\n32 tests, 0 benchmarks\n"
        for mutated in [text.replace(names[0], names[1]), text + "extra\n", text.replace("32 tests", "31 tests"), text.replace(names[0], "other::test")]:
            self.rejects(verify.parse_roster, mutated)


if __name__ == "__main__":
    unittest.main()
