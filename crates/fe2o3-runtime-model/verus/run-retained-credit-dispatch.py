#!/usr/bin/env python3
"""Portable, signed-source retained-credit dispatch qualification on Linux x86_64."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
CHECKER = V / "check-retained-credit-dispatch.py"
TEST = V / "test-run-retained-credit-dispatch.py"
SOURCE_TEST = V / "test-retained-credit-dispatch.py"
PROOF = V / "retained_credit_dispatch_v1.rs"
DOC = Path("docs/runtime-retained-credit-dispatch-v1.md")
CLASSIFIER = V / "check-completion-reconciliation-campaign.py"
OWNER = V / "check-journal-issuance.py"
IDENTITY = Path("docs/evidence/dev-completion-reconciliation-proof-2026-09-25/run.py")
LEAF = Path("docs/evidence/dev-completion-leaf-outcomes-2026-09-25/run.py")
CLOSURE = Path("examples/row_softmax_v1/verify-verus-closure.sh")
MANIFEST = V / "pins/VERUS_CLOSURE_MANIFEST"
PINS = {
    CLASSIFIER: "27fe721fdacd41acaf76c89c007137548fc1f674e1602b46ecde014aeacc5d84",
    OWNER: "36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480",
    IDENTITY: "9e3a1f4a81f665d9cc9f8479de0072c92e0fa3577d5b7ba1af6893d070624e76",
    LEAF: "3cf248a9d642713ea738f58b882c7ccd7c38d8b5ccc6cbfbe3dfe246b03f412c",
    CLOSURE: "c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c",
    MANIFEST: "f06883e4ce463bcb9a3c8f911064ac85054c7822dc331db1a79f75f9e8878b01",
}
SIGNER = "harmenon@amd.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICITzoV64zd4tYeZhvOi+mnwQaxEI4rFvXeC3HxileBS\n"
SIGNATURE = 'Good "git" signature for harmenon@amd.com with ED25519 key SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg\n'
CLOSURE_OK = "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n"
FLAGS = ("accepted", "source_unchanged", "tools_unchanged", "raw_unchanged", "namespace_unchanged", "trees_unchanged")
CONTRACTS = {
    "context": "out == context_observes(self, id, device, byte_len)",
    "runtime": "out == runtime_observes(self, device, credits, expected)",
    "account": "out == account_observes(self, credits, expected)",
    "domain": "out == domain_account_observes(self, root, slot, owner, expected)",
    "composed": "out == composed_observes(self, credit, bytes)",
}
SCOPE = (
    "Serialized five-body retained-credit dispatch component only. Three full41 positives and25 "
    "family-bound logical negatives; no CPU, native, concurrency/freshness, Arc/Mutex/map refinement, "
    "authority, performance or HIP/HSA parity claim. Host tools are measured with continuity, not "
    "authenticated against the historical S2 host profile. Python/system libraries, dynamic loader, "
    "kernel, procfs and Rust compiler/libraries remain trusted. Full Verus/vstd/Z3 release is pinned."
)
UTILITY_NAMES = ("git", "ssh-keygen", "timeout", "sh", "find", "sort", "stat", "wc", "sha256sum",
                 "awk", "grep", "sed", "tr", "uname", "mktemp", "rm", "rustup")
CONTROL_COUNT = 15


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    with path.open("x") as stream:
        json.dump(value, stream, sort_keys=True, indent=2)
        stream.write("\n")


def strict_json(classifier, text):
    def reject(value):
        raise ValueError("nonfinite JSON: " + value)
    return json.loads(text, object_pairs_hook=classifier.strict_object, parse_constant=reject)


def ordinary(path):
    need(path.is_absolute() and path.resolve() == path and path.is_file() and not path.is_symlink(),
         "ordinary canonical file: " + str(path))
    return path


def load(path, name, digest=None):
    ordinary(path)
    raw = path.read_bytes()
    need(digest is None or hashlib.sha256(raw).hexdigest() == digest, "pinned helper: " + str(path))
    module = types.ModuleType(name)
    module.__file__ = str(path)
    sys.modules[name] = module
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def helpers():
    for path, expected in PINS.items():
        need(sha(ordinary(ROOT / path)) == expected, "unchanged shared helper: " + str(path))
    return (load(ROOT / CHECKER, "dispatch_source"),
            load(ROOT / CLASSIFIER, "dispatch_classifier", PINS[CLASSIFIER]),
            load(ROOT / OWNER, "dispatch_owner", PINS[OWNER]),
            load(ROOT / IDENTITY, "dispatch_identity", PINS[IDENTITY]),
            load(ROOT / LEAF, "dispatch_leaf", PINS[LEAF]))


def environment(output, verus):
    home = Path.home().resolve()
    cargo = Path(os.environ.get("CARGO_HOME", home / ".cargo")).resolve()
    rustup = Path(os.environ.get("RUSTUP_HOME", home / ".rustup")).resolve()
    return {"HOME": str(home), "PATH": "/usr/bin:/bin:" + str(cargo / "bin"),
            "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "TZ": "UTC", "CARGO_HOME": str(cargo),
            "RUSTUP_HOME": str(rustup), "CARGO_NET_OFFLINE": "true", "TMPDIR": str(output / "tmp"),
            "VERUS_Z3_PATH": str(verus.parent / "z3"), "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_NO_REPLACE_OBJECTS": "1"}


def tool_paths(env):
    result = {"python": Path(sys.executable).resolve()}
    for name in UTILITY_NAMES:
        selected = shutil.which(name, path=env["PATH"])
        need(selected is not None, "required host tool: " + name)
        result[name] = Path(selected)
    toolchain = Path(env["RUSTUP_HOME"]) / "toolchains/1.97.1-x86_64-unknown-linux-gnu"
    result["rustc"] = toolchain / "bin/rustc"
    return result


def tools_inventory(paths, verus, env):
    need(tool_paths(env) == paths, "unchanged host tool search resolution")
    result = {}
    for name, path in paths.items():
        resolved = ordinary(path.resolve())
        result[name] = {"path": str(path), "resolved": str(resolved), "sha256": sha(resolved),
                        "bytes": resolved.stat().st_size, "mode": format(resolved.stat().st_mode & 0o7777, "o")}
    records = re.findall(r"^required=([^|]+)\|([^|]+)\|(\d+)\|([0-9a-f]{64})$",
                         (ROOT / MANIFEST).read_text(), re.M)
    need(len(records) == 3 and {name for name, *_ in records} == {"verus", "rust_verify", "z3"},
         "exact pinned release executables")
    for name, mode, size, digest in records:
        path = ordinary(verus.parent / name)
        need(sha(path) == digest and path.stat().st_size == int(size)
             and format(path.stat().st_mode & 0o7777, "o") == mode, "pinned release executable: " + name)
        result["release/" + name] = {"sha256": digest, "bytes": int(size), "mode": mode}
    return result


def git(paths, env, *args, data=None):
    command = [str(paths["git"]), "--no-replace-objects", "--no-pager", "-c", "gc.auto=0", *args]
    return subprocess.run(command, cwd=ROOT, env=env, input=data, capture_output=True,
                          check=True, timeout=180).stdout


def inventory(check, paths, env):
    sources = check.snapshot()
    check.audit(sources)
    need(check.EXPECTED_VERIFIED == 41 and check.MUTANT_COUNT == 25 and len(check.FILES) == 14,
         "exact component policy")
    need(git(paths, env, "status", "--porcelain=v1", "-z", "--untracked-files=all") == b"",
         "clean signed checkout required")
    head = git(paths, env, "rev-parse", "HEAD").decode().strip()
    need(re.fullmatch(r"[0-9a-f]{40}", head) is not None, "SHA1 Git commit required")
    names = {os.fsdecode(name) for name in git(paths, env, "ls-files", "--cached", "-z").split(b"\0") if name}
    names = {name for name in names if name.startswith(("crates/", "examples/", ".cargo/"))
             or name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", str(DOC), str(IDENTITY), str(LEAF))}
    need({str(p) for p in (*PINS, CHECKER, TEST, SOURCE_TEST, DOC, Path(__file__).relative_to(ROOT))} <= names,
         "all campaign dependencies tracked")
    return {"commit": head, "files": {name: {"sha256": sha(ordinary(ROOT / name)),
            "bytes": (ROOT / name).stat().st_size} for name in sorted(names)}}


def bind_blobs(paths, env, before):
    selectors = ["crates", "examples", ".cargo", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
                 str(DOC), str(IDENTITY), str(LEAF)]
    objects = {}
    for entry in git(paths, env, "ls-tree", "-r", "-z", before["commit"], "--", *selectors).split(b"\0"):
        if not entry:
            continue
        header, raw_name = entry.split(b"\t", 1)
        mode, kind, oid = header.decode().split()
        name = os.fsdecode(raw_name)
        need(mode in ("100644", "100755") and kind == "blob" and name not in objects,
             "distinct ordinary signed blob")
        objects[name] = oid
    need(objects.keys() == before["files"].keys(), "complete signed source roster")
    data = git(paths, env, "cat-file", "--batch", data="".join(oid + "\n" for oid in objects.values()).encode())
    offset, bound = 0, {}
    for name, oid in objects.items():
        end = data.index(b"\n", offset)
        actual, kind, size = data[offset:end].decode().split()
        size = int(size)
        value = data[end + 1:end + 1 + size]
        offset = end + 1 + size
        need(actual == oid and kind == "blob" and size >= 0 and len(value) == size
             and data[offset:offset + 1] == b"\n"
             and hashlib.sha1(b"blob " + str(size).encode() + b"\0" + value).hexdigest() == oid,
             "authenticated signed blob framing")
        offset += 1
        measured = {"sha256": hashlib.sha256(value).hexdigest(), "bytes": size}
        need(measured == before["files"][name], "signed bytes differ: " + name)
        bound[name] = {**measured, "git_blob": oid}
    need(offset == len(data), "no trailing Git batch data")
    return {"signed_commit": before["commit"], "expected_signer": SIGNER, "files": bound}


def sites(check):
    proof = (ROOT / PROOF).read_text().splitlines()
    result = {}
    for family, expression in CONTRACTS.items():
        contract = [i + 1 for i, line in enumerate(proof) if line.strip() == "ensures " + expression + ","]
        invocation = check.INVOCATIONS[family]
        call = [i + 1 for i, line in enumerate(proof) if check.compact(line) == invocation]
        body, macro = check.BODIES[family], invocation.split("!")[0]
        definition = [i + 1 for i, line in enumerate((ROOT / body).read_text().splitlines())
                      if line.startswith("macro_rules! " + macro + " {")]
        need(len(contract) == len(call) == len(definition) == 1, "unique contract/call/definition")
        result[family] = {"contract_line": contract[0], "contract_text": proof[contract[0] - 1],
                          "call_line": call[0], "call_text": proof[call[0] - 1], "body": str(body),
                          "macro": macro + "!", "definition_line": definition[0], "selector": check.SELECTORS[family]}
    return result


def span_on(span, path, line, text=None):
    return (isinstance(span, dict) and type(span.get("file_name")) is str
            and Path(span["file_name"]).is_absolute() and Path(span["file_name"]).resolve() == path
            and type(span.get("line_start")) is int and type(span.get("line_end")) is int
            and span["line_start"] == span["line_end"] == line
            and (text is None or (isinstance(span.get("text"), list) and len(span["text"]) == 1
                 and isinstance(span["text"][0], dict) and span["text"][0].get("text") == text)))


def body_span_matches(span, path):
    if (not isinstance(span, dict) or type(span.get("file_name")) is not str
            or not Path(span["file_name"]).is_absolute() or Path(span["file_name"]).resolve() != path
            or span.get("is_primary") is not False or type(span.get("line_start")) is not int
            or type(span.get("line_end")) is not int):
        return False
    lines = path.read_text().splitlines()
    first, last = span["line_start"], span["line_end"]
    return (1 <= first <= last <= len(lines) and isinstance(span.get("text"), list)
            and all(isinstance(row, dict) for row in span["text"])
            and [row.get("text") for row in span["text"]] == lines[first - 1:last])


def family_error_join(classifier, root, site, stderr):
    try:
        for line in stderr.splitlines():
            row = strict_json(classifier, line)
            if row.get("level") != "error" or row.get("message") != "postcondition not satisfied":
                continue
            spans = row.get("spans", [])
            contract = any(s.get("is_primary") is True and span_on(s, root / PROOF,
                           site["contract_line"], site["contract_text"]) for s in spans)
            expanded = False
            for span in spans:
                if not body_span_matches(span, root / site["body"]):
                    continue
                expansion = span.get("expansion")
                if isinstance(expansion, dict):
                    expanded |= (expansion.get("macro_decl_name") == site["macro"]
                                 and span_on(expansion.get("span"), root / PROOF, site["call_line"], site["call_text"])
                                 and span_on(expansion.get("def_site_span"), root / site["body"], site["definition_line"]))
            if contract and expanded:
                return True
        return False
    except (ValueError, TypeError, KeyError, AttributeError):
        return False


def positive(classifier, identity, check, root, status, stdout, stderr):
    try:
        strict_json(classifier, stdout)
        for line in stderr.splitlines():
            strict_json(classifier, line)
        expected = {"encountered-error": False, "encountered-vir-error": False,
                    "errors": 0, "is-verifying-entire-crate": True, "success": True, "verified": 41}
        return (type(status) is int and classifier.proof_positive(status, stdout, stderr, identity.VERIFIER,
                expected, {str(root / path) for path in check.FILES}))
    except (ValueError, TypeError, KeyError):
        return False


def negative(classifier, identity, check, leaf, root, path, focus, status, stdout, stderr):
    try:
        if type(status) is not int or status != 1:
            return False
        strict_json(classifier, stdout)
        for line in stderr.splitlines():
            strict_json(classifier, line)
        families = [name for name, body in check.BODIES.items() if body == path and check.SELECTORS[name] == focus]
        return (len(families) == 1 and classifier.logical_negative(check.selection_notes(leaf, focus),
                status, stdout, stderr, identity.VERIFIER, {str(root / p) for p in check.FILES})
                and family_error_join(classifier, root, sites(check)[families[0]], stderr))
    except (ValueError, TypeError, KeyError):
        return False


def mutations(check):
    rows = check.mutations(check.snapshot())
    need(len(rows) == len(set(rows.values())) == 25 and {p for p, _b, _s in rows.values()} == set(check.BODIES.values()),
         "all25 distinct actual five-body mutations")
    return rows


def stages(mutants):
    return ("00-signature", "01-controls", "02-source-calibration", "03-release-before",
            "04-positive-before", "05-relocated-positive",
            *[f"{i:02d}-negative-{name}" for i, name in enumerate(mutants, 6)],
            "31-positive-after", "32-release-after")


def command(paths, verus, root, focus=None):
    return [str(paths["timeout"]), "--foreground", "--signal=TERM", "--kill-after=5", "120", str(verus),
            "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating", "--output-json", "--error-format=json",
            "--no-report-long-running", "--num-threads", "4", "--multiple-errors", "1",
            *([] if focus is None else ["--verify-function", focus, "--verify-root"]), str(root / PROOF)]


def tree(root):
    need(root.is_dir() and not root.is_symlink() and root.resolve() == root, "ordinary generated source root")
    result = {}
    for path in sorted(root.rglob("*")):
        need(not path.is_symlink() and path.resolve() == path, "no generated source symlink")
        if path.is_file():
            result[str(path.relative_to(root))] = sha(path)
        else:
            need(path.is_dir(), "ordinary generated source entry")
    return result


def generated_tree_valid(expected, measured, mutation):
    if mutation is None:
        return measured == expected
    path, body = mutation
    digest = hashlib.sha256(body.encode()).hexdigest()
    return str(path) in expected and digest != expected[str(path)] and measured == {**expected, str(path): digest}


def namespace_identity():
    identity = {"recorder_pid": os.getpid(), "procfs_self_pid": os.readlink("/proc/self"),
                "pid_namespace": os.readlink("/proc/self/ns/pid"),
                "child_pid_namespace": os.readlink("/proc/self/ns/pid_for_children")}
    need(identity["procfs_self_pid"] == str(identity["recorder_pid"])
         and re.fullmatch(r"pid:\[\d+\]", identity["pid_namespace"]) is not None
         and identity["pid_namespace"] == identity["child_pid_namespace"], "consistent live child PID namespace")
    return identity


def independent_absence(output, namespace, attempted, classifier):
    need(namespace_identity() == namespace, "unchanged census namespace")
    groups = []
    # Only this invocation's explicit attempted roster is admissible, never historical records.
    for name in attempted:
        record = strict_json(classifier, (output / name / "owned/record.json").read_text())
        group = record.get("process_group")
        need(type(group) is int and group > 1 and group not in groups, "distinct fresh recorded PGID")
        groups.append(group)
    members, uncertain = [], []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            raw = (entry / "stat").read_text()
            fields = raw[raw.rindex(")") + 2:].split()
            if int(fields[2]) in groups:
                members.append({"pid": int(entry.name), "pgid": int(fields[2]), "sid": int(fields[3])})
        except FileNotFoundError:
            continue
        except (OSError, ValueError, IndexError) as error:
            uncertain.append({"pid": entry.name, "error": str(error)})
    need(namespace_identity() == namespace, "unchanged namespace after census")
    return {"recorded_groups": sorted(groups), "members": members, "uncertain": uncertain,
            "all_recorded_groups_absent": not members and not uncertain,
            "namespace": namespace, "host_wide_absence_claimed": False, "historical_groups_probed": False}


def census_valid(value, attempted):
    groups = value.get("recorded_groups")
    return (type(groups) is list and len(groups) == len(set(groups)) == len(attempted) > 0
            and all(type(group) is int and group > 1 for group in groups)
            and value.get("all_recorded_groups_absent") is True and value.get("members") == []
            and value.get("uncertain") == [] and value.get("host_wide_absence_claimed") is False
            and value.get("historical_groups_probed") is False)


def receipt_valid(record, argv, status):
    return (type(status) is int and record.get("command") == argv and type(record.get("status")) is int
            and record["status"] == status and record.get("group_absent") is True
            and type(record.get("process_group")) is int and record["process_group"] > 1
            and "exception" not in record)


def complete(rows, expected, counts):
    return (len(expected) == len(set(expected)) == 33 and [row.get("name") for row in rows] == list(expected)
            and all(all(row.get(flag) is True for flag in FLAGS) for row in rows)
            and counts == {"full_positive": 3, "logical_negative": 25})


def arguments(argv):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--campaign", action="store_true", required=True)
    parser.add_argument("--verus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    ordinary(args.verus)
    need(args.verus.name == "verus", "named verifier executable")
    need(args.output.is_absolute() and args.output.resolve() == args.output and not args.output.exists(),
         "fresh canonical absolute output")
    need(not args.output.is_relative_to(ROOT) and not ROOT.is_relative_to(args.output)
         and not args.output.is_relative_to(args.verus.parent) and not args.verus.parent.is_relative_to(args.output),
         "output disjoint from source and verifier")
    need(args.output.parent.is_dir(), "existing output parent")
    return args


def campaign(args):
    check, classifier, owner, identity, leaf = helpers()
    env = environment(args.output, args.verus)
    paths = tool_paths(env)
    tools_before = tools_inventory(paths, args.verus, env)
    before = inventory(check, paths, env)
    binding = bind_blobs(paths, env, before)
    mutants = mutations(check)
    expected = stages(mutants)
    proof_closure = {str(path): sha(ROOT / path) for path in check.FILES}
    raw = {ROOT / path: sha(ROOT / path) for path in (*PINS, CHECKER, TEST, SOURCE_TEST, DOC,
                                                    Path(__file__).relative_to(ROOT))}
    output = args.output
    output.mkdir()
    (output / "tmp").mkdir()
    signer = output / "allowed-signers"
    signer.write_text(SIGNER)
    raw[signer] = sha(signer)
    namespace = namespace_identity()
    for name, value in (("source-before", before), ("tools-before", tools_before), ("namespace-before", namespace),
                        ("signed-inputs", binding), ("environment", env),
                        ("raw-pins", {str(p): h for p, h in raw.items()}),
                        ("plan", {"scope": SCOPE, "stages": expected, "proof_closure": proof_closure,
                                  "contract_sites": sites(check), "solver_attempts": 28,
                                  "mutations": {name: {"path": str(path), "selector": focus,
                                      "sha256": check.sha(body)} for name, (path, body, focus) in mutants.items()}})):
        save(output / (name + ".json"), value)
    rows, attempted, errors, generated, raw_results = [], [], [], {}, {}
    counts = {"full_positive": 0, "logical_negative": 0}
    closure = [str(paths["sh"]), str(ROOT / CLOSURE), str(args.verus.parent), str(ROOT / MANIFEST)]

    def raw_unchanged():
        return all(path.is_file() and sha(path) == digest for path, digest in {**raw, **raw_results}.items())

    def continuity():
        return {"source_unchanged": inventory(check, paths, env) == before,
                "tools_unchanged": tools_inventory(paths, args.verus, env) == tools_before,
                "raw_unchanged": raw_unchanged(), "namespace_unchanged": namespace_identity() == namespace,
                "trees_unchanged": all(tree(root) == value for root, value in generated.items())}

    def freeze_tree(root, mutation=None):
        root.mkdir()
        for name in proof_closure:
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(mutation[1].encode() if mutation is not None and name == str(mutation[0])
                               else (ROOT / name).read_bytes())
        measured = tree(root)
        need(generated_tree_valid(proof_closure, measured, mutation), "exact generated fourteen-file tree")
        generated[root] = measured

    def stage(name, argv, classify, seconds=130, closing=False):
        if closing:
            need(raw_unchanged() and tools_inventory(paths, args.verus, env) == tools_before
                 and namespace_identity() == namespace, "closing release tool/script/namespace identity")
        else:
            need(all(continuity().values()), "pre-stage continuity")
        folder = output / name
        folder.mkdir()
        save(folder / "command.json", {"argv": argv, "cwd": str(ROOT), "timeout_seconds": seconds})
        attempted.append(name)
        if "-negative-" in name:
            counts["logical_negative"] += 1
        elif name in (expected[4], expected[5], expected[-2]):
            counts["full_positive"] += 1
        print("START " + name, flush=True)
        status, stdout, stderr = owner.run_owned(argv, seconds, folder / "owned", env)
        record = strict_json(classifier, (folder / "owned/record.json").read_text())
        accepted = (receipt_valid(record, argv, status) and (folder / "owned/stdout.log").read_text() == stdout
                    and (folder / "owned/stderr.log").read_text() == stderr and classify(status, stdout, stderr))
        for path in (folder / "command.json", *(folder / "owned" / p for p in ("record.json", "stdout.log", "stderr.log"))):
            raw_results[path] = sha(path)
        row = {"name": name, "status": status, "accepted": bool(accepted), **continuity()}
        rows.append(row)
        save(folder / "result.json", row)
        raw_results[folder / "result.json"] = sha(folder / "result.json")
        print("END " + name + " accepted=" + str(accepted), flush=True)
        need(all(row[key] is True for key in FLAGS), "rejected campaign stage: " + name)

    def exact(text):
        return lambda status, stdout, stderr: type(status) is int and status == 0 and stdout == text and not stderr

    previous_cwd = Path.cwd()
    previous_handlers = {number: signal.getsignal(number) for number in owner.SIGNALS}
    try:
        for number in previous_handlers:
            signal.signal(number, owner.interrupted)
        os.chdir(ROOT)
        freeze_tree(output / "relocated")
        for name, (path, body, _focus) in mutants.items():
            freeze_tree(output / (name + "-source"), (path, body))
        save(output / "generated-before.json", {str(path): value for path, value in generated.items()})
        stage(expected[0], [str(paths["git"]), "--no-replace-objects", "-c", "gc.auto=0", "-c", "gpg.format=ssh",
              "-c", "gpg.ssh.program=" + str(paths["ssh-keygen"]), "-c", "gpg.ssh.allowedSignersFile=" + str(signer),
              "verify-commit", before["commit"]],
              lambda s, o, e: type(s) is int and s == 0 and not o and e == SIGNATURE, 60)
        stage(expected[1], [str(paths["python"]), "-I", "-B", str(ROOT / TEST)],
              lambda s, o, e: type(s) is int and s == 0 and not o and e.endswith("\nOK\n")
              and re.findall(r"^Ran (\d+) tests in [0-9.]+s$", e, re.M) == [str(CONTROL_COUNT)], 60)
        stage(expected[2], [str(paths["python"]), "-I", "-B", str(ROOT / SOURCE_TEST)],
              exact("retained dispatch calibration: 4 groups passed; 25 logical mutants constructed, not executed\n"), 60)
        stage(expected[3], closure, exact(CLOSURE_OK))
        for name, root in ((expected[4], ROOT), (expected[5], output / "relocated")):
            stage(name, command(paths, args.verus, root),
                  lambda s, o, e, root=root: positive(classifier, identity, check, root, s, o, e))
        for index, (name, (path, _body, focus)) in enumerate(mutants.items(), 6):
            root = output / (name + "-source")
            stage(expected[index], command(paths, args.verus, root, focus),
                  lambda s, o, e, root=root, path=path, focus=focus:
                  negative(classifier, identity, check, leaf, root, path, focus, s, o, e))
        stage(expected[-2], command(paths, args.verus, ROOT),
              lambda s, o, e: positive(classifier, identity, check, ROOT, s, o, e))
    except BaseException as error:
        errors.append(type(error).__name__ + ": " + str(error))
    finally:
        try:
            if expected[3] in attempted:
                try:
                    stage(expected[-1], closure, exact(CLOSURE_OK), closing=True)
                except BaseException as error:
                    errors.append("closing release: " + type(error).__name__ + ": " + str(error))
            closing = {}
            for label, action in (("source", lambda: inventory(check, paths, env)),
                    ("tools", lambda: tools_inventory(paths, args.verus, env)), ("raw", raw_unchanged),
                    ("namespace", namespace_identity),
                    ("census", lambda: independent_absence(output, namespace, attempted, classifier)),
                    ("trees", lambda: {str(root): tree(root) for root in generated})):
                try:
                    closing[label] = action()
                    save(output / (label + "-after.json"), closing[label])
                except BaseException as error:
                    errors.append(label + ": " + type(error).__name__ + ": " + str(error))
            accepted = (not errors and complete(rows, expected, counts) and closing.get("source") == before
                        and closing.get("tools") == tools_before and closing.get("raw") is True
                        and closing.get("namespace") == namespace and census_valid(closing.get("census", {}), attempted)
                        and closing.get("trees") == {str(root): value for root, value in generated.items()})
            result = {"accepted": accepted, "scope": SCOPE, "stages": rows, "errors": errors,
                      "attempted_stages": attempted, "solver_attempts": counts,
                      "fresh_groups_closed": census_valid(closing.get("census", {}), attempted),
                      "historical_groups_probed": False, "signed_commit": before["commit"],
                      "signature_accepted": bool(rows and rows[0]["name"] == expected[0] and rows[0]["accepted"])}
            save(output / "raw-results.json", {str(path): digest for path, digest in raw_results.items()})
            save(output / "results.json", result)
            print(json.dumps({k: v for k, v in result.items() if k != "stages"}, indent=2), flush=True)
        finally:
            try:
                os.chdir(previous_cwd)
            finally:
                for number, handler in previous_handlers.items():
                    signal.signal(number, handler)
    return 0 if accepted else 1


def main(argv=None):
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B without -O")
    return campaign(arguments(sys.argv[1:] if argv is None else argv))


if __name__ == "__main__":
    raise SystemExit(main())
