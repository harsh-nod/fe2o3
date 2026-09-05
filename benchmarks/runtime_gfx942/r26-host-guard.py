#!/usr/bin/env python3

"""Fail-closed host-placement and interference evidence for the R26 runner."""

from __future__ import annotations

import argparse
import hashlib
import os
import pathlib
import re
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Callable, Sequence


TOPOLOGY_SCHEMA = "fe2o3.r26-host-topology.v1"
MONITOR_SCHEMA = "fe2o3.r26-kfd-queue-monitor.v2"
POLL_INTERVAL_NS = 2_000_000
MAX_OBSERVATION_GAP_NS = 10_000_000
PROCESS_GROUP_GRACE_SECONDS = 5.0
PROCESS_GROUP_POLL_SECONDS = 0.01
MAX_TEXT_BYTES = 4096
MAX_TOPOLOGY_NODES = 256
MAX_KFD_PROCESSES = 65_536
MAX_QUEUES_PER_PROCESS = 4096
MAX_ID = 1_048_575
MAX_ID_SET_CARDINALITY = 65_536
MAX_TARGET_OUTPUT_BYTES = 1 << 20
EXPECTED_AMD_VENDOR = 0x1002
EXPECTED_GFX_TARGET_VERSION = 90_402
MANAGED_SIGNALS = (signal.SIGHUP, signal.SIGINT, signal.SIGQUIT, signal.SIGTERM)

BDF_PATTERN = re.compile(
    r"(?P<domain>[0-9a-f]{4}):(?P<bus>[0-9a-f]{2}):"
    r"(?P<device>[01][0-9a-f])\.(?P<function>[0-7])"
)
UNIQUE_ID_PATTERN = re.compile(r"(?:0x)?([0-9a-f]{16})")
DECIMAL_PATTERN = re.compile(r"0|[1-9][0-9]*")
HEX_PATTERN = re.compile(r"0x[0-9a-f]+")


class GuardError(Exception):
    pass


@dataclass(frozen=True)
class ProcessObservation:
    ppid: int
    process_group: int
    start_time: int


@dataclass
class ObservationCadence:
    maximum_gap_ns: int
    previous_ns: int | None = None
    observed_maximum_gap_ns: int = 0
    observations: int = 0

    def observe(self, now_ns: int) -> None:
        if self.previous_ns is not None:
            gap = now_ns - self.previous_ns
            if gap <= 0:
                raise GuardError("queue census clock did not advance")
            self.observed_maximum_gap_ns = max(self.observed_maximum_gap_ns, gap)
            if gap > self.maximum_gap_ns:
                raise GuardError(
                    f"queue census observation {self.observations + 1} gap exceeded: "
                    f"observed_ns={gap} maximum_ns={self.maximum_gap_ns}"
                )
        self.previous_ns = now_ns
        self.observations += 1


def _read_text(path: pathlib.Path, description: str) -> str:
    try:
        data = path.read_bytes()
    except OSError as error:
        raise GuardError(f"cannot read {description}: {path}: {error}") from error
    if not data or len(data) > MAX_TEXT_BYTES or b"\0" in data:
        raise GuardError(f"{description} is empty or oversized: {path}")
    try:
        text = data.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise GuardError(f"{description} is not ASCII: {path}") from error
    if not text:
        raise GuardError(f"{description} is empty: {path}")
    return text


def _parse_decimal(text: str, description: str, maximum: int) -> int:
    if DECIMAL_PATTERN.fullmatch(text) is None:
        raise GuardError(f"{description} is not canonical decimal")
    value = int(text)
    if value > maximum:
        raise GuardError(f"{description} exceeds {maximum}")
    return value


def _parse_bdf(value: str) -> tuple[str, int, int]:
    normalized = value.lower()
    match = BDF_PATTERN.fullmatch(normalized)
    if match is None:
        raise GuardError("PCI BDF must have canonical dddd:bb:dd.f shape")
    domain = int(match.group("domain"), 16)
    location = (
        int(match.group("bus"), 16) << 8
        | int(match.group("device"), 16) << 3
        | int(match.group("function"), 16)
    )
    return normalized, domain, location


