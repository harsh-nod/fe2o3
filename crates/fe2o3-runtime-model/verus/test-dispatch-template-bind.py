#!/usr/bin/env python3
"""Calibrate exact binder joins, declared adapters and logical classifications."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import re
import runpy
import types

r = runpy.run_path(str(Path(__file__).with_name("check-dispatch-template-bind.py")))
need, root = r["need"], r["ROOT"]
inputs = {path: (root / path).read_text() for path in r["FILES"]}


def rejects(operation):
    try:
        operation()
    except ValueError:
        return
    raise ValueError("hostile binder calibration accepted")


def canonical_with_includes(source):
    protected, includes = source, {}
    for index, include in enumerate(re.findall(r'include!\("[^"]+"\);', source)):
        marker = "__roster_test_include_" + str(index) + "__"
        need(marker not in source, "reserved canonical include marker")
        protected = protected.replace(include, marker)
        includes[marker] = include
    code = r["compact"](protected)
    for marker, include in includes.items():
        code = code.replace(marker, include)
    return code


r["audit"](inputs)
lint = "#![allow(unused_macros)]"
for replacement in ("", "#![allow(warnings)]", lint + lint, "/* " + lint + " */",
                    "#![expect(unused_macros)]"):
    rejects(lambda: r["audit"]({**inputs, r["PROOF"]: inputs[r["PROOF"]].replace(lint, replacement)}))
rejects(lambda: r["audit"]({**inputs, r["PROOF"]: inputs[r["PROOF"]] + "\n#[allow(unused_variables)] fn ignored() {}"}))
for path in inputs:
    for suffix in ('\nassume(false);', '\n#[verifier::external_body]', '\nmod foreign;',
                   '\ninclude!("/foreign.rs");', '\ninclude_str!("/foreign.rs");',
                   '\ninclude_bytes!("/foreign.rs");', '\nenv!("FOREIGN");', '\noption_env!("FOREIGN");'):
        rejects(lambda: r["audit"]({**inputs, path: inputs[path] + suffix}))
    rejects(lambda: r["audit"]({key: text for key, text in inputs.items() if key != path}))
rejects(lambda: r["audit"]({**inputs, Path("/foreign.rs"): ""}))
for path, edges in r["EDGES"].items():
    for statement, _ in edges:
        for replacement in ("", statement + statement, statement.replace(".rs", "-foreign.rs"),
                            "/* " + statement + " */", "// " + statement + "\n", 'r###"' + statement + '"###',
                            "#[cfg(any())]\n" + statement, "#[cfg_attr(all(), cfg(any()))]\n" + statement):
            rejects(lambda: r["audit"]({**inputs, path: inputs[path].replace(statement, replacement)}))

binding = (root / "crates/fe2o3-kfd/src/queue_dispatch_binding.rs").read_text()
completion = (root / "crates/fe2o3-kfd/src/queue_completion.rs").read_text()
body = inputs[r["BODY"]]
r["binder_wiring"](binding, body)
guard = r["CONDITIONAL_GUARD"]
canonical = canonical_with_includes(binding)
exact_guard = r["compact"](guard)
need(canonical.count(exact_guard) == 1, "one actual conditional-fill precheck")
for before, after in (
    (guard, ""),
    ("&self.conditional_fill", "&other.conditional_fill"),
    ("self.packets.iter().any(|packet| packet.conditional_fill)", "false"),
    ("self.packets.iter().any(|packet| packet.conditional_fill)", "true"),
    ("(Some(premises), true) => premises.revalidate(self)?", "(Some(_), true) => {}"),
    ("premises.revalidate(self)?", "premises.revalidate(other)?"),
    ("(None, false) => {}", "(None, _) => {}"),
    ("_ => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase)", "_ => {}"),
):
    before, after = r["compact"](before), r["compact"](after)
    need(exact_guard.count(before) == 1, "conditional guard refusal-control site")
    rewritten_guard = exact_guard.replace(before, after)
    need(rewritten_guard != exact_guard, "changed rejecting precheck")
    rejects(lambda: r["binder_wiring"](canonical.replace(exact_guard, rewritten_guard), body))
call = "dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)"
call = r["compact"](call)
need(canonical.count(call) == 1, "exact reached shared-body site")
without_guard = canonical.replace(exact_guard, "")
rejects(lambda: r["binder_wiring"](without_guard.replace(call, call + ";" + exact_guard), body))
rejects(lambda: r["binder_wiring"](without_guard + "fnunrelated(){" + exact_guard + "}", body))
conditional_source = (root / r["CONDITIONAL_SOURCE"]).read_text()
r["conditional_guard_wiring"](binding, conditional_source)
readonly_sources = {path: (root / path).read_text() for path in r["CONDITIONAL_READONLY_SOURCES"]}
r["conditional_readonly_sources"](readonly_sources)
for path in readonly_sources:
    rejects(lambda: r["conditional_readonly_sources"]({p: s for p, s in readonly_sources.items() if p != path}))
    rejects(lambda: r["conditional_readonly_sources"]({**readonly_sources, path: readonly_sources[path] + "\nfn effect() {}"}))
rejects(lambda: r["conditional_readonly_sources"]({**readonly_sources, Path("/foreign.rs"): ""}))
for owner, target, declaration, name in r["CONDITIONAL_MODULE_EDGES"]:
    need(target in readonly_sources, "full concrete cohort branch source is retained")
    for replacement in (
        "", declaration + declaration, declaration.replace(".rs", "-foreign.rs"),
        "/* " + declaration + " */", "// " + declaration.replace("\n", "\n// "),
        'r###"' + declaration + '"###', "#[cfg(any())]\n" + declaration,
        "#[cfg_attr(all(), cfg(any()))]\n" + declaration,
        "fn unrelated() { " + declaration + " }",
    ):
        changed = readonly_sources[owner].replace(declaration, replacement)
        rejects(lambda: r["conditional_cohort_wiring"]({**readonly_sources, owner: changed}))
    for suffix in ("\nmod " + name + " {}", "\nuse other as " + name + ";"):
        rejects(lambda: r["conditional_cohort_wiring"](
            {**readonly_sources, owner: readonly_sources[owner] + suffix}))
    rejects(lambda: r["conditional_cohort_wiring"](
        {p: s for p, s in readonly_sources.items() if p != target}))
for replacement in ("", "#[cfg(any())]\n", "// "):
    declaration = '#[path = "queue_dispatch_binding/conditional_fill.rs"]\nmod conditional_fill;'
    changed = binding.replace(declaration, replacement + declaration if replacement else "")
    rejects(lambda: r["conditional_guard_wiring"](changed, conditional_source))
for suffix in ("\nmod conditional_fill {}", "\nuse other as conditional_fill;"):
    rejects(lambda: r["conditional_guard_wiring"](binding + suffix, conditional_source))
for suffix in ("\nimpl Drop for ConditionalFillStorageV1 { fn drop(&mut self) {} }",
               "\nuse std::cell::Cell;", "\nfn injected_effect() {}"):
    rejects(lambda: r["conditional_guard_wiring"](binding, conditional_source + suffix))
r["hash_wiring"](completion)
r["hash_wiring"](canonical_with_includes(completion))
for statement in ("use core::hash::{Hash, Hasher};", "use sha2::{Digest, Sha256};"):
    need(completion.count(statement) >= 1, "trusted Hash import mutation site")
    for replacement in (statement.replace("core::hash", "crate::other").replace("sha2", "crate::other"),
                        "#[cfg(any())]\n" + statement, "fn unrelated() { " + statement + " }",
                        "/* " + statement + " */", statement + statement,
                        statement.replace("use ", "pub use ")):
        rejects(lambda: r["hash_wiring"](completion.replace(statement, replacement, 1)))
for shadow in ("use crate::other::{Hash};", "use crate::other::Other as Hasher;", "use crate::other::*;",
               "use crate::other::{sha2};", "use crate::other::Other as core;",
               "type Sha256 = Other;", "struct Digest;", "trait Hash {}", "mod sha2 {}", "mod core {}",
               "macro_rules! Hash { () => {} }", "extern crate other as sha2;"):
    rejects(lambda: r["hash_wiring"](completion + "\n" + shadow))
for old, new in (
    ("dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)", "Ok(unimplemented!())"),
    ("dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)", "dispatch_bind_templates_body!(dispatch_rust_expr, self, N, other)"),
    ("pub(super) fn bind_templates<const N: usize>", "#[cfg(any())]\npub(super) fn bind_templates<const N: usize>"),
    ("completion_template_dispatch_roster_v1,", "wrong_roster as completion_template_dispatch_roster_v1,"),
):
    need(old in binding, "production binder mutation site")
    rejects(lambda: r["binder_wiring"](binding.replace(old, new), body))
rejects(lambda: r["binder_wiring"](binding + "\nmacro_rules! dispatch_bind_templates_body { () => { 0 } }", body))
canonical_binding = canonical_with_includes(binding)
import_match = re.search(r"usesuper::completion::\{[^}]+\};", canonical_binding)
need(import_match is not None, "real sibling import mutation site")
actual_import = import_match.group()
for replacement in ("#[cfg(any())]" + actual_import, "fnunrelated(){" + actual_import + "}"):
    rejects(lambda: r["binder_wiring"](canonical_binding.replace(actual_import, replacement), body))
for old, new in (
    ("packet_count: values.len(),", "packet_count: 1,"),
    ("|template| template.generations()", "|template| template.generations().clone()"),
    ("completion_hash_roster_body!(completion_rust_expr, values, project, hasher)",
     "completion_hash_roster_body!(completion_rust_expr, values, other, hasher)"),
):
    need(old in completion, "projected Hash mutation site")
    rejects(lambda: r["hash_wiring"](completion.replace(old, new)))
for name, expected in r["HASH_WRAPPERS"].items():
    code = canonical_with_includes(completion)
    expected = r["compact"](expected)
    need(expected in code, "active Hash wrapper calibration")
    for replacement in ("WRONG", "/* " + expected + " */ WRONG", 'r###"' + expected + '"### WRONG',
                        "#[cfg(any())]" + expected, "fnunrelated(){" + expected + "}"):
        rejects(lambda: r["hash_wiring"](code.replace(expected, replacement)))
    rejects(lambda: r["hash_wiring"](code + expected))
for expected in (
    "structCompletionOccurrenceHasherV1(Sha256);",
    "implHasherforCompletionOccurrenceHasherV1{fnfinish(&self)->u64{0}fnwrite(&mutself,bytes:&[u8]){self.0.update(bytes);}}",
    "pub(super)constfngenerations(self)->CompletionDispatchGenerationBindingV1{self.generations}",
):
    need(code.count(expected) == 1, "concrete adapter ownership mutation site")
    for replacement in ("#[cfg(any())]" + expected, "fnunrelated(){" + expected + "}"):
        rejects(lambda: r["hash_wiring"](code.replace(expected, replacement)))
    if expected.startswith("pub(super)constfngenerations"):
        rejects(lambda: r["hash_wiring"](code.replace(expected, "") + expected))

# Reuse the existing field-complete preflight/schema joins, including the
# explicit transparent numeric-ID projection and opaque non-Copy credit framing.
guard_path = root / r["V"] / "test-dispatch-template-preflight.py"
guard_raw = guard_path.read_bytes()
need(hashlib.sha256(guard_raw).hexdigest() ==
     "954f1a51be2f6d593c35470940328ae20aae3ac27a96e76438c485235fc49cac", "authenticated inherited preflight calibration")
guard = types.ModuleType("binder_preflight_calibration")
guard.__file__ = str(guard_path)
captured = io.StringIO()
with contextlib.redirect_stdout(captured):
    exec(compile(guard_raw, guard.__file__, "exec"), guard.__dict__)
need(captured.getvalue() == "PASS: dispatch template preflight calibration (5 groups)\n", "inherited exact preflight joins")
for name in ("CompletionPacketTemplateV1", "CompletionDispatchGenerationBindingV1"):
    need(guard.shape(completion, name) == guard.shape(inputs[r["PROOF"]], name), "full composed template schema")
need(guard.base.variants(guard.shape(inputs[r["CANCEL"]], "Gfx942CompletionErrorV1")) ==
     {"ZeroPacketCount", "StaleBatchGeneration"}, "exact reachable completion refusals")
need(guard.base.variants(guard.shape(inputs[r["CANCEL"]], "Gfx942CompletionErrorV1")) <=
     guard.base.variants(guard.shape(completion, "Gfx942CompletionErrorV1")), "real completion error variants")
for path, names in ((guard.base.sources["identity"], ("DeviceKeyV1", "VmKeyV1", "QueueKeyV1")),
                    (guard.base.sources["memory"], ("MemoryAllocationKeyV1", "MemoryMappingKeyV1"))):
    code = r["compact"](path)
    for name in names:
        declaration = "#[derive(Clone,Copy,Debug,Eq,Hash,Ord,PartialEq,PartialOrd)]pubstruct" + name + "{"
        need(code.count(declaration) == 1, "derived nested identity Hash remains in field order")

cases = r["mutations"](body)
need(len(cases) == 12, "exact binder mutation count")
for changed, selector in cases.values():
    need(changed != body and selector == "*bind_templates", "actual binder executable mutant")
    rejects(lambda: r["binder_wiring"](binding, changed))
rejects(lambda: r["mutations"](body * 2))
roster_body = inputs[r["ROSTER_BODY"]]
trait_hash = "core::hash::Hash::hash(&$value, $hasher)"
need(roster_body.count(trait_hash) == 2, "both trusted Hash adapters use anchored trait dispatch")
for index in range(2):
    before, after = roster_body.split(trait_hash, index + 1)[:index + 1], roster_body.split(trait_hash, index + 1)[index + 1]
    changed = trait_hash.join(before) + "$value.hash($hasher)" + after
    rejects(lambda: r["hash_wiring"](completion, changed))
# With trait UFCS this unrelated inherent method cannot replace the roster feed.
r["hash_wiring"](completion + "\nimpl CompletionDispatchGenerationBindingV1 { fn hash(&self, _: &mut impl Hasher) {} }\n")
roster_cases = r["mutations"](roster_body, True)
need(len(roster_cases) == 12, "exact roster control/feed mutation count")
for changed, selector in roster_cases.values():
    need(changed != roster_body and selector == "*hash_completion_dispatch_roster_projected_v1", "actual shared roster mutant")
    rejects(lambda: r["hash_wiring"](completion, changed))
rejects(lambda: r["mutations"](roster_body * 2, True))

campaign = r["campaign"]()
need(campaign.MULTIPLE_ERRORS == "1", "exact increased diagnostic enumeration without classifier relaxation")
classifier, verifier = campaign.inherited(), {"version": "calibration-only"}
leaf = classifier.inherited()
path = "/snapshot/dispatch_template_bind_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 0}}
error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
         "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
for selector in ("*bind_templates", "*hash_completion_dispatch_roster_projected_v1"):
    notes = r["selection_notes"](leaf, selector)
    check = lambda messages: classifier.logical_negative(notes, 1, json.dumps(result),
        "\n".join(json.dumps(item) for item in messages), verifier, {path})
    for message in notes.SELECTION_NOTES:
        note = dict(error, level="note", message=message, spans=[])
        need(check([note, error]), "exact selected logical binder failure")
        need(not check([dict(note, message=message + " unknown"), error]), "unknown selector rejected")
    for message in notes.LOGICAL_ERRORS:
        need(check([dict(error, message=message)]), "known logical failure accepted")
    for message in ("type annotations needed", "Resource limit (rlimit) exceeded", "internal error",
                    "recommendation not met", "unsupported feature", "loop must have a decreases clause"):
        need(not check([dict(error, message=message)]), "non-logical failure never counts as a negative")
    need(not check([dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]), "foreign diagnostic refused")
    need(not check([dict(error, level="warning")]), "warning is not a proof failure")
print("PASS: dispatch template binder calibration (5 groups)")
