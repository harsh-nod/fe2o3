#!/usr/bin/env python3

import importlib.util
from pathlib import Path
import unittest


SPEC = importlib.util.spec_from_file_location(
    "xgmi_peer_hot_results", Path(__file__).with_name("results.py")
)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("cannot load results.py")
results = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(results)


UIDS = ["0xab83d2ffef0d3cdf", "0xd2e26fef80cf5c33"]
UID_PAIR = "ab83d2ffef0d3cdf,d2e26fef80cf5c33"


def row(backend: str) -> bytes:
    metrics = (
        "forward_p50_ns=100000 forward_p95_ns=110000 "
        "forward_p50_GBps=10.486 reverse_p50_ns=200000 "
        "reverse_p95_ns=220000 reverse_p50_GBps=5.243"
    )
    if backend == "hip":
        text = (
            "backend=hip schema=fe2o3.xgmi-peer-persistent-hot-benchmark.v1 "
            "surface=native-api devices=0,1 "
            f"unique_ids={UID_PAIR} "
            "targets=gfx942:sramecc+:xnack-,gfx942:sramecc+:xnack- "
            "bytes=1048576 depth=1 warmups=10 samples=30 peer_access=enabled "
            "measurement=persistent-hot mapping_lifetime=process-persistent-hot "
            "prime_batches=1 direction=forward-then-reverse outstanding_depth=1 "
            "engine_parallelism=runtime-selected-unknown "
            "progress=peer-async-then-stream-synchronize "
            "timing=native-enqueue-through-observed-completion canaries=pass "
            f"teardown=explicit {metrics}"
        )
    elif backend == "hsa":
        text = (
            "backend=hsa schema=fe2o3.xgmi-peer-persistent-hot-benchmark.v1 "
            "surface=native-api measurement=persistent-hot "
            "mapping_lifetime=process-persistent-hot prime_batches=1 "
            "direction=forward-then-reverse outstanding_depth=1 "
            "engine_parallelism=runtime-selected-unknown "
            "progress=peer-async-then-signal-wait-reset "
            "timing=native-enqueue-through-observed-completion canaries=pass "
            "teardown=explicit gpu_indices=0,1 "
            f"unique_ids={UID_PAIR} targets=gfx942,gfx942 xnack=disabled "
            f"bytes=1048576 depth=1 warmups=10 samples=30 {metrics}"
        )
    elif backend == "kfd":
        text = (
            "backend=kfd schema=fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1 "
            "surface=runtime-facade "
            f"unique_ids={UID_PAIR} target=gfx942:xnack- bytes=1048576 depth=1 "
            "queue_depth=1 batch_size=1 direction=forward-then-reverse "
            "outstanding_depth=1 engine_parallelism=ordered-single-sdma "
            "warmups=10 samples=30 measurement=persistent-hot "
            "peer_access=topology-xgmi "
            "mapping_lifetime=persistent-no-host-access-between-timed-rounds "
            "prime_batches=1 doorbells_per_batch=1 "
            "progress=explicit-exact-roster-aggregate-wait "
            "aggregate_roster=exact-round-submissions background_progress=false "
            "forward_engine=topology-selected reverse_engine=topology-selected "
            f"{metrics} canaries=pass teardown=explicit "
            "timing=facade-enqueue-through-aggregate-close"
        )
    else:
        raise AssertionError(backend)
    return (text + "\n").encode("ascii")


def mutate(data: bytes, key: str, value: str) -> bytes:
    tokens = data.decode("ascii").rstrip("\n").split(" ")
    prefix = key + "="
    matches = [index for index, token in enumerate(tokens) if token.startswith(prefix)]
    if len(matches) != 1:
        raise AssertionError((key, matches))
    tokens[matches[0]] = prefix + value
    return (" ".join(tokens) + "\n").encode("ascii")