def _parse_unique_id(value: str) -> tuple[str, int]:
    normalized = value.lower()
    match = UNIQUE_ID_PATTERN.fullmatch(normalized)
    if match is None or match.group(1) == "0" * 16:
        raise GuardError("GPU unique ID must be exactly 16 nonzero hexadecimal digits")
    digits = match.group(1)
    return f"0x{digits}", int(digits, 16)


def _parse_id_list(text: str, description: str) -> set[int]:
    values: set[int] = set()
    for component in text.split(","):
        bounds = component.split("-", 1)
        if len(bounds) == 1:
            start = end = _parse_decimal(bounds[0], description, MAX_ID)
        else:
            start = _parse_decimal(bounds[0], description, MAX_ID)
            end = _parse_decimal(bounds[1], description, MAX_ID)
            if start > end:
                raise GuardError(f"{description} has a descending range")
        if end - start + 1 > MAX_ID_SET_CARDINALITY:
            raise GuardError(f"{description} range is oversized")
        for value in range(start, end + 1):
            if value in values:
                raise GuardError(f"{description} contains a duplicate ID")
            values.add(value)
            if len(values) > MAX_ID_SET_CARDINALITY:
                raise GuardError(f"{description} is oversized")
    if not values:
        raise GuardError(f"{description} is empty")
    return values


def _format_id_list(values: set[int]) -> str:
    if not values:
        raise GuardError("cannot format an empty ID set")
    ordered = sorted(values)
    ranges: list[str] = []
    start = previous = ordered[0]
    for value in ordered[1:]:
        if value == previous + 1:
            previous = value
            continue
        ranges.append(str(start) if start == previous else f"{start}-{previous}")
        start = previous = value
    ranges.append(str(start) if start == previous else f"{start}-{previous}")
    return ",".join(ranges)


def _parse_status(path: pathlib.Path) -> tuple[set[int], set[int]]:
    text = _read_text(path, "process status")
    retained: dict[str, str] = {}
    for line in text.splitlines():
        key, separator, value = line.partition(":")
        if not separator or key not in {"Cpus_allowed_list", "Mems_allowed_list"}:
            continue
        if key in retained:
            raise GuardError(f"process status repeats {key}")
        retained[key] = value.strip()
    if set(retained) != {"Cpus_allowed_list", "Mems_allowed_list"}:
        raise GuardError("process status omits CPU or memory-node allowance")
    return (
        _parse_id_list(retained["Cpus_allowed_list"], "allowed CPU list"),
        _parse_id_list(retained["Mems_allowed_list"], "allowed memory-node list"),
    )


def _parse_properties(path: pathlib.Path) -> dict[str, int]:
    text = _read_text(path, "KFD topology properties")
    properties: dict[str, int] = {}
    for line in text.splitlines():
        fields = line.split()
        if len(fields) != 2 or not fields[0].replace("_", "").isalnum():
            raise GuardError(f"malformed KFD topology property: {path}")
        name, raw_value = fields
        if name in properties:
            raise GuardError(f"duplicate KFD topology property {name}")
        properties[name] = _parse_decimal(
            raw_value, f"KFD topology property {name}", (1 << 64) - 1
        )
    return properties


def _numeric_directories(
    root: pathlib.Path, description: str, maximum_entries: int
) -> list[tuple[int, pathlib.Path]]:
    try:
        entries = list(os.scandir(root))
    except OSError as error:
        raise GuardError(f"cannot enumerate {description}: {root}: {error}") from error
    if len(entries) > maximum_entries:
        raise GuardError(f"{description} exceeds {maximum_entries} entries")
    output: list[tuple[int, pathlib.Path]] = []
    for entry in entries:
        if DECIMAL_PATTERN.fullmatch(entry.name) is None:
            raise GuardError(f"{description} contains a nonnumeric entry")
        try:
            is_directory = entry.is_dir(follow_symlinks=False)
        except OSError as error:
            raise GuardError(f"cannot inspect {description} entry: {error}") from error
        if not is_directory:
            raise GuardError(f"{description} contains a nondirectory entry")
        output.append((int(entry.name), pathlib.Path(entry.path)))
    output.sort()
    return output


