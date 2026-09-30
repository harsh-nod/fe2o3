import copy
import hashlib
import json
from pathlib import Path
import re
import tomllib
import unittest

import xgmi_retained_host_diagnostic as diagnostic
import xgmi_peer_series_results as ordinary

ROOT = Path(__file__).resolve().parents[2]
EXPECTED = {"unique_ids": ["ab83d2ffef0d3cdf", "d2e26fef80cf5c33"],
            "gpu_ids": [23018, 29122], "engines": [12, 12], "depth": 16}


def fixture():
    summary = dict(diagnostic.FIXED, unique_ids=",".join(EXPECTED["unique_ids"]),
                   gpu_ids="23018,29122", depth="16", forward_engine="12", reverse_engine="12",
                   forward_entry_ns="6500000", reverse_entry_ns="6500000",
                   forward_finish_ns="6500", reverse_finish_ns="6500")
    rows = [summary]
    for direction in ("forward", "reverse"):
        for index in range(10):
            row = {key: "0" for key in diagnostic.SAMPLE_KEYS}
            row.update(row="sample", direction=direction, index=str(index), elapsed_ns="10000",
                       submit_open_ns="1000", prepare_ns="1000", publish_ns="1000",
                       submit_close_ns="1000", submit_total_ns="4100", wait_open_ns="1000",
                       validation_ns="100", scan_ns="1000", retirement_ns="100", wait_close_ns="1000",
                       wait_total_ns="3300", first_observed_ns="10", all_observed_ns="900",
                       counters_status="available", rounds="1", observations="16",
                       cpu_status="available", thread_cpu_ns="800")
            rows.append(row)
    return rows


def encode(rows):
    return ("\n".join(" ".join(key + "=" + value for key, value in row.items()) for row in rows) + "\n").encode("ascii")


