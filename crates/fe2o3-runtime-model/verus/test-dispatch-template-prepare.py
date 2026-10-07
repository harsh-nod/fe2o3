#!/usr/bin/env python3
"""Fail-closed source joins, raw metadata schemas and diagnostic calibration."""
import hashlib
import functools
import json
from pathlib import Path
import re
import runpy
import types

r = runpy.run_path(str(Path(__file__).with_name("check-dispatch-template-prepare.py")))
need, root = r["need"], r["ROOT"]
inputs = {path: (root / path).read_text() for path in r["FILES"]}
lexer_path = root / r["V"] / "check-negative-quality.py"
lexer_raw = lexer_path.read_bytes()
need(hashlib.sha256(lexer_raw).hexdigest() ==
     "74c83c0776206e6c486ebb0bf9fbf6bcfdf207a1f86c22bed6054b5f7438c5fd", "authenticated Rust lexical helper")
lexer = types.ModuleType("template_prepare_rust_lexer")
lexer.__file__ = str(lexer_path)
exec(compile(lexer_raw, lexer.__file__, "exec"), lexer.__dict__)
code_only = functools.lru_cache(maxsize=12)(lexer.code_only)


def rejects(operation):
    try:
        operation()
    except ValueError:
        return
    raise ValueError("hostile calibration accepted")


r["audit"](inputs)
for path in inputs:
    for suffix in ('\nassume(false);', '\n#[verifier::external_body]', '\nmod foreign;',
                   '\ninclude!("/foreign.rs");', '\ninclude_str!("/foreign.rs");',
                   '\ninclude_bytes!("/foreign.rs");', '\nenv!("FOREIGN");', '\noption_env!("FOREIGN");'):
        rejects(lambda: r["audit"]({**inputs, path: inputs[path] + suffix}))
    rejects(lambda: r["audit"]({p: text for p, text in inputs.items() if p != path}))
rejects(lambda: r["audit"]({**inputs, Path("/foreign.rs"): ""}))
for path, edges in r["EDGES"].items():
    for statement, _ in edges:
        for replacement in ("", statement + statement, statement.replace(".rs", "-foreign.rs"),
                            "/* " + statement + " */", "// " + statement + "\n", 'r###"' + statement + '"###'):
            rejects(lambda: r["audit"]({**inputs, path: inputs[path].replace(statement, replacement)}))


def compact(text):
    return re.sub(r",([)}])", r"\1", re.sub(r"\s+", "", text))


def active_includes(source, expected):
    marker = "__fe2o3_template_include_guard_"
    need(marker not in source, "reserved include sentinel")
    replaced, sentinels = source, []
    for index, statement in enumerate(expected):
        need(source.count(statement) == 1, "exact production include spelling")
        sentinel = marker + str(index) + "__"
        replaced = replaced.replace(statement, sentinel)
        sentinels.append(sentinel)
    checked = compact(code_only(replaced))
    for sentinel in sentinels:
        need(checked.count(sentinel) == 1, "active production include")
        prefix = checked[:checked.index(sentinel)]
        need(prefix.count("{") == prefix.count("}"), "root production include")
    need("include!(" not in checked, "exact active include set")


BINDING_INCLUDES = [f'include!("queue_dispatch_binding/{name}_body.rs");'
                    for name in ("epoch_cancel", "epoch_reserve", "cancel_binding", "template_prepare", "template_preflight", "template_bind")]
COMPLETION_INCLUDES = [f'include!("queue_completion/{name}_body.rs");'
                      for name in ("event_release", "bound_cancel", "rollback_adapters", "event_bind", "event_issue", "batch_bind", "dispatch_roster")]