def _record(prefix: str, fields: dict[str, str]) -> str:
    return prefix + " " + " ".join(f"{key}={value}" for key, value in fields.items())


def _sealed_record(prefix: str, fields: dict[str, str], digest_field: str) -> str:
    payload = (_record(prefix, fields) + "\n").encode()
    sealed = dict(fields)
    sealed[digest_field] = hashlib.sha256(payload).hexdigest()
    return _record(prefix, sealed)


def topology_record(
    *,
    gpu_index: int,
    pci_bdf: str,
    unique_id: str,
    pci_root: pathlib.Path,
    topology_root: pathlib.Path,
    status_path: pathlib.Path,
) -> str:
    if gpu_index < 0:
        raise GuardError("GPU index must be nonnegative")
    normalized_bdf, domain, location = _parse_bdf(pci_bdf)
    normalized_unique_id, numeric_unique_id = _parse_unique_id(unique_id)

    device_path = pci_root / normalized_bdf
    try:
        canonical_device = device_path.resolve(strict=True)
    except OSError as error:
        raise GuardError(f"cannot resolve selected PCI device: {error}") from error
    if not canonical_device.is_dir() or canonical_device.name.lower() != normalized_bdf:
        raise GuardError("selected PCI path does not resolve to its exact BDF")
    vendor = _read_text(device_path / "vendor", "PCI vendor").lower()
    if HEX_PATTERN.fullmatch(vendor) is None or int(vendor, 16) != EXPECTED_AMD_VENDOR:
        raise GuardError("selected PCI device is not AMD")
    sysfs_unique = _read_text(device_path / "unique_id", "PCI unique ID").lower()
    if UNIQUE_ID_PATTERN.fullmatch(sysfs_unique) is None:
        raise GuardError("PCI unique ID is not canonical")
    if int(sysfs_unique, 16) != numeric_unique_id:
        raise GuardError("PCI unique ID does not match the selected ROCm device")
    raw_numa_node = _read_text(device_path / "numa_node", "PCI NUMA node")
    if raw_numa_node.startswith("-"):
        raise GuardError("selected PCI device has no local NUMA node")
    numa_node = _parse_decimal(raw_numa_node, "PCI NUMA node", MAX_ID)
    device_local_cpus = _parse_id_list(
        _read_text(device_path / "local_cpulist", "PCI local CPU list"),
        "PCI local CPU list",
    )
    allowed_cpus, allowed_mem_nodes = _parse_status(status_path)
    if numa_node not in allowed_mem_nodes:
        raise GuardError("selected GPU NUMA node is outside Mems_allowed_list")
    local_allowed_cpus = device_local_cpus & allowed_cpus
    if not local_allowed_cpus:
        raise GuardError("no selected-GPU-local CPU is allowed")

    nodes_root = topology_root / "nodes"
    matches: list[tuple[int, int]] = []
    for node_id, node_path in _numeric_directories(
        nodes_root, "KFD topology node directory", MAX_TOPOLOGY_NODES
    ):
        gpu_id = _parse_decimal(
            _read_text(node_path / "gpu_id", "KFD GPU ID"),
            "KFD GPU ID",
            (1 << 32) - 1,
        )
        if gpu_id == 0:
            continue
        properties = _parse_properties(node_path / "properties")
        required = {
            "domain",
            "location_id",
            "unique_id",
            "vendor_id",
            "gfx_target_version",
        }
        if not required <= properties.keys():
            raise GuardError(f"KFD GPU node {node_id} omits identity properties")
        if properties["domain"] == domain and properties["location_id"] == location:
            if properties["unique_id"] != numeric_unique_id:
                raise GuardError("KFD unique ID does not match the selected PCI device")
            if properties["vendor_id"] != EXPECTED_AMD_VENDOR:
                raise GuardError("KFD node does not report AMD vendor identity")
            if properties["gfx_target_version"] != EXPECTED_GFX_TARGET_VERSION:
                raise GuardError("KFD node is not gfx942")
            matches.append((node_id, gpu_id))
    if len(matches) != 1:
        raise GuardError("selected PCI device does not have one exact KFD node")
    kfd_node, kfd_gpu_id = matches[0]

    nonlocal_allowed_cpus = allowed_cpus - device_local_cpus
    if nonlocal_allowed_cpus:
        observer_cpu = min(nonlocal_allowed_cpus)
        measurement_cpus = local_allowed_cpus
    else:
        if len(local_allowed_cpus) < 2:
            raise GuardError("no disjoint CPU is available for the queue observer")
        observer_cpu = max(local_allowed_cpus)
        measurement_cpus = local_allowed_cpus - {observer_cpu}

    fields = {
        "schema": TOPOLOGY_SCHEMA,
        "placement": "taskset-cpulist-then-numactl-physcpubind-membind-v1",
        "gpu_index": str(gpu_index),
        "pci_bdf": normalized_bdf,
        "unique_id": normalized_unique_id,
        "numa_node": str(numa_node),
        "device_local_cpu_list": _format_id_list(device_local_cpus),
        "allowed_cpu_list": _format_id_list(allowed_cpus),
        "allowed_mem_node_list": _format_id_list(allowed_mem_nodes),
        "measurement_cpu_list": _format_id_list(measurement_cpus),
        "observer_cpu": str(observer_cpu),
        "kfd_node": str(kfd_node),
        "kfd_gpu_id": str(kfd_gpu_id),
    }
    return _sealed_record("topology", fields, "topology_sha256")


