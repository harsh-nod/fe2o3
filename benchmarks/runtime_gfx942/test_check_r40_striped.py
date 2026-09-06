#!/usr/bin/env python3

from __future__ import annotations

import base64
import hashlib
import importlib.util
import pathlib
import types
import unittest
from decimal import Decimal
from unittest import mock


CHECKER_PATH = pathlib.Path(__file__).with_name("check-r40-striped.py")
SPEC = importlib.util.spec_from_file_location("fe2o3_check_r40_striped", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)

R26 = CHECKER.load_r26_checker()


def valid_row(
    backend: str, workload_id: str = "bytes4096-q2-combined", base: int = 1000
) -> dict[str, str]:
    bytes_count, queue_count, kind = CHECKER.WORKLOADS[
        CHECKER.WORKLOAD_IDS.index(workload_id)
    ]
    row = {
        "backend": backend,
        **CHECKER.FIXED_ROW,
        "workload_id": workload_id,
        "unique_id": CHECKER.EXPECTED_UNIQUE_ID.removeprefix("0x"),
        "bytes": str(bytes_count),
        "logical_queue_count": str(queue_count),
        "per_queue_depth": str(112 // queue_count),
    }
    if backend == "kfd":
        roles = ([] if kind == "standalone" else ["h2d", "d2h"]) + [
            f"striped{index}" for index in range(queue_count)
        ]
        queue_ids = [1000 + index for index in range(len(roles))]
        queue_roster = ",".join(
            f"{role}:{queue_id}"
            for role, queue_id in zip(roles, queue_ids, strict=True)
        )
        engines = ([] if kind == "standalone" else [1, 0]) + [
            index % 2 for index in range(queue_count)
        ]
        engine_placement = ",".join(
            f"{role}:{queue_id}:{engine}"
            for role, queue_id, engine in zip(roles, queue_ids, engines, strict=True)
        )
        row.update(
            {
                "api": "native-kfd-sdma",
                "resource_profile": (
                    "striped16"
                    if kind == "standalone"
                    else f"combined-striped{queue_count}"
                ),
                "physical_engine_count": "2",
                "directional_queue_count": "0" if kind == "standalone" else "2",
                "striped_queue_count": str(queue_count),
                "queue_ids": queue_roster,
                "queue_ids_sha256": hashlib.sha256(
                    queue_roster.encode("ascii")
                ).hexdigest(),
                "engine_placement": engine_placement,
                "engine_placement_sha256": hashlib.sha256(
                    engine_placement.encode("ascii")
                ).hexdigest(),
                "directional_smoke": (
                    "not-applicable" if kind == "standalone" else "pass"
                ),
                "aggregate_poll_smoke": "pass",
                "destroy": "pass",
            }
        )
    elif backend == "hip":
        row.update(
            {
                "api": "hip",
                "resource_profile": f"nonblocking-streams-q{queue_count}",
                "physical_engine_count": "n/a",
            }
        )
    else:
        row.update(
            {
                "api": "hsa-amd-memory-async-copy",
                "resource_profile": f"logical-dependency-width-q{queue_count}",
                "physical_engine_count": "not-observed",
            }
        )
    for direction_index, direction in enumerate(CHECKER.DIRECTIONS):
        submit = [base // 4 + direction_index + index for index in range(30)]
        wait = [base - base // 4 + direction_index + index for index in range(30)]
        e2e = [left + right for left, right in zip(submit, wait, strict=True)]
        for component, values in (
            ("submit", submit),
            ("wait", wait),
            ("e2e", e2e),
        ):
            stem = f"{direction}_{component}"
            row[f"{stem}_samples_ns"] = ",".join(str(value) for value in values)
            row[f"{stem}_p50_ns"] = str(CHECKER.percentile(values, 1, 2))
            row[f"{stem}_p95_ns"] = str(CHECKER.percentile(values, 19, 20))
        exact = Decimal(bytes_count * 112) / Decimal(CHECKER.percentile(e2e, 1, 2))
        row[f"{direction}_e2e_p50_GBps"] = f"{exact:.9f}"
    return row


def valid_set(kfd_base: int = 900, reference_base: int = 1000) -> dict:
    rows = {}
    for slot in CHECKER.BACKEND_ORDERS:
        for workload_id in CHECKER.WORKLOAD_IDS:
            rows[slot, workload_id, "kfd"] = valid_row("kfd", workload_id, kfd_base)
            rows[slot, workload_id, "hsa"] = valid_row(
                "hsa", workload_id, reference_base
            )
            rows[slot, workload_id, "hip"] = valid_row(
                "hip", workload_id, reference_base
            )
    return rows


def fake_r26() -> object:
    return types.SimpleNamespace(
        R26_SYSTEM_IDENTITY_SCHEMA=R26.R26_SYSTEM_IDENTITY_SCHEMA,
        R26_EXACT_SYSTEM_IDENTITY_FIELDS=frozenset(
            {
                "schema",
                "observation_edge",
                "pci_bdf",
                "pci_numa_node",
                "gpu_node_id",
                "gpu_guid",
            }
        ),
        R26_EDGE_VARIANT_SYSTEM_IDENTITY_FIELDS=frozenset({"observation_edge"}),
        R26_EXACT_TOPOLOGY_FIELDS=R26.R26_EXACT_TOPOLOGY_FIELDS,
        R26_TOPOLOGY_SEALED_FIELDS=R26.R26_TOPOLOGY_SEALED_FIELDS,
        R26_EXACT_MONITOR_FIELDS=R26.R26_EXACT_MONITOR_FIELDS,
        R26_MONITOR_SEALED_FIELDS=R26.R26_MONITOR_SEALED_FIELDS,
        R26_MONITOR_SCHEMA=R26.R26_MONITOR_SCHEMA,
        r26_parse_id_list=R26.r26_parse_id_list,
        r26_validate_system_identity=lambda identity, context: None,
    )


def canonical(prefix: str, fields: dict[str, str], order: tuple[str, ...]) -> str:
    return " ".join((prefix,) + tuple(f"{key}={fields[key]}" for key in order))


def valid_context(slot: int) -> dict[str, str]:
    context = {field: "x" for field in CHECKER.CONTEXT_FIELDS}
    context.update(
        {
            "schema": CHECKER.EVIDENCE_SCHEMA,
            "git_commit": "1" * 40,
            "target": "gfx942:xnack-",
            "gpu_index": "2",
            "unique_id": CHECKER.EXPECTED_UNIQUE_ID,
            "uuid": "GPU-d2e26fef80cf5c33",
            "depth": "112",
            "warmups": "10",
            "samples": "30",
            "bytes_set": "4096,1048576",
            "logical_queue_counts": "2,4,8,14,16",
            "profiles": "combined-striped2,combined-striped4,combined-striped8,combined-striped14,striped16",
            "max_busy_percent": "5",
            "phase_timeout_seconds": "180",
            "build_environment": "env-i-explicit-home-toolchain-path-cargo-incremental-0-private-target-v1",
            "execution_environment": "env-i-lang-c-lc-all-c-path-usr-sbin-usr-bin-sbin-bin-v1",
            "telemetry_command": "rocm-smi-showuse-showclocks-showpower",
            "placement": "taskset-cpulist-then-numactl-physcpubind-membind-v1",
            "interference_monitor": "selected-kfd-gpu-process-tree-census-v2",
            "monitor_interval_us": "2000",
            "monitor_maximum_gap_us": "10000",
            "counterbalance_design": "cyclic-latin-square-3-backends-workload-forward-reverse-rotate5-v1",
            "counterbalance_slots": "3",
            "counterbalance_slot": str(slot),
            "counterbalance_set_id": "2" * 64,
            "backend_order": ",".join(CHECKER.BACKEND_ORDERS[slot]),
            "workload_order": ",".join(CHECKER.WORKLOAD_ORDERS[slot]),
            "claim_scope": "single-mi300x-gpu2-striped-copy-only",
        }
    )
    for field in CHECKER.CONTEXT_FIELDS:
        if field.endswith("_sha256"):
            context[field] = "3" * 64
    topology = {
        "schema": R26.R26_TOPOLOGY_SCHEMA,
        "placement": context["placement"],
        "gpu_index": "2",
        "pci_bdf": "0000:05:00.0",
        "unique_id": CHECKER.EXPECTED_UNIQUE_ID,
        "numa_node": "0",
        "device_local_cpu_list": "0-2",
        "allowed_cpu_list": "0-3",
        "allowed_mem_node_list": "0",
        "measurement_cpu_list": "0-2",
        "observer_cpu": "3",
        "kfd_node": "2",
        "kfd_gpu_id": "28851",
    }
    context["topology_sha256"] = hashlib.sha256(
        (
            canonical("topology", topology, R26.R26_TOPOLOGY_SEALED_FIELDS) + "\n"
        ).encode()
    ).hexdigest()
    return context


def identity_line(edge: str) -> str:
    fields = {
        "schema": R26.R26_SYSTEM_IDENTITY_SCHEMA,
        "observation_edge": edge,
        "pci_bdf": "0000:05:00.0",
        "pci_numa_node": "0",
        "gpu_node_id": "2",
        "gpu_guid": "28851",
    }
    return " ".join(
        ("context", f"schema={fields['schema']}")
        + tuple(f"{key}={fields[key]}" for key in sorted(fields.keys() - {"schema"}))
    )


def valid_slot_lines(slot: int) -> list[str]:
    context = valid_context(slot)
    lines = [
        "context " + " ".join(f"{key}={value}" for key, value in context.items()),
        identity_line("start"),
    ]
    topology_inner = {
        "schema": R26.R26_TOPOLOGY_SCHEMA,
        "placement": context["placement"],
        "gpu_index": "2",
        "pci_bdf": "0000:05:00.0",
        "unique_id": CHECKER.EXPECTED_UNIQUE_ID,
        "numa_node": "0",
        "device_local_cpu_list": "0-2",
        "allowed_cpu_list": "0-3",
        "allowed_mem_node_list": "0",
        "measurement_cpu_list": "0-2",
        "observer_cpu": "3",
        "kfd_node": "2",
        "kfd_gpu_id": "28851",
    }
    topology_digest = hashlib.sha256(
        (
            canonical("topology", topology_inner, R26.R26_TOPOLOGY_SEALED_FIELDS) + "\n"
        ).encode()
    ).hexdigest()
    telemetry_bytes = b"GPU[2] : GPU use (%): 0\nGPU[2] : Clock: 1\nGPU[2] : Power: 1\n"
    telemetry_digest = hashlib.sha256(telemetry_bytes).hexdigest()
    telemetry_base64 = base64.b64encode(telemetry_bytes).decode()
    sequence = 0
    for workload_id in CHECKER.WORKLOAD_ORDERS[slot]:
        for backend in CHECKER.BACKEND_ORDERS[slot]:
            phase_id = f"{workload_id}.{backend}"
            lines.append(
                f"phase slot={slot} sequence={sequence} workload_id={workload_id} "
                f"backend={backend} phase_id={phase_id}"
            )
            topologies = []
            for edge in ("start", "end"):
                fields = {
                    "slot": str(slot),
                    "phase": phase_id,
                    "edge": edge,
                    **topology_inner,
                    "topology_sha256": topology_digest,
                }
                topologies.append(
                    canonical(
                        "topology",
                        fields,
                        ("slot", "phase", "edge")
                        + R26.R26_TOPOLOGY_SEALED_FIELDS
                        + ("topology_sha256",),
                    )
                )
            telemetry = [
                f"telemetry phase={phase_id} edge={edge} gpu_busy_percent=0 "
                f"telemetry_sha256={telemetry_digest} telemetry_base64={telemetry_base64}"
                for edge in ("start", "end")
            ]
            row = valid_row(backend, workload_id, 900 if backend == "kfd" else 1000)
            row_line = " ".join(f"{key}={value}" for key, value in row.items())
            monitor_inner = {
                "schema": R26.R26_MONITOR_SCHEMA,
                "status": "clean",
                "monitor": context["interference_monitor"],
                "schedule": "absolute-monotonic-raw-deadline-v1",
                "kfd_gpu_id": "28851",
                "root_pid": str(10000 + slot * 100 + sequence),
                "process_group": str(10000 + slot * 100 + sequence),
                "observer_cpu": "3",
                "interval_us": "2000",
                "maximum_gap_us": "10000",
                "observed_maximum_gap_us": "2500",
                "observations": "4",
                "target_selected_queue_observations": "3",
                "foreign_selected_queues": "0",
                "terminal_selected_queues": "0",
                "target_exit_code": "0",
                "target_reaped": "1",
                "process_group_absent": "1",
                "target_output_bytes": str(len((row_line + "\n").encode())),
                "target_output_sha256": hashlib.sha256(
                    (row_line + "\n").encode()
                ).hexdigest(),
            }
            monitor_digest = hashlib.sha256(
                (
                    canonical("monitor", monitor_inner, R26.R26_MONITOR_SEALED_FIELDS)
                    + "\n"
                ).encode()
            ).hexdigest()
            monitor = canonical(
                "monitor",
                {
                    "slot": str(slot),
                    "phase": phase_id,
                    **monitor_inner,
                    "monitor_sha256": monitor_digest,
                },
                ("slot", "phase") + R26.R26_MONITOR_SEALED_FIELDS + ("monitor_sha256",),
            )
            lines.extend(
                [
                    topologies[0],
                    telemetry[0],
                    monitor,
                    telemetry[1],
                    topologies[1],
                    row_line,
                ]
            )
            sequence += 1
    lines.append(identity_line("end"))
    return [line + "\n" for line in lines]


class R40StripedCheckerTests(unittest.TestCase):
    def test_full_three_slot_evidence_fixture(self) -> None:
        with mock.patch.object(CHECKER, "load_r26_checker", return_value=fake_r26()):
            output = CHECKER.check_set(valid_slot_lines(slot) for slot in range(3))
        self.assertIn("phases=90", output[-1])
        self.assertIn("set_validation_status=pass", output[-1])

    def test_monitor_target_digest_mutation_is_rejected(self) -> None:
        lines = valid_slot_lines(0)
        monitor_index = next(
            index for index, line in enumerate(lines) if line.startswith("monitor ")
        )
        lines[monitor_index] = lines[monitor_index].replace(
            "target_output_sha256=", "target_output_sha256=0", 1
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "monitor seal mismatch"):
            CHECKER.validate_slot(lines, fake_r26())

    def test_kfd_nonzero_post_busy_is_rejected(self) -> None:
        lines = valid_slot_lines(0)
        telemetry_index = next(
            index
            for index, line in enumerate(lines)
            if line.startswith("telemetry ")
            and "phase=bytes4096-q2-combined.kfd" in line
            and "edge=end" in line
        )
        lines[telemetry_index] = lines[telemetry_index].replace(
            "gpu_busy_percent=0", "gpu_busy_percent=1"
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "KFD post-phase"):
            CHECKER.validate_slot(lines, fake_r26())

    def test_valid_rows_recompute_all_metrics(self) -> None:
        for backend in ("kfd", "hsa", "hip"):
            CHECKER.validate_row(valid_row(backend))

    def test_zero_logical_queue_count_is_not_admitted(self) -> None:
        row = valid_row("hip")
        row["logical_queue_count"] = "0"
        with self.assertRaisesRegex(CHECKER.CheckError, "logical_queue_count"):
            CHECKER.validate_row(row)

    def test_e2e_must_equal_submit_plus_wait_per_sample(self) -> None:
        row = valid_row("hsa")
        values = row["h2d_e2e_samples_ns"].split(",")
        values[17] = str(int(values[17]) + 1)
        row["h2d_e2e_samples_ns"] = ",".join(values)
        with self.assertRaisesRegex(CHECKER.CheckError, "e2e sample"):
            CHECKER.validate_row(row)

    def test_summary_cannot_be_forged(self) -> None:
        row = valid_row("hip")
        row["d2h_wait_p95_ns"] = str(int(row["d2h_wait_p95_ns"]) + 1)
        with self.assertRaisesRegex(CHECKER.CheckError, "inconsistent"):
            CHECKER.validate_row(row)

    def test_throughput_cannot_be_forged(self) -> None:
        row = valid_row("kfd")
        row["h2d_e2e_p50_GBps"] = "999.000000000"
        with self.assertRaisesRegex(CHECKER.CheckError, "throughput"):
            CHECKER.validate_row(row)

    def test_kfd_custody_digest_is_required(self) -> None:
        row = valid_row("kfd")
        row["queue_ids_sha256"] = "0" * 63
        with self.assertRaisesRegex(CHECKER.CheckError, "queue_ids_sha256"):
            CHECKER.validate_row(row)

    def test_kfd_queue_digest_must_match_retained_preimage(self) -> None:
        row = valid_row("kfd")
        row["queue_ids"] = row["queue_ids"].replace("h2d:1000", "h2d:4000")
        row["engine_placement"] = row["engine_placement"].replace(
            "h2d:1000:1", "h2d:4000:1"
        )
        row["engine_placement_sha256"] = hashlib.sha256(
            row["engine_placement"].encode("ascii")
        ).hexdigest()
        with self.assertRaisesRegex(CHECKER.CheckError, "does not match its preimage"):
            CHECKER.validate_row(row)

    def test_kfd_engine_digest_must_match_retained_preimage(self) -> None:
        row = valid_row("kfd")
        row["engine_placement_sha256"] = "0" * 64
        with self.assertRaisesRegex(CHECKER.CheckError, "does not match its preimage"):
            CHECKER.validate_row(row)

    def test_kfd_queue_ids_must_be_distinct(self) -> None:
        row = valid_row("kfd")
        row["queue_ids"] = row["queue_ids"].replace("d2h:1001", "d2h:1000")
        row["engine_placement"] = row["engine_placement"].replace(
            "d2h:1001", "d2h:1000"
        )
        row["queue_ids_sha256"] = hashlib.sha256(
            row["queue_ids"].encode("ascii")
        ).hexdigest()
        row["engine_placement_sha256"] = hashlib.sha256(
            row["engine_placement"].encode("ascii")
        ).hexdigest()
        with self.assertRaisesRegex(CHECKER.CheckError, "duplicate queue ID"):
            CHECKER.validate_row(row)

    def test_kfd_queue_role_order_and_cardinality_are_exact(self) -> None:
        row = valid_row("kfd")
        entries = row["queue_ids"].split(",")
        row["queue_ids"] = ",".join(entries[1:])
        row["queue_ids_sha256"] = hashlib.sha256(
            row["queue_ids"].encode("ascii")
        ).hexdigest()
        with self.assertRaisesRegex(CHECKER.CheckError, "invalid cardinality"):
            CHECKER.validate_row(row)

        row = valid_row("kfd")
        entries = row["queue_ids"].split(",")
        entries[0], entries[1] = entries[1], entries[0]
        row["queue_ids"] = ",".join(entries)
        row["queue_ids_sha256"] = hashlib.sha256(
            row["queue_ids"].encode("ascii")
        ).hexdigest()
        with self.assertRaisesRegex(CHECKER.CheckError, "canonical role order"):
            CHECKER.validate_row(row)

    def test_kfd_queue_ids_are_canonical_u32(self) -> None:
        for invalid in ("01", str(1 << 32), "9" * 5000):
            row = valid_row("kfd")
            row["queue_ids"] = row["queue_ids"].replace("h2d:1000", f"h2d:{invalid}")
            row["queue_ids_sha256"] = hashlib.sha256(
                row["queue_ids"].encode("ascii")
            ).hexdigest()
            with self.assertRaisesRegex(CHECKER.CheckError, "queue ID"):
                CHECKER.validate_row(row)

    def test_kfd_engine_placement_must_match_queue_and_profile(self) -> None:
        row = valid_row("kfd")
        row["engine_placement"] = row["engine_placement"].replace(
            "h2d:1000:1", "h2d:1000:0"
        )
        row["engine_placement_sha256"] = hashlib.sha256(
            row["engine_placement"].encode("ascii")
        ).hexdigest()
        with self.assertRaisesRegex(CHECKER.CheckError, "violates the gfx942 profile"):
            CHECKER.validate_row(row)

        row = valid_row("kfd")
        row["engine_placement"] = row["engine_placement"].replace(
            "striped0:1002:0", "striped0:4000:0"
        )
        row["engine_placement_sha256"] = hashlib.sha256(
            row["engine_placement"].encode("ascii")
        ).hexdigest()
        with self.assertRaisesRegex(CHECKER.CheckError, "does not match queue_ids"):
            CHECKER.validate_row(row)

    def test_standalone_roster_forbids_directional_roles(self) -> None:
        row = valid_row("kfd", "bytes4096-q16-standalone")
        self.assertTrue(row["queue_ids"].startswith("striped0:"))
        row["queue_ids"] = row["queue_ids"].replace("striped0:1000", "h2d:1000")
        row["engine_placement"] = row["engine_placement"].replace(
            "striped0:1000:0", "h2d:1000:0"
        )
        row["queue_ids_sha256"] = hashlib.sha256(
            row["queue_ids"].encode("ascii")
        ).hexdigest()
        row["engine_placement_sha256"] = hashlib.sha256(
            row["engine_placement"].encode("ascii")
        ).hexdigest()
        with self.assertRaisesRegex(CHECKER.CheckError, "canonical role order"):
            CHECKER.validate_row(row)

    def test_directional_smoke_matches_resource_kind(self) -> None:
        standalone = valid_row("kfd", "bytes4096-q16-standalone")
        CHECKER.validate_row(standalone)
        standalone["directional_smoke"] = "pass"
        with self.assertRaisesRegex(CHECKER.CheckError, "directional_smoke"):
            CHECKER.validate_row(standalone)

        combined = valid_row("kfd")
        combined["directional_smoke"] = "not-applicable"
        with self.assertRaisesRegex(CHECKER.CheckError, "directional_smoke"):
            CHECKER.validate_row(combined)

    def test_bounded_parity_accepts_pre_registered_limits(self) -> None:
        output, demonstrated = CHECKER.validate_performance(valid_set())
        self.assertTrue(demonstrated)
        self.assertIn("bounded_parity_status=demonstrated", output[0])
        self.assertIn("ten_x_status=not-demonstrated", output[-2])

    def test_one_bad_slot_is_retained_as_non_parity_evidence(self) -> None:
        rows = valid_set()
        key = (2, CHECKER.WORKLOAD_IDS[0], "kfd")
        rows[key] = valid_row("kfd", CHECKER.WORKLOAD_IDS[0], 1300)
        output, demonstrated = CHECKER.validate_performance(rows)
        self.assertFalse(demonstrated)
        self.assertIn("bounded_parity_status=not-demonstrated", output[-1])
        with self.assertRaisesRegex(CHECKER.CheckError, "parity was required"):
            CHECKER.validate_performance(rows, require_parity=True)

    def test_median_uses_paired_slot_ratios(self) -> None:
        rows = valid_set()
        workload = CHECKER.WORKLOAD_IDS[0]
        for slot, (kfd_base, ref_base) in enumerate(
            ((100, 1000), (1000, 100), (1000, 1000))
        ):
            rows[slot, workload, "kfd"] = valid_row("kfd", workload, kfd_base)
            rows[slot, workload, "hsa"] = valid_row("hsa", workload, ref_base)
        output, demonstrated = CHECKER.validate_performance(rows)
        self.assertFalse(demonstrated)
        hsa_line = next(
            line
            for line in output
            if f"workload_id={workload}" in line
            and "direction=h2d" in line
            and "reference=hsa" in line
        )
        self.assertIn("median_latency_ratio=1.000000", hsa_line)
        self.assertIn("bounded_parity_status=not-demonstrated", hsa_line)

    def test_ten_x_requires_every_matched_cell(self) -> None:
        output, _ = CHECKER.validate_performance(valid_set(40, 1000))
        self.assertIn("ten_x_status=demonstrated", output[-2])
        rows = valid_set(40, 1000)
        rows[1, CHECKER.WORKLOAD_IDS[-1], "kfd"] = valid_row(
            "kfd", CHECKER.WORKLOAD_IDS[-1], 110
        )
        output, _ = CHECKER.validate_performance(rows)
        self.assertIn("ten_x_status=not-demonstrated", output[-2])


if __name__ == "__main__":
    unittest.main()
