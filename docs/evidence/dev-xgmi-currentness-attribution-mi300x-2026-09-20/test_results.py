#!/usr/bin/env python3

import importlib.util
from pathlib import Path
import unittest


SPEC = importlib.util.spec_from_file_location(
    "xgmi_aggregate_attribution_results", Path(__file__).with_name("results.py")
)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("cannot load results.py")
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)

UIDS = ["0xab83d2ffef0d3cdf", "0xd2e26fef80cf5c33"]
UID_PAIR = "ab83d2ffef0d3cdf,d2e26fef80cf5c33"


def summary(mode: str) -> bytes:
    diagnostic = " diagnostic=aggregate-currentness-attribution" if mode == "on" else ""
    return (
        "backend=kfd schema=fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1 "
        f"surface=runtime-facade unique_ids={UID_PAIR} target=gfx942:xnack- "
        "bytes=1048576 depth=1 queue_depth=1 batch_size=1 "
        "direction=forward-then-reverse outstanding_depth=1 "
        "engine_parallelism=ordered-single-sdma warmups=10 samples=30 "
        "measurement=persistent-hot peer_access=topology-xgmi "
        "mapping_lifetime=persistent-no-host-access-between-timed-rounds "
        "prime_batches=1 doorbells_per_batch=1 "
        "progress=explicit-exact-roster-aggregate-wait "
        "aggregate_roster=exact-round-submissions background_progress=false "
        "forward_engine=topology-selected reverse_engine=topology-selected "
        "forward_p50_ns=100000 forward_p95_ns=110000 "
        "forward_p50_GBps=10.486 reverse_p50_ns=200000 "
        "reverse_p95_ns=220000 reverse_p50_GBps=5.243 "
        "canaries=pass teardown=explicit "
        f"timing=facade-enqueue-through-aggregate-close{diagnostic}"
    ).encode()


def record(ordinal: int) -> bytes:
    source, destination = (
        (UIDS[0][2:], UIDS[1][2:]) if ordinal % 2 == 0 else (UIDS[1][2:], UIDS[0][2:])
    )
    nested = " ".join(
        f"{edge}_{key}={value * scale}"
        for edge, scale in (("opening", 1), ("closing", 2))
        for key, value in (
            ("source_before_ns", 1),
            ("peer_before_ns", 2),
            ("topology_discovery_ns", 11),
            ("route_and_equality_ns", 3),
            ("source_after_ns", 4),
            ("peer_after_ns", 5),
            ("pair_total_ns", 26),
            ("topology_tree_ns", 1),
            ("topology_initial_identity_ns", 2),
            ("topology_render_correlation_ns", 3),
            ("topology_closing_identity_ns", 4),
            ("topology_total_ns", 10),
        )
    )
    return (
        "schema=fe2o3.xgmi-aggregate-currentness-attribution.v1 backend=kfd "
        f"ordinal={ordinal} backend_submission={ordinal + 7} "
        f"source_uid={source} destination_uid={destination} "
        "admission_validation_ns=2 preparation_ns=3 opening_currentness_ns=100 "
        "submission_ns=7 wait_ns=11 closing_currentness_ns=200 "
        f"settlement_ns=17 total_ns=361 {nested} authority=none teardown=explicit "
        "timing=backend-aggregate-currentness-host-only"
    ).encode()


def transcript(mode: str) -> bytes:
    rows = [] if mode == "off" else [record(index) for index in range(82)]
    return b"\n".join([*rows, summary(mode)]) + b"\n"


def mutate_row(data: bytes, row: int, key: str, value: str) -> bytes:
    lines = data[:-1].split(b"\n")
    tokens = lines[row].decode().split(" ")
    prefix = key + "="
    matches = [index for index, token in enumerate(tokens) if token.startswith(prefix)]
    if len(matches) != 1:
        raise AssertionError((key, matches))
    tokens[matches[0]] = prefix + value
    lines[row] = " ".join(tokens).encode()
    return b"\n".join(lines) + b"\n"


