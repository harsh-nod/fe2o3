#!/usr/bin/env python3
"""CPU-only synthetic calibration of the new envelope and one-shot scheduler."""

import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import check
import run
from protocol import (
    BDF,
    CHECKS,
    NATIVE,
    NATIVE_ENV,
    OBSERVER,
    SOURCE,
    TOPOLOGY,
    UID,
    VISIBILITY,
)


def stamp(ns):
    return {
        "utc": f"2026-09-18T10:00:{ns // 10**9:02d}.{ns % 10**9:09d}Z",
        "monotonic_ns": ns,
    }


def observer_fixture(
    start=3_100_000_000, *, busy=0, vram=298647552, attached=False, capture_error=False
):
    count = 0

    def clock():
        nonlocal count
        value = stamp(start + count * 100_000)
        count += 1
        return value

    def sysfs(_root, _bdf):
        return {
            "started": clock(),
            "finished": clock(),
            "path": "/sys/bus/pci/devices/" + BDF,
            "values": {
                "unique_id": UID[2:],
                "mem_info_vram_used": str(vram),
                "mem_info_vis_vram_used": str(vram),
                "mem_info_gtt_used": "25231360",
                "gpu_busy_percent": str(busy),
                "mem_busy_percent": "0",
            },
            "errors": {},
        }

    def captured(command):
        status = command[-1] == "--json"
        text = (
            json.dumps(
                {
                    "card4": {
                        "Unique ID": UID,
                        "PCI Bus": BDF,
                        "GPU use (%)": str(busy),
                        "VRAM Total Used Memory (B)": str(vram),
                    }
                }
            )
            if status
            else (
                "===== ROCm System Management Interface =====\n===== GPUs Indexed by PID =====\n"
                + (
                    "PID 999 is using 1 DRM device(s):\n4\n"
                    if attached
                    else "PID 999 is using 0 DRM device(s)\n"
                )
                + "==========\n===== End of ROCm SMI Log =====\n"
            )
        )
        return {
            "command": command,
            "started": clock(),
            "finished": clock(),
            "exit": 1 if capture_error else 0,
            "error": None,
            "stdout": text,
            "stderr": "injected\n" if capture_error else "",
        }

    with (
        patch.object(check.observer, "stamp", clock),
        patch.object(check.observer, "capture_sysfs", sysfs),
    ):
        row = check.observer.observe(
            4, BDF, UID, Path("/opt/rocm/bin/rocm-smi"), run=captured
        )
    row["index"] = 0
    end = {
        "schema": check.observer.SCHEMA,
        "record": "complete",
        "observations": 1,
        "refused": int(bool(row["reasons"])),
        "all_endpoints_admitted": not row["reasons"],
        "performance_accepted": False,
    }
    receipt = {
        "started": stamp(start - 1),
        "reaped": stamp(start + 20_000_000),
        "finished": stamp(start + 21_000_000),
        "exit": int(bool(row["reasons"])),
        "error": None,
        "group_absent": True,
    }
    return row, end, receipt


def encoded(row, end):
    return json.dumps(row) + "\n" + json.dumps(end) + "\n"


def payload():
    schema = check.hip.SCHEMA
    lines = [
        f"schema={schema} record=config device_index=0 unique_id={UID[2:]} target=gfx942:sramecc+:xnack- xnack=disabled bytes=268435456 depth=1 warmups=3 samples=10 host_allocation=hipHostMallocDefault stream=nonblocking engine=runtime_selected allocator_benchmark=disabled"
    ]
    for index in range(13):
        lines.append(
            f"schema={schema} record=round index={index} phase={'warmup' if index < 3 else 'sample'} pattern={(index * 67 + 1) % 251 + 1} checked_bytes=268435456 h2d_total_ns=123 d2h_total_ns=456"
        )
    lines.append(
        f"schema={schema} record=complete validated_rounds=13 measured_rounds=10 allocations_released=3 streams_destroyed=1"
    )
    return "\n".join(lines) + "\n"


