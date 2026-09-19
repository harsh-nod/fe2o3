#!/usr/bin/env python3
"""Prepare, replay and seal CPU-only XGMI peer-batch evidence."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run verification with python3 -I")

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import stat
import subprocess

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PREFIX = "docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/"
SCHEMA = "fe2o3.xgmi-peer-batch-cpu.v1"
DOCUMENT = "docs/runtime-xgmi-peer-batch-v1.md"
FIXED_SOURCE = "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
ACCEPT = "docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18/accept.py"
ACCEPT_SHA = "be5998bbd3bde933c48d4ae3d6e12ae8231da62cffd254abdb7c7ebe3f7dc8bd"
QUALIFY = PREFIX + "qualify.py"
FIXED_TOOLS = {
    ACCEPT: ACCEPT_SHA,
    "docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18/qualify.py": "43397a7a269a21dd317ba0e0d8137943800441f6e17eacbeee4e3766c684e7a3",
    "docs/evidence/dev-xgmi-pair-currentness-cpu-2026-09-18/qualify.py": "06f5d1e8ba5f8e8afee3c7a3dce2373a64b9c1cb9621314613ef2eaa7da38686",
    "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py": "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
    "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py": "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953",
}
LOCAL_TOOLS = tuple(
    PREFIX + name for name in ("qualify.py", "verify.py", "test_verify.py")
)
REQUIRED_SOURCE = {
    "crates/fe2o3-kfd/src/sdma.rs",
    "crates/fe2o3-kfd/src/sdma/tests/xgmi_batch_wait.rs",
    "crates/fe2o3-runtime/examples/gfx942-runtime-xgmi-peer-benchmark.rs",
    "crates/fe2o3-runtime/src/context.rs",
    "crates/fe2o3-runtime/src/context/peer_batch.rs",
    "crates/fe2o3-runtime/src/context/tests/peer_batch_tests.rs",
    "crates/fe2o3-runtime/src/context/versions/submissions.rs",
    "crates/fe2o3-runtime/src/kfd_backend.rs",
    "crates/fe2o3-runtime/src/kfd_backend/xgmi_batch.rs",
    "crates/fe2o3-runtime/src/kfd_backend/xgmi_batch/tests.rs",
    "crates/fe2o3-runtime/src/kfd_backend/xgmi_diagnostic.rs",
}
BINDING_KEYS = {
    "schema",
    "execution_root",
    "source_base",
    "source_snapshot_sha256",
    "source_files",
    "rosters",
    "tools",
    "document",
    "toolchain",
    "commands",
}


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


# Authenticate the parser helper before executing any repository helper code.
need(sha(ROOT / ACCEPT) == ACCEPT_SHA, "pinned acceptance parser")
spec = importlib.util.spec_from_file_location("peer_batch_cpu_parsers", ROOT / ACCEPT)
A = importlib.util.module_from_spec(spec)
spec.loader.exec_module(A)
read = A.read


def inventory(archive):
    need(archive.is_dir() and not archive.is_symlink(), "ordinary archive directory")
    files, directories = {}, set()
    for path in sorted(archive.rglob("*")):
        mode = path.lstat().st_mode
        need(not stat.S_ISLNK(mode), "no archive symlinks")
        name = path.relative_to(archive).as_posix()
        if stat.S_ISDIR(mode):
            directories.add(name)
        else:
            need(stat.S_ISREG(mode), "ordinary archive members")
            files[name] = sha(path)
    return files, directories


def seal(archive, create):
    files, _ = inventory(archive)
    files.pop("SHA256SUMS", None)
    manifest = "".join(digest + "  " + name + "\n" for name, digest in files.items())
    path = archive / "SHA256SUMS"
    if create:
        with path.open("x") as output:
            output.write(manifest)
    else:
        need(path.is_file() and not path.is_symlink(), "ordinary archive seal")
        need(path.read_text() == manifest, "complete archive seal")


def expected_tools(archive):
    result = dict(FIXED_TOOLS)
    for relative in LOCAL_TOOLS:
        result[relative] = sha(archive / Path(relative).name)
    return result


def authenticate_tools(archive, tools):
    need(set(tools) == set(FIXED_TOOLS) | set(LOCAL_TOOLS), "exact tool keyset")
    need(tools == expected_tools(archive), "exact frozen tool hashes")
    for name, digest in tools.items():
        need(sha(ROOT / name) == digest, "pinned repository tool: " + name)


def authenticated_commands(archive, execution_root, tools):
    authenticate_tools(archive, tools)
    spec = importlib.util.spec_from_file_location(
        "peer_batch_cpu_commands", ROOT / QUALIFY
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.configure(execution_root, Path(execution_root) / PREFIX)
    return module.commands(), module.SELECTOR


def roster(output):
    names = re.findall(r"^(\S+): test$", output, re.MULTILINE)
    need(len(names) == len(set(names)), "unique roster test names")
    need(output.endswith(f"{len(names)} tests, 0 benchmarks\n"), "exact roster closure")
    return sorted(names)


def roster_binding(names):
    canonical = "\n".join(names) + "\n"
    return {
        "count": len(names),
        "sha256": hashlib.sha256(canonical.encode()).hexdigest(),
    }


def validate_binding(binding):
    need(set(binding) == BINDING_KEYS, "exact binding keyset")
    need(binding["schema"] == SCHEMA, "binding schema")
    need(
        isinstance(binding["execution_root"], str)
        and Path(binding["execution_root"]).is_absolute(),
        "absolute execution root",
    )
    need(re.fullmatch(r"[0-9a-f]{40}", binding["source_base"]), "source base")
    need(
        re.fullmatch(r"[0-9a-f]{64}", binding["source_snapshot_sha256"]),
        "source snapshot digest",
    )
    need(
        type(binding["source_files"]) is int and binding["source_files"] > 0,
        "source count",
    )
    need(type(binding["commands"]) is int and binding["commands"] > 0, "command count")
    need(set(binding["rosters"]) == {"kfd", "runtime", "example"}, "roster keys")
    for value in binding["rosters"].values():
        need(
            set(value) == {"count", "sha256"}
            and type(value["count"]) is int
            and value["count"] > 0
            and re.fullmatch(r"[0-9a-f]{64}", value["sha256"]),
            "roster binding",
        )
    need(
        set(binding["document"]) == {"path", "sha256"}
        and binding["document"]["path"] == DOCUMENT
        and re.fullmatch(r"[0-9a-f]{64}", binding["document"]["sha256"]),
        "document binding",
    )
    need(
        set(binding["toolchain"]) == {"rustc_stdout_sha256", "cargo_stdout_sha256"}
        and all(
            re.fullmatch(r"[0-9a-f]{64}", value)
            for value in binding["toolchain"].values()
        ),
        "toolchain binding",
    )
    need(
        isinstance(binding["tools"], dict)
        and all(
            isinstance(name, str) and re.fullmatch(r"[0-9a-f]{64}", digest)
            for name, digest in binding["tools"].items()
        ),
        "tool binding",
    )


def expected_inventory(commands, prepared):
    top = {
        "qualify.py",
        "verify.py",
        "test_verify.py",
        "README.md",
        "tools.json",
    }
    if prepared:
        top.add("binding.json")
    files = top | {
        "raw/" + name + "/" + item
        for name, _, _ in commands
        for item in ("receipt.json", "stdout", "stderr")
    }
    directories = {"raw"} | {"raw/" + name for name, _, _ in commands}
    return files, directories


def derive(archive, *, binding=None, live=False):
    tools = read(archive / "tools.json")
    execution_root = (
        binding["execution_root"]
        if binding is not None
        else read(archive / "raw/source-before/receipt.json")["cwd"]
    )
    commands, _selector = authenticated_commands(archive, execution_root, tools)
    files, directories = inventory(archive)
    expected_files, expected_directories = expected_inventory(
        commands, binding is not None
    )
    need(set(files) - {"SHA256SUMS"} == expected_files, "exact archive file roster")
    need(directories == expected_directories, "exact archive directory roster")

    A.ROOT = Path(execution_root)
    previous = 0
    rosters = {}
    examples = {}
    for name, command, seconds in commands:
        folder = archive / "raw" / name
        previous = A.receipt(folder, command, seconds, previous)
        output = (folder / "stdout").read_text()
        if name.endswith("-roster"):
            target, package, _ = name.split("-")
            rosters[(target, package)] = roster(output)
        elif name in ("gnu-kfd", "musl-kfd", "gnu-runtime", "musl-runtime"):
            A.passing_tests(output, rosters[tuple(name.split("-"))])
        elif name in ("gnu-example", "musl-example", "example-feature-off"):
            names = sorted(re.findall(r"^test (\S+) \.\.\. ok$", output, re.MULTILINE))
            A.passing_tests(output, names)
            examples[name] = names
    for package in ("kfd", "runtime"):
        need(
            rosters[("gnu", package)] == rosters[("musl", package)],
            "cross-target roster",
        )
    need(
        examples["gnu-example"]
        == examples["musl-example"]
        == examples["example-feature-off"],
        "feature-on/off example membership",
    )
    need(
        any(
            name.startswith("sdma::tests::xgmi_batch_wait::")
            for name in rosters[("gnu", "kfd")]
        )
        and any(
            name.startswith("context::tests::peer_batch_tests::")
            for name in rosters[("gnu", "runtime")]
        )
        and any(
            name.startswith("kfd_backend::xgmi_batch::")
            for name in rosters[("gnu", "runtime")]
        ),
        "peer-batch test families selected",
    )

    before = archive / "raw/source-before/stdout"
    after = archive / "raw/source-after/stdout"
    snapshot_sha = sha(before)
    need(snapshot_sha == sha(after), "unchanged source snapshot bytes")
    source = read(before)
    need(
        set(source) == {"base", "files"}
        and re.fullmatch(r"[0-9a-f]{40}", source["base"])
        and isinstance(source["files"], dict)
        and REQUIRED_SOURCE <= set(source["files"]),
        "exact source schema and required peer-batch files",
    )
    need(
        all(
            isinstance(name, str) and re.fullmatch(r"[0-9a-f]{64}", digest)
            for name, digest in source["files"].items()
        ),
        "source file-map identities",
    )
    if live:
        current = json.loads(
            subprocess.check_output(
                ["python3", "-I", "-B", str(ROOT / FIXED_SOURCE)], cwd=ROOT
            ),
            object_pairs_hook=A.object_pairs,
        )
        need(current["files"] == source["files"], "live qualified source equality")

    A.unsafe_policy((archive / "raw/unsafe-source/stdout").read_text())
    derived = {
        "schema": SCHEMA,
        "execution_root": execution_root,
        "source_base": source["base"],
        "source_snapshot_sha256": snapshot_sha,
        "source_files": len(source["files"]),
        "rosters": {
            "kfd": roster_binding(rosters[("gnu", "kfd")]),
            "runtime": roster_binding(rosters[("gnu", "runtime")]),
            "example": roster_binding(examples["gnu-example"]),
        },
        "tools": tools,
        "document": {"path": DOCUMENT, "sha256": sha(ROOT / DOCUMENT)},
        "toolchain": {
            "rustc_stdout_sha256": sha(archive / "raw/rustc/stdout"),
            "cargo_stdout_sha256": sha(archive / "raw/cargo/stdout"),
        },
        "commands": len(commands),
    }
    if binding is not None:
        validate_binding(binding)
        need(binding == derived, "exact frozen qualification binding")
    return derived


def prepare_binding(archive=HERE):
    need(not (archive / "binding.json").exists(), "binding already exists")
    need(not (archive / "SHA256SUMS").exists(), "seal exists before binding")
    binding = derive(archive, live=True)
    with (archive / "binding.json").open("x") as output:
        json.dump(binding, output, indent=2, sort_keys=True)
        output.write("\n")
    return binding


def verify_bundle(archive=HERE, live=False):
    binding = read(archive / "binding.json")
    derived = derive(archive, binding=binding, live=live)
    return {
        "cpu_qualification": True,
        "source_base": derived["source_base"],
        "source_snapshot_sha256": derived["source_snapshot_sha256"],
        "source_files": derived["source_files"],
        "commands": derived["commands"],
        "native_execution": False,
        "formal_refinement": False,
        "performance_acceptance": False,
    }


def verify_archive(archive=HERE, *, live=False, allow_unsealed=False):
    report = verify_bundle(archive, live=live)
    if not allow_unsealed or (archive / "SHA256SUMS").exists():
        seal(archive, False)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--prepare-binding", action="store_true")
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    if args.prepare_binding:
        need(not args.live, "binding preparation already requires live equality")
        report = prepare_binding()
    else:
        report = verify_bundle(live=args.live)
        if args.seal:
            seal(HERE, True)
        elif not args.allow_unsealed or (HERE / "SHA256SUMS").exists():
            seal(HERE, False)
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
