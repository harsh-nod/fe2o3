import importlib.util
import pathlib
import unittest


MODULE_PATH = (
    pathlib.Path(__file__).parents[2]
    / "benchmarks"
    / "runtime_gfx942"
    / "check-parity.py"
)
SPEC = importlib.util.spec_from_file_location("check_parity", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECK_PARITY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK_PARITY)


def row(backend: str, scale: float = 1.0) -> str:
    fields = {
        "backend": backend,
        "schema": "fe2o3.async-copy-benchmark.v1",
        "unique_id": "6ced1647a296545c",
        "bytes": "1048576",
        "depth": "16",
        "warmups": "10",
        "samples": "30",
        "h2d_p50_ns": 100 * scale,
        "h2d_p95_ns": 120 * scale,
        "h2d_p50_GBps": 20 / scale,
        "d2h_p50_ns": 80 * scale,
        "d2h_p95_ns": 100 * scale,
        "d2h_p50_GBps": 25 / scale,
    }
    return " ".join(f"{key}={value}" for key, value in fields.items())


def benchmark_context(schema: str, depth: int) -> str:
    fields = {
        "schema": (
            "fe2o3.async-copy-benchmark.v1"
            if "async-copy" in schema
            else "fe2o3.xgmi-peer-benchmark.v1"
        ),
        "git_commit": "a0695a10e49ea6c4a211811e88a0f4da8ca46044",
        "target": "gfx942:xnack-",
        "gpu_indices": "0,1",
        "unique_ids": "0x6ced1647a296545c,0xab83d2ffef0d3cdf",
        "bytes": "1048576",
        "depths": str(depth),
        "warmups": "10",
        "samples": "30",
        "max_busy_percent": "5",
        "phase_timeout_seconds": "120",
        "rocm_version": "7.2.4",
        "rustc": "rustc_1.97.1",
    }
    if "async-copy" in schema:
        fields.update(
            kfd_profile="directional",
            sdma_manifest_sha256="7" * 64,
        )
    else:
        fields.update(
            kfd_surface="runtime-facade",
            timing="submit-through-observed-completion",
            setup_validation="outside-timing",
        )
    return "context " + " ".join(f"{key}={value}" for key, value in fields.items())


def evidence(rows: list[str], schema: str, depth: int = 16) -> list[str]:
    multi = schema != "fe2o3.async-copy-benchmark.v1"
    phase_suffix = "-multi" if "multi-device" in schema else ""
    depth_field = "depth_per_device" if phase_suffix else "depth"
    load = "0,0" if multi else "0"
    phases = []
    for backend in ("kfd", "hsa", "hip"):
        for edge in ("start", "end"):
            phases.append(
                " ".join(
                    (
                        "context",
                        f"phase={backend}{phase_suffix}",
                        f"{depth_field}={depth}",
                        f"gpu_busy_{edge}_percent={load}",
                    )
                )
            )
    return [benchmark_context(schema, depth), *phases, *rows]