class ResultParserTests(unittest.TestCase):
    def assert_rejected(self, data: bytes, backend: str = "hip", ids=UIDS):
        with self.assertRaises(ValueError):
            results.parse_result(data, backend, ids)

    def test_valid_self_contained_rows_for_all_producers(self):
        for backend in ("hip", "hsa", "kfd"):
            with self.subTest(backend=backend):
                parsed = results.parse_result(row(backend), backend, UIDS)
                self.assertEqual(parsed["backend"], backend)
                self.assertEqual(parsed["unique_ids"], UID_PAIR)
                self.assertEqual(parsed["forward_p50_ns"], "100000")

    def test_expected_ids_are_canonicalized_but_output_must_be_canonical(self):
        upper = ["0xAB83D2FFEF0D3CDF", "0xD2E26FEF80CF5C33"]
        self.assertEqual(
            results.parse_result(row("kfd"), "kfd", upper)["unique_ids"], UID_PAIR
        )
        for ids in (
            [UIDS[0]],
            (UIDS[0], UIDS[1]),
            [UIDS[0][2:], UIDS[1]],
            ["0x0", UIDS[1]],
            [UIDS[0], UIDS[0]],
            ["0x10000000000000000", UIDS[1]],
        ):
            with self.subTest(ids=ids):
                self.assert_rejected(row("kfd"), "kfd", ids)
        self.assert_rejected(mutate(row("kfd"), "unique_ids", UID_PAIR.upper()), "kfd")

    def test_framing_size_and_ascii_are_strict(self):
        valid = row("hip")
        cases = [
            valid[:-1],
            valid + b"\n",
            b" " + valid,
            valid[:-1] + b" \n",
            valid.replace(b" surface=", b"  surface=", 1),
            valid.replace(b" surface=", b"\tsurface=", 1),
            valid.replace(b"hip", b"h\xffp", 1),
            b"x" * 8192 + b"\n",
        ]
        for data in cases:
            with self.subTest(data=data[:40]):
                self.assert_rejected(data)

    def test_duplicate_extra_missing_and_extra_word_are_rejected(self):
        valid = row("hip")
        cases = [
            valid[:-1] + b" backend=hip\n",
            valid[:-1] + b" extra=value\n",
            valid.replace(b" surface=native-api", b"", 1),
            valid[:-1] + b" word\n",
            valid.replace(b"surface=native-api", b"surface=native=api", 1),
        ]
        for data in cases:
            with self.subTest(data=data[-50:]):
                self.assert_rejected(data)

    def test_backend_and_constant_mutations_are_rejected(self):
        cases = {
            "schema": "wrong.v1",
            "surface": "driver",
            "bytes": "1048575",
            "depth": "2",
            "warmups": "9",
            "samples": "29",
            "measurement": "remap-per-round",
            "mapping_lifetime": "other",
            "prime_batches": "0",
            "direction": "reverse-then-forward",
            "outstanding_depth": "2",
            "engine_parallelism": "unknown",
            "progress": "other",
            "timing": "other",
            "canaries": "fail",
            "teardown": "implicit",
        }
        for backend in ("hip", "hsa", "kfd"):
            for key, value in cases.items():
                with self.subTest(backend=backend, key=key):
                    self.assert_rejected(mutate(row(backend), key, value), backend)
        self.assert_rejected(row("hip"), "unknown")
        self.assert_rejected(row("hip"), "hsa")

    def test_backend_specific_constants_are_rejected(self):
        mutations = [
            ("hip", "devices", "1,0"),
            ("hip", "peer_access", "topology-xgmi"),
            ("hsa", "gpu_indices", "1,0"),
            ("hsa", "xnack", "enabled"),
            ("kfd", "target", "gfx942:xnack+"),
            ("kfd", "queue_depth", "2"),
            ("kfd", "batch_size", "2"),
            ("kfd", "doorbells_per_batch", "2"),
            ("kfd", "aggregate_roster", "complete-ready-or-inflight"),
            ("kfd", "background_progress", "true"),
            ("kfd", "forward_engine", "runtime-selected"),
            ("kfd", "reverse_engine", "runtime-selected"),
        ]
        for backend, key, value in mutations:
            with self.subTest(backend=backend, key=key):
                self.assert_rejected(mutate(row(backend), key, value), backend)

    def test_wrong_uid_or_order_is_rejected(self):
        replacements = [
            "d2e26fef80cf5c33,ab83d2ffef0d3cdf",
            "ab83d2ffef0d3cdf,0000000000000001",
            "0xab83d2ffef0d3cdf,d2e26fef80cf5c33",
            "ab83d2ffef0d3cdf,d2e26fef80cf5c3",
        ]
        for backend in ("hip", "hsa", "kfd"):
            for value in replacements:
                with self.subTest(backend=backend, value=value):
                    self.assert_rejected(
                        mutate(row(backend), "unique_ids", value), backend
                    )

    def test_target_spellings_are_narrow(self):
        for target in ("gfx942:xnack-", "gfx942:sramecc-:xnack-"):
            pair = f"{target},{target}"
            self.assertEqual(
                results.parse_result(mutate(row("hip"), "targets", pair), "hip", UIDS)[
                    "targets"
                ],
                pair,
            )
        for target in (
            "gfx942:sramecc+:xnack+",
            "gfx941:sramecc+:xnack-",
            "gfx942:sramecc+:xnack-,gfx942:sramecc-:xnack-",
            "gfx942,gfx942",
        ):
            with self.subTest(target=target):
                self.assert_rejected(mutate(row("hip"), "targets", target))
        for target in ("gfx942", "gfx942:xnack-", "gfx942,gfx941"):
            with self.subTest(target=target):
                self.assert_rejected(mutate(row("hsa"), "targets", target), "hsa")

    def test_integer_metrics_are_positive_canonical_and_bounded(self):
        invalid = ("-1", "0", "+1", "01", str(1 << 64), "1.0", "nan")
        for key in (
            "forward_p50_ns",
            "forward_p95_ns",
            "reverse_p50_ns",
            "reverse_p95_ns",
        ):
            for value in invalid:
                with self.subTest(key=key, value=value):
                    self.assert_rejected(mutate(row("hip"), key, value))

    def test_quantile_inversion_is_rejected(self):
        self.assert_rejected(mutate(row("hip"), "forward_p95_ns", "99999"))
        self.assert_rejected(mutate(row("hip"), "reverse_p95_ns", "199999"))

    def test_bandwidth_format_and_value_are_strict(self):
        for key in ("forward_p50_GBps", "reverse_p50_GBps"):
            for value in ("nan", "inf", "-1.000", "1", "1.00", "01.000", "+1.000"):
                with self.subTest(key=key, value=value):
                    self.assert_rejected(mutate(row("hip"), key, value))
        self.assert_rejected(mutate(row("hip"), "forward_p50_GBps", "10.485"))
        self.assert_rejected(mutate(row("hip"), "reverse_p50_GBps", "5.244"))

    def test_bandwidth_rounding_tolerance_accepts_source_precision(self):
        parsed = results.parse_result(
            mutate(
                mutate(
                    mutate(row("hip"), "forward_p50_ns", str((1 << 64) - 1)),
                    "forward_p95_ns",
                    str((1 << 64) - 1),
                ),
                "forward_p50_GBps",
                "0.000",
            ),
            "hip",
            UIDS,
        )
        self.assertEqual(parsed["forward_p50_GBps"], "0.000")


if __name__ == "__main__":
    unittest.main()