class ParserTests(unittest.TestCase):
    def test_exact_complete_roster_and_explicit_no_authority(self):
        parsed = diagnostic.parse(encode(fixture()), EXPECTED)
        self.assertEqual(parsed["authority"], "none")
        self.assertNotIn("performance_acceptance", parsed)
        self.assertEqual(len(parsed["samples"]), 20)
        self.assertEqual(parsed["samples"][0]["observations"], 16)

    def test_ordinary_parser_cannot_accept_diagnostic_stream(self):
        with self.assertRaises(ValueError):
            ordinary.parse_result(encode(fixture()), backend="kfd", unique_ids=EXPECTED["unique_ids"],
                                  copy_bytes=1048576, depth=16, warmups=2, samples=10,
                                  kfd_gpu_ids=EXPECTED["gpu_ids"], kfd_engines=EXPECTED["engines"])

    def test_unknown_missing_duplicate_and_extra_rows_fail(self):
        good = encode(fixture())
        bad = [good + b"\n", good[:-1], good.replace(b"row=summary", b"row=summary row=summary", 1),
               good.replace(b"row=summary", b"row=summary surprise=1", 1),
               good.replace(b"authority=none ", b"", 1), good + good.splitlines()[1] + b"\n",
               b"\n".join(good.splitlines()[:-1]) + b"\n", good.replace(b" ", b"\t", 1)]
        for raw in bad:
            with self.subTest(raw=raw[:80]), self.assertRaises((ValueError, UnicodeError)):
                diagnostic.parse(raw, EXPECTED)

    def test_identity_profile_order_and_controls_are_bound(self):
        for key, value in (("schema", "ordinary"), ("authority", "completion"), ("depth", "32"),
                           ("policy_sha256", "0" * 64), ("wait_policy", "short-sleep"),
                           ("gpu_ids", "29122,23018"), ("forward_engine", "13"), ("warmups", "1"),
                           ("unique_ids", ",".join(reversed(EXPECTED["unique_ids"])))):
            rows = fixture()
            rows[0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                diagnostic.parse(encode(rows), EXPECTED)
        for key, value in (("index", "1"), ("direction", "reverse")):
            rows = fixture()
            rows[1][key] = value
            with self.assertRaises(ValueError):
                diagnostic.parse(encode(rows), EXPECTED)

    def test_bad_numeric_and_inconsistent_phase_ranges_fail(self):
        for key, value in (("elapsed_ns", "0"), ("scan_ns", "-1"), ("spins", "00"),
                           ("rounds", str(1 << 64)), ("scan_ns", "4000"),
                           ("all_observed_ns", "1001"), ("first_observed_ns", "901"),
                           ("submit_total_ns", "8000"), ("rounds", "2"),
                           ("observations", "15"), ("observations", "17")):
            rows = fixture()
            rows[1][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                diagnostic.parse(encode(rows), EXPECTED)

    def test_unavailable_invalid_observations_are_not_zero(self):
        rows = fixture()
        rows[1]["cpu_status"] = "unavailable"
        with self.assertRaises(ValueError):
            diagnostic.parse(encode(rows), EXPECTED)
        for key in diagnostic.CPU:
            rows[1][key] = "none"
        rows[1]["counters_status"] = "invalid"
        with self.assertRaises(ValueError):
            diagnostic.parse(encode(rows), EXPECTED)
        for key in diagnostic.COUNTERS:
            rows[1][key] = "none"
        rows[1]["prepare_ns"] = "none"
        parsed = diagnostic.parse(encode(rows), EXPECTED)
        self.assertIsNone(parsed["samples"][0]["thread_cpu_ns"])
        self.assertIsNone(parsed["samples"][0]["rounds"])

    def test_sleep_counters_keep_ordinary_policy_bounds(self):
        rows = fixture()
        rows[1].update(rounds="84", observations="100", spins="64", yields="16", sleeps="3",
                       requested_sleep_ns="175000", max_requested_sleep_ns="100000")
        diagnostic.parse(encode(rows), EXPECTED)
        for key, value in (("spins", "63"), ("yields", "17"), ("max_requested_sleep_ns", "1000001"),
                           ("requested_sleep_ns", "99999")):
            changed = copy.deepcopy(rows)
            changed[1][key] = value
            with self.assertRaises(ValueError):
                diagnostic.parse(encode(changed), EXPECTED)
        changed = fixture()
        changed[1].update(rounds="2", observations="16", yields="1")
        with self.assertRaises(ValueError):
            diagnostic.parse(encode(changed), EXPECTED)

    def test_expected_controls_reject_bool_aliases_and_extra_keys(self):
        for changed in (dict(EXPECTED, depth=True), dict(EXPECTED, engines=[True, 12]),
                        dict(EXPECTED, extra=1), dict(EXPECTED, gpu_ids=[1, 1])):
            with self.assertRaises(ValueError):
                diagnostic.parse(encode(fixture()), changed)


class SourceTests(unittest.TestCase):
    def test_diagnostic_example_is_feature_gated_and_controls_are_closed_before_admission(self):
        manifest = tomllib.loads((ROOT / "crates/fe2o3-kfd/Cargo.toml").read_text("ascii"))
        example = [item for item in manifest["example"] if item["name"] == "kfd-xgmi-retained-host-diagnostic"]
        self.assertEqual(example, [{"name": "kfd-xgmi-retained-host-diagnostic", "required-features": ["hardware-diagnostic"]}])
        source = (ROOT / "crates/fe2o3-kfd/examples/kfd-xgmi-retained-host-diagnostic.rs").read_text("ascii")
        for declaration in ("const BYTES: usize = 1_048_576;", "const CANARY: usize = 32;",
                            "const WARMUPS: usize = 2;", "const SAMPLES: usize = 10;"):
            self.assertIn(declaration, source)
        main = source.split("fn main()", 1)[1]
        self.assertLess(main.index("diagnostic controls out of range"), main.index("let left_device = admit"))
        self.assertIn("--reviewed-mi300x-retained-host-diagnostic", main)

    def test_producer_keeps_timing_scope_and_delays_output_until_validation_and_teardown(self):
        source = (ROOT / "crates/fe2o3-kfd/examples/kfd-xgmi-retained-host-diagnostic.rs").read_text("ascii")
        body = source.split("fn series(", 1)[1].split("fn inspect(", 1)[0]
        ordered = ["let entry_ns", "for round in 0..1 + WARMUPS + SAMPLES", "let requests",
                   "let started", ".submit_batch_diagnostic_v1", ".wait_batch_for_diagnostic_v1",
                   "let elapsed_ns", "completed.len()", "completed.into_copies()",
                   "if round > WARMUPS", "scope.finish()"]
        positions = [body.index(token) for token in ordered]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("Duration::from_secs(30)", body)
        main = source.split("fn main()", 1)[1]
        self.assertLess(main.index("reverse_queue.destroy_and_release"), main.index("forward_queue.destroy_and_release"))
        self.assertLess(main.index("forward_queue.destroy_and_release"), main.index("if invalid != 0"))
        self.assertLess(main.index("if invalid != 0"), main.index("println!"))

    def test_producer_format_key_rosters_match_the_distinct_parser(self):
        source = (ROOT / "crates/fe2o3-kfd/examples/kfd-xgmi-retained-host-diagnostic.rs").read_text("ascii")
        for start, expected in (("fn print_sample(", diagnostic.SAMPLE_KEYS),
                                ("fn main()", set(diagnostic.FIXED) | diagnostic.SUMMARY_VARIABLE)):
            body = source.split(start, 1)[1].split("concat!(", 1)[1].split(")", 1)[0]
            literals = re.findall(r'"(?:[^"\\]|\\.)*"', body)
            format_string = "".join(json.loads(value) for value in literals)
            self.assertEqual(set(re.findall(r"(?:^| )([a-z][a-z0-9_]*)=", format_string)), expected)

    def test_original_benchmarks_policy_and_operational_bodies_are_unchanged(self):
        pins = {
            "crates/fe2o3-kfd/examples/kfd-sdma-xgmi-peer-benchmark.rs": "afbc397af8427b2c7b929c356f1fa38075edeee0c279c9db17000280ec9cd808",
            "crates/fe2o3-kfd/examples/kfd_sdma_xgmi_peer_benchmark/retained_series.rs": "f51f6bf4d7de4d0bdf1452d3f28e28490424d1da795704c9f7c008624a7e4da8",
            "benchmarks/runtime_gfx942/xgmi_peer_hip.cpp": "15377c48b71066b9d5dfab8c1579c2a13b971e1dacfb3de2f4e57c293b0dad3c",
            "benchmarks/runtime_gfx942/xgmi_peer_hsa.cpp": "ff7570330c7ecdeeec4558f16035fdbb4418d871d61072c7973f048e841924c7",
            "crates/fe2o3-kfd/src/wait.rs": "2e869b7c0f0fcd5de5984f797452de4d8eb8a35e6cddc28dc77807913d34ca39",
            "crates/fe2o3-kfd/src/currentness.rs": "9c42efb6c2fb3f0352e6fda0bf88a91006437e02e06f656e126d48356bb8c3f3",
            "crates/fe2o3-kfd/src/sdma/retained_pair_operation_body.rs": "54236918b01d7167878c4827e28ab0c4a103e3f3b069d311b56931daad86b7ac",
            "crates/fe2o3-kfd/src/sdma/retained_pair_policy_v1.txt": diagnostic.POLICY,
            "crates/fe2o3-kfd/src/shared_memory/pair_operational.rs": "dd6bf6f9decf9f3135393dc6b937c7bea31e7a25ce8b12821713de8d69d6eca0",
        }
        for path, digest in pins.items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), digest, path)

    def test_wait_body_is_baseline_plus_only_explicit_instrumentation(self):
        source = (ROOT / "crates/fe2o3-kfd/src/sdma.rs").read_text("ascii")
        body = source.split("    fn wait_many_xgmi_with_timer<", 1)[1].split("    fn observe_progress_in_current_scope(", 1)[0]
        self.assertIn("let validation_started = timer.start();\n        self.require_live()?;", body)
        body = body.split("        self.require_live()?;", 1)[1]
        for statement in (
            "        timer.end(XgmiWaitPhase::Validation, validation_started);\n",
            "        timer.begin_scan();\n", "            timer.scan_round();\n",
            "                timer.observation(observed == i64::from(expected));\n",
            "        timer.finish_scan();\n", "        let retirement_started = timer.start();\n",
            "        timer.end(XgmiWaitPhase::Retirement, retirement_started);\n",
        ):
            self.assertEqual(body.count(statement), 1, statement)
            body = body.replace(statement, "")
        self.assertEqual(body.count("            timer.pause(&mut wait);"), 1)
        body = body.replace("            timer.pause(&mut wait);", "            wait.pause();")
        self.assertEqual(hashlib.sha256(body.encode("ascii")).hexdigest(),
                         "3d2ff578498de8cf0b31412f3e54a8abf90d156615cd1de3f68a8540b95d11d9")
        packet = source.split("impl Gfx942SdmaCopySubmissionV1 {", 1)[1].split("#[derive(Debug)]", 1)[0]
        self.assertEqual(hashlib.sha256(packet.encode("ascii")).hexdigest(),
                         "9c044f298a949f97fbb7feeb1885262c668d2c93caa43a9155f7a25786c7eb1e")

    def test_profiled_wrapper_preserves_retained_currentness_and_success_only_exit(self):
        source = (ROOT / "crates/fe2o3-kfd/src/sdma/retained_pair.rs").read_text("ascii")
        for start, end in (("    pub fn submit_batch_diagnostic_v1(", "    /// Uses the original deadline"),
                           ("    pub fn wait_batch_for_diagnostic_v1(", "    fn wait(")):
            body = source.split(start, 1)[1].split(end, 1)[0]
            self.assertIn("run_diagnostic_operation(&mutself.scope.context", re.sub(r"\s+", "", body))
            self.assertIn("XgmiRouteCurrentnessV1::OrdinaryRetainedPair", body)
            self.assertNotIn("XgmiRouteCurrentnessV1::Full", body)
            self.assertIn("|timer| timer.finish(started)", body)
        bridge = source.split("fn run_diagnostic_operation<", 1)[1].split("struct Scope<", 1)[0]
        self.assertIn("Result<T, E>: TerminalOutcome", bridge)
        self.assertLess(bridge.index("run_operation(context"), bridge.index("?;"))
        self.assertLess(bridge.index("?;"), bridge.index("observe(measurement)"))
        source = (ROOT / "crates/fe2o3-kfd/src/sdma.rs").read_text("ascii")
        body = source.split("    fn wait_batch_with_timer<", 1)[1].split("    fn wait_for_with_currentness(", 1)[0]
        self.assertEqual(body.count("Self::validate_route_currentness("), 2)
        self.assertLess(body.index("XgmiWaitPhase::Opening"), body.index("owner.wait_many_xgmi_with_timer"))
        self.assertLess(body.index("owner.wait_many_xgmi_with_timer"), body.index("XgmiWaitPhase::Closing"))
        self.assertIn("Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate", body)


if __name__ == "__main__":
    unittest.main()
