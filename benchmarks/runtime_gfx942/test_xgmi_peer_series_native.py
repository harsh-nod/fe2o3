#!/usr/bin/env python3
"""Light synthetic controls. No compiler, native query, subprocess, or GPU calls."""

import contextlib
import copy
import hashlib
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import types
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import xgmi_peer_series_campaign as campaign
import xgmi_peer_series_host as host
import xgmi_peer_series_native as native
import xgmi_peer_series_observations as observations
from test_xgmi_peer_series_campaign import environment, inputs, receipts

BOOT = "00112233-4455-6677-8899-aabbccddeeff"
PHYSICAL = [{key: row[key] for key in ("physical_index", "unique_id", "pci_bdf")}
            for row in inputs()["admission"]["endpoints"]]


def query(backend):
    rows = [f"schema=xgmi-peer-query-v1 backend={backend} visible_count=2"]
    if backend == "kfd":
        rows.append(f"boot_id={BOOT} generation=17")
    for index, endpoint in enumerate(inputs()["admission"]["endpoints"]):
        rows.append(f"visible_index={index} unique_id={endpoint['unique_id']} "
                    f"pci_bdf={endpoint['pci_bdf']} target=gfx942:xnack-" +
                    (f" kfd_gpu_id={endpoint['kfd_gpu_id']}" if backend == "kfd" else ""))
    if backend == "kfd":
        rows += ["source_gpu_id=101 destination_gpu_id=202 engine_id=2",
                 "source_gpu_id=202 destination_gpu_id=101 engine_id=3"]
    return ("\n".join(rows) + "\n").encode("ascii")


