#!/usr/bin/env python3
"""Package accepted A2 evidence without rebuilding, reproving or deleting inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import sys
import tarfile
import types

BASE = Path(__file__).resolve().parent
REPO = Path("/home/harsh/.codex-tmp/fe2o3-a2-producer-composition-20260930-qualified")
PACKET = BASE / "signed-composition-attempt-1"
READBACK = BASE / "independent-readback-attempt-1"
OUT = BASE / "compact-publication-attempt-2"
PREPARED = BASE / "prepared-publication-v2.json"
COMMON = "ee849c0fc5bf6d7f7bc21e4044b2a152dbfab1f3"
COMMITS = ["10013c06a14e876fde141c769b5baf8bc42b3203", "6b9e5d87c4386692f7c213a95dd786ccdf42cb45"]
PACKET_TREE = "c76eec6b642205a2d70517c96bbea14d8fc0c813fd56c8fbbec02d233260bfdd"
READBACK_SHA = "0aa2bda66cccacc40c57627a8f0bc09a35904c6b97e7e76cba2bee9e81302d70"
ROOT_WRAPPER = Path("/home/harsh/.codex-tmp/fe2o3-a2-composition-root-readback-20261001.py")
ROOT_RESULT = ROOT_WRAPPER.with_suffix(".json")
LIMIT = 512 * 1024 * 1024
PINS = {
    "signed_composition_campaign_v1.py": "e3e27dc88cc80891fccfe6d2203759b8c638f97d2f07badd289098ab4163ee6d",
    "test_signed_composition_campaign_v1.py": "a6df9ec519689150350f48e93b000787c27064ac9607af2a80c4032addc73ade",
    "prepared-signed-composition-v1.json": "62f5718616a574224aa7119839188959fc8e0e54ee29c2c4cc2f6a8384cc515e",
    "candidate-source-before-signing-v1.json": "74de648c10955d7b71682c96658765fc2b3eaba8fbc7d5939ce5dd0acbaaf4d6",
    "bind_candidate_v1.py": "0f865d4ec4930280214ba6a86f28c588248abd2c6cefea4437ecb13388de578a",
    "test_bind_candidate_v1.py": "83ac378fd7003a0a26636aee371c517bdfcff40cd70b63c39c1bd2d4d94bc808",
    "audit_signed_composition_v1.py": "b9d4b1920bd6167108d0a5d25ff050f6500453bcee03ef90fe34cf77da90ae9d",
    "test_audit_signed_composition_v1.py": "f35ed76e60066b90fa3acdc780de26ac6c5cb8d2f2016e5ffe247cf9bc6de5c9",
    "capture_independent_readback_v1.py": "756e2b4848cb16b3e989378f45d254184c06b527cce41b7e15f69114c2bddf1e",
    "final_diagnostics_v1.py": "318a29f9067618c2168a8e1d3585b1e8e3f8c78abe40359d52b0a20458238be4",
    "test_final_diagnostics_v1.py": "3dd9d43a9200b973a289401e650042e18e0cb0496c3e33064a6de85879bdf603",
    "capture_fixture_corpus_v1.py": "8c0cd832b66d27fc4102b093418e1d04baa4e40b1270977b67af08c343713cff",
    "diagnostic-fixtures-v1.json": "50bbc125bf95aa9ee283d89a6aa784c8e8b935b1fb2d106ee942a3dc35c3041d",
}
README = """# Producer Input Composition Qualification

Development evidence, not release qualification or HIP/HSA parity.

Signed candidate: `6b9e5d87c4386692f7c213a95dd786ccdf42cb45`.
All 102 campaign stages passed: 38 validator, 22 fold and 21 unfiltered
composition mutations; nine full proofs (42/13/64 obligations per family,
each before, relocated and after); six release checks; five source-control
stages; and the signed-commit check. Obligations are not independent runtime
properties. The original recorder closed all 102 owned process groups.
Agent and root independent readbacks agreed exactly.

`campaign.tar.xz` contains the entire immutable 1,218-file accepted campaign,
its original stage streams, the independent readback capture, root readback
receipt, and reviewed qualification sources. Identical payloads use backwards
in-archive hardlinks; no symlinks, external links, nested historical archives
or copied worktrees are used. `archive-index.json` describes every member and
its original local source. The packaging controller verifies all decoded
members and archive links without extracting them or executing archived code.