COMPLETION_INCLUDES.append('include!("queue_dispatch_binding/template_prepare_body.rs");')
BINDING_WRAPPERS = [
    'macro_rules!dispatch_rust_expr{($body:expr)=>{$body};}',
    'fnprepared_kernarg_layout_matches_code(code_bound:bool,kernarg_layout_identity:[u8;32],dispatch_abi_identity:[u8;32])->bool{dispatch_template_abi_matches_body!(dispatch_rust_expr,code_bound,kernarg_layout_identity,dispatch_abi_identity)}',
    'fnprepare_dispatch_templates_v1(packets:&[PreparedDispatchPacketV1],code_identity:&[ResolvedCodeIdentityV1],queue:QueueKeyV1,generation:u64)->Result<Vec<CompletionPacketTemplateV1>,Gfx942DispatchBindingErrorV1>{dispatch_prepare_templates_body!(dispatch_rust_expr,packets,code_identity,queue,generation)}',
    'dispatch_bind_templates_body!(dispatch_rust_expr,self,N,queue)',
]
COMPLETION_WRAPPERS = [
    'macro_rules!completion_rust_expr{($body:expr)=>{$body};}',
    'pub(crate)constfnnew(queue:QueueKeyV1,code:MemoryMappingKeyV1,kernarg:MemoryMappingKeyV1,dispatch_generation:u64)->Self{dispatch_template_generation_new_body!(completion_rust_expr,queue,code,kernarg,dispatch_generation)}',
    'pub(crate)constfnnew(geometry:AqlDispatchGeometryV1,ordering:AqlDispatchOrderingV1,private_segment_size:u32,group_segment_size:u32,kernel_object:ObservedGpuAddressV1,kernarg_address:ObservedGpuAddressV1,kernarg_alignment:u64,generations:CompletionDispatchGenerationBindingV1)->Self{dispatch_template_new_body!(completion_rust_expr,geometry,ordering,private_segment_size,group_segment_size,kernel_object,kernarg_address,kernarg_alignment,generations)}',
]
# This pins the real caller's source order, not a theorem about its authority,
# allocation, hashing or reservation behavior. String values are not projected.
BINDER_GUARD = runpy.run_path(str(root / r["V"] / "check-dispatch-template-bind.py"))
BINDER_METHOD = compact(code_only(BINDER_GUARD["BINDER"]))
BINDER_BODY = (root / BINDER_GUARD["BODY"]).read_text()


def balanced_body(source, start, opener="{", closer="}"):
    depth, end = 1, start
    while depth and end < len(source):
        depth += (source[end] == opener) - (source[end] == closer)
        end += 1
    need(depth == 0, "closed source block")
    return source[start:end - 1]


def named_impl(source, name):
    source = compact(code_only(source))
    prefix = "impl" + name + "{"
    need(source.count(prefix) == 1, "unique implementation block " + name)
    return balanced_body(source, source.index(prefix) + len(prefix))


def binder_join(source):
    BINDER_GUARD["binder_wiring"](source, BINDER_BODY)
    code = compact(code_only(source))
    need(len(re.findall(r"fnbind_templates[<(]", code)) == 1, "unique template binder method")
    prefix = "implDispatchResourceOwnerV1{"
    owners = []
    for match in re.finditer(re.escape(prefix), code):
        body = balanced_body(code, match.end())
        if re.search(r"fnbind_templates[<(]", body):
            owners.append((match.start(), body))
    need(len(owners) == 1, "unique dispatch resource implementation containing binder")
    position, owner = owners[0]
    before = code[:position]
    need(before.count("{") == before.count("}") and not before.endswith("]"),
         "non-attributed root dispatch resource implementation")
    need(owner.count(BINDER_METHOD) == 1, "exact real binder source order")
    before = owner[:owner.index(BINDER_METHOD)]
    need(before.count("{") == before.count("}"), "direct dispatch resource method")
    need(BINDER_METHOD.count(BINDING_WRAPPERS[-1]) == 1, "exact single shared body call in binder")


def wiring(binding, completion):
    active_includes(binding, BINDING_INCLUDES)
    active_includes(completion, COMPLETION_INCLUDES)
    for source, patterns in ((binding, BINDING_WRAPPERS), (completion, COMPLETION_WRAPPERS)):
        code = compact(code_only(source))
        for pattern in patterns:
            need(code.count(pattern) == 1, "exact active production wrapper: " + pattern)
    for source, name in ((binding, "dispatch_rust_expr"), (completion, "completion_rust_expr")):
        need(len(re.findall(r"\bmacro_rules!" + name + r"\b", compact(code_only(source)))) == 1,
             "unique production expression macro")
    for name, pattern in zip(("CompletionDispatchGenerationBindingV1", "CompletionPacketTemplateV1"), COMPLETION_WRAPPERS[1:]):
        need(named_impl(completion, name).count(pattern) == 1, "constructor belongs to exact implementation")
    binder_join(binding)
    binding_code = compact(code_only(binding))
    for name in ("prepared_kernarg_layout_matches_code", "prepare_dispatch_templates_v1"):
        need(len(re.findall("fn" + name + r"[<(]", binding_code)) == 1, "unique production preparation helper")
    for pattern in BINDING_WRAPPERS[1:3]:
        location = binding_code.index(pattern)
        prefix = binding_code[:location]
        need(prefix.count("{") == prefix.count("}"), "root preparation helper")


