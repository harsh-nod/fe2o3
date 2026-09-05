#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import importlib.util
import os
import pathlib
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from contextlib import ExitStack
from unittest import mock


GUARD_PATH = pathlib.Path(__file__).with_name("r26-host-guard.py")
SPEC = importlib.util.spec_from_file_location("fe2o3_r26_host_guard", GUARD_PATH)
assert SPEC is not None and SPEC.loader is not None
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class TopologyFixture:
    bdf = "0000:05:00.0"
    unique_id = "0x6ced1647a296545c"

    def __init__(self, root: pathlib.Path) -> None:
        self.pci_root = root / "pci"
        self.topology_root = root / "topology"
        self.status_path = root / "status"
        device = self.pci_root / self.bdf
        device.mkdir(parents=True)
        (device / "vendor").write_text("0x1002\n", encoding="ascii")
        (device / "unique_id").write_text(
            self.unique_id.removeprefix("0x") + "\n", encoding="ascii"
        )
        (device / "numa_node").write_text("0\n", encoding="ascii")
        (device / "local_cpulist").write_text("0-7\n", encoding="ascii")
        nodes = self.topology_root / "nodes"
        cpu = nodes / "0"
        cpu.mkdir(parents=True)
        (cpu / "gpu_id").write_text("0\n", encoding="ascii")
        (cpu / "properties").write_text("cpu_cores_count 8\n", encoding="ascii")
        gpu = nodes / "2"
        gpu.mkdir()
        (gpu / "gpu_id").write_text("28851\n", encoding="ascii")
        (gpu / "properties").write_text(
            "domain 0\n"
            "location_id 1280\n"
            f"unique_id {int(self.unique_id, 16)}\n"
            "vendor_id 4098\n"
            "gfx_target_version 90402\n",
            encoding="ascii",
        )
        self.status_path.write_text(
            "Name:\ttest\nCpus_allowed_list:\t0-7,48-55\nMems_allowed_list:\t0-1\n",
            encoding="ascii",
        )

    def record(self) -> str:
        return GUARD.topology_record(
            gpu_index=0,
            pci_bdf=self.bdf,
            unique_id=self.unique_id,
            pci_root=self.pci_root,
            topology_root=self.topology_root,
            status_path=self.status_path,
        )


def fields(record: str) -> tuple[str, dict[str, str]]:
    prefix, *tokens = record.split()
    return prefix, dict(token.split("=", 1) for token in tokens)


def write_process(
    proc_root: pathlib.Path,
    pid: int,
    *,
    ppid: int,
    process_group: int,
    start_time: int,
) -> None:
    directory = proc_root / str(pid)
    directory.mkdir(parents=True)
    tail = [
        "S",
        str(ppid),
        str(process_group),
        "0",
        "0",
        "0",
        "0",
        "0",
        "0",
        "0",
        "0",
        "0",
        "0",
        "0",
        "0",
        "0",
        "0",
        "1",
        "0",
        str(start_time),
    ]
    (directory / "stat").write_text(
        f"{pid} (test process) {' '.join(tail)}\n", encoding="ascii"
    )


def write_queue(kfd_root: pathlib.Path, pid: int, queue: int, gpu_id: int) -> None:
    directory = kfd_root / str(pid) / "queues" / str(queue)
    directory.mkdir(parents=True)
    (directory / "gpuid").write_text(f"{gpu_id}\n", encoding="ascii")