def _read_process(proc_root: pathlib.Path, pid: int) -> ProcessObservation | None:
    path = proc_root / str(pid) / "stat"
    try:
        text = path.read_text(encoding="ascii")
    except FileNotFoundError:
        return None
    except (OSError, UnicodeError) as error:
        raise GuardError(
            f"cannot read process identity for PID {pid}: {error}"
        ) from error
    closing = text.rfind(")")
    if closing < 2 or closing + 2 >= len(text):
        raise GuardError(f"malformed process identity for PID {pid}")
    fields = text[closing + 2 :].split()
    if len(fields) < 20:
        raise GuardError(f"truncated process identity for PID {pid}")
    try:
        ppid = int(fields[1])
        process_group = int(fields[2])
        start_time = int(fields[19])
    except ValueError as error:
        raise GuardError(f"nonnumeric process identity for PID {pid}") from error
    if min(ppid, process_group, start_time) < 0:
        raise GuardError(f"negative process identity for PID {pid}")
    return ProcessObservation(ppid, process_group, start_time)


def _is_target_process(
    proc_root: pathlib.Path,
    pid: int,
    root_pid: int,
    root_start_time: int,
) -> bool:
    seen: set[int] = set()
    current = pid
    while current > 0 and len(seen) < 1024:
        if current in seen:
            raise GuardError("process ancestry contains a cycle")
        seen.add(current)
        observation = _read_process(proc_root, current)
        if observation is None:
            return False
        if current == root_pid:
            return observation.start_time == root_start_time
        current = observation.ppid
    return False


def _target_process_identity(
    proc_root: pathlib.Path,
    pid: int,
    root_pid: int,
    root_start_time: int,
) -> ProcessObservation | None:
    before = _read_process(proc_root, pid)
    if before is None or not _is_target_process(
        proc_root, pid, root_pid, root_start_time
    ):
        return None
    after = _read_process(proc_root, pid)
    if after != before:
        raise GuardError(
            f"target process identity changed during authentication: PID {pid}"
        )
    return before


def selected_gpu_queue_owners(
    kfd_proc_root: pathlib.Path, selected_gpu_id: int
) -> list[tuple[int, int]]:
    owners: list[tuple[int, int]] = []
    for pid, process_path in _numeric_directories(
        kfd_proc_root, "KFD process directory", MAX_KFD_PROCESSES
    ):
        queues_path = process_path / "queues"
        queues = _numeric_directories(
            queues_path,
            f"KFD queue directory for PID {pid}",
            MAX_QUEUES_PER_PROCESS,
        )
        for queue_id, queue_path in queues:
            gpu_id = _parse_decimal(
                _read_text(queue_path / "gpuid", "KFD queue GPU ID"),
                "KFD queue GPU ID",
                (1 << 32) - 1,
            )
            if gpu_id == selected_gpu_id:
                owners.append((pid, queue_id))
    return owners


