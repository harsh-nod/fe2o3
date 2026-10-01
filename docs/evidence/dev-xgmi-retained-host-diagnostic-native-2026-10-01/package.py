#!/usr/bin/env python3
"""Publish compact native24 raw evidence without altering its original records."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tarfile

BASE = Path("/home/harsh/.codex-tmp")
PREP = BASE / "fe2o3-xgmi-retained-host-diagnostic-native24-20260930-attempt-1"
REPO = BASE / "fe2o3-xgmi-retained-host-diagnostic-native-20260930"
OUT = BASE / "fe2o3-native24-publication-20261001-revision-2"
COMMIT = "a26dbebb57f948439a2e813c7a17c1142014ba1e"
PARENT = "4ad64047b0887a747a41aa7b0b9fa2f4206279c1"
PINS = {
    "analysis.py": (BASE / "fe2o3-native24-analysis-20261001.py", "93bda8d3973aa939fa04dd5ef6b1f9469c813b9ea0cb17e3c833c47559e54ebc"),
    "analysis.json": (BASE / "fe2o3-native24-analysis-20261001/analysis.json", "ed6cd5bcb90fd32b4a5c61e80180a3010cdb49a01d025fb87439aee887e07d30"),
    "analysis.md": (BASE / "fe2o3-native24-analysis-20261001/README.md", "6194468eff3d45d36ab93c0356e9e8765e2d8b13aa55e4e065c4f132c9af536b"),
    "root-readback.py": (BASE / "fe2o3-native24-root-readback-20261001.py", "e28243692bbfcc79556d9c7618a89221d9ed8a2498faac610f9b2a0f2e803451"),
    "root-readback.json": (BASE / "fe2o3-native24-root-readback-20261001.json", "3bd1fdabdcd8a65bc726835f9184a6ffaac63118a1bf1ac75d46bb015d9f9f84"),
    "root-preflight.py": (BASE / "fe2o3-native24-preflight-20261001.py", "896f2fcb43b914293161074013e2bfa4778eb2f1afffadb4fc2cfdae2904b2cf"),
    "root-preflight.json": (BASE / "fe2o3-native24-preflight-20261001.json", "54e6662a91d85504852d8d06f352bcd277ce285153296dba585ad3189bba3b0c"),
}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def load(path):
    return json.loads(path.read_bytes())


def save(path, value):
    with path.open("x", encoding="ascii") as stream:
        stream.write(json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + "\n")


def row(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path, "canonical regular raw file")
    return {"sha256": sha(path), "bytes": path.stat().st_size, "mode": path.stat().st_mode & 0o777}


def main():
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "python3 -I -B required")
    need(not OUT.exists(), "fresh publication directory")
    need(shutil.disk_usage(BASE).free >= 4 * 1024**3 + 128 * 1024**2, "local floor and publication budget")
    for path, digest in PINS.values():
        need(sha(path) == digest, "reviewed analysis/root audit pin")
    sys.path.insert(0, str(PREP / "source/benchmarks/runtime_gfx942"))
    import xgmi_retained_host_diagnostic_transport as extension
    transport, native, hot, ordinary, experiment, planner = extension.helpers()
    marker = load(PREP / "owner.json")
    binding = transport.read_binding(PREP, marker)
    extension.extended_binding(PREP, binding, transport)
    need(marker["commit"] == COMMIT, "signed source commit")
    need(sha(PREP / "source.tar.gz") == binding["payload_sha256"], "retained original source archive")
    transport.validate_archive(PREP / "source.tar.gz", binding["files"])
    collection = load(PREP / "remote-commands/collect/stdout")
    original_archive = PREP / "remote-commands/pull/stdout"
    need(sha(original_archive) == collection["archive_sha256"] ==
         "1375a5d7dd8968bd905a0a1ce74add0b3a2c38bf5336f44f63dc28a0592f2227", "exact native archive")
    transport.validate_archive(original_archive, collection["files"])
    need(transport.inventory(PREP / "readback") == collection["files"], "original extracted file closure")
    replay = extension.independent_replay(PREP / "readback", marker, binding, transport, hot, ordinary, experiment, planner)
    need(replay == load(PREP / "independent-replay.json") and replay["accepted"], "unchanged accepted replay")
    OUT.mkdir(mode=0o700)
    for name, (path, _) in PINS.items():
        shutil.copy2(path, OUT / name)
    shutil.copy2(PREP / "allowed-signers", OUT / "allowed-signers")
    shutil.copy2(Path(__file__).resolve(), OUT / "package.py")
    rec = native.FreshRecorder(OUT / "signature-records", REPO, hot)
    env = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C",
           "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_TERMINAL_PROMPT": "0"}
    prefix = ["/usr/bin/git", "-c", "gpg.format=ssh", "-c", "gpg.ssh.allowedSignersFile=" + str(OUT / "allowed-signers")]
    rec.run("signature", prefix + ["verify-commit", COMMIT], 60, env=env)
    rec.run("commit", ["/usr/bin/git", "cat-file", "commit", COMMIT], 60, env=env)
    rec.run("source-head", ["/usr/bin/git", "rev-parse", "HEAD"], 60, env=env)
    need((OUT / "signature-records/source-head/stdout").read_text().strip() == COMMIT, "frozen source HEAD")
    rec.run("bundle-create", ["/usr/bin/git", "bundle", "create", str(OUT / "signed-source.bundle"), "HEAD", "^" + PARENT], 180, env=env)
    rec.run("bundle-verify", ["/usr/bin/git", "bundle", "verify", str(OUT / "signed-source.bundle")], 60, env=env)
    rec.run("bundle-heads", ["/usr/bin/git", "bundle", "list-heads", str(OUT / "signed-source.bundle")], 60, env=env)
    need((OUT / "signature-records/bundle-heads/stdout").read_text().splitlines() == [COMMIT + " HEAD"], "exact source bundle head")
    signature = (OUT / "signature-records/signature/stderr").read_text()
    need('Good "git" signature for harmenon@amd.com' in signature and
         "SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg" in signature, "expected source signer")
    save(OUT / "signature-census.json", native.fresh_census(rec, hot, os.readlink("/proc/self/ns/pid")))
    originals = {}
    names = ("owner.json", "binding.json", "allowed-signers", "prepare-census.json", "transport-census.json",
             "transport-finished.json", "independent-replay.json")
    for name in names:
        originals["native24/" + name] = PREP / name
    for directory in ("commands", "remote-commands"):
        for path in sorted((PREP / directory).rglob("*")):
            if path.is_file():
                originals["native24/" + path.relative_to(PREP).as_posix()] = path
    rejected = BASE / "fe2o3-native24-publication-20261001"
    for name in ("package.py", "rejected.json"):
        originals["publication-rejection-1/" + name] = rejected / name
    for path in sorted((rejected / "signature-records").rglob("*")):
        if path.is_file():
            originals["publication-rejection-1/" + path.relative_to(rejected).as_posix()] = path
    originals["publication-resource-rejection-2.json"] = BASE / "fe2o3-native24-publication-20261001-revision-2-floor-rejection.json"
    pins = {name: {**row(path), "original_path": str(path)} for name, path in sorted(originals.items())}
    raw_archive = OUT / "raw.tar.xz"
    with tarfile.open(raw_archive, "x:xz", preset=6) as archive:
        for name, path in sorted(originals.items()):
            archive.add(path, arcname=name, recursive=False)
    with tarfile.open(raw_archive, "r:xz") as archive:
        seen = set()
        for member in archive:
            need(member.isfile() and member.name in pins and member.name not in seen, "exact public raw roster")
            expected = pins[member.name]
            need(member.size == expected["bytes"] and member.mode == expected["mode"], "raw size/mode")
            with archive.extractfile(member) as stream:
                need(hashlib.file_digest(stream, "sha256").hexdigest() == expected["sha256"], "public raw bytes")
            seen.add(member.name)
        need(seen == set(pins), "complete public raw archive")
    save(OUT / "raw-index.json", pins)
    need({name: {**row(path), "original_path": str(path)} for name, path in sorted(originals.items())} == pins,
         "original records unchanged after packaging")
    need(transport.inventory(PREP / "readback") == collection["files"], "four original ELFs and raw collection unchanged")
    summary = {"schema": "fe2o3.native24-compact-publication.v1", "source_commit": COMMIT,
               "bundle_prerequisite": PARENT, "marker": marker, "original_execution_session": 1309,
               "native_collection": {"archive_sha256": collection["archive_sha256"], "archive_bytes": collection["archive_bytes"],
                                     "files": len(collection["files"]), "archive_member": "native24/remote-commands/pull/stdout",
                                     "four_elf_sha256": load(PREP / "readback/campaign-1/binaries-before.json")},
               "raw_archive": row(raw_archive), "raw_files": len(pins),
               "raw_logical_bytes": sum(value["bytes"] for value in pins.values()),
               "source_binding": {"binding_sha256": sha(PREP / "binding.json"), "selected_objects": len(binding["selected_objects"]),
                                  "transport_files": len(binding["files"]), "signed_bundle_sha256": sha(OUT / "signed-source.bundle"),
                                  "source_payload_sha256": binding["payload_sha256"], "source_payload_bytes": binding["payload_bytes"],
                                  "source_payload_published": False, "source_payload_retained_at": str(PREP / "source.tar.gz")},
               "independent_replay": replay, "originals_deleted": False, "self_contained_full_replay": False,
               "new_gpu_execution": False, "runtime_source_changes": False, "performance_acceptance": False}
    save(OUT / "summary.json", summary)
    (OUT / "README.md").write_text("""# MI300X Native24 Retained-Host Diagnostic Evidence

