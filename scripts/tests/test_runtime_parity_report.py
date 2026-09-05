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
    if backend == "kfd":
        fields["profile"] = "directional"
    return " ".join(f"{key}={value}" for key, value in fields.items())


def d2d_row(backend: str, scale: float = 1.0) -> str:
    fields = {
        "backend": backend,
        "schema": "fe2o3.d2d-copy-benchmark.v1",
        "unique_id": "6ced1647a296545c",
        "bytes": "264239137",
        "depth": "1",
        "warmups": "10",
        "samples": "30",
        "device_index": "0",
        "target": "gfx942:xnack-",
        "xnack": "disabled",
        "d2d_p50_ns": 10_000_000 * scale,
        "d2d_p95_ns": 12_000_000 * scale,
        "d2d_p50_GBps": 26.424 / scale,
    }
    if backend == "kfd":
        fields.update(
            profile="same-device-d2d",
            packet_count="64",
            window_count="2",
            doorbells_per_copy="2",
            max_packets_per_window="63",
            validation="full-source-and-destination-every-round",
            teardown="explicit",
            progress="explicit-flush-then-wait",
            timing="facade-enqueue-flush-through-observed-completion",
        )
    return " ".join(f"{key}={value}" for key, value in fields.items())


def benchmark_context(schema: str, depth: int) -> str:
    fields = {
        "schema": (
            "fe2o3.async-copy-benchmark.v1"
            if "async-copy" in schema
            else schema
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
    if "async-copy" in schema or schema == "fe2o3.d2d-copy-benchmark.v1":
        fields.update(
            kfd_profile=(
                "same-device-d2d"
                if schema == "fe2o3.d2d-copy-benchmark.v1"
                else "directional"
            ),
            sdma_manifest_sha256="7" * 64,
        )
        if "async-copy" in schema:
            fields["kfd_multi_profile"] = "directional"
        else:
            fields.update(
                sdma_manifest_sha256=CHECK_PARITY.GFX942_SDMA_MANIFEST_SHA256,
                d2d_window_manifest_sha256=(
                    CHECK_PARITY.GFX942_D2D_WINDOW_MANIFEST_SHA256
                ),
                bytes="264239137",
                timing="submit-through-observed-completion",
                setup_validation="outside-timing",
                measurement="runtime-facade-r23-d2d-window",
            )
    else:
        fields.update(
            kfd_surface="runtime-facade",
            timing="submit-through-observed-completion",
            setup_validation="outside-timing",
            measurement="persistent-hot",
            mapping_lifetime="persistent-no-host-access-between-timed-rounds",
        )
    return "context " + " ".join(f"{key}={value}" for key, value in fields.items())


def evidence(rows: list[str], schema: str, depth: int = 16) -> list[str]:
    multi = schema in {
        "fe2o3.async-copy-multi-device-benchmark.v1",
        "fe2o3.xgmi-peer-benchmark.v1",
    }
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


def multi_device_rows() -> list[str]:
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
    return rows


class CheckParityTests(unittest.TestCase):
    def test_vecadd_hip_launch_matches_the_kfd_global_extent(self) -> None:
        repository = MODULE_PATH.parents[2]
        hip_source = (MODULE_PATH.parent / "vecadd_module_hip.cpp").read_text(
            encoding="utf-8"
        )
        kfd_source = (
            repository
            / "crates"
            / "fe2o3-runtime"
            / "src"
            / "qualification_gfx942_vecadd_v1.rs"
        ).read_text(encoding="utf-8")

        self.assertIn(
            "grid: [GFX942_VECADD_QUALIFICATION_ELEMENTS_V1 as u32, 1, 1]",
            kfd_source,
        )
        self.assertIn(
            "static_assert(kQualifiedGlobalExtent == kQualifiedElementCount)",
            hip_source,
        )
        self.assertIn(
            "hipModuleLaunchKernel(function, kQualifiedBlockCount, 1, 1,",
            hip_source,
        )
        self.assertNotIn("hipModuleLaunchKernel(function, grid", hip_source)

    def test_d2d_native_comparators_emit_the_checked_device_index_field(self) -> None:
        benchmark_dir = MODULE_PATH.parent
        for source_name in ("d2d_copy_hsa.cpp", "d2d_copy_hip.cpp"):
            source = (benchmark_dir / source_name).read_text(encoding="utf-8")
            output_literal = next(
                line for line in source.splitlines() if "backend=" in line
            )
            self.assertIn("device_index=", output_literal, source_name)
            self.assertNotIn("gpu_index=", output_literal, source_name)

    def test_accepts_matched_d2d_rows_within_explicit_thresholds(self) -> None:
        schema = "fe2o3.d2d-copy-benchmark.v1"
        output = CHECK_PARITY.check_rows(
            evidence(
                [d2d_row("kfd", 1.05), d2d_row("hsa"), d2d_row("hip")],
                schema,
                1,
            ),
            schema,
            1.1,
            0.9,
        )
        self.assertEqual(output[-1], "parity_status=pass")

    def test_rejects_d2d_context_without_exact_window_manifest(self) -> None:
        schema = "fe2o3.d2d-copy-benchmark.v1"
        lines = evidence(
            [d2d_row("kfd"), d2d_row("hsa"), d2d_row("hip")], schema, 1
        )
        lines[0] = lines[0].replace(
            "d2d_window_manifest_sha256="
            + CHECK_PARITY.GFX942_D2D_WINDOW_MANIFEST_SHA256,
            "d2d_window_manifest_sha256=" + "8" * 64,
        )
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "window manifest identity"):
            CHECK_PARITY.check_rows(lines, schema, 1.1, 0.9)

    def test_rejects_d2d_rows_outside_explicit_thresholds(self) -> None:
        schema = "fe2o3.d2d-copy-benchmark.v1"
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "parity_status=fail"):
            CHECK_PARITY.check_rows(
                evidence(
                    [d2d_row("kfd", 2.0), d2d_row("hsa"), d2d_row("hip")],
                    schema,
                    1,
                ),
                schema,
                1.1,
                0.9,
            )

    def test_rejects_d2d_kfd_row_with_wrong_window_accounting(self) -> None:
        schema = "fe2o3.d2d-copy-benchmark.v1"
        kfd = d2d_row("kfd").replace("doorbells_per_copy=2", "doorbells_per_copy=1")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "doorbells_per_copy"):
            CHECK_PARITY.check_rows(
                evidence([kfd, d2d_row("hsa"), d2d_row("hip")], schema, 1),
                schema,
                1.1,
                0.9,
            )

    def test_rejects_d2d_single_packet_or_inconsistent_metrics(self) -> None:
        schema = "fe2o3.d2d-copy-benchmark.v1"
        rows = [d2d_row("kfd"), d2d_row("hsa"), d2d_row("hip")]
        short = [value.replace("bytes=264239137", "bytes=1048576") for value in rows]
        lines = evidence(short, schema, 1)
        lines[0] = lines[0].replace("bytes=264239137", "bytes=1048576")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "cross-window"):
            CHECK_PARITY.check_rows(lines, schema, 1.1, 0.9)

        inconsistent = rows.copy()
        inconsistent[0] = inconsistent[0].replace(
            "d2d_p50_GBps=26.424", "d2d_p50_GBps=20.000"
        )
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "inconsistent"):
            CHECK_PARITY.check_rows(
                evidence(inconsistent, schema, 1), schema, 1.1, 0.9
            )

        inverted = rows.copy()
        inverted[0] = inverted[0].replace(
            "d2d_p95_ns=12000000.0", "d2d_p95_ns=9000000"
        )
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "below p50"):
            CHECK_PARITY.check_rows(evidence(inverted, schema, 1), schema, 1.1, 0.9)

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

    def test_accepts_exact_bounded_striped_profile(self) -> None:
        striped = row("kfd").replace("profile=directional", "profile=striped16")
        striped += (
            " configured_queues=16 concurrency=16 doorbells_per_batch=16"
            " queue_depth=1 batch_size=16 direction=h2d-then-d2h"
        )
        lines = evidence(
            [striped, row("hsa"), row("hip")],
            "fe2o3.async-copy-benchmark.v1",
        )
        lines[0] = lines[0].replace("kfd_profile=directional", "kfd_profile=striped16")
        output = CHECK_PARITY.check_rows(
            lines,
            "fe2o3.async-copy-benchmark.v1",
            1.0,
            1.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")

    def test_rejects_striped_profile_methodology_substitution(self) -> None:
        striped = row("kfd").replace("profile=directional", "profile=striped16")
        striped += (
            " configured_queues=16 concurrency=8 doorbells_per_batch=16"
            " queue_depth=1 batch_size=16 direction=h2d-then-d2h"
        )
        lines = evidence(
            [striped, row("hsa"), row("hip")],
            "fe2o3.async-copy-benchmark.v1",
        )
        lines[0] = lines[0].replace("kfd_profile=directional", "kfd_profile=striped16")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "concurrency"):
            CHECK_PARITY.check_rows(
                lines,
                "fe2o3.async-copy-benchmark.v1",
                1.0,
                1.0,
            )

    def test_rejects_unbalanced_striped_profile(self) -> None:
        lines = evidence(
            [row("kfd").replace("profile=directional", "profile=striped3"), row("hsa"), row("hip")],
            "fe2o3.async-copy-benchmark.v1",
        )
        lines[0] = lines[0].replace("kfd_profile=directional", "kfd_profile=striped3")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "unsupported"):
            CHECK_PARITY.check_rows(
                lines,
                "fe2o3.async-copy-benchmark.v1",
                1.0,
                1.0,
            )

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

    def test_rejects_missing_declared_depth_and_profile_substitution(self) -> None:
        lines = evidence(
            [row("kfd"), row("hsa"), row("hip")],
            "fe2o3.async-copy-benchmark.v1",
        )
        lines[0] = lines[0].replace("depths=16", "depths=1,16")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "declared depths"):
            CHECK_PARITY.check_rows(
                lines,
                "fe2o3.async-copy-benchmark.v1",
                1.0,
                1.0,
            )

        substituted = evidence(
            [row("kfd").replace("profile=directional", "profile=generic"), row("hsa"), row("hip")],
            "fe2o3.async-copy-benchmark.v1",
        )
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "profile does not match"):
            CHECK_PARITY.check_rows(
                substituted,
                "fe2o3.async-copy-benchmark.v1",
                1.0,
                1.0,
            )

    def test_preserves_exact_decimal_threshold(self) -> None:
        kfd = row("kfd").replace("h2d_p50_ns=100.0", "h2d_p50_ns=1.00000000000000015")
        hsa = row("hsa").replace("h2d_p50_ns=100.0", "h2d_p50_ns=1")
        hip = row("hip").replace("h2d_p50_ns=100.0", "h2d_p50_ns=1")
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "parity_status=fail"):
            CHECK_PARITY.check_rows(
                evidence([kfd, hsa, hip], "fe2o3.async-copy-benchmark.v1"),
                "fe2o3.async-copy-benchmark.v1",
                CHECK_PARITY.Decimal("1.00000000000000012"),
                CHECK_PARITY.Decimal("1"),
            )

    def test_accepts_multi_device_schema(self) -> None:
        output = CHECK_PARITY.check_rows(
            evidence(multi_device_rows(), "fe2o3.async-copy-multi-device-benchmark.v1", 8),
            "fe2o3.async-copy-multi-device-benchmark.v1",
            1.0,
            1.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")

    def test_separates_single_and_multi_device_kfd_profiles(self) -> None:
        single = evidence(
            [row("kfd").replace("profile=directional", "profile=generic"), row("hsa"), row("hip")],
            "fe2o3.async-copy-benchmark.v1",
        )
        single[0] = single[0].replace("kfd_profile=directional", "kfd_profile=generic")
        output = CHECK_PARITY.check_rows(
            single,
            "fe2o3.async-copy-benchmark.v1",
            1.0,
            1.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")

        multi = evidence(
            multi_device_rows(),
            "fe2o3.async-copy-multi-device-benchmark.v1",
            8,
        )
        multi[0] = multi[0].replace("kfd_profile=directional", "kfd_profile=generic")
        output = CHECK_PARITY.check_rows(
            multi,
            "fe2o3.async-copy-multi-device-benchmark.v1",
            1.0,
            1.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")

        substituted = multi.copy()
        substituted[0] = substituted[0].replace(
            "kfd_multi_profile=directional", "kfd_multi_profile=generic"
        )
        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "kfd_multi_profile"):
            CHECK_PARITY.check_rows(
                substituted,
                "fe2o3.async-copy-multi-device-benchmark.v1",
                1.0,
                1.0,
            )

        legacy = multi.copy()
        legacy[0] = legacy[0].replace(" kfd_multi_profile=directional", "")
        output = CHECK_PARITY.check_rows(
            legacy,
            "fe2o3.async-copy-multi-device-benchmark.v1",
            1.0,
            1.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")

    def test_accepts_xgmi_schema(self) -> None:
        rows = []
        for backend in ("kfd", "hsa", "hip"):
            fields = [
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
            ]
            if backend == "kfd":
                fields.extend(
                    (
                        "surface=runtime-facade",
                        "target=gfx942:xnack-",
                        "queue_depth=8",
                        "batch_size=8",
                        "direction=forward-then-reverse",
                        "outstanding_depth=8",
                        "engine_parallelism=ordered-single-sdma",
                        "measurement=persistent-hot",
                        "peer_access=topology-xgmi",
                        "mapping_lifetime=persistent-no-host-access-between-timed-rounds",
                        "prime_batches=1",
                        "doorbells_per_batch=1",
                        "progress=explicit-flush-then-wait",
                        "background_progress=false",
                        "forward_engine=topology-selected",
                        "reverse_engine=topology-selected",
                        "canaries=pass",
                        "teardown=explicit",
                        "timing=facade-enqueue-flush-through-observed-completion",
                    )
                )
            rows.append(" ".join(fields))
        persistent = rows[0]
        diagnostic = persistent.replace(
            "measurement=persistent-hot",
            "measurement=remap-per-round",
        ).replace(
            "mapping_lifetime=persistent-no-host-access-between-timed-rounds",
            "mapping_lifetime=host-access-between-rounds",
        ).replace(
            "prime_batches=1",
            "prime_batches=0",
        )
        rows.insert(0, diagnostic)
        output = CHECK_PARITY.check_rows(
            evidence(rows, "fe2o3.xgmi-peer-benchmark.v1", 8),
            "fe2o3.xgmi-peer-benchmark.v1",
            1.0,
            1.0,
        )
        self.assertEqual(output[-1], "parity_status=pass")

        with self.assertRaisesRegex(CHECK_PARITY.CheckError, "remap diagnostic"):
            CHECK_PARITY.check_rows(
                evidence(rows[1:], "fe2o3.xgmi-peer-benchmark.v1", 8),
                "fe2o3.xgmi-peer-benchmark.v1",
                1.0,
                1.0,
            )


if __name__ == "__main__":
    unittest.main()
