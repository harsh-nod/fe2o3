#!/usr/bin/env python3
"""Independent records and hostile controls for the matched hot batch parser."""

from decimal import Decimal
import importlib.util
from pathlib import Path
import unittest


SPEC = importlib.util.spec_from_file_location(
    "hot_results", Path(__file__).with_name("xgmi_peer_hot_results.py")
)
RESULTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RESULTS)
IDS = ["0xb7baafd0fb173d8e", "0x10a254ce4987e716"]


def record(backend, depth, copy_bytes=1048576, warmups=10, samples=30):
    fields = dict(
        backend=backend, bytes=str(copy_bytes), depth=str(depth),
        warmups=str(warmups), samples=str(samples),
        unique_ids="b7baafd0fb173d8e,10a254ce4987e716",
        direction="forward-then-reverse", outstanding_depth=str(depth),
        measurement="persistent-hot", prime_batches="1", canaries="pass",
        teardown="explicit", forward_p50_ns="30000", forward_p95_ns="31000",
        reverse_p50_ns="32000", reverse_p95_ns="34000",
        forward_p50_GBps=f"{Decimal(copy_bytes * depth) / 30000:.3f}",
        reverse_p50_GBps=f"{Decimal(copy_bytes * depth) / 32000:.3f}",
    )
    if backend == "kfd":
        fields.update(
            schema="fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1",
            surface="runtime-facade", target="gfx942:xnack-",
            queue_depth=str(depth), batch_size=str(depth),
            engine_parallelism="ordered-single-sdma", peer_access="topology-xgmi",
            mapping_lifetime="persistent-no-host-access-between-timed-rounds",
            doorbells_per_batch="1", progress="explicit-exact-roster-aggregate-wait",
            aggregate_roster="exact-round-submissions", background_progress="false",
            forward_engine="topology-selected", reverse_engine="topology-selected",
            timing="facade-enqueue-through-aggregate-close",
        )
    else:
        fields.update(
            schema="fe2o3.xgmi-peer-persistent-hot-benchmark.v1",
            surface="native-api", mapping_lifetime="process-persistent-hot",
            engine_parallelism="runtime-selected-unknown",
            timing="native-enqueue-through-observed-completion",
        )
        if backend == "hip":
            fields.update(devices="0,1", targets="gfx942:sramecc+:xnack-,gfx942:sramecc+:xnack-",
                          peer_access="enabled", progress="peer-async-then-stream-synchronize")
        else:
            fields.update(gpu_indices="0,1", targets="gfx942,gfx942", xnack="disabled",
                          progress="peer-async-then-signal-wait-reset")
    return fields


def encode(fields):
    return (" ".join(f"{key}={value}" for key, value in fields.items()) + "\n").encode()


