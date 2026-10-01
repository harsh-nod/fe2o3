import copy
import json
from pathlib import Path
import re
import tomllib
import unittest

import xgmi_retained_host_diagnostic as baseline
import xgmi_retained_wait_cadence as cadence
from test_xgmi_retained_host_diagnostic import EXPECTED, encode, fixture as baseline_fixture

ROOT = Path(__file__).resolve().parents[2]
EXAMPLE = "crates/fe2o3-kfd/examples/kfd-xgmi-retained-cadence-experiment.rs"


def fixture(policy="ceiling-25us", mode="profiled"):
    rows = baseline_fixture()
    rows[0].update(schema=cadence.SCHEMA, wait_policy="explicit-cadence-experiment",
                   cadence=policy, mode=mode, sleep_ceiling_ns=str(cadence.CEILINGS[policy]),
                   timing="instrumented-host-only" if mode == "profiled" else
                   "native-enqueue-through-paired-operational-completion")
    if mode == "ordinary":
        rows[1:] = [{key: row[key] for key in ("row", "direction", "index", "elapsed_ns")}
                    for row in rows[1:]]
    return rows, dict(EXPECTED, cadence=policy, mode=mode)


class ParserTests(unittest.TestCase):
    def test_closed_policy_mode_matrix_has_no_performance_or_formal_authority(self):
        for policy in cadence.CEILINGS:
            for mode in cadence.MODES:
                rows, expected = fixture(policy, mode)
                parsed = cadence.parse(encode(rows), expected)
                self.assertEqual(len(parsed["samples"]), 20)
                self.assertEqual(parsed["authority"], "none")
                self.assertIs(parsed["performance_acceptance"], False)
                self.assertIs(parsed["formal_refinement"], False)
                self.assertEqual(parsed["instrumentation_perturbs_timing_and_readiness"], mode == "profiled")
                self.assertEqual("rounds" in parsed["samples"][0], mode == "profiled")

    def test_existing_and_experiment_schemas_cannot_be_substituted(self):
        rows, expected = fixture()
        with self.assertRaises(ValueError):
            baseline.parse(encode(rows), EXPECTED)
        with self.assertRaises(ValueError):
            cadence.parse(encode(baseline_fixture()), expected)
        ordinary_rows, ordinary_expected = fixture(mode="ordinary")
        with self.assertRaises(ValueError):
            cadence.parse(encode(rows), ordinary_expected)
        with self.assertRaises(ValueError):
            cadence.parse(encode(ordinary_rows), expected)

    def test_summary_controls_identity_and_order_are_exact(self):
        for key, value in (("schema", baseline.FIXED["schema"]), ("authority", "completion"),
                           ("cadence", "ordinary-1ms"), ("mode", "ordinary"), ("sleep_ceiling_ns", "25001"),
                           ("timing", "native"), ("wait_policy", "ordinary"), ("depth", "32"),
                           ("policy_sha256", "0" * 64), ("gpu_ids", "29122,23018"),
                           ("forward_engine", "13"), ("warmups", "1"), ("forward_entry_ns", "0"),
                           ("unique_ids", ",".join(reversed(EXPECTED["unique_ids"])))):
            rows, expected = fixture()
            rows[0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                cadence.parse(encode(rows), expected)
        for key, value in (("index", "1"), ("direction", "reverse")):
            rows, expected = fixture()
            rows[1][key] = value
            with self.assertRaises(ValueError):
                cadence.parse(encode(rows), expected)

    def test_expected_controls_reject_open_values_aliases_and_bad_endpoints(self):
        rows, expected = fixture()
        for changed in (dict(expected, cadence="custom"), dict(expected, cadence=True),
                        dict(expected, mode="instrumented"), dict(expected, mode=True),
                        dict(expected, depth=True), dict(expected, engines=[True, 12]),
                        dict(expected, extra=1), dict(expected, gpu_ids=[1, 1]),
                        dict(expected, unique_ids=["ABC", "abc"]), dict(expected, engines=[1, 12])):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                cadence.parse(encode(rows), changed)

    def test_missing_extra_duplicate_noncanonical_and_unbounded_rows_fail(self):
        for mode in cadence.MODES:
            rows, expected = fixture(mode=mode)
            good = encode(rows)
            bad = [good + b"\n", good[:-1], good.replace(b"row=summary", b"row=summary row=summary", 1),
                   good.replace(b"row=summary", b"row=summary surprise=1", 1),
                   good.replace(b"authority=none ", b"", 1), good + good.splitlines()[1] + b"\n",
                   b"\n".join(good.splitlines()[:-1]) + b"\n", good.replace(b" ", b"\t", 1),
                   good.replace(b" ", b"  ", 1), b"x" * 131073, good + b"\xff\n"]
            for raw in bad:
                with self.subTest(mode=mode, raw=raw[:40]), self.assertRaises((ValueError, UnicodeError)):
                    cadence.parse(raw, expected)

    def test_numeric_ranges_and_nested_phases(self):
        for key, value in (("elapsed_ns", "0"), ("scan_ns", "-1"), ("spins", "00"),
                           ("rounds", str(1 << 64)), ("scan_ns", "4000"),
                           ("all_observed_ns", "1001"), ("first_observed_ns", "901"),
                           ("submit_total_ns", "8000"), ("rounds", "2"),
                           ("observations", "15"), ("observations", "17")):
            rows, expected = fixture()
            rows[1][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                cadence.parse(encode(rows), expected)

    def test_sleep_cap_changes_without_spin_or_yield_phase_changes(self):
        for policy, maximum in cadence.CEILINGS.items():
            rows, expected = fixture(policy)
            rows[1].update(rounds="84", observations="100", spins="64", yields="16", sleeps="3",
                           requested_sleep_ns=str(maximum * 3), max_requested_sleep_ns=str(maximum))
            cadence.parse(encode(rows), expected)
            for key, value in (("spins", "63"), ("yields", "17"),
                               ("max_requested_sleep_ns", str(maximum + 1)),
                               ("requested_sleep_ns", str(maximum - 1))):
                changed = copy.deepcopy(rows)
                changed[1][key] = value
                with self.subTest(policy=policy, key=key), self.assertRaises(ValueError):
                    cadence.parse(encode(changed), expected)

    def test_missing_diagnostics_remain_missing_not_zero(self):
        rows, expected = fixture()
        rows[1]["cpu_status"] = "unavailable"
        with self.assertRaises(ValueError):
            cadence.parse(encode(rows), expected)
        for key in baseline.CPU:
            rows[1][key] = "none"
        rows[1]["counters_status"] = "invalid"
        with self.assertRaises(ValueError):
            cadence.parse(encode(rows), expected)
        for key in baseline.COUNTERS:
            rows[1][key] = "none"
        rows[1]["prepare_ns"] = "none"
        sample = cadence.parse(encode(rows), expected)["samples"][0]
        for key in (*baseline.CPU, *baseline.COUNTERS, "prepare_ns"):
            self.assertIsNone(sample[key])


class SourceTests(unittest.TestCase):
    def test_separate_gated_producer_and_closed_pre_admission_controls(self):
        manifest = tomllib.loads((ROOT / "crates/fe2o3-kfd/Cargo.toml").read_text("ascii"))
        entries = [row for row in manifest["example"] if row["name"] == "kfd-xgmi-retained-cadence-experiment"]
        self.assertEqual(entries, [{"name": "kfd-xgmi-retained-cadence-experiment", "required-features": ["hardware-diagnostic"]}])
        source = (ROOT / EXAMPLE).read_text("ascii")
        main = source.split("fn main()", 1)[1]
        for token in ("args.len() != 6", "unknown closed cadence", "unknown instrumentation mode",
                      "diagnostic controls out of range"):
            self.assertLess(main.index(token), main.index("let left_device = admit"))
        self.assertIn('"ordinary-1ms" => Gfx942XgmiRetainedWaitCadenceV1::Ordinary1ms', main)
        self.assertIn('"ceiling-25us" => Gfx942XgmiRetainedWaitCadenceV1::Ceiling25us', main)

    def test_original_setup_validation_and_cleanup_are_identical(self):
        old = (ROOT / "crates/fe2o3-kfd/examples/kfd-xgmi-retained-host-diagnostic.rs").read_text("ascii")
        new = (ROOT / EXAMPLE).read_text("ascii")
        for start, old_end, new_end in (("fn pattern(", "fn series(", "fn series<"),
                                        ("fn inspect(", "fn optional(", "fn optional("),
                                        ("    let left_device = admit", "    let forward_samples = series", "    let (forward_samples, reverse_samples)"),
                                        ("    if forward_pairs.len()", "    println!(", "    println!(")):
            self.assertEqual(old.split(start, 1)[1].split(old_end, 1)[0],
                             new.split(start, 1)[1].split(new_end, 1)[0])
        main = new.split("fn main()", 1)[1]
        self.assertLess(main.index("forward_queue.destroy_and_release"), main.index("if invalid != 0"))
        self.assertLess(main.index("if invalid != 0"), main.index("println!"))

    def test_mode_specialization_has_matching_timer_boundaries(self):
        source = (ROOT / EXAMPLE).read_text("ascii")
        body = source.split("fn series<const PROFILED: bool>(", 1)[1].split("fn inspect(", 1)[0]
        tokens = ["let requests", "let started", "if PROFILED", ".submit_batch_diagnostic_v1",
                  ".wait_batch_for_cadence_diagnostic_v1", ".submit_batch(requests)",
                  ".wait_batch_for_cadence_experiment_v1", "let elapsed_ns", "completed.len()",
                  "completed.into_copies()", "if round > WARMUPS", "scope.finish()"]
        self.assertEqual([body.index(token) for token in tokens], sorted(body.index(token) for token in tokens))
        self.assertEqual(body.count("Duration::from_secs(30), cadence"), 2)
        self.assertIn("(completed, None)", body)
        self.assertIn("for round in 0..1 + WARMUPS + SAMPLES", body)

    def test_format_rosters_match_distinct_parser(self):
        source = (ROOT / EXAMPLE).read_text("ascii")
        for start, expected in (("fn print_sample(", baseline.SAMPLE_KEYS),
                                ("fn main()", set(baseline.FIXED) | baseline.SUMMARY_VARIABLE |
                                 {"mode", "cadence", "sleep_ceiling_ns"})):
            body = source.split(start, 1)[1].split("concat!(", 1)[1].split(")", 1)[0]
            literals = re.findall(r'"(?:[^"\\]|\\.)*"', body)
            fields = set(re.findall(r"(?:^| )([a-z][a-z0-9_]*)=", "".join(json.loads(value) for value in literals)))
            self.assertEqual(fields, expected)
        self.assertIn('"row=sample direction={} index={} elapsed_ns={}"', source)


if __name__ == "__main__":
    unittest.main()