def _disappeared_during_census(error: GuardError) -> bool:
    return isinstance(error.__cause__, FileNotFoundError)


def _path_is_confirmed_absent(path: pathlib.Path, description: str) -> bool:
    try:
        path.stat(follow_symlinks=False)
    except FileNotFoundError:
        return True
    except OSError as error:
        raise GuardError(
            f"cannot confirm {description} disappearance: {error}"
        ) from error
    return False


def _live_queue_directories(
    root: pathlib.Path,
    description: str,
    maximum_entries: int,
    target_owned: bool,
) -> tuple[list[tuple[int, pathlib.Path]], bool]:
    try:
        entries = list(os.scandir(root))
    except OSError as error:
        raise GuardError(f"cannot enumerate {description}: {root}: {error}") from error
    if len(entries) > maximum_entries:
        raise GuardError(f"{description} exceeds {maximum_entries} entries")
    output: list[tuple[int, pathlib.Path]] = []
    vanished = False
    for entry in entries:
        if DECIMAL_PATTERN.fullmatch(entry.name) is None:
            raise GuardError(f"{description} contains a nonnumeric entry")
        entry_path = pathlib.Path(entry.path)
        try:
            is_directory = entry.is_dir(follow_symlinks=False)
        except FileNotFoundError as error:
            if target_owned and _path_is_confirmed_absent(
                entry_path, f"target KFD queue entry {entry.name}"
            ):
                vanished = True
                continue
            raise GuardError(f"cannot inspect {description} entry: {error}") from error
        except OSError as error:
            raise GuardError(f"cannot inspect {description} entry: {error}") from error
        if not is_directory:
            if target_owned and _path_is_confirmed_absent(
                entry_path, f"target KFD queue entry {entry.name}"
            ):
                vanished = True
                continue
            raise GuardError(f"{description} contains a nondirectory entry")
        output.append((int(entry.name), entry_path))
    output.sort()
    return output, vanished


def foreign_selected_gpu_queues(
    *,
    kfd_proc_root: pathlib.Path,
    proc_root: pathlib.Path,
    selected_gpu_id: int,
    root_pid: int,
    root_start_time: int,
) -> list[tuple[int, int]]:
    _, foreign = classify_selected_gpu_queues(
        kfd_proc_root=kfd_proc_root,
        proc_root=proc_root,
        selected_gpu_id=selected_gpu_id,
        root_pid=root_pid,
        root_start_time=root_start_time,
    )
    return foreign


def classify_selected_gpu_queues(
    *,
    kfd_proc_root: pathlib.Path,
    proc_root: pathlib.Path,
    selected_gpu_id: int,
    root_pid: int,
    root_start_time: int,
) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
    target: list[tuple[int, int]] = []
    foreign: list[tuple[int, int]] = []
    processes = _numeric_directories(
        kfd_proc_root, "KFD process directory", MAX_KFD_PROCESSES
    )
    for pid, process_path in processes:
        target_identity = _target_process_identity(
            proc_root, pid, root_pid, root_start_time
        )
        selected_queues: list[int] = []
        vanished = False
        queues_path = process_path / "queues"
        try:
            queues, listing_vanished = _live_queue_directories(
                queues_path,
                f"KFD queue directory for PID {pid}",
                MAX_QUEUES_PER_PROCESS,
                target_identity is not None,
            )
            vanished = listing_vanished
        except GuardError as error:
            if (
                target_identity is not None
                and _disappeared_during_census(error)
                and _path_is_confirmed_absent(
                    process_path, f"target KFD process directory for PID {pid}"
                )
            ):
                vanished = True
                queues = []
            else:
                raise
        for queue_id, queue_path in queues:
            try:
                raw_gpu_id = _read_text(
                    queue_path / "gpuid", "KFD queue GPU ID"
                )
            except GuardError as error:
                if (
                    target_identity is not None
                    and _disappeared_during_census(error)
                    and _path_is_confirmed_absent(
                        queue_path, f"target KFD queue {pid}/{queue_id}"
                    )
                ):
                    vanished = True
                    continue
                raise
            gpu_id = _parse_decimal(
                raw_gpu_id,
                "KFD queue GPU ID",
                (1 << 32) - 1,
            )
            if gpu_id == selected_gpu_id:
                selected_queues.append(queue_id)
        if not selected_queues and not vanished:
            continue
        identity_after = _target_process_identity(
            proc_root, pid, root_pid, root_start_time
        )
        stable_target = (
            target_identity is not None and identity_after == target_identity
        )
        if vanished and not stable_target:
            raise GuardError(
                f"target process identity changed across queue disappearance: PID {pid}"
            )
        destination = target if stable_target else foreign
        destination.extend((pid, queue_id) for queue_id in selected_queues)
    return target, foreign


