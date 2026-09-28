#!/usr/bin/env python3
"""Synthetic seven-cell transcripts and full native receipt replay; no hardware claims."""
import copy
import hashlib
import json
from pathlib import Path
import runpy
import shutil
import struct
import tempfile
from types import SimpleNamespace
import unittest

HERE = Path(__file__).resolve().parent
s = SimpleNamespace(**runpy.run_path(str(HERE / "stage.py")))
t = SimpleNamespace(**runpy.run_path(str(HERE / "test_profile.py")))


def depth_fixture(p):
    scope = "80" * 32
    resource = lambda tag, value: t.p.resource(scope, tag, value)
    rows, events = [], []
    def add(kind, **fields):
        events.append({"kind": kind, **fields})
    add("module_loaded", module=resource(4, 400), artifact=t.p.content_sha(4872, p.SCALE.HSACO))
    add("kernel_resolved", kernel=resource(5, 300), module=resource(4, 400), name=t.p.content(b"vecadd"),
        signature=t.p.content(bytes.fromhex("558897b2c24edacb9a0d83a630d6f8480a74095771211fafc0fdb823d476c9a7")))
    for lane in range(2):
        add("stream_created", stream=resource(2, 100 + lane))
        add("native_queue_created", queue=resource(1, lane + 1))
        for ref in range(200 + 3 * lane, 203 + 3 * lane):
            add("allocation_created", allocation=resource(3, ref), memory_kind="host_visible", byte_len=4194304, alignment=4)
            add("host_write", allocation=resource(3, ref), byte_offset=0, content={"state": "range_only", "byte_len": 4194304})
        for index in range(1024):
            rows.append({"submission": 10000 + 2 * index + lane, "lane": lane, "stream": 100 + lane, "kernel": 300,
                         "allocations": list(range(200 + 3 * lane, 203 + 3 * lane)),
                         "ordered_predecessor": None if index == 0 else 9998 + 2 * index + lane,
                         "pipeline_phase": None if index == 0 else "published",
                         "native_receipt": list(hashlib.sha256(f"receipt-{lane}-{index}".encode()).digest()),
                         "dispatch_shape": list(hashlib.sha256(f"shape-{lane}".encode()).digest())})
    interleaved = [row for pair in zip(rows[:1024], rows[1024:]) for row in pair]
    for row in interleaved:
        add("dispatch_published", dispatch=resource(6, row["submission"]), queue=resource(1, row["lane"] + 1),
            stream=resource(2, row["stream"]), kernel=resource(5, row["kernel"]),
            dispatch_shape=t.p.content(bytes(row["dispatch_shape"])),
            launch={"grid": [1048576, 1, 1], "workgroup": [256, 1, 1], "dynamic_shared_bytes": 0},
            bindings=[{"allocation": resource(3, ref), "access": access, "byte_offset": 0,
                       "byte_len": 4194304, "kernarg_byte_offset": i * 16}
                      for i, (ref, access) in enumerate(zip(row["allocations"], ("read", "read", "write")))])
    for row in interleaved:
        add("dispatch_completed", dispatch=resource(6, row["submission"]), host_timing=dict.fromkeys(t.p.TIMINGS, 0))
    for ref in range(200, 206):
        add("host_read", allocation=resource(3, ref), byte_offset=0, content={"state": "range_only", "byte_len": 4194304})
    for row in reversed(rows):
        add("submission_released", dispatch=resource(6, row["submission"]))
    for ref in range(200, 206):
        add("allocation_released", allocation=resource(3, ref))
    for lane in range(2):
        add("stream_destroyed", stream=resource(2, 100 + lane))
        add("native_queue_destroyed", queue=resource(1, lane + 1))
    add("module_unloaded", module=resource(4, 400))
    patterns = [[2 * i for i in range(1024)], [i % 256 for i in range(1024)],
                [2 * i + i % 256 for i in range(1024)]]
    outputs = [hashlib.sha256(struct.pack("<1024f", *(n / 4 for n in pattern)) * 1024).hexdigest()
               for pattern in patterns] * 2
    value = {"schema": "fe2o3.scale-retained-depth-development.v1", "unique_id": t.p.UID,
             "hsaco_sha256": list(bytes.fromhex(p.SCALE.HSACO)), "depth_per_lane": 1024, "native_retained": 2048,
             "membership": p.SCALE.membership(rows), "receipts": rows, "host_table_peak_records": 10,
             "host_table_peak_payload_bytes": 32768, "final_buffer_sha256": outputs,
             "runtime_capacity_negative": "unchanged-retained-cut",
             "native_capacity_negative": "rejected-before-side-effect-1024-each-lane", "unfinished_gpu_count": None,
             "physical_overlap": "unmeasured", "host_table_final_records": 0, "cleanup": "complete"}
    return value, t.sealed(events, scope)