class ResultParserTests(unittest.TestCase):
    def rejected(self, data, mode="on", ids=UIDS):
        with self.assertRaises((ValueError, TypeError, UnicodeDecodeError)):
            R.parse_result(data, mode, ids)

    def test_exact_off_and_on_transcripts(self):
        off = R.parse_result(transcript("off"), "off", UIDS)
        self.assertEqual(off["mode"], "off")
        self.assertEqual(off["records"], [])
        self.assertNotIn("diagnostic", off["summary"])
        on = R.parse_result(transcript("on"), "on", UIDS)
        self.assertEqual(
            on["summary"]["diagnostic"], "aggregate-currentness-attribution"
        )
        self.assertEqual(len(on["records"]), 82)
        self.assertEqual(
            [
                (row["population"], row["round"], row["direction"])
                for row in on["records"][:4]
            ],
            [
                ("prime", 0, "forward"),
                ("prime", 0, "reverse"),
                ("warmup", 0, "forward"),
                ("warmup", 0, "reverse"),
            ],
        )
        self.assertEqual(on["records"][21]["population"], "warmup")
        self.assertEqual(on["records"][22]["population"], "sample")
        self.assertEqual(on["records"][81]["round"], 29)

    def test_mode_and_record_rosters_are_not_interchangeable(self):
        self.rejected(transcript("on"), "off")
        self.rejected(transcript("off"), "on")
        self.rejected(transcript("on"), "unknown")
        self.rejected(transcript("on").replace(b"\n", b"", 1))
        self.rejected(transcript("on") + record(82) + b"\n")

    def test_framing_ascii_duplicates_and_field_roster_are_strict(self):
        valid = transcript("on")
        for data in (
            valid[:-1],
            valid + b"\n",
            valid.replace(b" backend=kfd", b"  backend=kfd", 1),
            valid.replace(b" backend=kfd", b"\tbackend=kfd", 1),
            valid.replace(b"backend=kfd", b"backend=kfd backend=kfd", 1),
            valid.replace(b" authority=none", b"", 1),
            valid.replace(b" authority=none", b" extra=value authority=none", 1),
            valid.replace(b"backend=kfd", b"back\xffnd=kfd", 1),
            valid.replace(b"backend=kfd", b"word", 1),
        ):
            with self.subTest(data=data[:80]):
                self.rejected(data)

    def test_exact_ordinals_submissions_directions_and_uids(self):
        valid = transcript("on")
        for row, key, value in (
            (0, "ordinal", "1"),
            (0, "backend_submission", "8"),
            (1, "source_uid", UIDS[0][2:]),
            (1, "destination_uid", UIDS[1][2:]),
            (81, "backend_submission", "89"),
        ):
            with self.subTest(row=row, key=key):
                self.rejected(mutate_row(valid, row, key, value))
        self.rejected(valid, ids=[UIDS[1], UIDS[0]])
        self.rejected(valid, ids=[UIDS[0]])
        self.rejected(valid, ids=(UIDS[0], UIDS[1]))
        self.rejected(valid, ids=[UIDS[0], UIDS[0]])

    def test_diagnostic_constants_and_durations_are_strict(self):
        valid = transcript("on")
        for key, value in (
            ("schema", "wrong.v1"),
            ("backend", "hip"),
            ("authority", "owner"),
            ("teardown", "implicit"),
            ("timing", "device"),
        ):
            self.rejected(mutate_row(valid, 0, key, value))
        for key in (*R._PHASES, "total_ns"):
            for value in ("-1", "+1", "01", "1.0", str(1 << 64), "nan"):
                with self.subTest(key=key, value=value):
                    self.rejected(mutate_row(valid, 0, key, value))
        self.assertEqual(
            R.parse_result(mutate_row(valid, 0, "wait_ns", "0"), "on", UIDS)["records"][
                0
            ]["durations_ns"]["wait_ns"],
            0,
        )
        self.rejected(mutate_row(valid, 0, "total_ns", "57"))
        overflow = mutate_row(valid, 0, "admission_validation_ns", str((1 << 64) - 1))
        self.rejected(overflow)

    def test_summary_constants_metrics_and_diagnostic_marker_are_strict(self):
        off = transcript("off")
        on = transcript("on")
        summary_row = 82
        for data, row, key, value, mode in (
            (off, 0, "depth", "2", "off"),
            (off, 0, "unique_ids", UID_PAIR.upper(), "off"),
            (off, 0, "forward_p95_ns", "99999", "off"),
            (off, 0, "forward_p50_GBps", "10.485", "off"),
            (on, summary_row, "diagnostic", "xgmi-host-stages-v1", "on"),
            (on, summary_row, "canaries", "fail", "on"),
        ):
            with self.subTest(key=key, mode=mode):
                self.rejected(mutate_row(data, row, key, value), mode)
        self.rejected(
            off[:-1] + b" diagnostic=aggregate-currentness-attribution\n", "off"
        )
        on_without = on.replace(
            b" diagnostic=aggregate-currentness-attribution", b"", 1
        )
        self.rejected(on_without, "on")

    def test_nested_roster_and_canonical_timing_are_mandatory(self):
        valid = transcript("on")
        fields = R._tokens(record(0))
        self.assertEqual(len(fields), 41)
        for edge in ("opening", "closing"):
            for name in R._NESTED:
                key = edge + "_" + name
                for value in (
                    "unavailable",
                    "-1",
                    "+1",
                    "01",
                    "1.0",
                    str(1 << 64),
                    "nan",
                ):
                    with self.subTest(key=key, value=value):
                        self.rejected(mutate_row(valid, 0, key, value))
                token = f" {key}={fields[key]}".encode()
                self.rejected(valid.replace(token, b"", 1))
                self.rejected(valid.replace(token, token + token, 1))

    def test_nested_checked_sums_and_all_containment_edges(self):
        valid = transcript("on")
        for edge, scale, outer in (("opening", 1, 100), ("closing", 2, 200)):
            for name, value in (
                ("pair_total_ns", 26 * scale - 1),
                ("topology_total_ns", 10 * scale - 1),
                ("topology_total_ns", 11 * scale + 1),
                ("pair_total_ns", outer + 1),
                ("source_before_ns", (1 << 64) - 1),
                ("topology_tree_ns", (1 << 64) - 1),
            ):
                with self.subTest(edge=edge, field=name):
                    self.rejected(mutate_row(valid, 0, edge + "_" + name, str(value)))
        equal = valid
        for key, value in (
            ("opening_currentness_ns", 26),
            ("closing_currentness_ns", 52),
            ("total_ns", 118),
            ("opening_topology_total_ns", 11),
            ("closing_topology_total_ns", 22),
        ):
            equal = mutate_row(equal, 0, key, str(value))
        row = R.parse_result(equal, "on", UIDS)["records"][0]
        self.assertEqual(row["durations_ns"]["total_ns"], 118)
        self.assertEqual(row["currentness_ns"]["opening"]["pair_total_ns"], 26)
        self.assertEqual(row["currentness_ns"]["closing"]["pair_total_ns"], 52)
        self.assertEqual(row["currentness_ns"]["opening"]["topology_total_ns"], 11)
        self.assertEqual(row["currentness_ns"]["closing"]["topology_total_ns"], 22)

    def test_old_diagnostic_schema_and_labels_cannot_pass_as_currentness(self):
        valid = transcript("on")
        self.rejected(
            valid.replace(
                b"aggregate-currentness-attribution.v1",
                b"aggregate-host-attribution.v1",
                1,
            )
        )
        self.rejected(
            valid.replace(
                b"diagnostic=aggregate-currentness-attribution",
                b"diagnostic=aggregate-host-attribution",
            )
        )


if __name__ == "__main__":
    unittest.main()
