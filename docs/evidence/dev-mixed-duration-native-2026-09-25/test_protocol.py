#!/usr/bin/env python3
"""Synthetic rejection calibration, never native execution evidence."""
from pathlib import Path
import sys
from types import ModuleType
import unittest

path = Path(sys.argv.pop(1)).resolve() / "protocol.py"
p = ModuleType("mixed_duration_protocol_test")
p.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), "exec"), p.__dict__)
oracle = p.load_module(path.parent / "oracle.py", p.ORACLE_SHA, "mixed_duration_oracle_test")


def profiles():
    rows = ["running 1 test", "test " + p.TESTS["profiles"] + " ... "]
    for name in ("short", "long"):
        rows += ["mixed_duration variant=" + name.title() + " observed_hex=" + oracle.expected(name).hex(),
                 "mixed_duration variant=" + name.title() + " shutdown=synthetic physical_overlap=not_measured"]
    return "\n".join([*rows, "ok", "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1446 filtered out; finished in 1.00s", ""])


class ProtocolTests(unittest.TestCase):
    def test_fixed_cases_and_command(self):
        self.assertEqual(len(p.CASE_NAMES), 2)
        for case, name in p.CASE_NAMES:
            command = p.command(Path("/owned"), case)
            self.assertIn(name, command)
            self.assertEqual(command[-4:], ["--ignored", "--nocapture", "--test-threads=1", "--color=never"])

    def test_complete_outputs_and_summary(self):
        self.assertEqual(p.transcript("profiles", profiles(), "")["harness_passes"], 1)
        for old, new in (("1446 filtered", "1421 filtered"), ("1 passed", "0 passed"),
                         (p.TESTS["profiles"], p.TESTS["owner"]), ("variant=Long", "variant=Short")):
            with self.assertRaises(ValueError):
                p.transcript("profiles", profiles().replace(old, new), "")
        with self.assertRaises(ValueError):
            p.transcript("profiles", profiles(), "warning")

    def test_every_observed_byte_rejected(self):
        for name in ("short", "long"):
            output = oracle.expected(name)
            for index in range(len(output)):
                bad = bytearray(output)
                bad[index] ^= 1
                with self.assertRaises(ValueError):
                    p.outputs("profiles", profiles().replace(output.hex(), bad.hex()))
            for bad in (output[:-1], output + b"\0", oracle.initial()):
                with self.assertRaises(ValueError):
                    p.outputs("profiles", profiles().replace(output.hex(), bad.hex()))

    def test_owner_profile_requires_complete_history(self):
        for text in ("", "mixed_duration_owner_profile_json={}",
                     'mixed_duration_owner_profile_json={"schema":"wrong"}'):
            with self.assertRaises((ValueError, KeyError)):
                p.owner_profile(text)


if __name__ == "__main__":
    unittest.main()