binding = (root / "crates/fe2o3-kfd/src/queue_dispatch_binding.rs").read_text()
completion = (root / "crates/fe2o3-kfd/src/queue_completion.rs").read_text()
BINDER_GUARD["conditional_guard_wiring"](
    binding, (root / BINDER_GUARD["CONDITIONAL_SOURCE"]).read_text())
BINDER_GUARD["conditional_readonly_sources"]({
    path: (root / path).read_text() for path in BINDER_GUARD["CONDITIONAL_READONLY_SOURCES"]})
wiring(binding, completion)
for column, source, includes, patterns in (
    (0, binding, BINDING_INCLUDES, BINDING_WRAPPERS),
    (1, completion, COMPLETION_INCLUDES, COMPLETION_WRAPPERS),
):
    # These wrappers contain no string literals. Include spelling stays intact
    # while comments, raw strings and ordinary strings cannot supply a wrapper.
    canonical = source
    for pattern in patterns:
        active = compact(code_only(canonical))
        need(active.count(pattern) == 1, "hostile wrapper site")
        # Work on a compact canonical copy, restoring exact include literals.
        temporary = canonical
        tags = {}
        for index, statement in enumerate(includes):
            tag = "__canonical_include_" + str(index) + "__"
            need(tag not in temporary, "reserved canonical include marker")
            temporary = temporary.replace(statement, tag)
            tags[tag] = statement
        normalized = compact(code_only(temporary))
        for tag, statement in tags.items():
            normalized = normalized.replace(tag, statement)
        texts = [binding, completion]
        texts[column] = normalized
        wiring(*texts)
        for replacement in ("WRONG", "/* " + pattern + " */ WRONG", "// " + pattern + "\nWRONG",
                            'r###"' + pattern + '"### WRONG'):
            texts[column] = normalized.replace(pattern, replacement)
            rejects(lambda: wiring(*texts))
        texts[column] = normalized + pattern
        rejects(lambda: wiring(*texts))
        if pattern.startswith("macro_rules!"):
            texts[column] = normalized + pattern.replace("$body:expr", "$wrong:expr")
            rejects(lambda: wiring(*texts))
        elif pattern.startswith("fn"):
            texts[column] = normalized + re.sub(r"dispatch_\w+_body!", "wrong_body!", pattern)
            rejects(lambda: wiring(*texts))
    for statement in includes:
        for replacement in ("", statement + statement, statement.replace(".rs", "-foreign.rs"),
                            "/* " + statement + " */", "// " + statement + "\n", 'r###"' + statement + '"###'):
            texts = [binding, completion]
            texts[column] = source.replace(statement, replacement)
            rejects(lambda: wiring(*texts))

call = "dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)"
need(binding.count(call) == 1, "real binder call relocation site")
moved_call = binding.replace(call, "unimplemented!()") + '''
impl DispatchResourceOwnerV1 {
    #[allow(dead_code)]
    fn unrelated_template_call_for_guard(&self, queue: QueueKeyV1, generation: u64)
        -> Result<(), Gfx942DispatchBindingErrorV1> {
        ''' + call + '''
    }
}
'''
need(compact(code_only(moved_call)).count(BINDING_WRAPPERS[-1]) == 1, "decoy preserves global active call")
rejects(lambda: wiring(moved_call, completion))
for old, new in (
    ("dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)", "dispatch_bind_templates_body!(dispatch_rust_expr, self, N, other_queue)"),
    ("dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)", "dispatch_bind_templates_body!(dispatch_rust_expr, other, N, queue)"),
    ("dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)", "dispatch_bind_templates_body!(dispatch_rust_expr, self, M, queue)"),
):
    rejects(lambda: binder_join(binding.replace(old, new)))
for old, new in (("$owner.preflight_templates::<$n>($queue)?", "0"),
                 ("$owner.generation.reserve($queue, $roster)?", "$owner.generation.reserve($queue, other_roster)?")):
    need(old in BINDER_BODY, "shared composition mutation site")
    rejects(lambda: BINDER_GUARD["binder_wiring"](binding, BINDER_BODY.replace(old, new)))


def shape(source, name):
    source = code_only(source)
    source = re.sub(r"\bpub(?:\([^)]*\))?\s+", "", source)
    matches = list(re.finditer(r"\b(?:struct|enum)\s+" + name + r"\s*([({])", source))
    need(len(matches) == 1, "unique shape " + name)
    opener = matches[0].group(1)
    return compact(balanced_body(source, matches[0].end(), opener, ")" if opener == "(" else "}")).rstrip(",")