`signed-candidate.bundle` contains BOTH unpublished signed side commits,
`10013c06a14e876fde141c769b5baf8bc42b3203` and
`6b9e5d87c4386692f7c213a95dd786ccdf42cb45`, over public prerequisite
`ee849c0fc5bf6d7f7bc21e4044b2a152dbfab1f3`. Import it into a repository already
containing that public ancestor. The bundle is not a standalone repository.

## Replay Boundary

This packet is NOT self-contained full replay. The original tools (including
the pinned Verus release), live original and retained CPU ELF paths, complete
historical discovery/calibration directories, and fixed local source checkout
paths are external prerequisites. Their exact identities and hashes are in
`external-replay-prerequisites.json` and the preserved campaign preparation.
Recreating missing paths or obtaining those historical inputs is separate work.
The archived auditor intentionally fails closed when those inputs are absent.
The publication controller packages bytes; it does not rerun Verus, a CPU test,
Clippy, a GPU test, or the already completed independent auditor.

Historical raw buffers explicitly pinned by the campaign remain included.
Entire earlier campaigns are not recursively embedded. The interrupted original
33-stage calibration remains incomplete; its rejected/observational captures
and the accepted 15-stage observational continuation are never promoted to
qualified kills. All 81 qualified negatives are fresh signed-campaign runs.

## Scope

