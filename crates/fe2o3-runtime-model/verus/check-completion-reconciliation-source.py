#!/usr/bin/env python3
"""Allowlisted syntactic correspondence, not a semantic-equivalence proof."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
BASELINE = "53aba0d658f0d874f3c4f7ef18ff35f971504375"
BODY = Path("crates/fe2o3-runtime/src/context/completion_reconciliation_body.rs")
CONTEXT = BODY.with_name("peer_reconciliation.rs")
BASELINE_PINS = {BODY: "cf368c97bc902b533f86eea327286521788055253e7a26607ed4210fa78ddd9f",
                 CONTEXT: "0edf452b5e54a3b1408367d86f1fe5bce4490584d449f728a7a8d4adaff7e485"}
GIT = Path("/usr/bin/git")
RUSTFMT = Path("/home/harsh/.rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu/bin/rustfmt")
TOOL_PINS = {GIT: "2a8c18fbf43da9f692d75474c72bea9dfd796c260b0f3dfe456376abc3bbd668",
             RUSTFMT: "a9137d0c198ceb6c72193d517d3c9007b3ec7a90d3d10ec6889773eca48261b4"}
TOOL_ENV = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C", "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_NO_REPLACE_OBJECTS": "1"}
COMMENT = "// One executable planner body; custody, journal and settlement effects stay in its adapters.\n"
OLD_PREFIX = COMMENT + """macro_rules! completion_reconciliation_body {
    ($syntax:ident, $context:ident, $requested:ident, [$($on_step:tt)*]) => {
        $syntax!({
"""
PREFIX = COMMENT + """macro_rules! completion_reconciliation_body {
    ($syntax:ident, $context:ident, $requested:ident, [$($on_step:tt)*]) => {
        completion_reconciliation_body!(@annotated $syntax, $context, $requested,
            path, length, id, validated, remaining, record, dependencies, state, dependency,
            [], [$($on_step)*], [], [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $context:ident, $requested:ident,
     $path:ident, $length:ident, $id:ident, $validated:ident, $remaining:ident,
     $record:ident, $dependencies:ident, $state:ident, $dependency:ident,
     [$($invariants:tt)*], [$($on_step:tt)*], [$($after_validation:tt)*],
     [$($after_pop:tt)*], [$($after_advance:tt)*], [$($after_descend:tt)*],
     [$($after_settlement:tt)*], [$($before_dependency:tt)*], [$($before_input:tt)*]) => {
        $syntax!({
"""
SUFFIX = "\n        })\n    };\n}\n"
LOCALS = ("path", "length", "id", "validated", "remaining", "record", "dependencies", "state", "dependency")
HOOKS = ("invariants", "on_step", "after_validation", "after_pop", "after_advance", "after_descend",
         "after_settlement", "before_dependency", "before_input")
INVOCATION = "completion_reconciliation_body!(completion_settlement_rust_expr, self, requested, [])"
HOOK_SITES = (
    "while $remaining > 0\n                $($invariants)*\n            {",
    "$remaining -= 1;\n                $($on_step)*\n                let $record =",
    "$validated = Some($id);\n                }\n                $($after_validation)*\n                if $record.status.is_terminal()",
    "$id = $path[$length];\n                    $validated = None;\n                    $($after_pop)*\n                    continue;",
    ".cursor += 1;\n                            $($after_advance)*\n                        }",
    "$id = $dependency.submission;\n                            $validated = None;\n                            $($after_descend)*\n                        }",
    "let $dependency = *$dependency;\n                    $($before_dependency)*\n                    match",
    "$($before_input)*\n                match $context.directed_input_status_v1($id)?",
)


def need(value, message):
    if not value:
        raise ValueError(message)


def authenticate(path):
    need(path.is_file() and not path.is_symlink(), "ordinary tool executable")
    need(hashlib.sha256(path.read_bytes()).hexdigest() == TOOL_PINS[path], "tool identity: " + str(path))


def git(*args):
    authenticate(GIT)
    result = subprocess.check_output([str(GIT), "--no-replace-objects", "--no-pager", "-c", "core.fsmonitor=false",
                                      "-c", "core.hooksPath=/dev/null", *args], cwd=ROOT, env=TOOL_ENV)
    authenticate(GIT)
    return result


def baseline(path):
    data = git("show", BASELINE + ":" + str(path))
    need(hashlib.sha256(data).hexdigest() == BASELINE_PINS[path], "authenticated baseline blob")
    return data.decode()


def once(text, old, new):
    need(text.count(old) == 1, "unique syntax: " + old)
    return text.replace(old, new)


def unwrap(text, prefix):
    need(text.startswith(prefix) and text.endswith(SUFFIX), "closed macro wrapper")
    return text[len(prefix):-len(SUFFIX)]


def erase_hook(text, hook, count):
    token = "$($" + hook + ")*"
    need(text.count(token) == count, "hook multiplicity: " + hook)
    text, erased = re.subn(r"(?m)^ *" + re.escape(token) + r"\n", "", text)
    need(erased == count, "standalone annotation: " + hook)
    return text


def identifiers(text, names):
    def replace(match):
        name = match[1]
        need(name in names, "closed macro identifier: " + name)
        return "self" if name == "context" else name
    text = re.sub(r"\$([a-z_]+)", replace, text)
    need("$" not in text, "no residual macro syntax")
    return text


def previous_body(text):
    return identifiers(erase_hook(unwrap(text, OLD_PREFIX), "on_step", 1), ("context", "requested"))


def normalized_body(text):
    text = unwrap(text, PREFIX)
    for site in HOOK_SITES:
        need(text.count(site) == 1, "exact annotation position: " + site)
    need(len(re.findall(r"\);\n( +)\$\(\$after_settlement\)\*\n\1result\?;", text)) == 3,
         "settlement annotation before each Result propagation")
    for hook in HOOKS:
        text = erase_hook(text, hook, 3 if hook == "after_settlement" else 1)
    text = identifiers(text, (*LOCALS, "context", "requested"))
    text = once(text, "let mut length = 0usize;", "let mut length = 0;")
    text = once(text, "            let mut remaining = 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1;\n", "")
    text = once(text, "while remaining > 0\n            {\n                remaining -= 1;",
                "for _ in 0..(2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1) {")
    need(not re.search(r"\bremaining\b", text), "closed fuel use")
    text = once(text, "if let Some(dependency) = dependencies.get(state.cursor) {\n"
                "                    let dependency = *dependency;",
                "if let Some(dependency) = dependencies.get(state.cursor).copied() {")
    for indent in (" " * 28, " " * 24):
        call = ("self.transition_submission_status(\n" + indent + "    id,\n" + indent
                + "    RuntimeCompletionStatusV1::QuiescentWithoutResult,\n" + indent + ")")
        text = once(text, "let result = " + call + ";\n" + indent + "result?;", call + "?;")
    call = "self.transition_submission_status(id, RuntimeCompletionStatusV1::Succeeded)"
    text = once(text, "let result = " + call + ";\n                        result?;", call + "?;")
    need(text.count("id: id,") == 2, "two explicit Observe identities")
    return text.replace("id: id,", "id,")


def canonical(text):
    source = "impl Witness { fn plan(&mut self, requested: RuntimeSubmissionIdV1) -> Result<CompletionStepV1, RuntimeValidationErrorV1> {\n" + text + "\n} }\n"
    authenticate(RUSTFMT)
    result = subprocess.run([str(RUSTFMT), "--edition", "2024", "--emit", "stdout", "--config-path", "/dev/null"], input=source,
                            text=True, capture_output=True, check=True, cwd=ROOT, timeout=30, env=TOOL_ENV)
    authenticate(RUSTFMT)
    need(not result.stderr, "clean Rustfmt parser")
    return result.stdout


def compare(previous, candidate, context, baseline_context):
    need(hashlib.sha256(previous.encode()).hexdigest() == BASELINE_PINS[BODY], "baseline macro bytes")
    need(hashlib.sha256(baseline_context.encode()).hexdigest() == BASELINE_PINS[CONTEXT], "baseline context bytes")
    need(context == baseline_context, "unchanged production invocation and adapters")
    need(context.count(INVOCATION) == 1, "one empty-hook production invocation")
    before, after = canonical(previous_body(previous)), canonical(normalized_body(candidate))
    need(before == after, "non-allowlisted planner change")
    return before, after


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    previous, candidate = baseline(BODY), (ROOT / BODY).read_text()
    before, after = compare(previous, candidate, (ROOT / CONTEXT).read_text(), baseline(CONTEXT))
    args.output.mkdir(parents=True, exist_ok=False)
    artifacts = {"baseline-macro.rs": previous, "candidate-macro.rs": candidate,
                 "normalized-before.rs": before, "normalized-after.rs": after}
    for name, text in artifacts.items():
        (args.output / name).write_text(text)
    record = dict(baseline=BASELINE, statement=__doc__, normalized_equal=True,
                  sha256={name: hashlib.sha256(text.encode()).hexdigest() for name, text in artifacts.items()},
                  executable_sha256={str(path): digest for path, digest in TOOL_PINS.items()},
                  transformations=["closed annotations/identifiers", "usize literal", "explicit bounded fuel",
                                   "immediate dependency dereference", "three Result bindings", "two explicit identity fields"])
    (args.output / "comparison.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    print("PASS: allowlisted completion planner syntax correspondence")


if __name__ == "__main__":
    main()