def transcript(p, case):
    def shutdown(depth=False):
        budget, records = (134217728, 512) if depth else (67108864, 128)
        return ("PrimaryHostUsage { before: Some(" + p.account("HostVisible", budget, records, 4096, 1)
                + "), completed: Some(" + p.account("HostVisible", budget, records) + "), observations: [1, 1] }")
    lines = ["running 1 test", "test " + p.TESTS[case] + " ... "]
    if case == "depth":
        value, profile = depth_fixture(p)
        lines += ["scale_depth_receipts_json=" + json.dumps(value), "profile_json=" + json.dumps(profile),
                  "scale_host_shutdown=" + shutdown(True)]
    else:
        profiles = case in ("profiles-short", "profiles-long")
        variants = {"profiles-short": ["Short"], "profiles-long": ["Long"], "backpressure": ["Short", "Long"],
                    "owner": ["Long", "Short"], "timeout": ["Long"], "drop": ["Long"]}[case]
        prefix = "mixed_duration" if profiles else "mixed_duration_owner" if case == "owner" else "mixed_observer"
        for variant in variants:
            lines.append(prefix + " variant=" + variant + " observed_hex=" + p.ORACLE.expected(variant.lower()).hex())
            if profiles:
                lines.append(prefix + " variant=" + variant + " shutdown=" + shutdown() + " physical_overlap=not_measured")
        if not profiles:
            ids = "[10, 11]" if len(variants) == 2 else "[10]"
            lines.append(prefix + " ids=" + ids + " prefix=Some(Ok(Pending)) shutdown=" + shutdown() + " physical_overlap=not_measured")
            lines.append(prefix + "_profile_json=" + json.dumps(t.fixture(case)))
            if case != "owner":
                lines.append("mixed_observer_case=" + {
                    "timeout": "timeout recovered_same_operation=true", "drop": "drop autonomous_completion=true",
                    "backpressure": "backpressure command_queue_full=1 rejected_launch_issued=false recovered=true native_slot_saturation=not_measured"}[case])
    lines += ["ok", "", f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {p.FILTERED} filtered out; finished in 1.00s", ""]
    return "\n".join(lines)


def synthetic_native(folder, protocol):
    s2, p, n, b, _, v = s.module(HERE / "verify.py").helpers(protocol)
    folder.mkdir()
    shutil.copytree(protocol, folder / "protocol")
    (folder / "payload").mkdir()
    for name in n.PAYLOAD - {"runtime-tests"}:
        shutil.copy2(protocol / name, folder / "payload" / name)
    (folder / "payload/runtime-tests").write_bytes(b"synthetic ELF stand-in, never executed")
    remote = folder / "remote"
    remote.mkdir()
    shutil.copytree(folder / "payload", remote / "artifacts")
    binding = {"commit": p.COMMIT, "payload": b.inventory(folder / "payload"), "order": p.CASE_NAMES,
               "outer_seconds": n.REMOTE_SECONDS, "protocol": b.inventory(protocol), "build_sha256": "synthetic",
               "source_files": {}, "device": [p.GPU, p.BDF, p.UID], "target": "synthetic", "features": "synthetic",
               "build_environment": {}}
    b.write_json(folder / "binding.json", binding)
    marker = {"path": p.PREFIX + "a" * 16, "commit": p.COMMIT, "binding_sha256": p.sha(folder / "binding.json")}
    b.write_json(folder / "owner.json", marker)
    root = Path(marker["path"])
    old = s.OLD / "raw/campaign1/remote"
    cursor, ordinal = 100000000000, 0
    def stamp(tick):
        return {"realtime_ns": tick + 1000000000000000000, "monotonic_ns": tick}
    def record(name, argv, env, bound, stdout, duration=100000000):
        nonlocal cursor, ordinal
        ordinal += 1
        target = remote / name
        target.mkdir()
        (target / "stdout.log").write_text(stdout)
        (target / "stderr.log").write_text("")
        row = {"name": name, "command": argv, "cwd": str(root), "environment": env,
               "started": stamp(cursor), "spawned": stamp(cursor + 10), "status": 0, "error": None,
               "process_group": 1000 + ordinal, "group_absent": True, "t0": stamp(cursor + duration),
               "outer_bound_seconds": bound, "finished": stamp(cursor + duration + 1),
               "stdout_sha256": p.sha(target / "stdout.log"), "stderr_sha256": p.sha(target / "stderr.log")}
        b.write_json(target / "record.json", row)
        active = remote / f"active-{ordinal:03}"
        active.mkdir()
        b.write_json(active / "receipt.json", {"pid": row["process_group"], "command": argv, "started": stamp(cursor + 5),
                                               "purpose": "record group before unblocking managed signals"})
        cursor += duration + 100
        return row
    def observe(name):
        data = [p.parse(line) for line in (old / "01-queued-producer-before/stdout.log").read_text().splitlines()]
        start = p.stamp(data[0]["started"])
        shift = cursor + 1000 - start
        def move(value):
            if isinstance(value, dict):
                if set(value) == {"monotonic_ns", "utc"}:
                    value["monotonic_ns"] += shift
                else:
                    for child in value.values():
                        move(child)
            elif isinstance(value, list):
                for child in value:
                    move(child)
        move(data)
        duration = p.stamp(data[0]["finished"]) - cursor + 1000
        record(name, ["/usr/bin/python3", "-I", "-B", str(root / "observer.py"), "--gpu-index", str(p.GPU),
                      "--pci-bdf", p.BDF, "--unique-id", p.UID], p.environment(root), n.OBSERVE_SECONDS,
               "".join(json.dumps(row) + "\n" for row in data), duration)
    record("topology", ["/usr/bin/python3", "-I", "-B", str(root / "topology.py"), "topology", "--gpu-index", str(p.GPU),
                        "--pci-bdf", p.BDF, "--unique-id", p.UID], p.environment(root), 100,
           (old / "topology/stdout.log").read_text())
    record("placement", ["/usr/bin/numactl", "--physcpubind=0-47", "--membind=0", "/usr/bin/numactl", "--show"],
           p.environment(root), 30, (old / "placement/stdout.log").read_text())
    cases = []
    for index, (case, _) in enumerate(p.CASE_NAMES, 1):
        label = f"{index:02}-{case}"
        shutil.copy2(old / "01-queued-producer-resources.json", remote / (label + "-resources.json"))
        observe(label + "-before")
        text = transcript(p, case)
        row = record(label + "-test", p.command(root, case), p.environment(root, case), p.test_seconds(case), text)
        observe(label + "-immediate")
        cursor = row["t0"]["monotonic_ns"] + 20 * 10**9
        observe(label + "-delayed")
        cases.append({"case": case, "ordinal": index, "failures": [], "transcript": p.transcript(case, text, ""),
                      "post_observations": {"immediate": "strict_pass", "delayed": "strict_pass"}})
    (remote / "outer").mkdir()
    b.write_json(remote / "outer/receipt.json", {"pid": 999, "controller_pid": 998, "purpose": "remote timeout process group"})
    b.write_json(remote / "finished.json", {"commit": p.COMMIT, "cases": cases, "failures": [], "spawn_failures": [],
                                           "complete_matrix": True, "exclusive_reservation": False,
                                           "performance_acceptance": False, "formal_refinement": False})
    b.write_json(folder / "remote-inventory.json", b.inventory(remote))
    return p, n, b, v


class ProtocolTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="fe2o3-async-calibration-")
        cls.root = Path(cls.temp.name)
        cls.protocol = cls.root / "protocol"
        s.stage(cls.protocol)
        cls.p = s.module(cls.protocol / "protocol.py")

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_all_transcripts(self):
        for case in self.p.TESTS:
            with self.subTest(case=case):
                text = transcript(self.p, case)
                self.assertEqual(self.p.transcript(case, text, "")["harness_passes"], 1)
                for bad in (text.replace(f"{self.p.FILTERED} filtered", "1446 filtered"), text.replace("1 passed;", "0 passed;"),
                            text + "panicked at bad\n", text.replace("observations: [1, 1]", "observations: [1, 0]")):
                    with self.assertRaises(ValueError):
                        self.p.transcript(case, bad, "")
                with self.assertRaises(ValueError):
                    self.p.transcript(case, text, "unexpected stderr")

    def test_every_observed_byte(self):
        text = transcript(self.p, "owner")
        original = self.p.ORACLE.expected("long")
        for index in range(384):
            bad = bytearray(original)
            bad[index] ^= 1
            with self.assertRaises(ValueError):
                self.p.transcript("owner", text.replace(original.hex(), bad.hex()), "")

    def test_profiles_are_independently_complete(self):
        for case, variant, other in (("profiles-short", "Short", "Long"), ("profiles-long", "Long", "Short")):
            text = transcript(self.p, case)
            output = "mixed_duration variant=" + variant + " observed_hex=" + self.p.ORACLE.expected(variant.lower()).hex()
            opposite = "mixed_duration variant=" + other + " observed_hex=" + self.p.ORACLE.expected(other.lower()).hex()
            for bad in (text.replace(output, ""), text + output + "\n", text + opposite + "\n",
                        text.replace("variant=" + variant + " shutdown=", "variant=" + other + " shutdown="),
                        text + "unexpected variant=" + other + "\n"):
                with self.subTest(case=case), self.assertRaises(ValueError):
                    self.p.transcript(case, bad, "")
            other_case = "profiles-long" if variant == "Short" else "profiles-short"
            pair = [line for line in transcript(self.p, other_case).splitlines() if line.startswith("mixed_duration variant=")]
            body = [line for line in text.splitlines() if not line.startswith("mixed_duration variant=")]
            for bad in ("\n".join(body + pair) + "\n", text + "\n".join(pair) + "\n"):
                with self.subTest(case=case), self.assertRaisesRegex(ValueError, "exact full-output order"):
                    self.p.transcript(case, bad, "")

    def test_depth_membership_and_profile(self):
        value, profile = depth_fixture(self.p)
        self.assertEqual(self.p.SCALE.check(value, profile, t.p)["events"], 6179)
        mutations = [lambda v: v["receipts"][1].__setitem__("native_receipt", v["receipts"][0]["native_receipt"]),
                     lambda v: v["receipts"][1].__setitem__("ordered_predecessor", 777),
                     lambda v: v["receipts"][0].__setitem__("pipeline_phase", "published"),
                     lambda v: v.__setitem__("unfinished_gpu_count", 2048),
                     lambda v: v["final_buffer_sha256"].__setitem__(0, "0" * 64)]
        for mutate in mutations:
            bad = copy.deepcopy(value)
            mutate(bad)
            bad["membership"] = self.p.SCALE.membership(bad["receipts"])
            with self.assertRaises(ValueError):
                self.p.SCALE.check(bad, profile, t.p)
        for mutation in ("queue", "binding", "shape", "order", "early"):
            events = [copy.deepcopy(row["event"]) for row in profile["events"]]
            pubs = [row for row in events if row["kind"] == "dispatch_published"]
            if mutation == "queue":
                pubs[0]["queue"] = pubs[1]["queue"]
            elif mutation == "binding":
                pubs[0]["bindings"][1]["kernarg_byte_offset"] = 8
            elif mutation == "shape":
                pubs[0]["dispatch_shape"] = t.p.content(b"wrong")
            else:
                indexes = [i for i, row in enumerate(events) if row["kind"] == "dispatch_completed"]
                if mutation == "order":
                    a, b = indexes[0], indexes[2]
                    events[a], events[b] = events[b], events[a]
                else:
                    row = events.pop(indexes[0])
                    events.insert(events.index(pubs[-1]), row)
            with self.assertRaises(ValueError):
                self.p.SCALE.check(value, t.sealed(events, profile["capture_scope"]), t.p)

    def test_fixed_commands(self):
        root = Path("/owned")
        for case in self.p.TESTS:
            argv = self.p.command(root, case)
            self.assertEqual(argv[2:4], ["--kill-after=10s", "600s"] if case == "depth" else ["--kill-after=5s", "180s"])
            self.assertEqual(self.p.test_seconds(case), 620 if case == "depth" else 200)
            self.assertIn(self.p.TESTS[case], argv)
            self.assertNotIn("FE2O3_TEST_NATIVE_ISOLATED", self.p.environment(root, case))

    def test_authentication_precedes_retained_execution(self):
        verify = s.module(HERE / "verify.py")
        for index, name in enumerate(("protocol.py", "native.py", "profile.py", "protocol-base.py")):
            folder = self.root / ("corrupt-" + str(index))
            shutil.copytree(self.protocol, folder)
            (folder / name).write_text("raise RuntimeError('unauthenticated bytes executed')\n")
            with self.assertRaisesRegex(ValueError, "pinned helper"):
                verify.helpers(folder)

    def test_full_native_replay_and_resealed_mutations(self):
        folder = self.root / "synthetic"
        p, n, b, v = synthetic_native(folder, self.protocol)
        v.verify_native(folder, p, n, b, False)
        remote = folder / "remote"
        self.assertEqual(len(list(remote.glob("*/record.json"))), 30)
        self.assertEqual(len(list(remote.glob("active-*/receipt.json"))), 30)
        self.assertEqual(sum(len(list(remote.glob("*-" + suffix))) for suffix in ("before", "immediate", "delayed")), 21)
        def overwrite(path, value):
            path.write_text(json.dumps(value, indent=2) + "\n")
        def reject(path, mutate):
            original = path.read_bytes()
            value = json.loads(original)
            mutate(value)
            overwrite(path, value)
            overwrite(folder / "remote-inventory.json", b.inventory(remote))
            with self.assertRaises(ValueError):
                v.verify_native(folder, p, n, b, False)
            path.write_bytes(original)
            overwrite(folder / "remote-inventory.json", b.inventory(remote))
        reject(remote / "07-depth-test/record.json", lambda row: row.__setitem__("outer_bound_seconds", 200))
        reject(remote / "03-owner-test/record.json", lambda row: row.__setitem__("status", False))
        reject(remote / "03-owner-test/record.json", lambda row: row.__setitem__("group_absent", False))
        reject(remote / "active-004/receipt.json", lambda row: row.__setitem__("pid", 99))
        reject(remote / "04-timeout-resources.json", lambda row: row.__setitem__("disk_free", True))
        reject(remote / "finished.json", lambda row: row.__setitem__("cases", row["cases"][:-1]))
        # Rehash both the altered raw observation and inventory to reach strict endpoint checks.
        path = remote / "05-drop-delayed/stdout.log"
        original, receipt = path.read_bytes(), (remote / "05-drop-delayed/record.json").read_bytes()
        data = [p.parse(line) for line in original.splitlines()]
        data[0]["sysfs"][1]["values"]["mem_busy_percent"] = "1"
        path.write_text("".join(json.dumps(row) + "\n" for row in data))
        row = json.loads(receipt)
        row["stdout_sha256"] = p.sha(path)
        overwrite(path.parent / "record.json", row)
        overwrite(folder / "remote-inventory.json", b.inventory(remote))
        with self.assertRaises(ValueError):
            v.verify_native(folder, p, n, b, False)
        path.write_bytes(original)
        (path.parent / "record.json").write_bytes(receipt)
        overwrite(folder / "remote-inventory.json", b.inventory(remote))
        v.verify_native(folder, p, n, b, False)


if __name__ == "__main__":
    unittest.main()