def _raw_monotonic_ns() -> int:
    return time.clock_gettime_ns(time.CLOCK_MONOTONIC_RAW)


def _process_group_exists(process_group: int) -> bool:
    try:
        os.killpg(process_group, 0)
    except ProcessLookupError:
        return False
    except PermissionError as error:
        raise GuardError(
            f"cannot inspect target process group {process_group}: {error}"
        ) from error
    return True


def _wait_for_process_group_absence(
    process: subprocess.Popen[bytes], timeout_seconds: float
) -> bool:
    deadline = time.monotonic() + timeout_seconds
    while True:
        leader_exit_code = process.poll()
        if leader_exit_code is not None and not _process_group_exists(process.pid):
            return True
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return False
        time.sleep(min(PROCESS_GROUP_POLL_SECONDS, remaining))


def _terminate_process_group(process: subprocess.Popen[bytes]) -> None:
    process.poll()
    if _process_group_exists(process.pid):
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        except PermissionError as error:
            raise GuardError(
                f"cannot terminate target process group: {error}"
            ) from error
    elif process.poll() is None:
        try:
            process.terminate()
        except ProcessLookupError:
            pass
    if _wait_for_process_group_absence(process, PROCESS_GROUP_GRACE_SECONDS):
        return
    if _process_group_exists(process.pid):
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except PermissionError as error:
            raise GuardError(f"cannot kill target process group: {error}") from error
    if process.poll() is None:
        try:
            process.kill()
        except ProcessLookupError:
            pass
    if not _wait_for_process_group_absence(process, PROCESS_GROUP_GRACE_SECONDS):
        raise GuardError("target process group survived SIGKILL")


def _sleep_until_observation_deadline(
    *,
    deadline_ns: int,
    clock: Callable[[], int],
    sleeper: Callable[[float], None],
) -> None:
    while True:
        remaining_ns = deadline_ns - clock()
        if remaining_ns <= 0:
            return
        sleeper(remaining_ns / 1_000_000_000)


def _hash_file(path: pathlib.Path) -> tuple[int, str]:
    digest = hashlib.sha256()
    byte_count = 0
    try:
        with path.open("rb") as source:
            while chunk := source.read(64 * 1024):
                byte_count += len(chunk)
                if byte_count > MAX_TARGET_OUTPUT_BYTES:
                    raise GuardError("target stdout exceeds the monitor limit")
                digest.update(chunk)
    except OSError as error:
        raise GuardError(f"cannot authenticate target stdout: {error}") from error
    if byte_count == 0:
        raise GuardError("target stdout is empty")
    return byte_count, digest.hexdigest()


