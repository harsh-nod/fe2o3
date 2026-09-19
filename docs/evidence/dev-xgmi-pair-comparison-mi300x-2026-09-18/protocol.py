#!/usr/bin/env python3
"""Pinned cohort, source transport, command, and transcript definitions."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import argparse
import gzip
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PRIOR = "docs/evidence/dev-xgmi-retirement-attribution-mi300x-2026-09-18/"
ATTRIBUTION_SHA = "94acb8893839768bf2a6247a81f8504f6d90c35729dc6670ebda4dd474f1acab"
BASE_SHA = "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7"
ATTRIBUTION_PATH = HERE / "attribution.py"
if not ATTRIBUTION_PATH.exists():
    ATTRIBUTION_PATH = ROOT / (PRIOR + "campaign.py")


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    path = Path(path)
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_pinned(path, digest, name):
    need(sha(path) == digest, "pinned helper: " + str(path))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


C = load_pinned(ATTRIBUTION_PATH, ATTRIBUTION_SHA, "xgmi_comparison_parser")
B = load_pinned(C.BASE_PATH, BASE_SHA, "xgmi_comparison_owner")
PREFIX = "/home/harsh/fe2o3-xgmi-pair-comparison-20260918."
LOCAL_PREFIX = "fe2o3-xgmi-pair-comparison-20260918."
B.PREFIX = PREFIX
read, parse_json, same_json = C.read, C.parse_json, C.same_json
devices, observer_command = C.devices, C.observer_command
parse_transcript, summarize = C.parse_transcript, C.summarize
SOURCE_PATHS = C.SOURCE_PATHS
SIGNERS, SIGNERS_SHA, SSH = C.SIGNERS, C.SIGNERS_SHA, C.SSH
SELECTOR, SELECTOR_SHA = C.SELECTOR, C.SELECTOR_SHA
COHORTS = {
    "baseline": {
        "commit": "84b61ee39817cee8bfe3cd68312f0a05543bc9af",
        "cpu": "docs/evidence/dev-xgmi-retirement-cpu-2026-09-18",
        "seal": "b2d79602fd7200ab9a50553b0eeed4b40be901e3df6099264a1e61723805533b",
        "source_snapshot": "524e4c3cf2e61a03495d75cbc61979ed8724e9d8ee81d0d7fcd6723f8e05415c",
        "file_map_sha256": "2fc57219cd7be108b20e45ec3570d02d9a63b1855d03e42f061f2d5b8d96f81b",
        "modes_sha256": "5b585d5a42add6413820d7e7c435d9e46957a431df6629b2cd9d57d424824ed2",
        "tree": "383a748dc84bb9d61770ca19af59c4d414a09445",
        "tar_sha256": "62bff66e1f43583f56882a922ae39fbbdb2f88be593714ad6a39ffce531f02f0",
        "tar_bytes": 11958904,
        "files": 5568,
    },
    "candidate": {
        "commit": "72eb6b3052b803a26a4007dca10fa2e26138eed1",
        "cpu": "docs/evidence/dev-xgmi-pair-currentness-cpu-2026-09-18",
        "seal": "7996f7daafcada9cdf40c18821e2d1b743205a929a52b464172bafbc529bdf5e",
        "source_snapshot": "87462da0047e92771bdb69f20c4db1eec8cd89e60ee839b89074090562f2f872",
        "file_map_sha256": "d2ccbd08b0cd7c4b9cbc58184c9a432aa0a0d21db5b9a2b6327a0d83c43c7ece",
        "modes_sha256": "19241cfb55338b68112fdf137e65dde7406dcf5cf6a7b161eff3cb505481f903",
        "tree": "3f31610747f2bab1c86161738f018bc12b1225c2",
        "tar_sha256": "dbcdc6128c2ca97ce5e46b4cfcf3a1e3e0d5fbeca2cb04f455b4ba4e1e787cbc",
        "tar_bytes": 11965419,
        "files": 5572,
    },
}
PHASES = [
    ["baseline-on1", "baseline", True],
    ["candidate-on1", "candidate", True],
    ["candidate-on2", "candidate", True],
    ["baseline-on2", "baseline", True],
    ["candidate-off1", "candidate", False],
    ["baseline-off1", "baseline", False],
    ["baseline-off2", "baseline", False],
    ["candidate-off2", "candidate", False],
]
PLAN = {
    "order": PHASES,
    "bytes": 1048576,
    "depth": 1,
    "warmups": 10,
    "samples": 30,
    "settled_seconds": 2,
    "delayed_seconds": 20,
}
ADDED = {
    "crates/fe2o3-kfd/src/currentness/full.rs",
    "crates/fe2o3-kfd/src/currentness/full/tests.rs",
    "crates/fe2o3-kfd/src/shared_memory/pair_currentness.rs",
    "crates/fe2o3-kfd/src/shared_memory/pair_currentness/tests.rs",
}
CHANGED = {
    "crates/fe2o3-kfd/src/currentness.rs",
    "crates/fe2o3-kfd/src/memory_linux.rs",
    "crates/fe2o3-kfd/src/shared_memory.rs",
    "crates/fe2o3-kfd/src/topology.rs",
}
STATIC_INPUTS = (
    "protocol.py",
    "campaign.py",
    "verify.py",
    "test_campaign.py",
    "test_verify.py",
    ".gitattributes",
)
PAYLOAD = (
    "protocol.py",
    "campaign.py",
    "attribution.py",
    "base.py",
    "source-baseline.tar.gz",
    "source-candidate.tar.gz",
)
MAX_TREE_BYTES = 4 * 1024 * 1024
MAX_BATCH_BYTES = 256 * 1024 * 1024
MAX_TRANSPORT_BYTES = 32 * 1024 * 1024
MAX_ARCHIVE_BYTES = 256 * 1024 * 1024


def canonical_path(name):
    need(
        type(name) is str
        and name
        and len(name.encode("utf-8")) <= 4096
        and not name.startswith("/")
        and all(part not in ("", ".", "..") for part in name.split("/"))
        and not any(ord(ch) < 32 or ch == "\\" for ch in name),
        "canonical relative source path",
    )


def file_map(files):
    need(type(files) is dict and files, "nonempty source map")
    for name, digest in files.items():
        canonical_path(name)
        need(
            type(digest) is str and re.fullmatch(r"[0-9a-f]{64}", digest),
            "source digest",
        )


def json_digest(value):
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def cpu_source(cohort):
    spec = COHORTS[cohort]
    archive = ROOT / spec["cpu"]
    need(sha(archive / "SHA256SUMS") == spec["seal"], "CPU seal identity")
    manifest = "".join(
        digest + "  " + name + "\n"
        for name, digest in B.inventory(archive).items()
        if name != "SHA256SUMS"
    )
    need((archive / "SHA256SUMS").read_text() == manifest, "CPU archive authenticated")
    snapshot = archive / "raw/source-before/stdout"
    need(sha(snapshot) == spec["source_snapshot"], "CPU source snapshot")
    files = read(snapshot)["files"]
    file_map(files)
    need(len(files) == spec["files"], "CPU source count")
    need(json_digest(files) == spec["file_map_sha256"], "CPU file-map identity")
    return files


def git(repo, arguments, data=None):
    result = subprocess.run(
        ["git", *arguments],
        cwd=repo,
        input=data,
        capture_output=True,
        timeout=120,
        check=True,
    )
    need(result.stderr == b"", "empty source-plumbing stderr")
    return result.stdout


def tree_entries(data, expected):
    need(len(data) <= MAX_TREE_BYTES, "bounded tree output")
    need(data.endswith(b"\0"), "complete tree framing")
    entries = {}
    for record in data[:-1].split(b"\0"):
        header, raw_path = record.split(b"\t", 1)
        mode, kind, oid = header.decode("ascii").split(" ")
        path = raw_path.decode("utf-8")
        canonical_path(path)
        need(path not in entries, "unique tree path")
        need(
            mode in ("100644", "100755") and kind == "blob", "ordinary signed tree blob"
        )
        need(re.fullmatch(r"[0-9a-f]{40}", oid), "tree blob identity")
        entries[path] = {"mode": mode, "oid": oid}
    need(set(entries) == set(expected), "exact signed selected source roster")
    return entries


def blob_contents(data, entries, expected):
    need(len(data) <= MAX_BATCH_BYTES, "bounded aggregate batch output")
    offset, contents = 0, {}
    for name in sorted(entries):
        end = data.find(b"\n", offset)
        need(end >= 0 and end - offset <= 128, "bounded complete blob header")
        header = data[offset:end].decode("ascii").split(" ")
        need(
            len(header) == 3
            and header[:2] == [entries[name]["oid"], "blob"]
            and re.fullmatch(r"0|[1-9][0-9]*", header[2]),
            "exact batch blob header",
        )
        size = int(header[2])
        need(size <= 256 * 1024 * 1024, "bounded source blob")
        start, offset = end + 1, end + 1 + size
        need(
            offset < len(data) and data[offset : offset + 1] == b"\n",
            "complete blob body",
        )
        value = data[start:offset]
        need(
            hashlib.sha256(value).hexdigest() == expected[name],
            "signed blob equals CPU source",
        )
        contents[name] = value
        offset += 1
    need(offset == len(data), "no extra batch output")
    return contents


def transport(contents, modes):
    output = io.BytesIO()
    with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
        with tarfile.open(
            fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT
        ) as archive:
            for name in sorted(contents):
                member = tarfile.TarInfo(name)
                member.mode = {"100644": 0o644, "100755": 0o755}[modes[name]]
                member.size = len(contents[name])
                archive.addfile(member, io.BytesIO(contents[name]))
    result = output.getvalue()
    validate_transport(
        result, {n: hashlib.sha256(v).hexdigest() for n, v in contents.items()}, modes
    )
    return result


def validate_transport(data, files, modes):
    need(len(data) <= MAX_TRANSPORT_BYTES, "bounded compressed source transport")
    with gzip.GzipFile(fileobj=io.BytesIO(data), mode="rb") as compressed:
        raw = compressed.read(MAX_ARCHIVE_BYTES + 1)
    need(len(raw) <= MAX_ARCHIVE_BYTES, "bounded expanded source transport")
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:") as archive:
        members = archive.getmembers()
        need(not archive.pax_headers, "no global transport overrides")
        B.validate_members(members, files)
        need([m.name for m in members] == sorted(files), "sorted transport roster")
        for member in members:
            need(
                member.mode == {"100644": 0o644, "100755": 0o755}[modes[member.name]]
                and member.uid == member.gid == member.mtime == 0
                and member.uname == member.gname == ""
                and not member.linkname
                and not member.sparse,
                "normalized transport metadata",
            )
            need(
                member.type == tarfile.REGTYPE
                and set(member.pax_headers) <= {"path"}
                and member.pax_headers.get("path", member.name) == member.name,
                "no transport metadata overrides",
            )
            need(
                hashlib.sha256(archive.extractfile(member).read()).hexdigest()
                == files[member.name],
                "exact transported source",
            )


def materialize(repo, commit, expected):
    file_map(expected)
    need(re.fullmatch(r"[0-9a-f]{40}", commit), "canonical pinned commit")
    need(
        git(repo, ["rev-parse", "--verify", commit + "^{commit}"]).decode().strip()
        == commit,
        "exact commit object",
    )
    tree = git(repo, ["rev-parse", "--verify", commit + "^{tree}"]).decode().strip()
    need(re.fullmatch(r"[0-9a-f]{40}", tree), "root tree identity")
    entries = tree_entries(
        git(repo, ["ls-tree", "-r", "-z", "--full-tree", commit, "--", *SOURCE_PATHS]),
        expected,
    )
    request = "".join(entries[name]["oid"] + "\n" for name in sorted(entries)).encode()
    contents = blob_contents(
        git(repo, ["cat-file", "--batch"], request), entries, expected
    )
    modes = {name: entries[name]["mode"] for name in sorted(entries)}
    data = transport(contents, modes)
    return {
        "tree": tree,
        "source_files": expected,
        "source_modes": modes,
        "tar_sha256": hashlib.sha256(data).hexdigest(),
    }, data


def pack(cohort, destination):
    spec = COHORTS[cohort]
    value, data = materialize(ROOT, spec["commit"], cpu_source(cohort))
    need(
        value["tree"] == spec["tree"]
        and json_digest(value["source_modes"]) == spec["modes_sha256"],
        "pinned signed source tree/modes",
    )
    need(
        len(data) == spec["tar_bytes"] and value["tar_sha256"] == spec["tar_sha256"],
        "fixed normalized cohort transport",
    )
    need(destination.name == "source-" + cohort + ".tar.gz", "cohort archive name")
    with destination.open("xb") as output:
        output.write(data)
    return {"cohort": cohort, "prerequisite": spec, **value}


def source_difference(cohorts):
    a, b = (cohorts[c]["source_files"] for c in COHORTS)
    difference = {
        "added": sorted(b.keys() - a.keys()),
        "removed": sorted(a.keys() - b.keys()),
        "changed": sorted(n for n in a.keys() & b.keys() if a[n] != b[n]),
    }
    need(
        difference
        == {"added": sorted(ADDED), "removed": [], "changed": sorted(CHANGED)},
        "reviewed exact source difference",
    )
    for name in a.keys() & b.keys():
        need(
            cohorts["baseline"]["source_modes"][name]
            == cohorts["candidate"]["source_modes"][name],
            "unchanged common source modes",
        )
    need(
        all(cohorts["candidate"]["source_modes"][n] == "100644" for n in ADDED),
        "ordinary added Rust sources",
    )
    return difference


def binary(cohort):
    return "target-" + cohort + "/release/examples/gfx942-runtime-xgmi-peer-benchmark"


def environment(owned, cohort):
    value = C.environment(owned)
    value["CARGO_TARGET_DIR"] = str(owned / ("target-" + cohort))
    return value


def build_specs(owned):
    result = []
    for name, command, seconds in C.builds()[:4]:
        result.append(
            (
                name,
                command,
                seconds,
                owned / "source-candidate",
                environment(owned, "candidate"),
            )
        )
    for cohort in COHORTS:
        _, command, seconds = C.builds()[-1]
        result.append(
            (
                "build-" + cohort,
                command,
                seconds,
                owned / ("source-" + cohort),
                environment(owned, cohort),
            )
        )
    need(len(result) == len({s[0] for s in result}), "unique build command names")
    return result


def phase_specs(owned, selected, phase):
    label, cohort, enabled = phase
    cwd, env = owned / ("source-" + cohort), environment(owned, cohort)
    result = [
        (label + "-before-gpu" + str(d[0]), observer_command(d), 100, cwd, env)
        for d in selected
    ]
    workload = [
        str(owned / binary(cohort)),
        *(d[2] for d in selected),
        "1048576",
        "1",
        "10",
        "30",
    ]
    if enabled:
        workload.append("--diagnose-xgmi")
    result.append((label, workload, 120, cwd, env))
    for suffix in ("settled", "delayed"):
        result.extend(
            (
                label + "-" + suffix + "-gpu" + str(d[0]),
                observer_command(d),
                100,
                cwd,
                env,
            )
            for d in selected
        )
    return result


def remote_specs(owned, selected):
    result = build_specs(owned) + [
        spec for phase in PHASES for spec in phase_specs(owned, selected, phase)
    ]
    need(len(result) == len({s[0] for s in result}), "unique remote command names")
    return result


def native_timeout():
    bounds = sum(
        spec[2] for spec in remote_specs(Path(PREFIX + "0" * 16), devices([1, 2]))
    )
    return bounds + len(PHASES) * (2 + 20) + 600


def validate_binding(binding):
    need(
        set(binding)
        == {
            "schema",
            "commit",
            "tools",
            "helpers",
            "devices",
            "plan",
            "cohorts",
            "source_difference",
            "payload",
        },
        "exact comparison binding keys",
    )
    need(
        binding["schema"] == "fe2o3.xgmi-pair-source-comparison.v1", "comparison schema"
    )
    need(re.fullmatch(r"[0-9a-f]{40}", binding["commit"]), "tooling commit")
    need(same_json(binding["plan"], PLAN), "predeclared balanced plan")
    need(
        binding["devices"] == devices([d[0] for d in binding["devices"]]),
        "exact physical device roster",
    )
    need(
        binding["helpers"] == {"attribution.py": ATTRIBUTION_SHA, "base.py": BASE_SHA},
        "helper identities",
    )
    need(set(binding["cohorts"]) == set(COHORTS), "exact cohorts")
    for name, spec in COHORTS.items():
        value = binding["cohorts"][name]
        need(
            set(value)
            == {
                "cohort",
                "prerequisite",
                "tree",
                "source_files",
                "source_modes",
                "tar_sha256",
            },
            "exact cohort keys",
        )
        need(
            value["cohort"] == name and same_json(value["prerequisite"], spec),
            "cohort prerequisite identity",
        )
        file_map(value["source_files"])
        need(len(value["source_files"]) == spec["files"], "cohort source count")
        need(
            json_digest(value["source_files"]) == spec["file_map_sha256"]
            and json_digest(value["source_modes"]) == spec["modes_sha256"]
            and value["tree"] == spec["tree"],
            "pinned source/tree/mode association",
        )
        need(
            set(value["source_modes"]) == set(value["source_files"])
            and all(m in ("100644", "100755") for m in value["source_modes"].values()),
            "exact source mode map",
        )
        need(
            re.fullmatch(r"[0-9a-f]{40}", value["tree"])
            and value["tar_sha256"] == spec["tar_sha256"],
            "cohort transport identities",
        )
    need(
        binding["source_difference"] == source_difference(binding["cohorts"]),
        "bound source difference",
    )
    need(
        set(binding["tools"]) == set(STATIC_INPUTS)
        and set(binding["payload"]) == set(PAYLOAD),
        "exact tool/payload rosters",
    )
    for values in (binding["tools"], binding["payload"]):
        need(
            all(
                type(v) is str and re.fullmatch(r"[0-9a-f]{64}", v)
                for v in values.values()
            ),
            "tool/payload digests",
        )
    for name in ("protocol.py", "campaign.py"):
        need(
            binding["payload"][name] == binding["tools"][name], "same local/remote tool"
        )
    for name, digest in binding["helpers"].items():
        need(binding["payload"][name] == digest, "same remote helper")
    for name in COHORTS:
        need(
            binding["payload"]["source-" + name + ".tar.gz"]
            == binding["cohorts"][name]["tar_sha256"],
            "bound cohort archive",
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cohort", choices=COHORTS)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    print(json.dumps(pack(args.cohort, args.destination), sort_keys=True))


if __name__ == "__main__":
    main()
