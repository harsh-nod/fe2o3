#!/usr/bin/env python3
"""Synthetic envelope tests using historical payloads, never native evidence."""

import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import check

REPO = "/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917"


def historical(path):
    data = subprocess.check_output(
        ["git", "-C", REPO, "show", check.COMMIT + ":" + path]
    )
    print(
        "historical_fixture_sha256="
        + hashlib.sha256(data).hexdigest()
        + " path="
        + path
    )
    return data.decode()


def blocks(text):
    current = []
    for line in text.splitlines():
        if line.startswith("schema="):
            current.append(line)
            if " record=complete " in line:
                yield "\n".join(current) + "\n"
                current = []


def stamp(number):
    seconds, nanos = divmod(number, 1_000_000_000)
    minutes, seconds = divmod(seconds, 60)
    hours, minutes = divmod(minutes, 60)
    return {
        "utc": f"2026-09-18T{hours:02d}:{minutes:02d}:{seconds:02d}.{nanos:09d}Z",
        "monotonic_ns": number,
    }


def observation(start, refused=False):
    def span(index):
        return {
            "started": stamp(start + index * 100),
            "finished": stamp(start + index * 100 + 50),
        }

    values = dict(
        unique_id=check.UID[2:],
        mem_info_vram_used="298647552",
        mem_info_vis_vram_used="0",
        mem_info_gtt_used="0",
        gpu_busy_percent="0",
        mem_busy_percent="0",
    )
    snapshots = [
        dict(
            span(index),
            path="/sys/bus/pci/devices/" + check.BDF,
            values=dict(values),
            errors={},
        )
        for index in (1, 3, 5)
    ]
    if refused:
        snapshots[0]["values"]["mem_info_vram_used"] = "648634368"
    prefix = ["/usr/bin/timeout", "--kill-after=5s", "20s", "/opt/rocm/bin/rocm-smi"]
    status = dict(
        span(2),
        command=prefix
        + [
            "--showuse",
            "--showmeminfo",
            "vram",
            "--showuniqueid",
            "--showbus",
            "--json",
        ],
        exit=0,
        error=None,
        stdout=json.dumps(
            {
                "card4": {
                    "Unique ID": check.UID,
                    "PCI Bus": check.BDF,
                    "GPU use (%)": "0",
                    "VRAM Total Used Memory (B)": "298647552",
                }
            }
        ),
        stderr="",
    )
    pids = dict(
        span(4),
        command=prefix + ["--showpidgpus"],
        exit=0,
        error=None,
        stdout="=== ROCm System Management Interface ===\n=== GPUs Indexed by PID ===\nPID 123 is using 1 DRM device(s):\n0\n===\n=== End of ROCm SMI Log ===\n",
        stderr="",
    )
    row = dict(
        schema="fe2o3.copy-host-observation.v1",
        record="observation",
        started=stamp(start),
        finished=stamp(start + 600),
        gpu_index=4,
        pci_bdf=check.BDF,
        unique_id=check.UID,
        index=0,
        vram_limit_exclusive=536870912,
        visibility_filters="removed-for-cli",
        sysfs=snapshots,
        status=status,
        pids=pids,
        selected_pids=[],
        endpoint_admitted=not refused,
        reasons=["sysfs-before-vram"] if refused else [],
        scope="sequential-endpoint-observations-not-continuous-monitoring-or-reservation",
    )
    end = dict(
        schema=row["schema"],
        record="complete",
        observations=1,
        refused=int(refused),
        all_endpoints_admitted=not refused,
        performance_accepted=False,
    )
    return row, end


class CheckerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.payloads = {}
        old = historical(
            "docs/evidence/dev-kfd-copy-progress-mi300x-2026-09-18/raw/benchmark.log"
        )
        smoke = historical(
            "docs/evidence/dev-kfd-native-wait-smoke-mi300x-2026-09-18/raw/native.log"
        )
        for block in blocks(old):
            if "wait_policy=slice50us" in block and "A" not in cls.payloads:
                cls.payloads["A"] = block
            if (
                "cpu_index=1 " in block
                and "host_grain=fine " in block
                and "requested_engine_mask=2 " in block
                and "D" not in cls.payloads
            ):
                cls.payloads["D"] = block
        for block in blocks(smoke):
            for cell, policy in (("B", "native-sleep1ms"), ("C", "native-sleep25us")):
                if "wait_policy=" + policy in block:
                    cls.payloads[cell] = block
        assert set(cls.payloads) == set("ABCD")

    def test_real_historical_payloads_and_refusals(self):
        for cell, text in self.payloads.items():
            check.payload(cell, text, "")
            for broken in (
                "unexpected\n" + text,
                text + "unexpected\n",
                "\n".join(text.splitlines()[:-1]) + "\n",
                text.replace("checked_bytes=268435456", "checked_bytes=1", 1),
            ):
                with (
                    self.subTest(cell=cell, broken=broken[:60]),
                    self.assertRaises((ValueError, AssertionError, KeyError)),
                ):
                    check.payload(cell, broken, "")
            with self.assertRaises(ValueError):
                check.payload(cell, text, "warning\n")

    def test_complete_endpoint_and_sticky_refusal(self):
        for refused in (False, True):
            row, end = observation(1000, refused)
            receipt = {
                "started": stamp(900),
                "finished": stamp(1700),
                "exit": int(refused),
            }
            result = check.endpoint(
                json.dumps(row) + "\n" + json.dumps(end) + "\n", receipt
            )
            self.assertEqual(result["admitted"], not refused)

    def test_endpoint_mutations(self):
        row, end = observation(1000)
        receipt = {"started": stamp(900), "finished": stamp(1700), "exit": 0}
        mutations = [
            lambda r: r.update(unique_id="0x0000000000000001"),
            lambda r: r["sysfs"][0].update(path="/sys/class/drm/card4/device"),
            lambda r: r["sysfs"].pop(),
            lambda r: r["pids"].update(stdout='{"success":true}\n'),
            lambda r: r["pids"].update(
                stdout=r["pids"]["stdout"].replace("\n0\n", "\n4\n")
            ),
            lambda r: r["sysfs"][0]["values"].update(mem_info_vram_used="536870912"),
            lambda r: r["status"].update(stderr="warning\n"),
            lambda r: r["status"]["command"].append("--unexpected"),
            lambda r: r["pids"].update(started=stamp(999)),
            lambda r: r.update(reasons=["invented"]),
        ]
        for mutate in mutations:
            changed = copy.deepcopy(row)
            mutate(changed)
            with (
                self.subTest(mutation=mutate),
                self.assertRaises((ValueError, KeyError, TypeError)),
            ):
                check.endpoint(
                    json.dumps(changed) + "\n" + json.dumps(end) + "\n", receipt
                )
        with self.assertRaises(ValueError):
            check.loads('{"a":1,"a":2}')

    def fixture(self, root, stop=False):
        results, campaign = root / "results", root / "results/campaign"
        campaign.mkdir(parents=True)
        for name in ("binaries.sha256", "platform.sha256"):
            (results / name).write_bytes(
                (check.HERE / "prepared-results" / name).read_bytes()
            )
        state = dict(
            schema="fe2o3.matched-campaign.v1",
            source_commit=check.COMMIT,
            orders=list(check.ORDERS),
            started=stamp(1),
            cells=[],
            failure=None,
            scope="guarded-shared-host-endpoints-not-reservation",
        )
        names, now = [], 1000

        def add(
            tag, argv, stdout="", stderr="", environment=None, status=0, duration=1000
        ):
            nonlocal now
            name = f"{len(names):03d}-{tag}"
            names.append(name)
            start, finish = now, now + duration
            now = finish + 1000
            for stream, text in (("stdout", stdout), ("stderr", stderr)):
                (campaign / f"{name}.{stream}").write_text(text)
            row = dict(
                schema="fe2o3.matched-command.v1",
                name=name,
                argv=argv,
                cwd=check.SOURCE,
                environment_override=environment or {},
                visibility_unset=check.VISIBILITY,
                started=stamp(start),
                finished=stamp(finish),
                pid=1000 + len(names),
                exit=status,
                error=None,
                group_absent=True,
                outer_bound_seconds=200,
                stdout_sha256=check.digest(campaign / f"{name}.stdout"),
                stderr_sha256=check.digest(campaign / f"{name}.stderr"),
            )
            (campaign / f"{name}.json").write_text(json.dumps(row))
            return name

        def identities(phase):
            for tag, remote, local in (
                (
                    "source",
                    check.OWNED + "/source-files.sha256",
                    check.HERE / "source-files.sha256",
                ),
                (
                    "binaries",
                    check.OWNED + "/results/binaries.sha256",
                    results / "binaries.sha256",
                ),
                (
                    "platform",
                    check.OWNED + "/results/platform.sha256",
                    results / "platform.sha256",
                ),
                (
                    "scripts",
                    check.OWNED + "/scripts.sha256",
                    check.HERE / "scripts.sha256",
                ),
            ):
                add(
                    tag + "-" + phase,
                    ["/usr/bin/sha256sum", "-c", remote],
                    "".join(path + ": OK\n" for path in check.manifest(local)),
                )

        def endpoint(tag, refused=False):
            row, end = observation(now + 100, refused)
            return add(
                tag,
                check.OBSERVER,
                json.dumps(row) + "\n" + json.dumps(end) + "\n",
                status=int(refused),
            )

        identities("before")
        add(
            "topology",
            [
                "/usr/bin/python3",
                "-B",
                check.SOURCE + "/benchmarks/runtime_gfx942/r26-host-guard.py",
                "topology",
                "--gpu-index",
                "4",
                "--pci-bdf",
                check.BDF,
                "--unique-id",
                check.UID,
            ],
            check.protocol.TOPOLOGY + "\n",
        )
        placement = (
            "\n".join(
                [
                    "policy: bind",
                    "preferred node: 1",
                    "physcpubind: " + " ".join(str(x) for x in range(48, 96)),
                    "cpubind: 1",
                    "nodebind: 1",
                    "membind: 1",
                    "preferred: 1",
                ]
            )
            + "\n"
        )
        add(
            "placement",
            [
                "/usr/bin/numactl",
                "--physcpubind=48-95",
                "--membind=1",
                "/usr/bin/numactl",
                "--show",
            ],
            placement,
        )
        add(
            "topology-check",
            [
                "/usr/bin/python3",
                "-B",
                check.OWNED + "/check.py",
                "--topology",
                check.OWNED + "/results/campaign/004-topology.stdout",
                check.OWNED + "/results/campaign/005-placement.stdout",
            ],
        )
        for key in check.EXPECTED[:1] if stop else check.EXPECTED:
            begin, cell = len(names), key[-1]
            endpoint(key + "-pre")
            if cell in check.MODES:
                command = check.PREFIX + [
                    check.OWNED
                    + "/target/release/examples/gfx942-runtime-directional-window-benchmark",
                    check.UID,
                    "268435456",
                    "3",
                    "10",
                    check.MODES[cell],
                ]
                environment = {}
            else:
                command = check.PREFIX + [
                    check.OWNED + "/hsa-copy-pool-engine",
                    "0",
                    "1",
                    "268435456",
                    "3",
                    "10",
                    check.UID,
                    "fine",
                    "engine1",
                ]
                environment = {"HSA_XNACK": "0", "ROCR_VISIBLE_DEVICES": "4"}
            native = add(
                key + "-native", command, self.payloads[cell], environment=environment
            )
            endpoint(key + "-immediate", stop)
            add(key + "-delay", ["/usr/bin/sleep", "20"], duration=20_000_000_000)
            endpoint(key + "-delayed")
            add(
                key + "-payload",
                [
                    "/usr/bin/python3",
                    "-B",
                    check.OWNED + "/check.py",
                    "--payload",
                    cell,
                    check.OWNED + "/results/campaign/" + native + ".stdout",
                    check.OWNED + "/results/campaign/" + native + ".stderr",
                ],
            )
            state["cells"].append(dict(key=key, records=names[begin:], admitted=True))
        identities("after")
        state.update(
            records=names,
            finished=stamp(now),
            exit=int(stop),
            failure="fixture sticky refusal" if stop else None,
        )
        (results / "campaign.json").write_text(json.dumps(state))
        return results

    def test_full_envelope_and_paired_process_summary(self):
        with tempfile.TemporaryDirectory(
            prefix="checker-fixture-", dir=check.HERE
        ) as scratch:
            results = self.fixture(Path(scratch))
            summary = check.audit(results, True)
            self.assertEqual(
                (
                    summary["processes"],
                    summary["endpoint_count"],
                    summary["validated_rounds"],
                ),
                (16, 48, 208),
            )
            self.assertTrue(
                all(
                    len(value["per_block"]) == 4
                    for value in summary["comparisons"].values()
                )
            )

    def test_incomplete_refuses_summary_even_after_good_delayed_endpoint(self):
        with tempfile.TemporaryDirectory(
            prefix="checker-fixture-", dir=check.HERE
        ) as scratch:
            results = self.fixture(Path(scratch), stop=True)
            self.assertFalse(check.audit(results, False)["complete_matched_campaign"])
            with self.assertRaises(ValueError):
                check.audit(results, True)

    def test_envelope_mutations(self):
        mutations = [
            (
                "008-1-A-native",
                lambda r: r["argv"].__setitem__(-1, "diagnostic-window-deadline"),
            ),
            (
                "008-1-A-native",
                lambda r: r.update(environment_override={"ROCR_VISIBLE_DEVICES": "4"}),
            ),
            ("008-1-A-native", lambda r: r.update(group_absent=False)),
            ("009-1-A-immediate", lambda r: r.update(started=stamp(1))),
            ("010-1-A-delay", lambda r: r.update(finished=r["started"])),
            ("011-1-A-delayed", lambda r: r.update(stdout_sha256="0" * 64)),
            ("102-4-B-payload", lambda r: r.update(exit=1)),
        ]
        with tempfile.TemporaryDirectory(
            prefix="checker-fixture-", dir=check.HERE
        ) as scratch:
            results = self.fixture(Path(scratch))
            for name, mutate in mutations:
                path = results / "campaign" / f"{name}.json"
                original = path.read_text()
                row = json.loads(original)
                mutate(row)
                path.write_text(json.dumps(row))
                with (
                    self.subTest(name=name),
                    self.assertRaises((ValueError, AssertionError)),
                ):
                    check.audit(results, True)
                path.write_text(original)
            state = results / "campaign.json"
            changed = json.loads(state.read_text())
            changed["cells"][1]["key"] = "1-D"
            state.write_text(json.dumps(changed))
            with self.assertRaises(ValueError):
                check.audit(results, True)


if __name__ == "__main__":
    unittest.main()
