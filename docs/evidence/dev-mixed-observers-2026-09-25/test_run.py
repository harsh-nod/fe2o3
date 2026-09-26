#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("observer_cpu_runner_test", Path(__file__).with_name("run.py"))
RUN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUN)


class RosterTests(unittest.TestCase):
    def test_exact_roster_and_new_test_names(self):
        names = ["existing::test_" + str(index) for index in range(1447)]
        names += ["new::" + name for name in (*RUN.CPU_NAMES, *RUN.NATIVE_NAMES)]
        def render(rows):
            return "\n".join(name + ": test" for name in rows) + "\n1452 tests, 0 benchmarks\n"
        self.assertEqual(RUN.roster(render(names)), names)
        for bad in ("", render(names[:-1]), render([*names[:-1], names[0]]),
                    render(names).replace(RUN.NATIVE_NAMES[0], "wrong_case"),
                    render(names).replace("1452 tests", "1447 tests")):
            with self.assertRaises(ValueError):
                RUN.roster(bad)


if __name__ == "__main__":
    unittest.main()