class CheckParityTests(unittest.TestCase):
    def test_accepts_complete_rows_within_explicit_thresholds(self) -> None:
        output = CHECK_PARITY.check_rows(
            evidence(
                [row("kfd", 1.05), row("hsa"), row("hip")],
                "fe2o3.async-copy-benchmark.v1",
            ),
            "fe2o3.async-copy-benchmark.v1",
            1.1,
            0.9,
        )
        self.assertEqual(output[-1], "parity_status=pass")

    def test_accepts_explicit_tenfold_speedup_thresholds(self) -> None:
        output = CHECK_PARITY.check_rows(
            evidence(
                [row("kfd", 0.1), row("hsa"), row("hip")],
                "fe2o3.async-copy-benchmark.v1",
            ),
            "fe2o3.async-copy-benchmark.v1",
            0.1,
            10.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")

    def test_rejects_slow_kfd_row(self) -> None:
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "parity_status=fail"):
            CHECK_PARITY.check_rows(
                evidence(
                    [row("kfd", 1.5), row("hsa"), row("hip")],
                    "fe2o3.async-copy-benchmark.v1",
                ),
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_rejects_missing_reference(self) -> None:
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "missing backends: hip"):
            CHECK_PARITY.check_rows(
                evidence(
                    [row("kfd"), row("hsa")],
                    "fe2o3.async-copy-benchmark.v1",
                ),
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_rejects_duplicate_backend_row(self) -> None:
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "duplicate kfd row"):
            CHECK_PARITY.check_rows(
                evidence(
                    [row("kfd"), row("kfd"), row("hsa"), row("hip")],
                    "fe2o3.async-copy-benchmark.v1",
                ),
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_rejects_invalid_metric(self) -> None:
        broken = row("kfd").replace("h2d_p50_ns=100.0", "h2d_p50_ns=nan")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "finite and positive"):
            CHECK_PARITY.check_rows(
                evidence(
                    [broken, row("hsa"), row("hip")],
                    "fe2o3.async-copy-benchmark.v1",
                ),
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_rejects_mismatched_sample_count(self) -> None:
        mismatched = row("hsa").replace("samples=30", "samples=31")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "mismatched"):
            CHECK_PARITY.check_rows(
                evidence(
                    [row("kfd"), mismatched, row("hip")],
                    "fe2o3.async-copy-benchmark.v1",
                ),
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_rejects_noncanonical_unique_id(self) -> None:
        broken = row("kfd").replace("6ced1647a296545c", "0x6CED1647A296545C")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "16 lowercase"):
            CHECK_PARITY.check_rows(
                evidence(
                    [broken, row("hsa"), row("hip")],
                    "fe2o3.async-copy-benchmark.v1",
                ),
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_rejects_zero_threshold(self) -> None:
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "finite and positive"):
            CHECK_PARITY.check_rows(
                evidence(
                    [row("kfd"), row("hsa"), row("hip")],
                    "fe2o3.async-copy-benchmark.v1",
                ),
                "fe2o3.async-copy-benchmark.v1",
                0.0,
                1.0,
            )

    def test_rejects_context_free_rows(self) -> None:
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "missing benchmark context"):
            CHECK_PARITY.check_rows(
                [row("kfd"), row("hsa"), row("hip")],
                "fe2o3.async-copy-benchmark.v1",
                1.0,
                1.0,
            )

    def test_rejects_row_device_not_bound_by_context(self) -> None:
        lines = evidence(
            [row("kfd"), row("hsa"), row("hip")],
            "fe2o3.async-copy-benchmark.v1",
        )
        lines[0] = lines[0].replace("0x6ced1647a296545c", "0x0123456789abcdef")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "context device IDs"):
            CHECK_PARITY.check_rows(
                lines,
                "fe2o3.async-copy-benchmark.v1",
                1.0,
                1.0,
            )

    def test_rejects_busy_or_incomplete_phase_evidence(self) -> None:
        lines = evidence(
            [row("kfd"), row("hsa"), row("hip")],
            "fe2o3.async-copy-benchmark.v1",
        )
        lines[1] = lines[1].replace("gpu_busy_start_percent=0", "gpu_busy_start_percent=6")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "phase load exceeds"):
            CHECK_PARITY.check_rows(
                lines,
                "fe2o3.async-copy-benchmark.v1",
                1.0,
                1.0,
            )

        del lines[1]
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "missing phase load"):
            CHECK_PARITY.check_rows(
                lines,
                "fe2o3.async-copy-benchmark.v1",
                1.0,
                1.0,
            )

    def test_extreme_finite_metrics_do_not_use_binary_float_ratios(self) -> None:
        fast = row("kfd").replace("h2d_p50_GBps=20.0", "h2d_p50_GBps=1e308")
        hsa = row("hsa").replace("h2d_p50_GBps=20.0", "h2d_p50_GBps=1e-308")
        hip = row("hip").replace("h2d_p50_GBps=20.0", "h2d_p50_GBps=1e-308")
        output = CHECK_PARITY.check_rows(
            evidence([fast, hsa, hip], "fe2o3.async-copy-benchmark.v1"),
            "fe2o3.async-copy-benchmark.v1",
            1.0,
            1.0,
        )
        bandwidth = [line for line in output if "metric=h2d_p50_GBps" in line]
        self.assertEqual(len(bandwidth), 2)
        self.assertTrue(all("ratio=inf" not in line for line in bandwidth))

    def test_accepts_multi_device_schema(self) -> None:
        rows = []
        for backend in ("kfd", "hsa", "hip"):
            rows.append(
                " ".join(
                    (
                        f"backend={backend}",
                        "schema=fe2o3.async-copy-multi-device-benchmark.v1",
                        "devices=2",
                        "unique_ids=6ced1647a296545c,ab83d2ffef0d3cdf",
                        "bytes=1048576",
                        "depth_per_device=8",
                        "warmups=10",
                        "samples=30",
                        "h2d_p50_ns=100",
                        "h2d_p95_ns=120",
                        "h2d_aggregate_p50_GBps=40",
                        "d2h_p50_ns=110",
                        "d2h_p95_ns=130",
                        "d2h_aggregate_p50_GBps=38",
                    )
                )
            )
        output = CHECK_PARITY.check_rows(
            evidence(rows, "fe2o3.async-copy-multi-device-benchmark.v1", 8),
            "fe2o3.async-copy-multi-device-benchmark.v1",
            1.0,
            1.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")

    def test_accepts_xgmi_schema(self) -> None:
        rows = []
        for backend in ("kfd", "hsa", "hip"):
            rows.append(
                " ".join(
                    (
                        f"backend={backend}",
                        "schema=fe2o3.xgmi-peer-benchmark.v1",
                        "unique_ids=6ced1647a296545c,ab83d2ffef0d3cdf",
                        "bytes=1048576",
                        "depth=8",
                        "warmups=10",
                        "samples=30",
                        "forward_p50_ns=100",
                        "forward_p95_ns=120",
                        "forward_p50_GBps=20",
                        "reverse_p50_ns=110",
                        "reverse_p95_ns=130",
                        "reverse_p50_GBps=18",
                    )
                )
            )
        output = CHECK_PARITY.check_rows(
            evidence(rows, "fe2o3.xgmi-peer-benchmark.v1", 8),
            "fe2o3.xgmi-peer-benchmark.v1",
            1.0,
            1.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")


if __name__ == "__main__":
    unittest.main()