class HotResultsTests(unittest.TestCase):
    def parse(self, fields, backend="kfd", depth=32, **controls):
        args = dict(backend=backend, unique_ids=IDS, copy_bytes=1048576,
                    depth=depth, warmups=10, samples=30)
        args.update(controls)
        return RESULTS.parse_result(encode(fields) if isinstance(fields, dict) else fields, **args)

    def test_all_backend_depth_cells(self):
        for backend in ("kfd", "hip", "hsa"):
            for depth in (1, 16, 32):
                with self.subTest(backend=backend, depth=depth):
                    fields = record(backend, depth)
                    self.assertEqual(self.parse(fields, backend, depth), fields)

    def test_independent_trial_controls(self):
        fields = record("kfd", 16, copy_bytes=4096, warmups=0, samples=1)
        self.assertEqual(self.parse(fields, depth=16, copy_bytes=4096, warmups=0, samples=1), fields)
        for key in ("copy_bytes", "depth", "warmups", "samples"):
            for value in (True, False, None, "32", 1.0, -1, 1 << 64):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    self.parse(record("kfd", 32), **{key: value})
        for controls in (dict(depth=0), dict(depth=2), dict(depth=33), dict(samples=0),
                         dict(copy_bytes=0), dict(copy_bytes=1 << 63),
                         dict(copy_bytes=(1 << 64) - 32, depth=1),
                         dict(warmups=(1 << 64) - 2, samples=1), dict(backend=[])):
            with self.subTest(controls=controls), self.assertRaises(ValueError):
                self.parse(record("kfd", 32), **controls)

    def test_every_fixed_field_is_enforced(self):
        for backend in ("kfd", "hip", "hsa"):
            original = record(backend, 32)
            for key in original:
                with self.subTest(backend=backend, key=key), self.assertRaises(ValueError):
                    self.parse({**original, key: "incorrect"}, backend)
                missing = dict(original)
                del missing[key]
                with self.subTest(missing=key), self.assertRaises(ValueError):
                    self.parse(missing, backend)

    def test_wrong_depths_and_bandwidth_numerators(self):
        for backend in ("kfd", "hip", "hsa"):
            for depth in (16, 32):
                original = record(backend, depth)
                keys = ("depth", "outstanding_depth")
                if backend == "kfd":
                    keys += ("queue_depth", "batch_size")
                for key in keys:
                    with self.subTest(key=key), self.assertRaises(ValueError):
                        self.parse({**original, key: "1"}, backend, depth)
                for direction, ns in (("forward", 30000), ("reverse", 32000)):
                    for volume in (1048576, 2 * 1048576 * depth):
                        fields = {**original, f"{direction}_p50_GBps": f"{Decimal(volume) / ns:.3f}"}
                        with self.subTest(backend=backend, depth=depth, volume=volume), self.assertRaises(ValueError):
                            self.parse(fields, backend, depth)
                with self.assertRaises(ValueError):
                    self.parse(original, backend, 1)

    def test_metrics_are_canonical_ordered_and_rounded(self):
        original = record("kfd", 32)
        for direction in ("forward", "reverse"):
            for suffix in ("p50_ns", "p95_ns"):
                for value in ("0", "-1", "+1", "030000", "1.0", "1e3", str(1 << 64)):
                    with self.subTest(value=value), self.assertRaises(ValueError):
                        self.parse({**original, f"{direction}_{suffix}": value})
            with self.assertRaises(ValueError):
                self.parse({**original, f"{direction}_p95_ns": "1"})
            for value in ("nan", "inf", "-1.000", "1", "1.00", "+1.000", "01.000", "0.000"):
                with self.subTest(value=value), self.assertRaises(ValueError):
                    self.parse({**original, f"{direction}_p50_GBps": value})
            key = f"{direction}_p50_GBps"
            with self.assertRaises(ValueError):
                self.parse({**original, key: f"{Decimal(original[key]) + Decimal('0.001'):.3f}"})

    def test_identity_and_target_substitution(self):
        for ids in ([], IDS[:1], IDS + IDS, tuple(IDS), [IDS[0], IDS[0]],
                    ["0x0", IDS[1]], ["0x10000000000000000", IDS[1]],
                    ["bad", IDS[1]], [None, IDS[1]], list(reversed(IDS))):
            with self.subTest(ids=ids), self.assertRaises(ValueError):
                self.parse(record("kfd", 32), unique_ids=ids)
        for backend in ("hip", "hsa"):
            for targets in ("gfx950,gfx950", "gfx942", "gfx942:xnack+,gfx942:xnack+",
                            "gfx942:xnack-,gfx950:xnack-"):
                with self.assertRaises(ValueError):
                    self.parse({**record(backend, 32), "targets": targets}, backend)

    def test_framing_and_diagnostic_contamination(self):
        raw = encode(record("kfd", 32))
        for bad in (b"", raw[:-1], raw + raw, b" " + raw, raw[:-1] + b" \n",
                    raw.replace(b" ", b"  ", 1), raw.replace(b" ", b"\t", 1),
                    raw + b"\n", raw[:-1] + b"\r\n", raw + b"diagnostic\n",
                    raw[:-1] + b" diagnostic=true\n", raw[:-1] + b" depth=32\n",
                    raw[:-1] + b" bad=\xff\n", b"x" * 8193, raw.decode()):
            with self.subTest(bad=str(bad)[:80]), self.assertRaises(ValueError):
                self.parse(bad)


if __name__ == "__main__":
    unittest.main()