class FakeRecorder:
    def __init__(self, root, faults):
        self.root, self.faults, self.names, self.tags = root, faults, [], []

    def run(self, tag, argv, **kwargs):
        self.tags.append(tag)
        name = f"{len(self.names):03d}-{tag}"
        self.names.append(name)
        row = {
            "name": name,
            "exit": 0,
            "error": None,
            "group_absent": True,
            "started": stamp(1_000_000_000),
            "reaped": stamp(3_000_000_000),
            "finished": stamp(2_000_000_000),
        }
        stdout, stderr = "", ""
        if tag in ("pre", "immediate", "delayed"):
            start = {
                "pre": 1_100_000_000,
                "immediate": 3_100_000_000,
                "delayed": 23_100_000_000,
            }[tag]
            options = self.faults.get(tag, {})
            observed, end, meta = observer_fixture(
                start=start + options.get("late", 0),
                **{k: v for k, v in options.items() if k != "late"},
            )
            row.update(meta)
            if tag == "pre":
                row["finished"] = stamp(2_000_000_000)
            stdout = encoded(observed, end)
        elif tag == "native":
            stdout = payload()
            row["exit"] = self.faults.get("native_exit", 0)
        (self.root / (name + ".stdout")).write_text(stdout)
        (self.root / (name + ".stderr")).write_text(stderr)
        return row