The actual shared validator and fold bodies are conditionally composed through
adapter contracts. Concrete journal forwarding, live-allocation validation,
fresh credit-lock observations, Arc identity and shared interior state are not
established by this theorem. Compiler/ISA refinement, allocation, unwinding,
hardware behavior, performance and HIP/HSA parity remain outside its scope.
The unchanged earlier CPU ELF is retained historical evidence, not a fresh
CPU run or qualification of a later source integration. Dynamic-loader
libraries are not hermetically attested.
"""


def need(value, message):
    if not value:
        raise ValueError(message)


def raw(path):
    need(path.is_file() and path.resolve() == path and not path.is_symlink(), "ordinary canonical input")
    return path.read_bytes()


def sha(path):
    return hashlib.sha256(raw(path)).hexdigest()


def save(path, data):
    with path.open("x", encoding="ascii") as stream:
        stream.write(json.dumps(data, sort_keys=True, indent=2) + "\n")


def tree(root):
    need(root.is_dir() and root.resolve() == root and not root.is_symlink(), "ordinary canonical source tree")
    paths = list(root.rglob("*"))
    need(all(path.resolve() == path and not path.is_symlink() for path in paths), "no redirected source entries")
    return {str(path.relative_to(root)): sha(path) for path in paths if path.is_file()}


def tree_digest(values):
    return hashlib.sha256(json.dumps(values, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def archive_name(name):
    need(type(name) is str and name.isascii() and "\0" not in name, "ASCII archive path")
    path = PurePosixPath(name)
    need(not path.is_absolute() and str(path) == name
         and path.parts and all(part not in ("", ".", "..") for part in path.parts), "canonical bounded archive path")


def inventory(sources):
    entries, first = {}, {}
    for name, path in sorted(sources.items()):
        archive_name(name)
        content = raw(path)
        mode = 0o755 if path.stat().st_mode & 0o111 else 0o644
        digest = hashlib.sha256(content).hexdigest()
        key = (digest, len(content), mode)
        entries[name] = {"origin": str(path), "sha256": digest, "bytes": len(content), "mode": mode,
                         "hardlink": first.get(key)}
        first.setdefault(key, name)
    return entries


def create_archive(path, entries):
    need(not path.exists(), "fresh archive output")
    with tarfile.open(path, "x:xz", format=tarfile.PAX_FORMAT, preset=6) as archive:
        for name, row in entries.items():
            info = tarfile.TarInfo(name)
            info.mode, info.uid, info.gid, info.mtime = row["mode"], 0, 0, 0
            if row["hardlink"] is not None:
                info.type, info.linkname, info.size = tarfile.LNKTYPE, row["hardlink"], 0
                archive.addfile(info)
            else:
                info.size = row["bytes"]
                with Path(row["origin"]).open("rb") as stream:
                    archive.addfile(info, stream)


def verify_archive(path, entries):
    seen, hashes = [], {}
    with tarfile.open(path, "r:xz") as archive:
        for info in archive:
            archive_name(info.name)
            need(info.name in entries and info.name not in hashes, "exact unique archive membership")
            row = entries[info.name]
            need(info.mode == row["mode"] and info.uid == info.gid == info.mtime == 0
                 and info.uname == info.gname == "" and set(info.pax_headers) <= {"path", "linkpath"}, "normalized safe archive metadata")
            if row["hardlink"] is not None:
                need(info.islnk() and info.size == 0 and info.linkname == row["hardlink"] and info.linkname in hashes,
                     "backwards exact internal hardlink, never a host path")
                observed = hashes[info.linkname]
            else:
                need(info.isfile() and not info.linkname and info.size == row["bytes"], "ordinary exact payload")
                digest, count = hashlib.sha256(), 0
                stream = archive.extractfile(info)
                need(stream is not None, "readable archive payload")
                while chunk := stream.read(1024 * 1024):
                    digest.update(chunk)
                    count += len(chunk)
                observed = (digest.hexdigest(), count, info.mode)
            need(observed == (row["sha256"], row["bytes"], row["mode"]), "decoded archive byte identity")
            hashes[info.name] = observed
            seen.append(info.name)
    need(seen == list(entries), "complete ordered archive roster")


def bundle_header(path):
    lines, size = [], 0
    with path.open("rb") as stream:
        while True:
            line = stream.readline(4096)
            size += len(line)
            need(line and size < 16384 and line.endswith(b"\n"), "bounded complete bundle header")
            if line == b"\n":
                break
            lines.append(line.decode().rstrip("\n"))
    need(lines[0] in ("# v2 git bundle", "# v3 git bundle"), "known Git bundle format")
    capabilities = [line for line in lines[1:] if line.startswith("@")]
    need(capabilities == ([] if lines[0] == "# v2 git bundle" else ["@object-format=sha1"]),
         "exact non-filtered SHA1 bundle capabilities")
    prerequisites = [line.split(" ", 1)[0][1:] for line in lines[1:] if line.startswith("-")]
    heads = [line.split(" ", 1) for line in lines[1:] if not line.startswith(("-", "@"))]
    need(prerequisites == [COMMON] and heads == [[COMMITS[-1], "HEAD"]], "exact public prerequisite and signed candidate head")
    return {"prerequisite": COMMON, "heads": heads, "format": lines[0]}


def bundle_objects(git):
    objects = git("rev-list", "--objects", COMMON + ".." + COMMITS[-1]).decode().splitlines()
    ids = sorted({line.split(" ", 1)[0] for line in objects})
    need(ids and all(re.fullmatch(r"[0-9a-f]{40}", value) for value in ids), "canonical bundle object identities")
    rows = git("cat-file", "--batch-check=%(objectname) %(objecttype) %(objectsize)",
               data=("\n".join(ids) + "\n").encode()).decode().splitlines()
    object_bytes = 0
    for oid, row in zip(ids, rows):
        fields = row.split()
        need(len(fields) == 3 and fields[0] == oid and fields[1] in ("commit", "tree", "blob")
             and fields[2].isdigit(), "exact typed bundle object inventory")
        object_bytes += int(fields[2])
    need(len(rows) == len(ids) and object_bytes <= 32 * 1024 * 1024, "bounded compact source delta, not complete checkout/history copy")
    return len(ids), object_bytes


def bundle_head(git):
    need(git("rev-parse", "HEAD").decode().strip() == COMMITS[-1]
         and not git("status", "--porcelain=v1", "--untracked-files=all"),
         "exact clean signed detached HEAD immediately before bundle export")


def context():
    need(all(sha(BASE / name) == digest for name, digest in PINS.items()), "reviewed qualification source pins")
    need(sha(ROOT_WRAPPER) == "5a3880e922b68554e1c52874c7cf7efabd5a7db7c6f1c533cbba18eeaca0765f"
         and sha(ROOT_RESULT) == sha(READBACK / "001-audit/owned/stdout.log") == READBACK_SHA,
         "exact matching independent agent/root readbacks")
    owner_path = BASE / "signed_composition_campaign_v1.py"
    owner = types.ModuleType("publication_owned_process_helpers")
    owner.__file__ = str(owner_path)
    exec(compile(raw(owner_path), str(owner_path), "exec"), owner.__dict__)
    c = owner.components()
    packet_tree = tree(PACKET)
    need(len(packet_tree) == 1218 and tree_digest(packet_tree) == PACKET_TREE, "entire immutable accepted102 campaign")
    result = json.loads(raw(ROOT_RESULT))
    need(result["independent_readback_accepted"] is True and result["stages"] == 102
         and result["packet_tree_sha256"] == PACKET_TREE and result["packet_files"] == 1218,
         "accepted exact prior independent audit; never rerun it during publication")
    history = json.loads(raw(BASE / "prepared-signed-composition-v1.json"))
    need(c.h.git("rev-list", "--reverse", COMMON + ".." + COMMITS[-1]).decode().splitlines() == COMMITS,
         "both unpublished side commits, no false assumption that10013 is public")
    object_count, object_bytes = bundle_objects(c.h.git)
    sources = {"campaign102/" + name: PACKET / name for name in packet_tree}
    sources.update({"independent-readback/" + name: READBACK / name for name in tree(READBACK)})
    sources.update({"qualification/" + name: BASE / name for name in PINS})
    sources.update({"root-readback/replay.py": ROOT_WRAPPER, "root-readback/result.json": ROOT_RESULT})
    entries = inventory(sources)
    source_bytes = sum(row["bytes"] for row in entries.values())
    need(source_bytes <= 256 * 1024 * 1024, "bounded total publication input bytes")
    artifact = history["retained_cpu_artifact"]
    need(all(sha(Path(artifact[key])) == artifact["sha256"] for key in ("source", "retained")), "retained original CPU ELF custody, no copy or execution")
    need(all(row["sha256"] != artifact["sha256"] for row in entries.values()), "CPU ELF is external, not archived")
    frozen = {"packet_tree_sha256": PACKET_TREE, "readback_sha256": READBACK_SHA, "source": c.source,
        "archive_entries": entries, "signed_commits": COMMITS, "public_prerequisite": COMMON,
        "bundle_object_count": object_count, "bundle_object_bytes": object_bytes,
        "retained_cpu_artifact": artifact, "tools": c.tools, "scope": history["scope"],
        "input_bytes": source_bytes, "unique_payload_bytes": sum(row["bytes"] for row in entries.values() if row["hardlink"] is None),
        "minimum_free_bytes": LIMIT, "maximum_publication_bytes": LIMIT,
        "publisher_sha256": sha(Path(__file__).resolve()), "publisher_controls_sha256": sha(BASE / "test_publish_composition_v2.py"),
        "self_contained_full_replay": False, "automatic_retries": False}
    return c, frozen, history, result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--prepared-sha256")
    args = parser.parse_args()
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize
         and args.prepare != args.execute, "explicit isolated prepare or separately reviewed one-shot publication")
    c, frozen, history, result = context()
    need(OUT.resolve() == OUT and not OUT.exists(), "fresh bounded publication directory")
    if args.prepare:
        need(args.prepared_sha256 is None, "preparation only")
        save(PREPARED, frozen)
        print(json.dumps({"prepared_sha256": sha(PREPARED), "members": len(frozen["archive_entries"]),
            "input_bytes": frozen["input_bytes"], "unique_payload_bytes": frozen["unique_payload_bytes"], "minimum_free_bytes": LIMIT}))
        return
    need(args.prepared_sha256 == sha(PREPARED) and json.loads(raw(PREPARED)) == frozen, "exact reviewed publication preparation")
    need(shutil.disk_usage(BASE).free >= LIMIT, "512MiB free for bounded one-shot packaging")
    OUT.mkdir()
    public = OUT / "public"
    public.mkdir()
    (OUT / "tmp").mkdir()
    managed = c.loader.load()
    c.loader.archive(OUT)
    env = c.d.environment(OUT)
    git = ["/usr/bin/git", "--no-replace-objects", "--no-pager", "-c", "gc.auto=0"]
    bundle = public / "signed-candidate.bundle"
    rows = []
    bundle_head(c.h.git)
    for label, argv in (("create", git + ["bundle", "create", str(bundle), "HEAD", "^" + COMMON]),
                        ("verify", git + ["bundle", "verify", str(bundle)]),
                        ("heads", git + ["bundle", "list-heads", str(bundle)])):
        folder = OUT / (str(len(rows) + 1).zfill(2) + "-bundle-" + label)
        folder.mkdir()
        save(folder / "command.json", {"argv": argv, "timeout": 130, "cwd": str(REPO), "environment": env})
        previous = Path.cwd()
        try:
            os.chdir(REPO)
            status, stdout, stderr = managed.run_owned(argv, 130, folder / "owned", env)
        finally:
            os.chdir(previous)
        record = json.loads(raw(folder / "owned/record.json"))
        c.base.owned_record(record, status, argv)
        need(type(status) is int and status == 0, "bounded Git bundle stage succeeded and group closed")
        need(raw(folder / "owned/stdout.log").decode() == stdout and raw(folder / "owned/stderr.log").decode() == stderr,
             "exact generated bundle raw streams")
        rows.append({"name": label, "command": argv, "record": record, "stdout": stdout, "stderr": stderr})
    need(rows[-1]["stdout"] == COMMITS[-1] + " HEAD\n" and rows[-1]["stderr"] == "", "exact single bundle head")
    header = bundle_header(bundle)
    archive = public / "campaign.tar.xz"
    create_archive(archive, frozen["archive_entries"])
    verify_archive(archive, frozen["archive_entries"])
    save(public / "archive-index.json", frozen["archive_entries"])
    save(public / "source-bundle-verification.json", {"header": header, "commits": COMMITS, "stages": rows})
    save(public / "external-replay-prerequisites.json", {"self_contained_full_replay": False,
        "fixed_source_repository": str(REPO), "source": frozen["source"], "public_git_prerequisite": COMMON,
        "tools": history["tools"], "complete_external_historical_trees": history["historical_trees"],
        "retained_cpu_artifact": history["retained_cpu_artifact"], "cpu_elf_included": False,
        "original33_calibration_complete": False, "historical_captures_are_qualified_kills": False,
        "dynamic_loader_libraries_hermetically_attested": False})
    with (public / "README.md").open("x", encoding="ascii") as stream:
        stream.write(README)
    for source, name in ((Path(__file__).resolve(), "publish.py"), (BASE / "test_publish_composition_v2.py", "test-publish.py")):
        with (public / name).open("xb") as stream:
            stream.write(raw(source))
    _, closing, _, _ = context()
    need(closing == frozen, "all sources, histories, CPU ELF bytes, tools and original packets unchanged")
    public_files = tree(public)
    total = sum(path.stat().st_size for path in OUT.rglob("*") if path.is_file())
    need(total <= LIMIT, "publication stayed under fixed512MiB bound")
    save(public / "summary.json", {"accepted": True, "signed_candidate": COMMITS[-1], "commits": COMMITS,
        "public_prerequisite": COMMON, "campaign_stages": 102, "fresh_negative_cases": 81,
        "independent_readbacks_agree": True, "families": result["families"], "packet_tree_sha256": PACKET_TREE,
        "archive_members": len(frozen["archive_entries"]), "archive_sha256": sha(archive), "bundle_sha256": sha(bundle),
        "input_bytes": frozen["input_bytes"], "unique_payload_bytes": frozen["unique_payload_bytes"],
        "preparation_sha256": args.prepared_sha256, "public_files_before_summary": public_files,
        "self_contained_full_replay": False, "cpu_elf_included": False, "new_solver_or_cpu_or_gpu_run": False,
        "scope": frozen["scope"]})
    need(sum(path.stat().st_size for path in OUT.rglob("*") if path.is_file()) <= LIMIT, "final storage bound")
    print(json.dumps({"public_directory": str(public), "summary_sha256": sha(public / "summary.json"),
        "archive_bytes": archive.stat().st_size, "bundle_bytes": bundle.stat().st_size, "files": tree(public)}, sort_keys=True))


if __name__ == "__main__":
    main()