class TopologyTests(unittest.TestCase):
    def test_exact_topology_produces_sealed_placement(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = TopologyFixture(pathlib.Path(temporary))
            record = fixture.record()
        prefix, observed = fields(record)
        self.assertEqual(prefix, "topology")
        self.assertEqual(observed["schema"], GUARD.TOPOLOGY_SCHEMA)
        self.assertEqual(
            observed["placement"],
            "taskset-cpulist-then-numactl-physcpubind-membind-v1",
        )
        self.assertEqual(observed["pci_bdf"], fixture.bdf)
        self.assertEqual(observed["unique_id"], fixture.unique_id)
        self.assertEqual(observed["measurement_cpu_list"], "0-7")
        self.assertEqual(observed["observer_cpu"], "48")
        self.assertEqual(observed["kfd_node"], "2")
        self.assertEqual(observed["kfd_gpu_id"], "28851")
        digest = observed.pop("topology_sha256")
        payload = GUARD._record("topology", observed) + "\n"
        self.assertEqual(digest, hashlib.sha256(payload.encode()).hexdigest())

    def test_reserves_one_local_cpu_when_no_nonlocal_cpu_is_allowed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = TopologyFixture(pathlib.Path(temporary))
            fixture.status_path.write_text(
                "Cpus_allowed_list:\t0-7\nMems_allowed_list:\t0\n",
                encoding="ascii",
            )
            _, observed = fields(fixture.record())
        self.assertEqual(observed["measurement_cpu_list"], "0-6")
        self.assertEqual(observed["observer_cpu"], "7")

    def test_rejects_identity_and_kfd_location_substitution(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = TopologyFixture(pathlib.Path(temporary))
            device = fixture.pci_root / fixture.bdf
            (device / "unique_id").write_text("0123456789abcdef\n", encoding="ascii")
            with self.assertRaisesRegex(GUARD.GuardError, "PCI unique ID"):
                fixture.record()

        with tempfile.TemporaryDirectory() as temporary:
            fixture = TopologyFixture(pathlib.Path(temporary))
            properties = fixture.topology_root / "nodes" / "2" / "properties"
            properties.write_text(
                properties.read_text(encoding="ascii").replace(
                    "location_id 1280", "location_id 9728"
                ),
                encoding="ascii",
            )
            with self.assertRaisesRegex(GUARD.GuardError, "one exact KFD node"):
                fixture.record()

    def test_rejects_disallowed_local_cpu_or_memory_node(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = TopologyFixture(pathlib.Path(temporary))
            fixture.status_path.write_text(
                "Cpus_allowed_list:\t48-55\nMems_allowed_list:\t0-1\n",
                encoding="ascii",
            )
            with self.assertRaisesRegex(GUARD.GuardError, "local CPU"):
                fixture.record()

        with tempfile.TemporaryDirectory() as temporary:
            fixture = TopologyFixture(pathlib.Path(temporary))
            fixture.status_path.write_text(
                "Cpus_allowed_list:\t0-7\nMems_allowed_list:\t1\n",
                encoding="ascii",
            )
            with self.assertRaisesRegex(GUARD.GuardError, "Mems_allowed_list"):
                fixture.record()


class QueueCensusTests(unittest.TestCase):
    def test_live_authentication_uses_captured_leaf_and_types_departure(self) -> None:
        root = GUARD.ProcessObservation(1, 100, 1000)
        child = GUARD.ProcessObservation(100, 100, 1001)
        for pid, observations in (
            (100, [root, None]),
            (101, [child, root, None]),
        ):
            with self.subTest(pid=pid), mock.patch.object(
                GUARD, "_read_process", side_effect=observations
            ) as read_process:
                authentication = GUARD._target_process_identity(
                    pathlib.Path("/proc"), pid, 100, 1000
                )
            self.assertEqual(authentication.identity, observations[0])
            self.assertTrue(authentication.departed)
            self.assertEqual(read_process.call_count, len(observations))

    def test_live_authentication_rejects_reuse_or_reparenting(self) -> None:
        root = GUARD.ProcessObservation(1, 100, 1000)
        child = GUARD.ProcessObservation(100, 100, 1001)
        changed = (
            [root, GUARD.ProcessObservation(1, 100, 2000)],
            [root, GUARD.ProcessObservation(1, 999, 1000)],
            [child, root, GUARD.ProcessObservation(1, 100, 1001)],
        )
        for observations in changed:
            with (
                self.subTest(observations=observations),
                mock.patch.object(GUARD, "_read_process", side_effect=observations),
                self.assertRaisesRegex(GUARD.GuardError, "identity changed"),
            ):
                GUARD._target_process_identity(
                    pathlib.Path("/proc"),
                    100 if observations[0] == root else 101,
                    100,
                    1000,
                )

    def test_live_authentication_preserves_stable_root_and_descendant(self) -> None:
        root = GUARD.ProcessObservation(1, 100, 1000)
        child = GUARD.ProcessObservation(100, 100, 1001)
        for pid, observations in (
            (100, [root, root]),
            (101, [child, root, child]),
        ):
            with self.subTest(pid=pid), mock.patch.object(
                GUARD, "_read_process", side_effect=observations
            ):
                authentication = GUARD._target_process_identity(
                    pathlib.Path("/proc"), pid, 100, 1000
                )
            self.assertEqual(authentication.identity, observations[0])
            self.assertFalse(authentication.departed)

    def test_live_authentication_does_not_authenticate_missing_ancestry(self) -> None:
        child = GUARD.ProcessObservation(100, 100, 1001)
        for observations in ([None], [child, None]):
            with self.subTest(observations=observations), mock.patch.object(
                GUARD, "_read_process", side_effect=observations
            ):
                authentication = GUARD._target_process_identity(
                    pathlib.Path("/proc"), 101, 100, 1000
                )
            self.assertIsNone(authentication.identity)
            self.assertFalse(authentication.departed)

    def test_esrch_is_absence_at_each_live_authentication_read_seam(self) -> None:
        with mock.patch.object(
            pathlib.Path,
            "read_text",
            side_effect=ProcessLookupError("process exited"),
        ):
            authentication = GUARD._target_process_identity(
                pathlib.Path("/proc"), 100, 100, 1000
            )
        self.assertIsNone(authentication.identity)
        self.assertFalse(authentication.departed)

        with tempfile.TemporaryDirectory() as temporary:
            proc_root = pathlib.Path(temporary) / "proc"
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            write_process(proc_root, 101, ppid=100, process_group=100, start_time=1001)
            root_stat = proc_root / "100" / "stat"
            original_read_text = pathlib.Path.read_text

            def ancestry_esrch(
                path: pathlib.Path, *args: object, **kwargs: object
            ) -> str:
                if path == root_stat:
                    raise ProcessLookupError("ancestor exited")
                return original_read_text(path, *args, **kwargs)

            with mock.patch.object(pathlib.Path, "read_text", new=ancestry_esrch):
                authentication = GUARD._target_process_identity(
                    proc_root, 101, 100, 1000
                )
            self.assertIsNone(authentication.identity)
            self.assertFalse(authentication.departed)

            root_reads = 0

            def closing_esrch(
                path: pathlib.Path, *args: object, **kwargs: object
            ) -> str:
                nonlocal root_reads
                if path == root_stat:
                    root_reads += 1
                    if root_reads == 2:
                        raise ProcessLookupError("target exited")
                return original_read_text(path, *args, **kwargs)

            with mock.patch.object(pathlib.Path, "read_text", new=closing_esrch):
                authentication = GUARD._target_process_identity(
                    proc_root, 100, 100, 1000
                )
            self.assertEqual(
                authentication.identity,
                GUARD.ProcessObservation(1, 100, 1000),
            )
            self.assertTrue(authentication.departed)

    def test_live_authentication_propagates_process_read_failure(self) -> None:
        for error in (PermissionError(13, "denied"), OSError(5, "I/O error")):
            with (
                self.subTest(error=error),
                mock.patch.object(pathlib.Path, "read_text", side_effect=error),
                self.assertRaisesRegex(GUARD.GuardError, "cannot read process identity"),
            ):
                GUARD._target_process_identity(
                    pathlib.Path("/proc"), 100, 100, 1000
                )

    def test_final_departure_confirmation_accepts_esrch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            process_path = root / "kfd-proc" / "100"
            process_path.mkdir(parents=True)
            process_path_identity = GUARD._directory_identity(
                process_path, "KFD process directory"
            )
            self.assertIsNotNone(process_path_identity)
            with mock.patch.object(
                pathlib.Path,
                "read_text",
                side_effect=ProcessLookupError("target exited"),
            ):
                GUARD._confirm_departed_target(
                    proc_root=root / "proc",
                    pid=100,
                    process_path=process_path,
                    process_path_identity=process_path_identity,
                )

    def test_kfd_esrch_is_not_typed_as_tolerable_disappearance(self) -> None:
        with (
            mock.patch.object(
                pathlib.Path,
                "read_bytes",
                side_effect=ProcessLookupError("KFD gpuid ESRCH"),
            ),
            self.assertRaises(GUARD.GuardError) as gpuid_failure,
        ):
            GUARD._read_text(pathlib.Path("/kfd/gpuid"), "KFD queue GPU ID")
        self.assertIsInstance(gpuid_failure.exception.__cause__, ProcessLookupError)
        self.assertFalse(GUARD._disappeared_during_census(gpuid_failure.exception))

        with (
            mock.patch.object(
                GUARD.os,
                "scandir",
                side_effect=ProcessLookupError("KFD queue enumeration ESRCH"),
            ),
            self.assertRaises(GUARD.GuardError) as enumeration_failure,
        ):
            GUARD._live_queue_directories(
                pathlib.Path("/kfd/queues"), "KFD queue directory", 1, True
            )
        self.assertIsInstance(
            enumeration_failure.exception.__cause__, ProcessLookupError
        )
        self.assertFalse(
            GUARD._disappeared_during_census(enumeration_failure.exception)
        )

    def test_initial_authentication_departure_is_reconfirmed_after_census(self) -> None:
        root_observation = GUARD.ProcessObservation(1, 100, 1000)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_queue(kfd_root, 100, 0, 28851)
            with mock.patch.object(
                GUARD,
                "_read_process",
                side_effect=[root_observation, None, None, None],
            ) as read_process:
                target, foreign = GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )
        self.assertEqual(target, [])
        self.assertEqual(foreign, [])
        self.assertEqual(read_process.call_count, 4)

    def test_initial_authentication_departure_rejects_pid_recreation(self) -> None:
        root_observation = GUARD.ProcessObservation(1, 100, 1000)
        replacement = GUARD.ProcessObservation(1, 100, 2000)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_queue(kfd_root, 100, 0, 28851)
            with (
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    side_effect=[root_observation, None, replacement],
                ),
                self.assertRaisesRegex(GUARD.GuardError, "identity changed"),
            ):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )

    def test_initial_departure_is_sticky_across_exact_identity_aba(self) -> None:
        root_observation = GUARD.ProcessObservation(1, 100, 1000)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_queue(kfd_root, 100, 0, 28851)
            with (
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    side_effect=[
                        root_observation,
                        None,
                        root_observation,
                        root_observation,
                    ],
                ),
                self.assertRaisesRegex(GUARD.GuardError, "identity changed"),
            ):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )

    def test_post_census_departure_is_reconfirmed(self) -> None:
        root_observation = GUARD.ProcessObservation(1, 100, 1000)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_queue(kfd_root, 100, 0, 28851)
            with mock.patch.object(
                GUARD,
                "_read_process",
                side_effect=[
                    root_observation,
                    root_observation,
                    root_observation,
                    None,
                    None,
                ],
            ):
                target, foreign = GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )
        self.assertEqual(target, [])
        self.assertEqual(foreign, [])

    def test_post_census_departure_rejects_recreation_before_decision(self) -> None:
        root_observation = GUARD.ProcessObservation(1, 100, 1000)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_queue(kfd_root, 100, 0, 28851)
            with (
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    side_effect=[
                        root_observation,
                        root_observation,
                        root_observation,
                        None,
                        root_observation,
                    ],
                ),
                self.assertRaisesRegex(GUARD.GuardError, "identity changed"),
            ):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )

    def test_foreign_owner_cannot_launder_through_departed_target_replacement(self) -> None:
        foreign_observation = GUARD.ProcessObservation(1, 200, 2000)
        init_observation = GUARD.ProcessObservation(0, 1, 1)
        replacement = GUARD.ProcessObservation(100, 100, 3000)
        root_observation = GUARD.ProcessObservation(1, 100, 1000)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_queue(kfd_root, 200, 0, 28851)
            with mock.patch.object(
                GUARD,
                "_read_process",
                side_effect=[
                    foreign_observation,
                    init_observation,
                    replacement,
                    root_observation,
                    None,
                ],
            ):
                target, foreign = GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )
        self.assertEqual(target, [])
        self.assertEqual(foreign, [(200, 0)])

    def test_departed_target_rejects_kfd_process_path_replacement(self) -> None:
        root_observation = GUARD.ProcessObservation(1, 100, 1000)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_queue(kfd_root, 100, 0, 28851)
            with (
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    side_effect=[root_observation, None, None, None],
                ),
                mock.patch.object(
                    GUARD,
                    "_directory_identity",
                    side_effect=[(1, 10, 0o40755), (1, 11, 0o40755)],
                ),
                self.assertRaisesRegex(GUARD.GuardError, "KFD process identity changed"),
            ):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )

    def test_whitelists_target_tree_and_rejects_only_foreign_selected_gpu(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            write_process(proc_root, 101, ppid=100, process_group=100, start_time=1001)
            write_process(proc_root, 200, ppid=1, process_group=200, start_time=2000)
            write_process(proc_root, 201, ppid=1, process_group=201, start_time=2001)
            write_queue(kfd_root, 100, 0, 28851)
            write_queue(kfd_root, 101, 1, 28851)
            write_queue(kfd_root, 200, 2, 28851)
            write_queue(kfd_root, 201, 3, 23018)
            foreign = GUARD.foreign_selected_gpu_queues(
                kfd_proc_root=kfd_root,
                proc_root=proc_root,
                selected_gpu_id=28851,
                root_pid=100,
                root_start_time=1000,
            )
            target, classified_foreign = GUARD.classify_selected_gpu_queues(
                kfd_proc_root=kfd_root,
                proc_root=proc_root,
                selected_gpu_id=28851,
                root_pid=100,
                root_start_time=1000,
            )
        self.assertEqual(foreign, [(200, 2)])
        self.assertEqual(target, [(100, 0), (101, 1)])
        self.assertEqual(classified_foreign, foreign)

    def test_pid_reuse_is_not_whitelisted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=2000)
            write_queue(kfd_root, 100, 0, 28851)
            foreign = GUARD.foreign_selected_gpu_queues(
                kfd_proc_root=kfd_root,
                proc_root=proc_root,
                selected_gpu_id=28851,
                root_pid=100,
                root_start_time=1000,
            )
        self.assertEqual(foreign, [(100, 0)])

    def test_missing_process_for_queue_owner_is_foreign(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            write_queue(kfd_root, 101, 0, 28851)
            target, foreign = GUARD.classify_selected_gpu_queues(
                kfd_proc_root=kfd_root,
                proc_root=proc_root,
                selected_gpu_id=28851,
                root_pid=100,
                root_start_time=1000,
            )
        self.assertEqual(target, [])
        self.assertEqual(foreign, [(101, 0)])

    def test_live_classifier_tolerates_confirmed_target_queue_disappearance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            write_queue(kfd_root, 100, 0, 28851)
            queues_path = kfd_root / "100" / "queues"
            queue_path = queues_path / "0"
            original = GUARD._live_queue_directories

            def enumerate_then_remove(
                path: pathlib.Path,
                description: str,
                maximum_entries: int,
                target_owned: bool,
            ) -> tuple[list[tuple[int, pathlib.Path]], bool]:
                entries = original(
                    path, description, maximum_entries, target_owned
                )
                if path == queues_path:
                    (queue_path / "gpuid").unlink()
                    queue_path.rmdir()
                return entries

            with mock.patch.object(
                GUARD, "_live_queue_directories", side_effect=enumerate_then_remove
            ):
                target, foreign = GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=proc_root,
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )
        self.assertEqual(target, [])
        self.assertEqual(foreign, [])

    def test_live_queue_enumerator_types_both_disappearance_shapes(self) -> None:
        class VanishedEntry:
            name = "0"

            def __init__(self, path: pathlib.Path, raises: bool) -> None:
                self.path = str(path)
                self.raises = raises

            def is_dir(self, *, follow_symlinks: bool) -> bool:
                self.test_follow_symlinks = follow_symlinks
                if self.raises:
                    raise FileNotFoundError(self.path)
                return False

        with tempfile.TemporaryDirectory() as temporary:
            queue_path = pathlib.Path(temporary) / "vanished" / "0"
            for raises in (True, False):
                with self.subTest(raises=raises):
                    entry = VanishedEntry(queue_path, raises)
                    with mock.patch.object(GUARD.os, "scandir", return_value=[entry]):
                        queues, vanished = GUARD._live_queue_directories(
                            queue_path.parent, "KFD queue directory", 1, True
                        )
                    self.assertEqual(queues, [])
                    self.assertTrue(vanished)
                    self.assertFalse(entry.test_follow_symlinks)

                    with (
                        mock.patch.object(GUARD.os, "scandir", return_value=[entry]),
                        self.assertRaises(GUARD.GuardError),
                    ):
                        GUARD._live_queue_directories(
                            queue_path.parent, "KFD queue directory", 1, False
                        )

    def test_live_classifier_rejects_process_disappearance_before_authentication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            queues_path = kfd_root / "100" / "queues"
            queues_path.mkdir(parents=True)
            process_path = queues_path.parent
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            original = GUARD._numeric_directories

            def enumerate_then_remove(
                path: pathlib.Path, description: str, maximum_entries: int
            ) -> list[tuple[int, pathlib.Path]]:
                entries = original(path, description, maximum_entries)
                if path == kfd_root:
                    queues_path.rmdir()
                    process_path.rmdir()
                return entries

            with (
                mock.patch.object(
                    GUARD, "_numeric_directories", side_effect=enumerate_then_remove
                ),
                self.assertRaisesRegex(
                    GUARD.GuardError, "disappeared before authentication"
                ),
            ):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=proc_root,
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )

    def test_vanished_foreign_kfd_owner_cannot_be_laundered_by_pid_reuse(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            write_process(proc_root, 200, ppid=1, process_group=200, start_time=2000)
            write_queue(kfd_root, 200, 0, 28851)
            queue_path = kfd_root / "200" / "queues" / "0"
            queues_path = queue_path.parent
            process_path = queues_path.parent
            original = GUARD._numeric_directories

            def enumerate_then_remove_and_reuse_pid(
                path: pathlib.Path, description: str, maximum_entries: int
            ) -> list[tuple[int, pathlib.Path]]:
                entries = original(path, description, maximum_entries)
                if path == kfd_root:
                    (queue_path / "gpuid").unlink()
                    queue_path.rmdir()
                    queues_path.rmdir()
                    process_path.rmdir()
                    (proc_root / "200" / "stat").unlink()
                    (proc_root / "200").rmdir()
                    write_process(
                        proc_root,
                        200,
                        ppid=100,
                        process_group=100,
                        start_time=3000,
                    )
                return entries

            with (
                mock.patch.object(
                    GUARD,
                    "_numeric_directories",
                    side_effect=enumerate_then_remove_and_reuse_pid,
                ),
                mock.patch.object(
                    GUARD,
                    "_target_process_identity",
                    wraps=GUARD._target_process_identity,
                ) as authenticate,
            ):
                with self.assertRaisesRegex(
                    GUARD.GuardError, "disappeared before authentication"
                ):
                    GUARD.classify_selected_gpu_queues(
                        kfd_proc_root=kfd_root,
                        proc_root=proc_root,
                        selected_gpu_id=28851,
                        root_pid=100,
                        root_start_time=1000,
                    )
            authenticate.assert_not_called()

    def test_strict_owner_census_rejects_queue_disappearance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_queue(kfd_root, 100, 0, 28851)
            queues_path = kfd_root / "100" / "queues"
            queue_path = queues_path / "0"
            original = GUARD._numeric_directories

            def enumerate_then_remove(
                path: pathlib.Path, description: str, maximum_entries: int
            ) -> list[tuple[int, pathlib.Path]]:
                entries = original(path, description, maximum_entries)
                if path == queues_path:
                    (queue_path / "gpuid").unlink()
                    queue_path.rmdir()
                return entries

            with (
                mock.patch.object(
                    GUARD, "_numeric_directories", side_effect=enumerate_then_remove
                ),
                self.assertRaisesRegex(GUARD.GuardError, "cannot read"),
            ):
                GUARD.selected_gpu_queue_owners(kfd_root, 28851)

    def test_live_classifier_rejects_foreign_queue_disappearance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            write_process(proc_root, 200, ppid=1, process_group=200, start_time=2000)
            write_queue(kfd_root, 200, 0, 28851)
            queues_path = kfd_root / "200" / "queues"
            queue_path = queues_path / "0"
            original = GUARD._live_queue_directories

            def enumerate_then_remove(
                path: pathlib.Path,
                description: str,
                maximum_entries: int,
                target_owned: bool,
            ) -> tuple[list[tuple[int, pathlib.Path]], bool]:
                entries = original(
                    path, description, maximum_entries, target_owned
                )
                if path == queues_path:
                    (queue_path / "gpuid").unlink()
                    queue_path.rmdir()
                return entries

            with (
                mock.patch.object(
                    GUARD, "_live_queue_directories", side_effect=enumerate_then_remove
                ),
                self.assertRaisesRegex(GUARD.GuardError, "cannot read"),
            ):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=proc_root,
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )

    def test_target_disappearance_preserves_foreign_queue_in_either_order(self) -> None:
        for target_pid, foreign_pid in ((200, 100), (100, 200)):
            with self.subTest(target_pid=target_pid, foreign_pid=foreign_pid):
                with tempfile.TemporaryDirectory() as temporary:
                    root = pathlib.Path(temporary)
                    proc_root = root / "proc"
                    kfd_root = root / "kfd-proc"
                    kfd_root.mkdir()
                    write_process(
                        proc_root, target_pid, ppid=1,
                        process_group=target_pid, start_time=target_pid * 10,
                    )
                    write_process(
                        proc_root, foreign_pid, ppid=1,
                        process_group=foreign_pid, start_time=foreign_pid * 10,
                    )
                    write_queue(kfd_root, target_pid, 1, 28851)
                    write_queue(kfd_root, foreign_pid, 0, 28851)
                    queues_path = kfd_root / str(target_pid) / "queues"
                    queue_path = queues_path / "1"
                    original = GUARD._live_queue_directories

                    def enumerate_then_remove(
                        path: pathlib.Path,
                        description: str,
                        maximum_entries: int,
                        target_owned: bool,
                    ) -> tuple[list[tuple[int, pathlib.Path]], bool]:
                        entries = original(
                            path, description, maximum_entries, target_owned
                        )
                        if path == queues_path:
                            (queue_path / "gpuid").unlink()
                            queue_path.rmdir()
                        return entries

                    with mock.patch.object(
                        GUARD,
                        "_live_queue_directories",
                        side_effect=enumerate_then_remove,
                    ):
                        target, foreign = GUARD.classify_selected_gpu_queues(
                            kfd_proc_root=kfd_root,
                            proc_root=proc_root,
                            selected_gpu_id=28851,
                            root_pid=target_pid,
                            root_start_time=target_pid * 10,
                        )
                self.assertEqual(target, [])
                self.assertEqual(foreign, [(foreign_pid, 0)])

    def test_selected_queue_requires_post_read_target_reauthentication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            write_queue(kfd_root, 100, 0, 28851)
            with mock.patch.object(
                GUARD, "_is_target_process", side_effect=[True, False]
            ):
                target, foreign = GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=proc_root,
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )
        self.assertEqual(target, [])
        self.assertEqual(foreign, [(100, 0)])

    def test_disappearance_requires_post_read_target_reauthentication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            write_queue(kfd_root, 100, 0, 28851)
            queues_path = kfd_root / "100" / "queues"
            queue_path = queues_path / "0"
            original = GUARD._live_queue_directories

            def enumerate_then_remove(
                path: pathlib.Path,
                description: str,
                maximum_entries: int,
                target_owned: bool,
            ) -> tuple[list[tuple[int, pathlib.Path]], bool]:
                entries = original(
                    path, description, maximum_entries, target_owned
                )
                if path == queues_path:
                    (queue_path / "gpuid").unlink()
                    queue_path.rmdir()
                return entries

            with (
                mock.patch.object(
                    GUARD, "_live_queue_directories", side_effect=enumerate_then_remove
                ),
                mock.patch.object(
                    GUARD, "_is_target_process", side_effect=[True, False]
                ),
                self.assertRaisesRegex(GUARD.GuardError, "identity changed"),
            ):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=proc_root,
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )

    def test_persistently_missing_target_gpuid_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            (kfd_root / "100" / "queues" / "0").mkdir(parents=True)
            with self.assertRaisesRegex(GUARD.GuardError, "cannot read"):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=proc_root,
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )

    def test_unreadable_or_malformed_gpuid_fails_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            write_process(proc_root, 100, ppid=1, process_group=100, start_time=1000)
            write_queue(kfd_root, 100, 0, 28851)
            gpu_id_path = kfd_root / "100" / "queues" / "0" / "gpuid"
            original = GUARD._read_text
            reads = 0

            def unreadable(path: pathlib.Path, description: str) -> str:
                nonlocal reads
                if path == gpu_id_path:
                    reads += 1
                    try:
                        raise PermissionError("denied")
                    except PermissionError as error:
                        raise GUARD.GuardError("cannot read KFD queue GPU ID") from error
                return original(path, description)

            with (
                mock.patch.object(GUARD, "_read_text", side_effect=unreadable),
                self.assertRaisesRegex(GUARD.GuardError, "cannot read"),
            ):
                GUARD.selected_gpu_queue_owners(kfd_root, 28851)
            with (
                mock.patch.object(GUARD, "_read_text", side_effect=unreadable),
                self.assertRaisesRegex(GUARD.GuardError, "cannot read"),
            ):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=proc_root,
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )
            self.assertEqual(reads, 2)

            gpu_id_path.write_text("028851\n", encoding="ascii")
            with self.assertRaisesRegex(GUARD.GuardError, "canonical decimal"):
                GUARD.selected_gpu_queue_owners(kfd_root, 28851)
            with self.assertRaisesRegex(GUARD.GuardError, "canonical decimal"):
                GUARD.classify_selected_gpu_queues(
                    kfd_proc_root=kfd_root,
                    proc_root=proc_root,
                    selected_gpu_id=28851,
                    root_pid=100,
                    root_start_time=1000,
                )

    def test_cadence_fails_closed_on_large_or_nonmonotonic_gap(self) -> None:
        self.assertEqual(GUARD.MAX_OBSERVATION_GAP_NS, 10_000_000)
        cadence = GUARD.ObservationCadence(GUARD.MAX_OBSERVATION_GAP_NS)
        cadence.observe(100)
        cadence.observe(2_000_100)
        with self.assertRaisesRegex(
            GUARD.GuardError,
            r"observation 3 gap exceeded: observed_ns=10000001 maximum_ns=10000000",
        ):
            cadence.observe(12_000_101)
        nonmonotonic = GUARD.ObservationCadence(10)
        nonmonotonic.observe(1)
        with self.assertRaisesRegex(GUARD.GuardError, "did not advance"):
            nonmonotonic.observe(1)

    def test_absolute_deadline_wait_does_not_reset_when_overdue(self) -> None:
        now_ns = 1_000_000
        sleeps: list[float] = []

        def clock() -> int:
            return now_ns

        def sleeper(seconds: float) -> None:
            nonlocal now_ns
            sleeps.append(seconds)
            now_ns += int(seconds * 1_000_000_000)

        GUARD._sleep_until_observation_deadline(
            deadline_ns=3_000_000, clock=clock, sleeper=sleeper
        )
        self.assertEqual(now_ns, 3_000_000)
        self.assertEqual(sleeps, [0.002])

        now_ns = 7_000_000
        GUARD._sleep_until_observation_deadline(
            deadline_ns=5_000_000, clock=clock, sleeper=sleeper
        )
        self.assertEqual(now_ns, 7_000_000)
        self.assertEqual(sleeps, [0.002])

    def test_cleanup_signals_group_even_after_leader_was_reaped(self) -> None:
        class ReapedProcess:
            pid = 100

            @staticmethod
            def poll() -> int:
                return 0

        process = ReapedProcess()
        with (
            mock.patch.object(GUARD.os, "killpg") as killpg,
            mock.patch.object(GUARD, "_process_group_exists", return_value=True),
            mock.patch.object(
                GUARD, "_wait_for_process_group_absence", return_value=True
            ) as wait_for_absence,
        ):
            GUARD._terminate_process_group(process)
        killpg.assert_called_once_with(100, signal.SIGTERM)
        wait_for_absence.assert_called_once_with(
            process, GUARD.PROCESS_GROUP_GRACE_SECONDS
        )

    def test_cleanup_escalates_and_rejects_a_surviving_process_group(self) -> None:
        class ReapedProcess:
            pid = 100

            @staticmethod
            def poll() -> int:
                return 0

        process = ReapedProcess()
        with (
            mock.patch.object(GUARD.os, "killpg") as killpg,
            mock.patch.object(GUARD, "_process_group_exists", return_value=True),
            mock.patch.object(
                GUARD,
                "_wait_for_process_group_absence",
                side_effect=[False, False],
            ),
            self.assertRaisesRegex(GUARD.GuardError, "survived SIGKILL"),
        ):
            GUARD._terminate_process_group(process)
        self.assertEqual(
            killpg.call_args_list,
            [mock.call(100, signal.SIGTERM), mock.call(100, signal.SIGKILL)],
        )

    def test_cleanup_removes_same_group_child_after_leader_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            child_pid_path = pathlib.Path(temporary) / "child.pid"
            program = """
import os
import pathlib
import signal
import sys
import time

child = os.fork()
if child == 0:
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    time.sleep(30)
    os._exit(0)
pathlib.Path(sys.argv[1]).write_text(str(child), encoding="ascii")
time.sleep(0.05)
os._exit(0)
"""
            process = subprocess.Popen(
                [sys.executable, "-c", program, str(child_pid_path)],
                start_new_session=True,
            )
            try:
                process.wait(timeout=2)
                self.assertTrue(child_pid_path.exists())
                self.assertTrue(GUARD._process_group_exists(process.pid))
                with mock.patch.object(GUARD, "PROCESS_GROUP_GRACE_SECONDS", 0.05):
                    GUARD._terminate_process_group(process)
                self.assertFalse(GUARD._process_group_exists(process.pid))
            finally:
                if GUARD._process_group_exists(process.pid):
                    os.killpg(process.pid, signal.SIGKILL)