class ObservationTests(unittest.TestCase):
    def test_exact_independent_queries_join_existing_planner(self):
        admission, incarnation = observations.join_admission(PHYSICAL,
            {backend: query(backend) for backend in ("kfd", "hip", "hsa")}, inventory_sha256="1" * 64)
        self.assertEqual(incarnation, {"boot_id": BOOT, "generation": 17})
        expected = inputs()["admission"]
        for key in ("endpoints", "routes", "visibility"):
            self.assertEqual(admission[key], expected[key])
        self.assertEqual(len(campaign.trial_specs(**{**inputs(), "admission": admission})["trials"]), 18)

    def test_missing_extra_truncated_nul_cr_and_noncanonical_queries_fail(self):
        for backend in ("kfd", "hip", "hsa"):
            good = query(backend)
            for bad in (good[:-1], good + b"\n", good.replace(b"\n", b"\r\n"), b"\0" + good,
                        good.replace(b"visible_count=2", b"visible_count=3"),
                        good.replace(b"visible_index=1", b"visible_index=0"),
                        good.replace(b"gfx942:xnack-", b"gfx942:xnack+"),
                        good.replace(b"b7baafd0fb173d8e", b"0000000000000000")):
                with self.subTest(backend=backend, bad=bad), self.assertRaises(ValueError):
                    observations.parse_query(bad, backend)

    def test_visibility_is_observed_not_inferred_from_filters(self):
        for backend in ("hip", "hsa", "kfd"):
            for old, new in ((b"0000:26:00.0", b"0000:27:00.0"),
                             (b"b7baafd0fb173d8e", b"b7baafd0fb173d8f")):
                queries = {name: query(name) for name in ("kfd", "hip", "hsa")}
                queries[backend] = queries[backend].replace(old, new)
                with self.subTest(backend=backend), self.assertRaises(ValueError):
                    observations.join_admission(PHYSICAL, queries, inventory_sha256="1" * 64)

    def test_reversed_route_and_aliased_gpu_identity_fail(self):
        for old, new in ((b"source_gpu_id=101 destination_gpu_id=202", b"source_gpu_id=202 destination_gpu_id=101"),
                         (b"kfd_gpu_id=202", b"kfd_gpu_id=101"), (b"engine_id=2", b"engine_id=4294967296")):
            queries = {name: query(name) for name in ("kfd", "hip", "hsa")}
            queries["kfd"] = queries["kfd"].replace(old, new)
            with self.assertRaises(ValueError):
                observations.join_admission(PHYSICAL, queries, inventory_sha256="1" * 64)

    def test_package_roster_selects_observed_tree_not_reviewed_label(self):
        self.assertEqual(host.package_source_root(b"/usr/src/amdgpu-1.2/a\n/usr/src/amdgpu-1.2/b\n"), "amdgpu-1.2")
        for raw in (b"", b"/usr/src/unrelated\n", b"/usr/src/amdgpu-1/a\n/usr/src/amdgpu-2/a\n"):
            with self.assertRaises(ValueError):
                host.package_source_root(raw)

    def test_package_root_and_source_leaf_must_be_canonical_without_aliases(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            root = parent / "amdgpu-1"
            root.mkdir()
            source = root / "source.c"
            source.write_bytes(b"source\n")
            self.assertEqual(host.canonical_source_path(root, "source.c"), source)
            alias = parent / "amdgpu-alias"
            alias.symlink_to(root, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "root"):
                host.canonical_source_path(alias, "source.c")
            (root / "alias.c").symlink_to(source)
            with self.assertRaisesRegex(ValueError, "aliases"):
                host.canonical_source_path(root, "alias.c")
            (root / "escape.c").symlink_to(parent / "outside.c")
            (parent / "outside.c").write_bytes(b"outside\n")
            with self.assertRaisesRegex(ValueError, "aliases"):
                host.canonical_source_path(root, "escape.c")
            nested = root / "real"
            nested.mkdir()
            (nested / "file.c").write_bytes(b"nested\n")
            (root / "alias-dir").symlink_to(nested, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "aliases"):
                host.canonical_source_path(root, "alias-dir/file.c")
            for name in ("../outside.c", "./source.c", str(source), "real//file.c"):
                with self.assertRaises(ValueError):
                    host.canonical_source_path(root, name)

    def test_all_result_backends_use_existing_strict_parser(self):
        plan = campaign.trial_specs(**inputs())
        for spec, record in zip(plan["trials"], receipts()):
            native.validate_trial(campaign, plan, spec, record)
            with self.assertRaises(ValueError):
                native.validate_trial(campaign, plan, spec, {**record, "stderr": b"warning"})


class SourceQueryTests(unittest.TestCase):
    def test_selected_helper_checkout_attributes_are_explicit_signed_source(self):
        path = "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/.gitattributes"
        self.assertIn(path, native.SOURCE_ROOTS)
        self.assertEqual(hashlib.sha256((native.ROOT / path).read_bytes()).hexdigest(),
                         "705fd4d6451a31d36b3df7de96f83f30ac976c9b4a6d1e51671d8e2f33e2d0da")

    def test_native_kfd_build_explicitly_enables_required_live_validation_feature(self):
        name, command, seconds = native.build_specs(Path("/private/binaries"))[0]
        self.assertEqual(name, "build-kfd")
        self.assertEqual(command, ["cargo", "build", "--frozen", "--release", "-p", "fe2o3-kfd",
                                   "--features", "live-validation", "--example", "kfd-sdma-xgmi-peer-benchmark"])
        self.assertEqual(seconds, 1200)

    def test_existing_cli_and_timing_producers_are_byte_preserved(self):
        expected = {
            "hip": "d1ba6715a3660274c37a4764a90dbdc66507ee1c77696c9b100a2d1b5dbcc91a",
            "hsa": "bc8fc5f7e24de6a9536194e28c92159fba4de9aafa7140ef6927af3fb5d7b3a9",
            "kfd": "5375595991d27061079e2e355945df9ed73c00b8a099245d3516026a403b5214",
        }
        for backend, digest in expected.items():
            if backend == "kfd":
                source = (native.ROOT / "crates/fe2o3-kfd/examples/kfd-sdma-xgmi-peer-benchmark.rs").read_text("ascii")
                start, end = source.index("fn inspect_peer_pair("), source.index("fn map_with_cleanup(")
                source = source[:start] + source[end:]
                start = source.index('    if args.first().map(String::as_str) == Some("--inspect-peer-pair")')
                end = source.index("    if args.len() != 6")
                source = source[:start] + source[end:]
            else:
                source = (native.HERE / f"xgmi_peer_{backend}.cpp").read_text("ascii")
                start = source.index("static int inspect_peer_pair()")
                end = source.index("static double gbps(" if backend == "hip" else "struct PoolSelection")
                source = source[:start] + source[end:]
                source = source.replace('  if (argc == 2 && std::strcmp(argv[1], "--inspect-peer-pair") == 0)\n'
                                        '    return inspect_peer_pair();\n', "", 1)
            self.assertEqual(hashlib.sha256(source.encode("ascii")).hexdigest(), digest, backend)

    def test_query_dispatch_precedes_workload_and_has_no_workload_calls(self):
        for backend in ("hip", "hsa", "kfd"):
            if backend == "kfd":
                path = native.ROOT / "crates/fe2o3-kfd/examples/kfd-sdma-xgmi-peer-benchmark.rs"
                start, end = "fn inspect_peer_pair(", "fn map_with_cleanup("
                required = ("admit_device(", "check_observable_currentness()", "admit_gfx942_xgmi_route(",
                            "render.pci_address()", "gpu.unique_id()", "recommended_engine_id()")
                main = "fn main()"
                forbidden = ("acquire_shared_gtt_memory_session", "allocate_pair", "retained_series::run", "QueueV1::create")
            else:
                path = native.HERE / f"xgmi_peer_{backend}.cpp"
                start = "static int inspect_peer_pair()"
                end = "static double gbps(" if backend == "hip" else "struct PoolSelection"
                required = (("hipGetDeviceCount(", "hipDeviceGetUuid(", "hipGetDeviceProperties(", "hipDeviceGetPCIBusId(")
                            if backend == "hip" else ("hsa_iterate_agents(", "HSA_AMD_AGENT_INFO_UUID",
                                "HSA_AMD_AGENT_INFO_BDFID", "HSA_AMD_AGENT_INFO_DOMAIN", "hsa_shut_down()"))
                main = "int main("
                forbidden = ("hipMalloc", "hipStreamCreate", "hipMemcpy", "hipDeviceEnablePeerAccess",
                             "hsa_amd_memory_pool_allocate", "hsa_amd_memory_async_copy", "hsa_queue_create")
            source = path.read_text("ascii")
            body = source.split(start, 1)[1].split(end, 1)[0]
            for name in required:
                self.assertIn(name, body)
            for name in forbidden:
                self.assertNotIn(name, body)
            entry = source.split(main, 1)[1]
            query_position = entry.index("--inspect-peer-pair")
            workload_position = entry.index("args.len() != 6" if backend == "kfd" else "argc != 9")
            self.assertLess(query_position, workload_position)
            self.assertIn("return inspect_peer_pair", entry[:workload_position])

    def test_query_spec_only_selects_visibility_and_never_workload_controls(self):
        dirty = {key: "untrusted" for key in campaign._CLEAR_ENVIRONMENT}
        specs = native.query_specs(Path("/private/binaries"), PHYSICAL, {**dirty, "PATH": "/usr/bin"},
                                   campaign._CLEAR_ENVIRONMENT)
        self.assertEqual([row[0] for row in specs], ["kfd", "hsa", "hip"])
        for backend, command, env in specs:
            self.assertEqual(command[1], "--inspect-peer-pair")
            self.assertEqual(env["HSA_XNACK"], "0")
            self.assertNotIn("--persistent-series", command)
            self.assertNotIn("--retained-pair-series-reviewed-mi300x", command)
            if backend != "kfd":
                self.assertEqual(len(command), 2)
                self.assertEqual(env["HIP_VISIBLE_DEVICES" if backend == "hip" else "ROCR_VISIBLE_DEVICES"], "1,2")
            for key in campaign._CLEAR_ENVIRONMENT:
                if key != "HSA_XNACK" and key != {"hip": "HIP_VISIBLE_DEVICES", "hsa": "ROCR_VISIBLE_DEVICES"}.get(backend):
                    self.assertNotIn(key, env)

    def test_source_snapshot_rejects_edits_missing_and_extra_helper_files(self):
        hot, _, _ = native.load_helpers()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, output = root / "source", root / "commands"
            source.mkdir()
            output.mkdir()
            (source / "helpers").mkdir()
            (source / "Cargo.toml").write_bytes(b"workspace\n")
            helper = source / "helpers/runner.py"
            helper.write_bytes(b"pass\n")
            tree = b""
            for relative in ("Cargo.toml", "helpers/runner.py"):
                data = (source / relative).read_bytes()
                oid = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()
                tree += f"100644 blob {oid}\t{relative}\0".encode("ascii")

            class GitRecorder:
                def run(self, name, command, seconds, **kwargs):
                    if command[:4] != ["/usr/bin/git", "--no-replace-objects", "-c", "gc.auto=0"]:
                        raise AssertionError("source reads must ignore replacement refs")
                    folder = output / name
                    folder.mkdir()
                    (folder / "stdout").write_bytes(b"1" * 40 + b"\n" if name.endswith("-head") else tree)
                    (folder / "stderr").write_bytes(b"")
                    return folder

            with mock.patch.object(native, "ROOT", source), mock.patch.object(native, "SOURCE_ROOTS", ("Cargo.toml", ".cargo", "helpers")):
                self.assertEqual(set(native.source_snapshot(GitRecorder(), "valid", "1" * 40, {}, hot)),
                                 {"Cargo.toml", "helpers/runner.py"})
                (source / ".cargo").mkdir()
                with self.assertRaisesRegex(RuntimeError, "unsigned optional"):
                    native.source_snapshot(GitRecorder(), "unsigned-optional", "1" * 40, {}, hot)
                (source / ".cargo").rmdir()
                (source / ".cargo").symlink_to(source / "missing")
                with self.assertRaisesRegex(RuntimeError, "unsigned optional"):
                    native.source_snapshot(GitRecorder(), "symlink-optional", "1" * 40, {}, hot)
                (source / ".cargo").unlink()
                helper.write_bytes(b"changed\n")
                with self.assertRaises(RuntimeError):
                    native.source_snapshot(GitRecorder(), "changed", "1" * 40, {}, hot)
                helper.unlink()
                with self.assertRaises(RuntimeError):
                    native.source_snapshot(GitRecorder(), "missing", "1" * 40, {}, hot)
                helper.write_bytes(b"pass\n")
                (source / "helpers/extra.py").write_bytes(b"unexpected\n")
                with self.assertRaises(RuntimeError):
                    native.source_snapshot(GitRecorder(), "extra", "1" * 40, {}, hot)

    def test_current_root_signed_absent_optional_cargo(self):
        hot, _, _ = native.load_helpers()
        self.assertFalse((native.ROOT / ".cargo").exists())
        data = (native.ROOT / "Cargo.toml").read_bytes()
        oid = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()
        tree = f"100644 blob {oid}\tCargo.toml\0".encode("ascii")
        with tempfile.TemporaryDirectory() as temporary:
            class GitRecorder:
                def run(self, name, command, seconds, **kwargs):
                    folder = Path(temporary) / name
                    folder.mkdir()
                    (folder / "stdout").write_bytes(b"1" * 40 + b"\n" if name.endswith("-head") else tree)
                    (folder / "stderr").write_bytes(b"")
                    return folder
            with mock.patch.object(native, "SOURCE_ROOTS", ("Cargo.toml", ".cargo")):
                self.assertEqual(set(native.source_snapshot(GitRecorder(), "actual-root", "1" * 40, {}, hot)),
                                 {"Cargo.toml"})


class CampaignControls(unittest.TestCase):
    def harness(self, root, *, bad_query=False, bad_workload=False, disk_error=False,
                receipt_error=False, interrupt=False, source_error=False, build_error=False):
        hot, _, _ = native.load_helpers()
        output = root / "campaign"
        args = types.SimpleNamespace(output=output, commit="1" * 40, devices=copy.deepcopy(PHYSICAL))
        called = []
        fixture = {row["name"]: row for row in receipts()}
        original_write = hot.B.write_json
        original_postflight = hot.B.settled_postflight

        class FakeRecorder:
            def __init__(self, output, cwd):
                self.output, self.cwd = output, cwd
                output.mkdir()

            def run(self, name, command, seconds, *, env=None):
                called.append(name)
                folder = self.output / name
                folder.mkdir()
                data = b""
                if name.startswith("build-") and not build_error:
                    backend = name.removeprefix("build-")
                    path = (Path(env["CARGO_TARGET_DIR"]) / "release/examples/kfd-sdma-xgmi-peer-benchmark"
                            if backend == "kfd" else Path(command[-1]))
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(backend.encode("ascii"))
                elif name.startswith("host-"):
                    if source_error or build_error:
                        data = b"{}"
                    else:
                        context = {"boot_id": BOOT, "unique_id": "0x" + PHYSICAL[0]["unique_id"],
                                   "pci_bdf": PHYSICAL[0]["pci_bdf"]}
                        for backend in ("kfd", "hip", "hsa"):
                            path = Path(command[command.index("--" + backend + "-binary") + 1])
                            context[backend + "_binary_sha256"] = hot.sha(path)
                        data = json.dumps({"schema": "fe2o3.xgmi-peer-series-host-observation.v1",
                                           "environment": environment(), "context": context,
                                           "continuity": {"boot_id": BOOT}}).encode("ascii")
                elif "--inspect-peer-pair" in command:
                    data = query(Path(command[0]).name.removesuffix("-series"))
                    if bad_query:
                        data = data.replace(b"0000:26:00.0", b"0000:27:00.0")
                elif name in fixture:
                    data = b"invalid\n" if bad_workload else fixture[name]["stdout"]
                for stream, raw in (("stdout", data), ("stderr", b"")):
                    (folder / stream).write_bytes(raw)
                if receipt_error and name == "d01-t01-kfd":
                    raise OSError("synthetic receipt-save error")
                original_write(folder / "receipt.json", {"pid": 10000 + len(called), "group_absent": True,
                    "stdout_sha256": hot.sha(folder / "stdout"), "stderr_sha256": hot.sha(folder / "stderr")})
                if build_error and name == "build-kfd":
                    raise RuntimeError("synthetic build failure")
                if interrupt and name == "d01-t01-kfd":
                    hot.B.interrupted(signal.SIGTERM, None)
                return folder

        def write(path, value):
            if disk_error and path.name == "finished.json":
                raise OSError("synthetic disk error")
            original_write(path, value)

        def source_snapshot(_rec, label, *_args):
            if source_error and label == "source-before":
                raise RuntimeError("synthetic opening source failure")
            return {"source": "2" * 64}

        handlers = {sig: signal.getsignal(sig) for sig in hot.B.MANAGED}
        with contextlib.ExitStack() as stack:
            stack.enter_context(mock.patch.object(hot.B, "Recorder", FakeRecorder))
            stack.enter_context(mock.patch.object(hot.B, "write_json", side_effect=write))
            stack.enter_context(mock.patch.object(hot.B, "settled_postflight", side_effect=lambda observe, name, failure=None:
                original_postflight(observe, name, failure, sleep=lambda _seconds: None)))
            stack.enter_context(mock.patch.object(hot, "parse_endpoint", side_effect=lambda raw, index, bdf, uid:
                {"gpu_index": index, "pci_bdf": bdf, "unique_id": uid}))
            stack.enter_context(mock.patch.object(native, "source_snapshot", side_effect=source_snapshot))
            stack.enter_context(mock.patch.object(native, "tool_snapshot", return_value={"tool": "3" * 64}))
            stack.enter_context(mock.patch.object(native.resource, "setrlimit"))
            # Any unintended process execution or historical PID probe is a test failure.
            stack.enter_context(mock.patch.object(hot.B.subprocess, "Popen", side_effect=AssertionError("process launched")))
            stack.enter_context(mock.patch.object(hot.B.os, "killpg", side_effect=AssertionError("historical PID probe")))
            error = None
            try:
                native.run_campaign(args, hot, campaign, observations)
            except BaseException as caught:
                error = caught
        self.assertEqual({sig: signal.getsignal(sig) for sig in handlers}, handlers)
        return output, called, error

    def test_complete_fake_campaign_joins_all_eighteen_and_cleans_only_private_scratch(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sentinel = root / "unrelated"
            sentinel.write_text("keep")
            output, called, error = self.harness(root)
            self.assertIsNone(error, repr(error))
            self.assertEqual(len([name for name in called if name in {row["name"] for row in receipts()}]), 18)
            self.assertFalse((output / "work").exists())
            self.assertEqual(sentinel.read_text(), "keep")
            final = json.loads((output / "finished.json").read_text())
            self.assertTrue(final["native_execution"])
            self.assertFalse(final["performance_acceptance"])
            self.assertEqual(len(json.loads((output / "replay.json").read_text())["trials"]), 18)
            self.assertTrue(all("admission-after-" + backend in called for backend in ("kfd", "hsa", "hip")))
            self.assertFalse((output / "admission-after-status.json").exists())

    def test_prebuild_failure_explicitly_skips_api_admission_but_keeps_physical_postflights(self):
        for options in ({"source_error": True}, {"build_error": True}):
            with self.subTest(options=options), tempfile.TemporaryDirectory() as temporary:
                output, called, error = self.harness(Path(temporary), **options)
                self.assertIsNotNone(error)
                self.assertFalse(any(name.startswith("admission-") or "-query-" in name for name in called))
                self.assertFalse(set(called) & {row["name"] for row in receipts()})
                for edge in ("settled", "delayed"):
                    for index in (1, 2):
                        self.assertIn(f"campaign-close-{edge}-gpu{index}", called)
                status = json.loads((output / "admission-after-status.json").read_bytes())
                self.assertEqual(status, {"performed": False, "status": "not-performed",
                    "reason": "native-binaries-not-built", "native_acceptance": False})
                final = json.loads((output / "finished.json").read_bytes())
                self.assertFalse(final["native_execution"])
                self.assertTrue(any("not performed: native binaries were not built" in row for row in final["failures"]))
                self.assertEqual(json.loads((output / "records.json").read_bytes()), [])
                self.assertFalse((output / "replay.json").exists())
                self.assertTrue((output / "fresh-census.json").exists())

    def test_bad_query_runs_zero_workloads_and_still_closes(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, called, error = self.harness(Path(temporary), bad_query=True)
            self.assertIsNotNone(error)
            self.assertFalse(set(called) & {row["name"] for row in receipts()})
            self.assertIn("host-after", called)
            self.assertTrue(any(name.startswith("campaign-close") for name in called))

    def test_bad_first_result_stops_before_second_workload(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, called, error = self.harness(Path(temporary), bad_workload=True)
            self.assertIsNotNone(error)
            self.assertEqual([name for name in called if name in {row["name"] for row in receipts()}], ["d01-t01-kfd"])
            self.assertIn("host-after", called)
            for suffix in ("settled", "delayed"):
                for index in (1, 2):
                    self.assertIn(f"d01-t01-kfd-{suffix}-gpu{index}", called)

    def test_signal_and_disk_error_restore_handlers(self):
        for options in ({"interrupt": True}, {"disk_error": True}):
            with tempfile.TemporaryDirectory() as temporary:
                output, called, error = self.harness(Path(temporary), **options)
                self.assertIsNotNone(error)
                self.assertIn("host-after", called)
                self.assertFalse((output / "work").exists())

    def test_receipt_save_failure_restores_handlers_and_preserves_unclosed_scratch(self):
        with tempfile.TemporaryDirectory() as temporary:
            output, called, error = self.harness(Path(temporary), receipt_error=True)
            self.assertIsNotNone(error)
            self.assertIn("host-after", called)
            self.assertNotIn("d01-t02-hsa", called)
            self.assertTrue((output / "work").exists())
            self.assertFalse(json.loads((output / "finished.json").read_text())["native_execution"])

    def test_stale_output_refused_without_touching_it(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(ValueError):
                native.fresh_output(root)
            self.assertTrue(root.is_dir())

    def test_fresh_census_rejects_raw_drift_extra_stages_and_missing_receipts(self):
        hot, _, _ = native.load_helpers()
        for mutation in ("raw", "extra", "missing", "namespace"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                folder = root / "one"
                folder.mkdir()
                (folder / "stdout").write_bytes(b"ok\n")
                (folder / "stderr").write_bytes(b"")
                hot.B.write_json(folder / "receipt.json", {"pid": 999, "group_absent": True,
                    "stdout_sha256": hot.sha(folder / "stdout"), "stderr_sha256": hot.sha(folder / "stderr")})
                recorder = types.SimpleNamespace(output=root, attempts=["one"], pins={"one": {
                    stream: hot.sha(folder / stream) for stream in ("receipt.json", "stdout", "stderr")}})
                namespace = os.readlink("/proc/self/ns/pid")
                with mock.patch.object(hot.B.os, "killpg", side_effect=AssertionError("historical PID probe")):
                    self.assertEqual(len(native.fresh_census(recorder, hot, namespace)["records"]), 1)
                    if mutation == "raw":
                        (folder / "stdout").write_bytes(b"changed\n")
                    elif mutation == "extra":
                        (root / "old-receipt").mkdir()
                    elif mutation == "missing":
                        (folder / "receipt.json").unlink()
                    else:
                        namespace = "pid:[0]"
                    with self.assertRaises(RuntimeError):
                        native.fresh_census(recorder, hot, namespace)


if __name__ == "__main__":
    unittest.main()