The original session `1309` completed with exit 0 at signed source `a26dbebb57f948439a2e813c7a17c1142014ba1e`. All 24 trials (18 ordinary, 6 profiled), 327 native stages and 6 transport stages closed successfully. Independent replay passed. The exact owned remote directory was collected, removed and checked absent. No additional GPU run is represented here.

`analysis.md` contains the ordinary comparison and separate host diagnostic distributions. `analysis.json` contains every phase/counter distribution, missing-value count and invocation identity. All 120 profiled samples have complete observations. This run does not establish performance acceptance, engine equivalence, formal source-to-device refinement, full HIP/HSA parity or exclusive GPU reservation.

## Contents and Binding

- `raw.tar.xz` preserves all 14 preparation stages and all 6 transport stages, their original stdout/stderr/receipts, ownership/source binding, and censuses. `raw-index.json` pins each original byte stream and mode.
- The byte-identical original 3,578,669-byte native collection archive occurs exactly once inside that archive, at `native24/remote-commands/pull/stdout`. It contains all 1,040 native collection files, including the four original unmodified ELFs in `campaign-1/binaries`, the 327 command records, source/tool/loader identities, observer records, monitor and final replay. Its SHA-256 is `1375a5d7dd8968bd905a0a1ce74add0b3a2c38bf5336f44f63dc28a0592f2227`.
- `signed-source.bundle` retains exactly the signed a26 commit over prerequisite `4ad64047b0887a747a41aa7b0b9fa2f4206279c1`, already public in both repositories. It is not a standalone repository. `signature-records/`, `signature-census.json` and `allowed-signers` retain fresh bounded signature/bundle checks and the exact signed commit bytes. The signer key is the reviewed project key; the included allowed-signers file is evidence of that reviewed identity, not an independent trust anchor.
- `root-preflight.*` and `root-readback.*` preserve independent source/signature/preparation and final all-member/command/stdin/cleanup audits. `analysis.py` and `package.py` are retained exact controllers, not runtime source changes.
- `publication-rejection-1/` inside the raw archive preserves the first packaging controller and all three closed Git records. That attempt stopped when Git refused a literal-hash bundle tip; the corrected attempt binds `HEAD` to the same signed a26 commit. The packaging rejection did not change native acceptance, rerun a workload, or alter original evidence.
- `publication-resource-rejection-2.json` records a later preparation rejected by the unchanged local reserve guard before any output directory or subprocess existed. A separately authorized preparation proceeded only after the reserve recovered; no GPU workload was retried.