class MonitorCommandTests(unittest.TestCase):
    def monitor_argv(
        self,
        *,
        kfd_root: pathlib.Path,
        output: pathlib.Path,
        command: list[str],
    ) -> list[str]:
        observer_cpu = min(os.sched_getaffinity(0))
        return [
            sys.executable,
            str(GUARD_PATH),
            "monitor",
            "--gpu-id",
            "28851",
            "--observer-cpu",
            str(observer_cpu),
            "--target-output",
            str(output),
            "--kfd-proc-root",
            str(kfd_root),
            "--",
            *command,
        ]

    def run_monitor(
        self, temporary: pathlib.Path, command: list[str], *, foreign: bool = False
    ) -> subprocess.CompletedProcess[str]:
        kfd_root = temporary / "kfd-proc"
        kfd_root.mkdir()
        if foreign:
            write_queue(kfd_root, 1, 0, 28851)
        output = temporary / "target.out"
        return subprocess.run(
            self.monitor_argv(kfd_root=kfd_root, output=output, command=command),
            check=False,
            capture_output=True,
            text=True,
        )

    def deterministic_monitor_argv(
        self,
        *,
        kfd_root: pathlib.Path,
        output: pathlib.Path,
        command: list[str],
    ) -> list[str]:
        observer_cpu = min(os.sched_getaffinity(0))
        program = """
import importlib.util
import pathlib
import signal
import sys

spec = importlib.util.spec_from_file_location("fe2o3_r26_guard_child", sys.argv[1])
guard = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = guard
spec.loader.exec_module(guard)
guard.PROCESS_GROUP_GRACE_SECONDS = 0.1
now_ns = 0

def clock():
    global now_ns
    now_ns += guard.POLL_INTERVAL_NS
    return now_ns

try:
    for signum in guard.MANAGED_SIGNALS:
        signal.signal(signum, guard._raise_signal_error)
    print(guard.monitor_target(
        selected_gpu_id=28851,
        observer_cpu=int(sys.argv[4]),
        target_output=pathlib.Path(sys.argv[3]),
        command=sys.argv[5:],
        kfd_proc_root=pathlib.Path(sys.argv[2]),
        proc_root=pathlib.Path("/proc"),
        clock=clock,
    ))
except guard.GuardError as error:
    print(f"r26-host-guard: {error}", file=sys.stderr)
    raise SystemExit(2)
"""
        return [
            sys.executable,
            "-c",
            program,
            str(GUARD_PATH),
            str(kfd_root),
            str(output),
            str(observer_cpu),
            *command,
        ]

    def monitor_direct(
        self,
        temporary: pathlib.Path,
        command: list[str],
        *,
        target_observed: bool = True,
    ) -> str:
        kfd_root = temporary / "kfd-proc"
        kfd_root.mkdir()
        observer_cpu = min(os.sched_getaffinity(0))
        now_ns = 0

        def deterministic_clock() -> int:
            nonlocal now_ns
            now_ns += GUARD.POLL_INTERVAL_NS
            return now_ns

        with ExitStack() as stack:
            stack.enter_context(mock.patch.object(GUARD.os, "sched_setaffinity"))
            stack.enter_context(
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                )
            )
            if target_observed:
                stack.enter_context(
                    mock.patch.object(
                        GUARD,
                        "classify_selected_gpu_queues",
                        return_value=([(123, 0)], []),
                    )
                )
            return GUARD.monitor_target(
                selected_gpu_id=28851,
                observer_cpu=observer_cpu,
                target_output=temporary / "target.out",
                command=command,
                kfd_proc_root=kfd_root,
                proc_root=pathlib.Path("/proc"),
                clock=deterministic_clock,
            )

    def test_monitor_buffers_target_output_until_clean_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            result = self.monitor_direct(
                root,
                [
                    sys.executable,
                    "-c",
                    "import time; time.sleep(0.03); print('backend=kfd')",
                ],
            )
            target_output = root / "target.out"
            retained_output = target_output.read_text(encoding="ascii")
        prefix, observed = fields(result)
        self.assertEqual(prefix, "monitor")
        self.assertEqual(observed["schema"], "fe2o3.r26-kfd-queue-monitor.v2")
        self.assertEqual(observed["status"], "clean")
        self.assertEqual(observed["monitor"], "selected-kfd-gpu-process-tree-census-v2")
        self.assertEqual(observed["schedule"], "absolute-monotonic-raw-deadline-v1")
        self.assertEqual(observed["process_group"], observed["root_pid"])
        self.assertEqual(observed["target_reaped"], "1")
        self.assertEqual(observed["process_group_absent"], "1")
        self.assertEqual(observed["terminal_selected_queues"], "0")
        self.assertEqual(observed["foreign_selected_queues"], "0")
        self.assertGreater(int(observed["target_selected_queue_observations"]), 0)
        self.assertGreaterEqual(int(observed["observations"]), 2)
        self.assertEqual(retained_output, "backend=kfd\n")
        self.assertEqual(
            observed["target_output_sha256"],
            hashlib.sha256(retained_output.encode()).hexdigest(),
        )

    def test_observer_is_pinned_before_the_prelaunch_census(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            order: list[str] = []
            observer_cpu = min(os.sched_getaffinity(0))

            def set_affinity(_pid: int, cpus: set[int]) -> None:
                self.assertEqual(cpus, {observer_cpu})
                order.append("set-affinity")

            def get_affinity(_pid: int) -> set[int]:
                order.append("get-affinity")
                return {observer_cpu}

            def initial_census(
                _root: pathlib.Path, _gpu_id: int
            ) -> list[tuple[int, int]]:
                order.append("initial-census")
                raise GUARD.GuardError("stop after initial census")

            with (
                mock.patch.object(GUARD.os, "sched_setaffinity", set_affinity),
                mock.patch.object(GUARD.os, "sched_getaffinity", get_affinity),
                mock.patch.object(
                    GUARD, "selected_gpu_queue_owners", side_effect=initial_census
                ),
                self.assertRaisesRegex(GUARD.GuardError, "stop after initial census"),
            ):
                GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=root / "target.out",
                    command=["fixed-target"],
                    kfd_proc_root=root / "kfd-proc",
                    proc_root=root / "proc",
                )
        self.assertEqual(order, ["set-affinity", "get-affinity", "initial-census"])

    def test_preopen_race_does_not_delete_unowned_target_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            observer_cpu = min(os.sched_getaffinity(0))

            def initial_census(
                _root: pathlib.Path, _gpu_id: int
            ) -> list[tuple[int, int]]:
                output.write_text("unowned\n", encoding="ascii")
                return []

            with (
                mock.patch.object(GUARD.os, "sched_setaffinity"),
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                ),
                mock.patch.object(
                    GUARD, "selected_gpu_queue_owners", side_effect=initial_census
                ),
                self.assertRaisesRegex(
                    GUARD.GuardError, "cannot create private target stdout"
                ),
            ):
                GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=output,
                    command=["fixed-target"],
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                )
            self.assertEqual(output.read_text(encoding="ascii"), "unowned\n")

    def test_launch_delay_remains_part_of_the_census_gap(self) -> None:
        class FakeProcess:
            pid = 100

            @staticmethod
            def poll() -> int:
                return 0

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            now_ns = 0
            process = FakeProcess()

            def launch(
                _command: list[str],
                *,
                stdout: object,
                start_new_session: bool,
            ) -> FakeProcess:
                nonlocal now_ns
                self.assertTrue(start_new_session)
                now_ns += GUARD.MAX_OBSERVATION_GAP_NS + 1
                stdout.write(b"backend=kfd\n")
                stdout.flush()
                return process

            observer_cpu = min(os.sched_getaffinity(0))
            with (
                mock.patch.object(GUARD.subprocess, "Popen", side_effect=launch),
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    return_value=GUARD.ProcessObservation(1, 100, 1000),
                ),
                mock.patch.object(
                    GUARD,
                    "classify_selected_gpu_queues",
                    return_value=([(100, 0)], []),
                ),
                mock.patch.object(GUARD.os, "sched_setaffinity"),
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                ),
                mock.patch.object(GUARD, "_terminate_process_group"),
                self.assertRaisesRegex(
                    GUARD.GuardError,
                    r"observation 2 gap exceeded: observed_ns=10000001 maximum_ns=10000000",
                ),
            ):
                GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=output,
                    command=["fixed-target"],
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    clock=lambda: now_ns,
                    sleeper=lambda _seconds: None,
                )
            self.assertFalse(output.exists())

    def test_target_process_group_must_match_its_root_pid(self) -> None:
        class FakeProcess:
            pid = 100

            @staticmethod
            def poll() -> int | None:
                return None

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            observer_cpu = min(os.sched_getaffinity(0))

            def launch(
                _command: list[str],
                *,
                stdout: object,
                start_new_session: bool,
            ) -> FakeProcess:
                self.assertTrue(start_new_session)
                stdout.write(b"backend=kfd\n")
                stdout.flush()
                return FakeProcess()

            with (
                mock.patch.object(GUARD.subprocess, "Popen", side_effect=launch),
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    return_value=GUARD.ProcessObservation(1, 99, 1000),
                ),
                mock.patch.object(GUARD.os, "sched_setaffinity"),
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                ),
                mock.patch.object(GUARD, "_terminate_process_group"),
                self.assertRaisesRegex(
                    GUARD.GuardError, "did not establish its dedicated process group"
                ),
            ):
                GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=output,
                    command=["fixed-target"],
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                )
            self.assertFalse(output.exists())

    def test_live_censuses_follow_absolute_deadlines_after_launch_probe(self) -> None:
        class FakeProcess:
            pid = 100

            def __init__(self) -> None:
                self.polls = 0

            def poll(self) -> int | None:
                self.polls += 1
                return None if self.polls < 3 else 0

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            observer_cpu = min(os.sched_getaffinity(0))
            now_ns = 0
            owner_calls = 0
            sleeps: list[int] = []

            def launch(
                _command: list[str],
                *,
                stdout: object,
                start_new_session: bool,
            ) -> FakeProcess:
                self.assertTrue(start_new_session)
                stdout.write(b"backend=kfd\n")
                stdout.flush()
                return FakeProcess()

            def classify(
                **_arguments: object,
            ) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
                nonlocal now_ns
                now_ns += 100_000
                return [(100, 0)], []

            def owners(_root: pathlib.Path, _gpu_id: int) -> list[tuple[int, int]]:
                nonlocal now_ns, owner_calls
                owner_calls += 1
                if owner_calls == 2:
                    now_ns += 100_000
                return []

            def sleeper(seconds: float) -> None:
                nonlocal now_ns
                duration_ns = int(seconds * 1_000_000_000)
                sleeps.append(duration_ns)
                now_ns += duration_ns

            with (
                mock.patch.object(GUARD.subprocess, "Popen", side_effect=launch),
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    return_value=GUARD.ProcessObservation(1, 100, 1000),
                ),
                mock.patch.object(
                    GUARD, "classify_selected_gpu_queues", side_effect=classify
                ),
                mock.patch.object(
                    GUARD, "selected_gpu_queue_owners", side_effect=owners
                ),
                mock.patch.object(GUARD, "_process_group_exists", return_value=False),
                mock.patch.object(GUARD.os, "sched_setaffinity"),
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                ),
            ):
                GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=output,
                    command=["fixed-target"],
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    clock=lambda: now_ns,
                    sleeper=sleeper,
                )
        self.assertEqual(sleeps, [1_900_000, 1_900_000])

    def test_terminal_selected_gpu_queue_rejects_success(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            observer_cpu = min(os.sched_getaffinity(0))
            now_ns = 0

            def clock() -> int:
                nonlocal now_ns
                now_ns += GUARD.POLL_INTERVAL_NS
                return now_ns

            with (
                mock.patch.object(GUARD.os, "sched_setaffinity"),
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                ),
                mock.patch.object(
                    GUARD,
                    "classify_selected_gpu_queues",
                    return_value=([(123, 0)], []),
                ),
                mock.patch.object(
                    GUARD,
                    "selected_gpu_queue_owners",
                    side_effect=[[], [(999, 7)]],
                ),
                self.assertRaisesRegex(
                    GUARD.GuardError, "selected-GPU queue remains after target exit"
                ),
            ):
                GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=output,
                    command=[
                        sys.executable,
                        "-c",
                        "import time; time.sleep(0.02); print('backend=kfd')",
                    ],
                    kfd_proc_root=kfd_root,
                    proc_root=pathlib.Path("/proc"),
                    clock=clock,
                    sleeper=lambda _seconds: None,
                )
            self.assertFalse(output.exists())

    def test_surviving_process_group_rejects_success_and_is_cleaned(self) -> None:
        class FakeProcess:
            pid = 100

            @staticmethod
            def poll() -> int:
                return 0

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            observer_cpu = min(os.sched_getaffinity(0))
            now_ns = 0

            def launch(
                _command: list[str],
                *,
                stdout: object,
                start_new_session: bool,
            ) -> FakeProcess:
                self.assertTrue(start_new_session)
                stdout.write(b"backend=kfd\n")
                stdout.flush()
                return FakeProcess()

            def classify(
                **_arguments: object,
            ) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
                nonlocal now_ns
                now_ns += GUARD.POLL_INTERVAL_NS
                return [(100, 0)], []

            with (
                mock.patch.object(GUARD.subprocess, "Popen", side_effect=launch),
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    return_value=GUARD.ProcessObservation(1, 100, 1000),
                ),
                mock.patch.object(
                    GUARD, "classify_selected_gpu_queues", side_effect=classify
                ),
                mock.patch.object(GUARD, "_process_group_exists", return_value=True),
                mock.patch.object(GUARD, "_terminate_process_group") as cleanup,
                mock.patch.object(GUARD.os, "sched_setaffinity"),
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                ),
                self.assertRaisesRegex(
                    GUARD.GuardError, "target process group remains after leader exit"
                ),
            ):
                GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=output,
                    command=["fixed-target"],
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    clock=lambda: now_ns,
                    sleeper=lambda _seconds: None,
                )
            cleanup.assert_called_once()
            self.assertFalse(output.exists())

    def test_terminal_queue_census_duration_participates_in_cadence(self) -> None:
        class FakeProcess:
            pid = 100

            @staticmethod
            def poll() -> int:
                return 0

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            observer_cpu = min(os.sched_getaffinity(0))
            now_ns = 0
            owner_calls = 0

            def launch(
                _command: list[str],
                *,
                stdout: object,
                start_new_session: bool,
            ) -> FakeProcess:
                self.assertTrue(start_new_session)
                stdout.write(b"backend=kfd\n")
                stdout.flush()
                return FakeProcess()

            def classify(
                **_arguments: object,
            ) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
                nonlocal now_ns
                now_ns += GUARD.POLL_INTERVAL_NS
                return [(100, 0)], []

            def owners(_root: pathlib.Path, _gpu_id: int) -> list[tuple[int, int]]:
                nonlocal now_ns, owner_calls
                owner_calls += 1
                if owner_calls == 2:
                    now_ns += GUARD.MAX_OBSERVATION_GAP_NS + 1
                return []

            with (
                mock.patch.object(GUARD.subprocess, "Popen", side_effect=launch),
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    return_value=GUARD.ProcessObservation(1, 100, 1000),
                ),
                mock.patch.object(
                    GUARD, "classify_selected_gpu_queues", side_effect=classify
                ),
                mock.patch.object(
                    GUARD, "selected_gpu_queue_owners", side_effect=owners
                ),
                mock.patch.object(GUARD, "_process_group_exists", return_value=False),
                mock.patch.object(GUARD, "_terminate_process_group"),
                mock.patch.object(GUARD.os, "sched_setaffinity"),
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                ),
                self.assertRaisesRegex(GUARD.GuardError, "gap exceeded"),
            ):
                GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=output,
                    command=["fixed-target"],
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    clock=lambda: now_ns,
                    sleeper=lambda _seconds: None,
                )
            self.assertFalse(output.exists())

    def test_empty_queue_view_cannot_certify_success(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            with self.assertRaisesRegex(GUARD.GuardError, "no target-owned"):
                self.monitor_direct(
                    root,
                    [
                        sys.executable,
                        "-c",
                        "import time; time.sleep(0.02); print('backend=kfd')",
                    ],
                    target_observed=False,
                )
            self.assertFalse((root / "target.out").exists())

    def test_monitor_accepts_an_ancestry_owned_child_queue(self) -> None:
        class FakeProcess:
            pid = 100

            def __init__(self) -> None:
                self.polls = 0

            def poll(self) -> int | None:
                self.polls += 1
                return None if self.polls == 1 else 0

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            proc_root = root / "proc"
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            now_ns = 0
            process = FakeProcess()

            def launch(
                _command: list[str],
                *,
                stdout: object,
                start_new_session: bool,
            ) -> FakeProcess:
                self.assertTrue(start_new_session)
                write_process(
                    proc_root, 100, ppid=1, process_group=100, start_time=1000
                )
                write_process(
                    proc_root, 101, ppid=100, process_group=100, start_time=1001
                )
                write_queue(kfd_root, 101, 0, 28851)
                stdout.write(b"backend=kfd\n")
                stdout.flush()
                return process

            def clock() -> int:
                nonlocal now_ns
                now_ns += GUARD.POLL_INTERVAL_NS
                return now_ns

            observer_cpu = min(os.sched_getaffinity(0))
            with (
                mock.patch.object(GUARD.subprocess, "Popen", side_effect=launch),
                mock.patch.object(GUARD.os, "sched_setaffinity"),
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                ),
                mock.patch.object(
                    GUARD,
                    "selected_gpu_queue_owners",
                    side_effect=[[], []],
                ),
                mock.patch.object(GUARD, "_process_group_exists", return_value=False),
            ):
                record = GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=output,
                    command=["fixed-target"],
                    kfd_proc_root=kfd_root,
                    proc_root=proc_root,
                    clock=clock,
                    sleeper=lambda _seconds: None,
                )
        _, observed = fields(record)
        self.assertEqual(observed["target_selected_queue_observations"], "2")

    def test_final_census_duration_participates_in_cadence(self) -> None:
        class FakeProcess:
            pid = 100

            def __init__(self) -> None:
                self.polls = 0

            def poll(self) -> int | None:
                self.polls += 1
                return None if self.polls == 1 else 0

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            now_ns = 0
            census = 0
            process = FakeProcess()

            def launch(
                _command: list[str],
                *,
                stdout: object,
                start_new_session: bool,
            ) -> FakeProcess:
                self.assertTrue(start_new_session)
                stdout.write(b"backend=kfd\n")
                stdout.flush()
                return process

            def classify(
                **_arguments: object,
            ) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
                nonlocal census, now_ns
                census += 1
                now_ns += (
                    GUARD.POLL_INTERVAL_NS
                    if census == 1
                    else GUARD.MAX_OBSERVATION_GAP_NS + 1
                )
                return [(100, 0)], []

            def sleep_until_deadline(seconds: float) -> None:
                nonlocal now_ns
                now_ns += int(seconds * 1_000_000_000)

            observer_cpu = min(os.sched_getaffinity(0))
            with (
                mock.patch.object(GUARD.subprocess, "Popen", side_effect=launch),
                mock.patch.object(
                    GUARD,
                    "_read_process",
                    return_value=GUARD.ProcessObservation(1, 100, 1000),
                ),
                mock.patch.object(
                    GUARD, "classify_selected_gpu_queues", side_effect=classify
                ),
                mock.patch.object(GUARD.os, "sched_setaffinity"),
                mock.patch.object(
                    GUARD.os, "sched_getaffinity", return_value={observer_cpu}
                ),
                mock.patch.object(GUARD, "_terminate_process_group"),
                self.assertRaisesRegex(GUARD.GuardError, "gap exceeded"),
            ):
                GUARD.monitor_target(
                    selected_gpu_id=28851,
                    observer_cpu=observer_cpu,
                    target_output=output,
                    command=["fixed-target"],
                    kfd_proc_root=kfd_root,
                    proc_root=root / "proc",
                    clock=lambda: now_ns,
                    sleeper=sleep_until_deadline,
                )
            self.assertEqual(census, 2)
            self.assertFalse(output.exists())

    def test_foreign_queue_fails_without_releasable_target_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            result = self.run_monitor(
                root,
                [sys.executable, "-c", "import time; time.sleep(2); print('bad')"],
                foreign=True,
            )
            output_exists = (root / "target.out").exists()
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")
        self.assertIn("selected-GPU queue exists before target launch", result.stderr)
        self.assertFalse(output_exists)

    def test_nonzero_target_fails_without_releasable_target_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            with self.assertRaisesRegex(GUARD.GuardError, "status 3"):
                self.monitor_direct(
                    root,
                    [
                        sys.executable,
                        "-c",
                        "import time; time.sleep(0.02); print('bad'); raise SystemExit(3)",
                    ],
                )
            output_exists = (root / "target.out").exists()
        self.assertFalse(output_exists)

    def test_termination_cleans_target_group_and_buffered_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            kfd_root = root / "kfd-proc"
            kfd_root.mkdir()
            output = root / "target.out"
            pid_file = root / "target.pid"
            command = [
                sys.executable,
                "-c",
                (
                    "import os,pathlib,signal,time; "
                    f"pathlib.Path({str(pid_file)!r}).write_text(str(os.getpid())); "
                    "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                    "time.sleep(10)"
                ),
            ]
            monitor = subprocess.Popen(
                self.deterministic_monitor_argv(
                    kfd_root=kfd_root, output=output, command=command
                ),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            deadline = time.monotonic() + 2
            while not pid_file.exists() and time.monotonic() < deadline:
                time.sleep(0.005)
            self.assertTrue(pid_file.exists())
            target_pid = int(pid_file.read_text(encoding="ascii"))
            monitor.terminate()
            time.sleep(0.02)
            monitor.terminate()
            stdout, stderr = monitor.communicate(timeout=7)
            target_exists = pathlib.Path(f"/proc/{target_pid}").exists()
            output_exists = output.exists()
        self.assertEqual(monitor.returncode, 2)
        self.assertEqual(stdout, "")
        self.assertIn("interrupted by signal", stderr)
        self.assertFalse(output_exists)
        self.assertFalse(target_exists)


if __name__ == "__main__":
    unittest.main()
