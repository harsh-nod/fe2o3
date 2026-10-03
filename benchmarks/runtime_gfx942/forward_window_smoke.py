#!/usr/bin/env python3
"""Selected three-GPU pending-frame forwarding correctness; no performance claim."""

import argparse
from functools import lru_cache
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import sys
from types import ModuleType

HERE = Path(__file__).resolve().parent
SHARED_SHA = "d586fd438e6bb4c399b9dc73bfea3475365c33cd3802defa186e10e81f859f65"
EXAMPLE = "gfx942-runtime-destination-segments-smoke"
CASES = tuple((f"{shape}-{'late' if late else 'queued'}-{'reverse' if reverse else 'forward'}",
               shape, late, reverse)
              for shape in ("overlap", "packets") for late in (False, True) for reverse in (False, True))


def load_shared():
    path = HERE / "selected_pair_smoke.py"
    if path.is_symlink() or not path.is_file():
        raise ValueError("ordinary shared controller required")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SHARED_SHA:
        raise ValueError("reviewed shared controller changed")
    module = ModuleType("forward_window_shared")
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


S = load_shared()


def endpoints(values):
    S.require(len(values) == 3, "exactly three selected devices required")
    selected = tuple(S.device(value) for value in values)
    S.require(all(len({row[column] for row in selected}) == 3 for column in range(3)),
              "selected indexes, PCI addresses and UIDs must each be distinct")
    return selected