class Tests(unittest.TestCase):
    def audit_fixture(self, destination):
        results = destination / "results"
        root = results / "smoke"
        root.mkdir(parents=True)
        here = Path(__file__).resolve().parent
        for name in ("platform.sha256", "binary.sha256"):
            shutil.copyfile(here / "prepared-results" / name, results / name)
        manifests = {
            "source": check.manifest(here / "source-files.sha256"),
            "binary": check.manifest(results / "binary.sha256"),
            "platform": check.manifest(results / "platform.sha256"),
            "scripts": check.manifest(here / "scripts.sha256"),
        }
        names, observations, records = [], {}, {}
        cursor = 1_000_000_000
        tags = (
            [(tag + "-before", argv) for tag, argv in CHECKS]
            + [
                ("topology", TOPOLOGY),
                ("pre", OBSERVER),
                ("native", NATIVE),
                ("immediate", OBSERVER),
                ("delayed", OBSERVER),
            ]
            + [(tag + "-after", argv) for tag, argv in CHECKS]
        )
        for index, (tag, argv) in enumerate(tags):
            name = f"{index:03d}-{tag}"
            stdout = ""
            meta = {
                "started": stamp(cursor),
                "reaped": stamp(cursor + 100_000),
                "finished": stamp(cursor + 200_000),
                "exit": 0,
            }
            if tag in ("pre", "immediate", "delayed"):
                start = {
                    "pre": 2_000_000_000,
                    "immediate": 3_100_000_000,
                    "delayed": 23_100_000_000,
                }[tag]
                observed, end, meta = observer_fixture(
                    start, busy=46 if tag == "immediate" else 0
                )
                stdout = encoded(observed, end)
            elif tag == "native":
                stdout = payload()
                meta = {
                    "started": stamp(cursor),
                    "reaped": stamp(3_000_000_000),
                    "finished": stamp(3_001_000_000),
                    "exit": 0,
                }
            elif tag == "topology":
                stdout = (
                    json.dumps(
                        {
                            "unique_id": UID[2:],
                            "numa_node": "1",
                            "local_cpulist": "48-95",
                            "affinity": list(range(48, 96)),
                            "placement_stdout": "\n".join(
                                [
                                    "policy: bind",
                                    "preferred node: 1",
                                    "physcpubind: " + " ".join(map(str, range(48, 96))),
                                    "cpubind: 1",
                                    "nodebind: 1",
                                    "membind: 1",
                                    "preferred: 1",
                                ]
                            )
                            + "\n",
                            "placement_stderr": "",
                            "placement_exit": 0,
                        }
                    )
                    + "\n"
                )
            else:
                stdout = "".join(
                    path + ": OK\n" for path in manifests[tag.rsplit("-", 1)[0]]
                )
            row = {
                "schema": "fe2o3.hip-smoke-command.v1",
                "name": name,
                "argv": argv,
                "cwd": str(SOURCE),
                "environment_override": NATIVE_ENV if tag == "native" else {},
                "visibility_unset": VISIBILITY,
                "pid": 1000 + index,
                "error": None,
                "group_absent": True,
                "outer_bound_seconds": 200 if tag == "native" else 70,
                **meta,
            }
            (root / (name + ".stdout")).write_text(stdout)
            (root / (name + ".stderr")).write_text("")
            row["stdout_sha256"] = check.digest(root / (name + ".stdout"))
            row["stderr_sha256"] = check.digest(root / (name + ".stderr"))
            (root / (name + ".json")).write_text(json.dumps(row))
            names.append(name)
            records[tag] = row
            if tag in ("pre", "immediate", "delayed"):
                checked = check.endpoint(stdout, row)
                observations[tag] = {
                    **checked,
                    "protocol_phase_accepted": check.phase_accepts(
                        tag, checked, stamp(3_000_000_000)
                    ),
                }
            cursor = meta["finished"]["monotonic_ns"] + 1_000_000
        state = {
            "schema": "fe2o3.hip-smoke-fixed-deadline.v1",
            "source_commit": check.COMMIT,
            "started": stamp(900_000_000),
            "native_launched": True,
            "native_reaped": records["native"]["reaped"],
            "observations": observations,
            "failures": [],
            "performance_accepted": False,
            "smoke_accepted": True,
            "records": names,
            "finished": stamp(cursor),
        }
        (results / "smoke.json").write_text(json.dumps(state))
        return results, records, state

    def test_complete_audit_accepts_busy_telemetry_without_reclassifying_it(self):
        with tempfile.TemporaryDirectory(prefix="hip-smoke-audit-fixture-") as temp:
            results, _, _ = self.audit_fixture(Path(temp))
            report = check.audit(results)
            self.assertTrue(report["smoke_accepted"])
            self.assertEqual(report["records"], 13)
            self.assertFalse(report["observations"]["immediate"]["original_admitted"])
            self.assertFalse(report["performance_accepted"])

    def test_complete_audit_rejects_boundaries_and_forged_receipts(self):
        for mutation in (
            "argv",
            "environment",
            "group",
            "binary",
            "extra",
            "t0",
            "payload",
            "missing-row",
            "raw-hash",
            "identity-output",
        ):
            with (
                self.subTest(mutation=mutation),
                tempfile.TemporaryDirectory(prefix="hip-smoke-audit-negative-") as temp,
            ):
                results, records, state = self.audit_fixture(Path(temp))
                root = results / "smoke"
                row = copy.deepcopy(records["native"])
                if mutation == "argv":
                    row["argv"] = [*row["argv"][:-1], "invalid"]
                if mutation == "environment":
                    row["environment_override"] = {
                        "ROCR_VISIBLE_DEVICES": "0",
                        "HSA_XNACK": "0",
                    }
                if mutation == "group":
                    row["group_absent"] = False
                if mutation == "binary":
                    (results / "binary.sha256").write_text("0" * 64 + "  /tmp/other\n")
                if mutation == "extra":
                    (root / "extra").write_text("unrecorded")
                if mutation == "t0":
                    state["native_reaped"] = stamp(2_999_999_999)
                if mutation == "payload":
                    path = root / (row["name"] + ".stdout")
                    path.write_text(
                        path.read_text().replace(
                            "allocations_released=3", "allocations_released=2"
                        )
                    )
                    row["stdout_sha256"] = check.digest(path)
                if mutation == "missing-row":
                    (root / (records["scripts-after"]["name"] + ".json")).unlink()
                if mutation == "raw-hash":
                    row["stdout_sha256"] = "0" * 64
                if mutation == "identity-output":
                    before = copy.deepcopy(records["source-before"])
                    path = root / (before["name"] + ".stdout")
                    path.write_text("")
                    before["stdout_sha256"] = check.digest(path)
                    (root / (before["name"] + ".json")).write_text(json.dumps(before))
                (root / (row["name"] + ".json")).write_text(json.dumps(row))
                (results / "smoke.json").write_text(json.dumps(state))
                with self.assertRaises(ValueError):
                    check.audit(results)

    def test_complete_strict_endpoint(self):
        row, end, receipt = observer_fixture()
        result = check.endpoint(encoded(row, end), receipt)
        self.assertTrue(check.phase_accepts("pre", result))

    def test_busy_preserves_original_failure_only_immediate_allows(self):
        row, end, receipt = observer_fixture(busy=46)
        result = check.endpoint(encoded(row, end), receipt)
        self.assertEqual(set(result["reasons"]), check.BUSY_ONLY)
        self.assertFalse(result["original_admitted"])
        self.assertEqual(receipt["exit"], 1)
        self.assertTrue(check.phase_accepts("immediate", result, stamp(3_000_000_000)))
        self.assertFalse(check.phase_accepts("pre", result))
        self.assertFalse(check.phase_accepts("delayed", result, stamp(1)))

    def test_nonbusy_faults_remain_sticky(self):
        for options in (
            {"vram": 536870912},
            {"attached": True},
            {"capture_error": True},
        ):
            with self.subTest(options=options):
                row, end, receipt = observer_fixture(**options)
                observed = check.endpoint(encoded(row, end), receipt)
                self.assertFalse(
                    check.phase_accepts("immediate", observed, stamp(3_000_000_000))
                )

    def test_actual_observer_start_controls_both_windows(self):
        observed = {"reasons": [], "selected_pids": [], "original_admitted": True}
        for phase, good, bad in [
            ("immediate", [0, 1_000_000_000], [-1, 1_000_000_001]),
            (
                "delayed",
                [20_000_000_000, 21_000_000_000],
                [19_999_999_999, 21_000_000_001],
            ),
        ]:
            for delta in good + bad:
                observed["started"] = stamp(2_000_000_000 + delta)
                self.assertEqual(
                    check.phase_accepts(phase, observed, stamp(2_000_000_000)),
                    delta in good,
                )

    def test_forged_admission_footer_exit_or_identity_rejected(self):
        original, terminal, receipt = observer_fixture(busy=46)
        for mutation in ("reasons", "admitted", "footer", "exit", "identity"):
            row, end, meta = copy.deepcopy((original, terminal, receipt))
            if mutation == "reasons":
                row["reasons"] = []
            if mutation == "admitted":
                row["endpoint_admitted"] = True
            if mutation == "footer":
                end["refused"] = 0
            if mutation == "exit":
                meta["exit"] = 0
            if mutation == "identity":
                row["pci_bdf"] = "0000:26:00.0"
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                check.endpoint(encoded(row, end), meta)

    def test_complete_envelope_and_unique_json(self):
        row, end, meta = observer_fixture()
        text = encoded(row, end)
        for broken in (
            text.splitlines()[0] + "\n",
            text + "{}\n",
            text[:-1],
            text.replace('"gpu_index": 4', '"gpu_index": 4, "gpu_index": 4'),
        ):
            with self.assertRaises(ValueError):
                check.endpoint(broken, meta)

    def test_native_payload_uses_pinned_independent_validator(self):
        self.assertTrue(
            check.hip.validate(payload(), "", 0, check.EXPECTED_HIP)["payload_valid"]
        )
        for text, err, status in (
            (payload().replace("streams_destroyed=1", "streams_destroyed=0"), "", 0),
            (payload(), "unexpected", 0),
            (payload(), "", 2),
        ):
            with self.assertRaises(ValueError):
                check.hip.validate(text, err, status, check.EXPECTED_HIP)

    def exercise(self, faults):
        with tempfile.TemporaryDirectory(prefix="hip-smoke-cpu-fixture-") as temp:
            recorder = FakeRecorder(Path(temp), faults)
            state = {
                "failures": [],
                "observations": {},
                "native_launched": False,
                "native_reaped": None,
            }
            with (
                patch.object(run.time, "monotonic_ns", return_value=2_000_000_000),
                patch.object(run, "sleep_until") as sleep,
            ):
                run.execute(recorder, state)
            return state, recorder.tags, sleep.call_args_list

    def test_no_native_on_preflight_refusal(self):
        state, tags, sleeps = self.exercise({"pre": {"attached": True}})
        self.assertFalse(state["native_launched"])
        self.assertNotIn("native", tags)
        self.assertEqual(sleeps, [])
        self.assertEqual(tags[-4:], [tag + "-after" for tag, _ in CHECKS])

    def test_busy_telemetry_success_is_not_observer_admission(self):
        state, tags, sleeps = self.exercise({"immediate": {"busy": 46}})
        self.assertEqual(state["failures"], [])
        self.assertFalse(state["observations"]["immediate"]["original_admitted"])
        self.assertEqual(tags.count("native"), 1)
        self.assertEqual(sleeps[0].args, (23_000_000_000,))

    def test_native_failure_still_collects_both_endpoints_no_retry(self):
        state, tags, sleeps = self.exercise({"native_exit": 2})
        self.assertTrue(state["failures"])
        self.assertEqual(
            [tag for tag in tags if tag in ("native", "immediate", "delayed")],
            ["native", "immediate", "delayed"],
        )
        self.assertEqual(len(sleeps), 1)

    def test_immediate_and_delayed_faults_never_rehabilitated(self):
        for faults in (
            {"immediate": {"vram": 536870912}},
            {"immediate": {"late": 1_000_000_000}},
            {"delayed": {"busy": 1}},
            {"delayed": {"late": 1_000_000_000}},
        ):
            with self.subTest(faults=faults):
                state, tags, sleeps = self.exercise(faults)
                self.assertTrue(state["failures"])
                self.assertEqual(tags.count("native"), 1)
                self.assertEqual(tags.count("delayed"), 1)
                self.assertEqual(len(sleeps), 1)


if __name__ == "__main__":
    unittest.main()