## Replay Limits

This compact publication is intentionally not self-contained for full original-controller replay. The 33,171,355-byte source transport payload, materialized selected checkout/Git object closure, and repository ancestors are not duplicated here. Their hashes, selected Git objects, modes and source-byte mappings remain in the archived binding and preparation records. The original payload and checkout remain retained at the local path recorded in `summary.json`; no original evidence or ELF was deleted.

An external reviewer can independently authenticate the commit using a separately trusted project signing key and the public repository, inspect/reparse every ordinary/profiled native output, validate all nested archive hashes and inspect every original ELF and recorded receipt. Repeating the exact full analysis/root audit additionally requires the byte-identical original source payload and prepared source closure, or an independently verified reconstruction matching all recorded hashes. Repacking an equivalent Git tree does not reproduce the original gzip payload identity or receive-stdin hash. The original root controllers also retain their absolute original paths; this packet makes no claim that copying them elsewhere is sufficient to run them unchanged.

The source already includes the previously published CPU qualification and its preserved rejections. This accepted native packet does not relabel those rejections or the preliminary misindexed availability observation as native admission. Actual campaign admission is in the 327 native command records.
""", encoding="ascii")
    save(OUT / "publication-index.json", {path.relative_to(OUT).as_posix(): row(path)
        for path in sorted(OUT.rglob("*")) if path.is_file()})
    print(json.dumps({"output": str(OUT), "summary_sha256": sha(OUT / "summary.json"),
                      "publication_index_sha256": sha(OUT / "publication-index.json"),
                      "raw_archive_sha256": sha(raw_archive), "raw_archive_bytes": raw_archive.stat().st_size,
                      "raw_files": len(pins), "source_bundle_bytes": (OUT / "signed-source.bundle").stat().st_size}, sort_keys=True))


if __name__ == "__main__":
    main()