@lru_cache(maxsize=4)
def oracle(shape, segmented=False):
    """Independent byte arithmetic; no runtime, Rust output or native data input."""
    packet = 0x3fffe0
    if shape == "overlap":
        lengths = (65729, 65857, 66030)
        lists = (((0, 0, 8193), (12288, 10000, 4097)),
                 ((7, 4096, 8195), (15000, 11000, 4099)))
        envelopes = (32768, 49152)
    else:
        S.require(shape == "packets", "known forwarding shape required")
        lengths = (2 * packet + 4289, 2 * packet + 4417, 2 * packet + 4590)
        lists = (((3, 41, packet + 17), (packet + 53, 7, 31)),
                 ((19, packet + 13, packet + 5), (packet + 79, 13, 37)))
        envelopes = (2 * packet + 2048, 2 * packet + 3072)
    mask = (1 << 64) - 1
    source = bytearray(lengths[0])
    for i in range(len(source)):
        x = (i + 0x9e3779b97f4a7c15) & mask
        x = ((x ^ (x >> 30)) * 0xbf58476d1ce4e5b9) & mask
        x = ((x ^ (x >> 27)) * 0x94d049bb133111eb) & mask
        source[i] = (x ^ (x >> 31)) & 255
    digests = []
    window = lengths[1] - 14
    forward = ((0, 5, window - 17), (window - 31, 1, 23), (192, 14, 9), (192, 14, 9))
    for round_ in range(2):
        payload = source if not round_ else source.translate(bytes(i ^ 73 for i in range(256)))
        frame = bytearray([0xa5 ^ (round_ * 29)]) * lengths[1]
        for base, segments in zip((17, 29), lists):
            for src, dst, size in segments:
                frame[53 + dst:53 + dst + size] = payload[base + src:base + src + size]
        target = bytearray([0xd3 ^ (round_ * 31)]) * lengths[2]
        if segmented:
            for src, dst, size in forward:
                target[37 + dst:37 + dst + size] = frame[7 + src:7 + src + size]
        else:
            target[37:37 + window] = frame[7:7 + window]
        host = bytearray([0x6c ^ (round_ * 41)]) * (lengths[2] + 42)
        host[19:19 + lengths[2]] = target
        digest = hashlib.sha256(b"fe2o3.segment-frame-peer-segments.full-bytes.v1\0" if segmented
                                else b"fe2o3.segment-frame-peer-window.full-bytes.v1\0")
        for data in (payload, frame, target, host):
            digest.update(len(data).to_bytes(8, "little"))
            digest.update(data)
        digests.append(digest.hexdigest())
    copied = tuple(sum(n for _, _, n in segments) for segments in lists)
    packets = tuple(sum((n + packet - 1) // packet for _, _, n in segments) for segments in lists)
    peer_packets = sum((n + packet - 1) // packet for _, _, n in forward) if segmented else (window + packet - 1) // packet
    return lengths, envelopes, copied, (*packets, peer_packets), tuple(digests)


def expected(selected, shape, late, segmented=False):
    lengths, envelopes, copied, packets, digests = oracle(shape, segmented)
    fields = dict(
        schema="fe2o3.segment-frame-peer-window.v1", authority="production-deny-all",
        transport="NATIVE-XGMI", devices=3, unique_ids=",".join(row[2] for row in selected),
        case=shape, late=str(late).lower(), contexts=1, rounds=2, kernels=0, modules=0,
        allocations=7, streams=6, source_bytes=lengths[0], frame_bytes=lengths[1], target_bytes=lengths[2],
        list_source_offsets="17,29", list_destination_offset=53, source_envelope=envelopes[0],
        destination_envelope=envelopes[1], descriptors_per_round="2,2",
        list_copied_bytes_per_round=",".join(map(str, copied)), packets_per_round=",".join(map(str, packets)),
        peer_source_offset=7, peer_target_offset=37, peer_bytes=lengths[1] - 14,
        readback_source_offset=0, readback_bytes=lengths[2],
        host_offset=19, host_suffix=23, host_bytes=lengths[2] + 42, lists=4,
        scalar_peers=2, d2h_copies=2, native_counter="0,3,6", completion_receipts=8,
        callbacks="exact-original-ids-once-successful",
        admission="lists-before-seed-scalar-after-latest-publication" if late else "all-four-before-progress",
        dependency="exact-latest-list-and-scalar-events", events="released-after-dependent-admission",
        descriptor_snapshot="caller-overwritten-after-admission",
        progress="latest-list-seed-then-final-readback-only" if late else "final-readback-only",
        publication_observed=str(late).lower(), publication_identity="ordered-roster-inference" if late else "not-sampled",
        native_at_peer_admission="1,4" if late else "0,3", retained_at_peer_admission="1,1" if late else "0,0",
        source="full-byte-pass", frame="full-byte-pass", initialized_complement="full-byte-pass",
        target_guards="full-byte-pass", host_guards="full-byte-pass",
        digest="domain-and-u64le-length-prefixed-source-frame-target-host", round_sha256=",".join(digests),
        payloads_changed="true", allocations_reused="true",
        results="released-readback-scalar-second-first-before-refresh", source_disposal="after-all-rounds-settled",
        drain="completed-tail-only", cleanup="logical-and-native-explicit", physical_overlap="unmeasured",
        performance_acceptance="false", formal_refinement="false")
    if segmented:
        fields.update(schema="fe2o3.segment-frame-peer-segments.v1", lists=6, scalar_peers=0,
                      dependency="exact-latest-list-events", peer_descriptors=4,
                      peer_target_envelope=lengths[2] - 44, peer_copied_bytes=lengths[1] + 10,
                      admission="parents-before-seed-list-after-latest-publication" if late else "all-four-before-progress",
                      results="released-readback-third-second-first-before-refresh")
    return {key: str(value) for key, value in fields.items()}


def campaign(recorder, helper, selected, binary, fd, identity, smi, segmented=False):
    report = dict(schema="fe2o3.forward-segments-smoke.v1" if segmented else "fe2o3.forward-window-smoke.v1", accepted=False, binary=str(binary),
                  binary_identity=identity, shared_sha256=SHARED_SHA, observer_sha256=S.OBSERVER_SHA,
                  devices=selected, started=S.stamp(), cases=[], errors=[],
                  scope="copy-only; point-observations-not-reservation; no-native-fault-injection")
    try:
        for name, shape, late, reverse in CASES:
            devices = tuple(reversed(selected)) if reverse else selected
            wanted = expected(devices, shape, late, segmented)
            row = dict(name=name, accepted=False, cleanup_observed=False)
            report["cases"].append(row)
            S.require(S.fingerprint(fd) == identity, "pinned executable changed before admission")
            S.observe_pair(recorder, helper, selected, smi, name + "-before")
            try:
                S.require(S.fingerprint(fd) == identity, "pinned executable changed during admission")
                mode = "segments" if segmented else "window"
                argv = [str(binary), f"--{'late-' if late else ''}forward-{mode}",
                        *(device[2] for device in devices), shape]
                receipt = recorder.run(name, argv, 180, executable=f"/proc/self/fd/{fd}", pass_fds=(fd,))
                row["pass"] = S.parse_pass(S.command_text(recorder, name, receipt), wanted)
                S.require(S.fingerprint(fd) == identity, "pinned executable changed during case")
                path_stat = binary.stat()
                S.require((path_stat.st_dev, path_stat.st_ino) == (identity["device"], identity["inode"]),
                          "executable pathname was replaced")
            finally:
                S.observe_pair(recorder, helper, selected, smi, name + "-after")
                row["cleanup_observed"] = True
            row["accepted"] = True
        report["accepted"] = True
    except BaseException as error:
        report["errors"].append(f"{type(error).__name__}: {error}")
        if not isinstance(error, Exception):
            raise
    finally:
        report["finished"] = S.stamp()
        report["commands"] = recorder.commands
        S.save(recorder.output / "result.json", report)
    return report


def main(argv=None):
    S.require(sys.flags.isolated and sys.flags.dont_write_bytecode, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-hardware", action="store_true", required=True)
    parser.add_argument("--segments", action="store_true", help="qualify ordered-list forwarding instead of scalar windows")
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--device", action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rocm-smi", type=Path, default=Path("/opt/rocm/bin/rocm-smi"))
    args = parser.parse_args(argv)
    selected = endpoints(args.device)
    S.require(re.fullmatch(r"[0-9a-f]{64}", args.binary_sha256), "canonical SHA256 required")
    binary = args.binary.absolute()
    S.require(binary.resolve(strict=True) == binary and binary.name == EXAMPLE, "canonical exact example path required")
    S.require(args.rocm_smi.is_absolute() and args.rocm_smi.is_file(), "absolute existing rocm-smi required")
    S.require(os.access("/dev/kfd", os.R_OK | os.W_OK), "read/write KFD access required")
    helper = S.load_observer()
    for number in S.MANAGED:
        signal.signal(number, S.interrupted)
    fd = os.open(binary, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        identity = S.fingerprint(fd)
        S.require(identity["sha256"] == args.binary_sha256, "binary SHA256 mismatch")
        args.output.mkdir(parents=False, exist_ok=False)
        result = campaign(S.Recorder(args.output), helper, selected, binary, fd, identity, args.rocm_smi, args.segments)
    finally:
        os.close(fd)
    print(json.dumps(dict(accepted=result["accepted"], cases=len(result["cases"]), errors=result["errors"],
                          results=str(args.output.absolute())), sort_keys=True))
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"forward-window smoke refused: {error}", file=sys.stderr)
        sys.exit(1)