def variants(body):
    entries, start, depth = [], 0, 0
    for index, char in enumerate(body + ","):
        if char in "({[<":
            depth += 1
        elif char in ")}]>":
            depth -= 1
        elif char == "," and depth == 0:
            entries.append(body[start:index])
            start = index + 1
        need(depth >= 0, "balanced variant payload")
    need(depth == 0 and len(entries) == len(set(entries)) and all(entries), "complete unique variants")
    return set(entries)


NUMERIC_IDS = ("PhysicalDeviceIdV1", "DeviceGenerationV1", "VmIdV1", "AllocationIdV1",
               "AllocationGenerationV1", "MappingIdV1", "QueueInstanceIdV1", "QueueGenerationV1")
IDENTITY_MACRO = ('macro_rules!numeric_identity{($(#[$meta:meta])*$name:ident)=>{'
                  '$(#[$meta])*#[derive(Clone,Copy,Debug,Eq,Hash,Ord,PartialEq,PartialOrd)]'
                  '#[repr(transparent)]pubstruct$name(pubu64);};}')


def schemas(sources, model):
    for origin, names in {
        "binding": ("PreparedDispatchPacketV1", "ResolvedCodeIdentityV1"),
        "completion": ("CompletionPacketTemplateV1", "CompletionDispatchGenerationBindingV1"),
        "aql": ("ObservedGpuAddressV1", "AqlDispatchGeometryV1"),
        "loader": ("KernelIdentityInputsV1",),
        "identity": ("DeviceKeyV1", "VmKeyV1", "QueueKeyV1"),
        "memory": ("MemoryAllocationKeyV1", "MemoryMappingKeyV1"),
    }.items():
        for name in names:
            need(shape(sources[origin], name) == shape(model, name), "exact raw schema " + name)
    identity = compact(code_only(sources["identity"]))
    need(identity.count(IDENTITY_MACRO) == 1, "exact numeric identity macro")
    need(len(re.findall(r"\bmacro_rules!numeric_identity\b", identity)) == 1, "unique numeric identity macro")
    for name in NUMERIC_IDS:
        need(identity.count("numeric_identity!(" + name + ");") == 1, "active numeric identity declaration")
        need(shape(model, name) == "u64", "exact numeric wrapper payload")
    # Only variant identity is used here, not the AQL wire discriminant.
    need(variants(shape(sources["aql"], "AqlDispatchOrderingV1")) == {
        "Independent=AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1",
        "WaitForPrior=AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_KERNEL_DISPATCH_HEADER_V1"}, "exact production ordering variants")
    need(variants(shape(model, "AqlDispatchOrderingV1")) == {"Independent", "WaitForPrior"}, "ordering variant projection")
    need(variants(shape(model, "Gfx942DispatchBindingErrorV1")) <=
         variants(shape(sources["binding"], "Gfx942DispatchBindingErrorV1")), "complete real error variants")


sources = {"binding": binding, "completion": completion}
for key, path in {
    "aql": "crates/fe2o3-aql/src/lib.rs",
    "loader": "crates/fe2o3-amdhsa-loader/src/kernel_closure.rs",
    "identity": "crates/fe2o3-runtime-model/src/identity.rs",
    "memory": "crates/fe2o3-runtime-model/src/memory_lifecycle.rs",
}.items():
    sources[key] = (root / path).read_text()
model = inputs[r["PROOF"]]
schemas(sources, model)
for field in ("code_index: usize", "conditional_fill: bool", "kernarg_alignment: u64", "materialized_sha256: [u8; 32]",
              "authenticated: KernelIdentityInputsV1", "closure_sha256: [u8; 32]", "dispatch_generation: u64",
              "struct QueueGenerationV1(u64)"):
    need(field in model, "hostile schema site")
    for replacement in ("wrong: u64", "/* " + field + " */ wrong: u64", 'r###"' + field + '"### wrong: u64'):
        rejects(lambda: schemas(sources, model.replace(field, replacement)))
for old, new in (("InvalidCode(&'static str)", "InvalidCode(u64)"),
                 ("InvalidKernarg { packet: usize, detail: &'static str }", "InvalidKernarg { packet: u64, detail: &'static str }")):
    need(old in binding, "production error mutation site")
    rejects(lambda: schemas({**sources, "binding": binding.replace(old, new)}, model))