def monitor_target(
    *,
    selected_gpu_id: int,
    observer_cpu: int,
    target_output: pathlib.Path,
    command: Sequence[str],
    kfd_proc_root: pathlib.Path,
    proc_root: pathlib.Path,
    clock: Callable[[], int] = _raw_monotonic_ns,
    sleeper: Callable[[float], None] = time.sleep,
) -> str:
    if not 0 < selected_gpu_id < (1 << 32):
        raise GuardError("selected KFD GPU ID is out of range")
    if not 0 <= observer_cpu <= MAX_ID:
        raise GuardError("observer CPU is out of range")
    if not command or not command[0]:
        raise GuardError("monitor requires a target command")
    if target_output.exists():
        raise GuardError("target stdout path already exists")

    process: subprocess.Popen[bytes] | None = None
    process_group_absence_verified = False
    target_output_created = False
    try:
        try:
            os.sched_setaffinity(0, {observer_cpu})
            if os.sched_getaffinity(0) != {observer_cpu}:
                raise GuardError("queue observer affinity did not take effect")
        except OSError as error:
            raise GuardError(f"cannot pin queue observer: {error}") from error

        cadence = ObservationCadence(MAX_OBSERVATION_GAP_NS)
        existing = selected_gpu_queue_owners(kfd_proc_root, selected_gpu_id)
        schedule_anchor_ns = clock()
        cadence.observe(schedule_anchor_ns)
        next_observation_deadline_ns = schedule_anchor_ns + POLL_INTERVAL_NS
        if existing:
            pid, queue = existing[0]
            raise GuardError(
                "selected-GPU queue exists before target launch: "
                f"pid={pid} queue={queue}"
            )
        try:
            output_fd = os.open(
                target_output,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW,
                0o600,
            )
            target_output_created = True
        except OSError as error:
            raise GuardError(f"cannot create private target stdout: {error}") from error
        with os.fdopen(output_fd, "wb") as output:
            try:
                process = subprocess.Popen(
                    command,
                    stdout=output,
                    start_new_session=True,
                )
            except OSError as error:
                raise GuardError(f"cannot start target command: {error}") from error

            root = _read_process(proc_root, process.pid)
            if root is None:
                raise GuardError("target exited before queue monitoring began")
            if root.process_group != process.pid:
                raise GuardError(
                    "target did not establish its dedicated process group: "
                    f"pid={process.pid} process_group={root.process_group}"
                )
            target_selected_queue_observations = 0
            while True:
                target, foreign = classify_selected_gpu_queues(
                    kfd_proc_root=kfd_proc_root,
                    proc_root=proc_root,
                    selected_gpu_id=selected_gpu_id,
                    root_pid=process.pid,
                    root_start_time=root.start_time,
                )
                cadence.observe(clock())
                target_selected_queue_observations += len(target)
                if foreign:
                    pid, queue = foreign[0]
                    raise GuardError(
                        f"foreign selected-GPU queue observed: pid={pid} queue={queue}"
                    )
                exit_code = process.poll()
                if exit_code is not None:
                    break
                _sleep_until_observation_deadline(
                    deadline_ns=next_observation_deadline_ns,
                    clock=clock,
                    sleeper=sleeper,
                )
                next_observation_deadline_ns += POLL_INTERVAL_NS

            if cadence.observations < 2:
                raise GuardError("target completed before two queue censuses")
            if target_selected_queue_observations == 0:
                raise GuardError("no target-owned selected-GPU queue was observed")
            if exit_code != 0:
                raise GuardError(f"target command exited with status {exit_code}")
            if _process_group_exists(process.pid):
                raise GuardError(
                    "target process group remains after leader exit: "
                    f"process_group={process.pid}"
                )
            process_group_absence_verified = True
            terminal_queues = selected_gpu_queue_owners(kfd_proc_root, selected_gpu_id)
            cadence.observe(clock())
            if terminal_queues:
                pid, queue = terminal_queues[0]
                raise GuardError(
                    "selected-GPU queue remains after target exit: "
                    f"pid={pid} queue={queue}"
                )

        output_bytes, output_sha256 = _hash_file(target_output)
        fields = {
            "schema": MONITOR_SCHEMA,
            "status": "clean",
            "monitor": "selected-kfd-gpu-process-tree-census-v2",
            "schedule": "absolute-monotonic-raw-deadline-v1",
            "kfd_gpu_id": str(selected_gpu_id),
            "root_pid": str(process.pid),
            "process_group": str(process.pid),
            "observer_cpu": str(observer_cpu),
            "interval_us": str(POLL_INTERVAL_NS // 1000),
            "maximum_gap_us": str(MAX_OBSERVATION_GAP_NS // 1000),
            "observed_maximum_gap_us": str(
                (cadence.observed_maximum_gap_ns + 999) // 1000
            ),
            "observations": str(cadence.observations),
            "target_selected_queue_observations": str(
                target_selected_queue_observations
            ),
            "foreign_selected_queues": "0",
            "terminal_selected_queues": "0",
            "target_exit_code": "0",
            "target_reaped": "1",
            "process_group_absent": "1",
            "target_output_bytes": str(output_bytes),
            "target_output_sha256": output_sha256,
        }
        return _sealed_record("monitor", fields, "monitor_sha256")
    except BaseException as original_error:
        cleanup_error: BaseException | None = None
        try:
            if process is not None and not process_group_absence_verified:
                _terminate_process_group(process)
        except BaseException as error:
            cleanup_error = error
        finally:
            if target_output_created:
                try:
                    target_output.unlink()
                except FileNotFoundError:
                    pass
                except OSError as error:
                    if cleanup_error is None:
                        cleanup_error = GuardError(
                            f"cannot delete rejected target output: {error}"
                        )
        if cleanup_error is not None:
            raise GuardError(
                f"target cleanup failed after {original_error}: {cleanup_error}"
            ) from cleanup_error
        raise


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="operation", required=True)

    topology = commands.add_parser("topology")
    topology.add_argument("--gpu-index", type=int, required=True)
    topology.add_argument("--pci-bdf", required=True)
    topology.add_argument("--unique-id", required=True)
    topology.add_argument(
        "--pci-root", type=pathlib.Path, default=pathlib.Path("/sys/bus/pci/devices")
    )
    topology.add_argument(
        "--topology-root",
        type=pathlib.Path,
        default=pathlib.Path("/sys/class/kfd/kfd/topology"),
    )
    topology.add_argument(
        "--status-path", type=pathlib.Path, default=pathlib.Path("/proc/self/status")
    )

    monitor = commands.add_parser("monitor")
    monitor.add_argument("--gpu-id", type=int, required=True)
    monitor.add_argument("--observer-cpu", type=int, required=True)
    monitor.add_argument("--target-output", type=pathlib.Path, required=True)
    monitor.add_argument(
        "--kfd-proc-root",
        type=pathlib.Path,
        default=pathlib.Path("/sys/class/kfd/kfd/proc"),
    )
    monitor.add_argument(
        "--proc-root", type=pathlib.Path, default=pathlib.Path("/proc")
    )
    monitor.add_argument("command", nargs=argparse.REMAINDER)
    return parser


def _run(arguments: argparse.Namespace) -> str:
    if arguments.operation == "topology":
        return topology_record(
            gpu_index=arguments.gpu_index,
            pci_bdf=arguments.pci_bdf,
            unique_id=arguments.unique_id,
            pci_root=arguments.pci_root,
            topology_root=arguments.topology_root,
            status_path=arguments.status_path,
        )
    command = list(arguments.command)
    if command and command[0] == "--":
        command.pop(0)
    return monitor_target(
        selected_gpu_id=arguments.gpu_id,
        observer_cpu=arguments.observer_cpu,
        target_output=arguments.target_output,
        command=command,
        kfd_proc_root=arguments.kfd_proc_root,
        proc_root=arguments.proc_root,
    )


def _raise_signal_error(signum: int, _frame: object) -> None:
    for managed_signal in MANAGED_SIGNALS:
        signal.signal(managed_signal, signal.SIG_IGN)
    raise GuardError(f"monitor interrupted by signal {signum}")


def main(argv: Sequence[str] | None = None) -> int:
    try:
        for signum in MANAGED_SIGNALS:
            signal.signal(signum, _raise_signal_error)
        arguments = _build_parser().parse_args(argv)
        print(_run(arguments))
    except GuardError as error:
        print(f"r26-host-guard: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
