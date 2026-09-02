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
        "bytes": "1048576",
        "depth": "16",
        "h2d_p50_ns": 100 * scale,
        "h2d_p95_ns": 120 * scale,
        "h2d_p50_GBps": 20 / scale,
        "d2h_p50_ns": 80 * scale,
        "d2h_p95_ns": 100 * scale,
        "d2h_p50_GBps": 25 / scale,
    }
    return " ".join(f"{key}={value}" for key, value in fields.items())


class CheckParityTests(unittest.TestCase):
    def test_accepts_complete_rows_within_explicit_thresholds(self) -> None:
        output = CHECK_PARITY.check_rows(
            [row("kfd", 1.05), row("hsa"), row("hip")],
            "fe2o3.async-copy-benchmark.v1",
            1.1,
            0.9,
        )
        self.assertEqual(output[-1], "parity_status=pass")

    def test_rejects_slow_kfd_row(self) -> None:
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "parity_status=fail"):
            CHECK_PARITY.check_rows(
                [row("kfd", 1.5), row("hsa"), row("hip")],
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_rejects_missing_reference(self) -> None:
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "missing backends: hip"):
            CHECK_PARITY.check_rows(
                [row("kfd"), row("hsa")],
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_rejects_duplicate_backend_row(self) -> None:
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "duplicate kfd row"):
            CHECK_PARITY.check_rows(
                [row("kfd"), row("kfd"), row("hsa"), row("hip")],
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_rejects_invalid_metric(self) -> None:
        broken = row("kfd").replace("h2d_p50_ns=100.0", "h2d_p50_ns=nan")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "finite and positive"):
            CHECK_PARITY.check_rows(
                [broken, row("hsa"), row("hip")],
                "fe2o3.async-copy-benchmark.v1",
                1.1,
                0.9,
            )

    def test_accepts_multi_device_schema(self) -> None:
        rows = []
        for backend in ("kfd", "hsa", "hip"):
            rows.append(
                " ".join(
                    (
                        f"backend={backend}",
                        "schema=fe2o3.async-copy-multi-device-benchmark.v1",
                        "bytes=1048576",
                        "depth_per_device=8",
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
            rows,
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
                        "bytes=1048576",
                        "depth=8",
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
            rows,
            "fe2o3.xgmi-peer-benchmark.v1",
            1.0,
            1.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")


if __name__ == "__main__":
    unittest.main()