for replacement in ("WRONG", "/* " + IDENTITY_MACRO + " */ WRONG", 'r###"' + IDENTITY_MACRO + '"### WRONG'):
    identity = compact(code_only(sources["identity"]))
    rejects(lambda: schemas({**sources, "identity": identity.replace(IDENTITY_MACRO, replacement)}, model))
rejects(lambda: schemas({**sources, "identity": sources["identity"] +
                        IDENTITY_MACRO.replace("pubu64", "pubu32")}, model))

cases = r["mutations"](inputs[r["BODY"]])
need(len(cases) == 34, "mutation count")
lookahead, lookahead_focus = cases["lookup-before-earlier-abi"]
need(lookahead.count("if $packets.len() > 1 && $packets[1].code_index >= $codes.len() {") == 1
     and lookahead.count("while ") == inputs[r["BODY"]].count("while ")
     and lookahead_focus == "*prepare_dispatch_templates_v1", "bounded later-code precedence mutation")
for data, focus in cases.values():
    need(data != inputs[r["BODY"]] and focus[1:] in r["METHODS"], "executable mutation")
rejects(lambda: r["mutations"](inputs[r["BODY"]] * 2))
rejects(lambda: r["mutations"](inputs[r["BODY"]].replace("packet.code_index >= $codes.len()", "false")))

campaign = r["campaign"]()
classifier, verifier = campaign.inherited(), {"version": "calibration-only"}
leaf = classifier.inherited()
path = "/snapshot/dispatch_template_prepare_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 0}}
error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
         "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
for name in r["METHODS"]:
    notes = r["selection_notes"](leaf, "*" + name)
    check = lambda messages: classifier.logical_negative(notes, 1, json.dumps(result),
        "\n".join(json.dumps(item) for item in messages), verifier, {path})
    for message in notes.SELECTION_NOTES:
        note = dict(error, level="note", message=message, spans=[])
        need(check([note, error]), "exact selected logical failure")
        need(not check([dict(note, message=message + " unknown"), error]), "unknown selector refused")
    for message in notes.LOGICAL_ERRORS:
        logical = dict(error, message=message)
        need(check([logical]), "known logical failure")
        for bad in (dict(logical, level="warning"), dict(logical, message=message + " unknown"),
                    dict(logical, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])):
            need(not check([bad]), "unauthenticated diagnostic refused")
    for bad in ("Resource limit (rlimit) exceeded", "type annotations needed", "internal error",
                "recommendation not met", "loop must have a decreases clause",
                "external_body/assume_specification not allowed with --no-cheating"):
        need(not check([error, dict(error, message=bad)]), "nonlogical mixed result refused")
positive = {"verus": verifier, "verification-results": campaign.EXPECTED}
need(classifier.proof_positive(0, json.dumps(positive), "", verifier, campaign.EXPECTED, {path}), "positive accepted")
need(not classifier.proof_positive(1, json.dumps(positive), "", verifier, campaign.EXPECTED, {path}), "failed positive refused")
bit_note = dict(error, level="note", message=r["BITVECTOR_ENUMERATION_NOTE"])
accepts_note = lambda diagnostic: classifier.proof_positive(0, json.dumps(positive), json.dumps(diagnostic),
    verifier, campaign.EXPECTED, {path})
need(accepts_note(bit_note), "exact informational bitvector enumeration accepted")
for malformed in (dict(bit_note, level="error"), dict(bit_note, level="warning"),
                  dict(bit_note, message=bit_note["message"] + " unknown"),
                  dict(bit_note, spans=[{"is_primary": True, "file_name": "/foreign.rs"}]),
                  dict(bit_note, children=[error])):
    need(not accepts_note(malformed), "noninformational or unauthenticated positive enumeration refused")
for name in r["METHODS"]:
    notes = r["selection_notes"](leaf, "*" + name)
    negative = lambda messages: classifier.logical_negative(notes, 1, json.dumps(result),
        "\n".join(json.dumps(item) for item in messages), verifier, {path})
    need(not negative([bit_note]), "enumeration alone is not a logical negative")
    need(negative([bit_note, error]), "exact enumeration accompanies an authenticated logical failure")
    for malformed in (dict(bit_note, level="error"), dict(bit_note, level="warning"),
                      dict(bit_note, message=bit_note["message"] + " unknown"),
                      dict(bit_note, message=bit_note["message"] + " Resource limit (rlimit) exceeded"),
                      dict(bit_note, children=[error])):
        need(not negative([malformed, error]), "noninformational or unknown negative enumeration refused")
print("PASS: dispatch template preparation calibration (5 groups)")
