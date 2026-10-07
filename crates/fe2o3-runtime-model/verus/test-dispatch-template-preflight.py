#!/usr/bin/env python3
"""Calibrate the retained-fact projection, active joins and strict proof outcomes."""
import contextlib
import functools
import hashlib
import io
import json
from pathlib import Path
import re
import runpy
import types

r = runpy.run_path(str(Path(__file__).with_name("check-dispatch-template-preflight.py")))
need, root = r["need"], r["ROOT"]
inputs = {path: (root / path).read_text() for path in r["FILES"]}
base_path = root / r["V"] / "test-dispatch-template-prepare.py"
base_raw = base_path.read_bytes()
need(hashlib.sha256(base_raw).hexdigest() ==
     "28d1dd49761bbe931de04ea9a726854eacef85f03a0db4d4160192c2885d73e7", "authenticated template join calibration")
base = types.ModuleType("preflight_template_join_calibration")
base.__file__ = str(base_path)
captured = io.StringIO()
with contextlib.redirect_stdout(captured):
    exec(compile(base_raw, base.__file__, "exec"), base.__dict__)
need(captured.getvalue() == "PASS: dispatch template preparation calibration (5 groups)\n", "existing template join calibration")
compact, code_only, rejects = base.compact, base.code_only, base.rejects
code_only = functools.lru_cache(maxsize=8)(code_only)
epoch_path = root / r["V"] / "test-dispatch-epoch-reserve.py"
epoch_raw = epoch_path.read_bytes()
need(hashlib.sha256(epoch_raw).hexdigest() ==
     "965a50bee64463bb42679cafefc81249ded7a5d6673bc19d6e8dc6e129ad2d66", "authenticated epoch source calibration")
epoch_guard = types.ModuleType("preflight_epoch_source_calibration")
epoch_guard.__file__ = str(epoch_path)
captured = io.StringIO()
with contextlib.redirect_stdout(captured):
    exec(compile(epoch_raw, epoch_guard.__file__, "exec"), epoch_guard.__dict__)
need(captured.getvalue() == "PASS: dispatch epoch reservation calibration (5 groups)\n", "existing epoch source calibration")

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


def shape(source, name):
    source = code_only(source)
    source = re.sub(r"\bpub(?:\([^)]*\))?\s+", "", source)
    matches = list(re.finditer(r"\b(?:struct|enum)\s+" + name + r"\b", source))
    need(len(matches) == 1, "unique retained-fact shape " + name)
    start = source.index("{", matches[0].end()) + 1
    return compact(base.balanced_body(source, start)).rstrip(",")


def direct_item(code, position, allow_verus=False):
    before = code[:position]
    parents = []
    for index, char in enumerate(before):
        if char == "{":
            parents.append(before[max(0, index - 6):index] == "verus!")
        elif char == "}":
            need(parents, "balanced item ancestry")
            parents.pop()
    need(not parents or (allow_verus and parents == [True]), "direct item ownership")
    need(not before.endswith("]"), "non-attributed item")


def block(source, prefix, allow_verus=False):
    code = compact(code_only(source))
    need(code.count(prefix) == 1, "unique retained-fact implementation")
    direct_item(code, code.index(prefix), allow_verus)
    return base.balanced_body(code, code.index(prefix) + len(prefix))


def owned_method(source, prefix, method, allow_verus=False):
    code = compact(code_only(source))
    contents = block(source, prefix, allow_verus)
    need(code.count(method) == 1 and contents.count(method) == 1, "unique actual owned method")
    direct_item(contents, contents.index(method))


COUNT_WRAPPER = ('fnvalidate_packet_count<constN:usize>()->Result<(),Gfx942DispatchBindingErrorV1>'
                 '{dispatch_template_packet_count_body!(dispatch_rust_expr,N)}')
VM_WRAPPER = ('constfnvm(&self)->fe2o3_runtime_model::VmKeyV1'
              '{dispatch_template_data_vm_body!(dispatch_rust_expr,self)}')
PREFLIGHT_WRAPPER = ('fnpreflight_templates<constN:usize>(&self,queue:QueueKeyV1)->Result<u64,Gfx942DispatchBindingErrorV1>'
                     '{dispatch_template_preflight_body!(dispatch_rust_expr,self,N,queue)}')
GETTERS = (
    ('#[allow(dead_code)]implSharedGttMappedResourceFactsV1{', 'mapping',
     'pub(crate)constfnmapping(&self)->MemoryMappingKeyV1{self.mapping}'),
    ('#[allow(dead_code,private_bounds)]impl<R,P,S>SharedGttQueueResourceAuthorityV1<R,P,S>whereR:SharedGttQueueResourceRoleV1,P:GttProfileV1,S:GpuMappedGttStateV1,{', 'facts',
     'pub(crate)constfnfacts(&self)->&SharedGttMappedResourceFactsV1{&self.facts}'),
    ('implGfx942DeviceMemoryDispatchFactsV1{', 'vm',
     'pub(crate)constfnvm(&self)->VmKeyV1{self.vm}'),
    ('implGfx942DeviceMemoryDispatchAuthorityV1{', 'facts',
     'pub(crate)constfnfacts(&self)->&Gfx942DeviceMemoryDispatchFactsV1{&self.facts}'),
)
MODEL_GETTERS = (
    ('implSharedGttMappedResourceFactsV1{',
     'fnmapping(&self)->(out:MemoryMappingKeyV1)ensuresout==self.mapping,{self.mapping}'),
    ('impl<C>SharedGttQueueResourceAuthorityV1<C>{',
     'fnfacts(&self)->(out:&SharedGttMappedResourceFactsV1)ensures*out==self.facts,{&self.facts}'),
    ('implGfx942DeviceMemoryDispatchFactsV1{',
     'fnvm(&self)->(out:VmKeyV1)ensuresout==self.vm,{self.vm}'),
    ('impl<D>Gfx942DeviceMemoryDispatchAuthorityV1<D>{',
     'fnfacts(&self)->(out:&Gfx942DeviceMemoryDispatchFactsV1)ensures*out==self.facts,{&self.facts}'),
    ('impl<D,H>DispatchDataAuthorityV1<D,H>{',
     'fnvm(&self)->(out:VmKeyV1)ensuresout==data_vm(*self),{dispatch_template_data_vm_body!(verus_exec_expr,self)}'),
)
EPOCH_METHODS = [pattern for pattern in epoch_guard.patterns if pattern.startswith((
    "fnpreflight_reservation(", "fnensure_not_poisoned("))]
need(len(EPOCH_METHODS) == 2, "composed epoch preflight method roster")
CAPACITY_METHOD = next(pattern for pattern in epoch_guard.patterns if pattern.startswith("pubconstfnslots("))


def wiring(binding, memory):
    code = compact(code_only(binding))
    base.binder_join(binding)
    for pattern, name in ((COUNT_WRAPPER, "validate_packet_count"), (PREFLIGHT_WRAPPER, "preflight_templates")):
        need(code.count(pattern) == 1 and len(re.findall("fn" + name + r"[<(]", code)) == 1,
             "unique exact preflight wrapper")
    direct_item(code, code.index(COUNT_WRAPPER))
    owned_method(binding, "implDispatchDataAuthorityV1{", VM_WRAPPER)
    owners = []
    prefix = "implDispatchResourceOwnerV1{"
    for match in re.finditer(re.escape(prefix), code):
        owner = base.balanced_body(code, match.end())
        if "fnpreflight_templates<" in owner:
            owners.append((match.start(), owner))
    need(len(owners) == 1, "unique real preflight owner")
    position, owner = owners[0]
    direct_item(code, position)
    need(owner.count(PREFLIGHT_WRAPPER) == 1 and owner.count(base.BINDER_METHOD) == 1,
         "preflight and actual binder share owner implementation")
    direct_item(owner, owner.index(PREFLIGHT_WRAPPER))
    for prefix, name, pattern in GETTERS:
        contents = block(memory, prefix)
        owned_method(memory, prefix, pattern)
        need(len(re.findall("fn" + name + r"[<(]", contents)) == 1,
             "exact retained-fact getter")
    for pattern in EPOCH_METHODS:
        owned_method(binding, "implDispatchGenerationOwnerV1{", pattern)
    owned_method(binding, "implGfx942FixedDispatchCapacityProfileV1{", CAPACITY_METHOD)


binding = (root / "crates/fe2o3-kfd/src/queue_dispatch_binding.rs").read_text()
memory = (root / "crates/fe2o3-kfd/src/shared_memory.rs").read_text()
wiring(binding, memory)
canonical_binding, canonical_memory = compact(code_only(binding)), compact(code_only(memory))
for column, source, patterns in ((0, canonical_binding, (COUNT_WRAPPER, VM_WRAPPER, PREFLIGHT_WRAPPER)),
                                 (1, canonical_memory, tuple(row[2] for row in GETTERS))):
    for pattern in patterns:
        for replacement in ("WRONG", "/* " + pattern + " */ WRONG", "// " + pattern + "\nWRONG",
                            'r###"' + pattern + '"### WRONG'):
            texts = [canonical_binding, canonical_memory]
            texts[column] = source.replace(pattern, replacement)
            rejects(lambda: wiring(*texts))
        texts = [canonical_binding, canonical_memory]
        texts[column] = source + pattern
        rejects(lambda: wiring(*texts))
        for replacement in ("#[cfg(any())]" + pattern, "fnunrelated(){" + pattern + "}"):
            texts[column] = source.replace(pattern, replacement)
            rejects(lambda: wiring(*texts))
for column, source, prefixes in ((0, canonical_binding, ("implDispatchDataAuthorityV1{", "implDispatchGenerationOwnerV1{",
                                                        "implGfx942FixedDispatchCapacityProfileV1{")),
                                  (1, canonical_memory, tuple(row[0] for row in GETTERS))):
    for prefix in prefixes:
        start = source.index(prefix)
        end = start + len(prefix) + len(base.balanced_body(source, start + len(prefix))) + 1
        original = source[start:end]
        for replacement in ("#[cfg(any())]" + original, "fnunrelated(){" + original + "}"):
            texts = [canonical_binding, canonical_memory]
            texts[column] = source[:start] + replacement + source[end:]
            rejects(lambda: wiring(*texts))
for pattern in EPOCH_METHODS + [CAPACITY_METHOD]:
    for replacement in (pattern[:pattern.index("{") + 1] + "Ok(())}",
                        "#[cfg(any())]" + pattern, "fnunrelated(){" + pattern + "}"):
        rejects(lambda: wiring(canonical_binding.replace(pattern, replacement), canonical_memory))
for old, new in (("dispatch_preflight_reservation_body!(dispatch_rust_expr, self, queue)", "Ok((0, 1, 1))"),
                 ("dispatch_not_poisoned_body!(dispatch_rust_expr, self)", "Ok(())")):
    need(old in binding, "actual composed epoch wrapper site")
    rejects(lambda: epoch_guard.wiring(binding.replace(old, new)))
preflight_call = base.BINDING_WRAPPERS[-1]
moved = canonical_binding.replace(preflight_call, "unimplemented!()") + "fnunrelated(){" + preflight_call + "}"
need(moved.count(preflight_call) == 1, "decoy retains the global composition call")
rejects(lambda: wiring(moved, canonical_memory))


def projections(binding, memory, model, epoch):
    model_code = compact(code_only(model))
    epoch_guard.schemas(binding, epoch, base.completion)
    for prefix, method in MODEL_GETTERS:
        owned_method(model, prefix, method, allow_verus=True)
    for name in ("SharedGttMappedResourceFactsV1", "Gfx942DeviceMemoryLayoutV1", "Gfx942DeviceMemoryDispatchFactsV1"):
        need(shape(memory, name) == shape(model, name), "full retained-fact fields " + name)
    for name in ("PreparedDispatchPacketV1", "ResolvedCodeIdentityV1"):
        need(shape(binding, name) == shape(model, name), "full packet/code metadata " + name)
    for origin, name in ((base.sources["loader"], "KernelIdentityInputsV1"),
                         (base.sources["aql"], "AqlDispatchGeometryV1")):
        need(shape(origin, name) == shape(model, name), "full inert metadata " + name)
    need(base.shape(base.sources["aql"], "ObservedGpuAddressV1") == base.shape(model, "ObservedGpuAddressV1"), "address wrapper")
    need(base.variants(shape(model, "AqlDispatchOrderingV1")) == {"Independent", "WaitForPrior"}, "ordering variants")
    need(shape(memory, "SharedGttQueueResourceAuthorityV1") ==
         "token:SharedGttAllocationV1<P,S>,facts:SharedGttMappedResourceFactsV1,role:PhantomData<R>",
         "complete allocation authority payload before projection")
    need(shape(model, "SharedGttQueueResourceAuthorityV1") == "credits:C,facts:SharedGttMappedResourceFactsV1", "token and role opaque credit projection")
    need(shape(memory, "Gfx942DeviceMemoryDispatchAuthorityV1") ==
         "lease:Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,facts:Gfx942DeviceMemoryDispatchFactsV1", "complete device authority payload")
    need(shape(model, "Gfx942DeviceMemoryDispatchAuthorityV1") == "lease:D,facts:Gfx942DeviceMemoryDispatchFactsV1", "opaque device lease projection")
    need(shape(binding, "DispatchDataAuthorityV1") == "Device(Gfx942DeviceMemoryDispatchAuthorityV1),HostVisible(HostDataAuthority)", "complete data authority variants")
    need(shape(model, "DispatchDataAuthorityV1") ==
         "Device(Gfx942DeviceMemoryDispatchAuthorityV1<D>),HostVisible(SharedGttQueueResourceAuthorityV1<H>)", "data authority payload projection")
    actual = shape(binding, "DispatchResourceOwnerV1")
    for old, new in (
        ("code:Vec<CodeAuthority>", "code:Vec<C>"),
        ("kernarg:KernargAuthority", "kernarg:SharedGttQueueResourceAuthorityV1<K>"),
        ("data:Vec<DispatchDataAuthorityV1>", "data:Vec<DispatchDataAuthorityV1<D,H>>"),
        ("data_premises:Vec<RetainedDataPremiseV1>", "data_premises:Vec<P>"),
        ("generation:DispatchGenerationOwnerV1", "generation:DispatchGenerationOwnerV1<E>"),
        ("persistent_control:PersistentFixedDispatchControlStateV1", "persistent_control:T"),
        ("conditional_fill:Option<Box<conditional_fill::ConditionalFillStorageV1>>", "conditional_fill:Option<Box<F>>"),
    ):
        need(actual.count(old) == 1, "unique owner payload projection")
        actual = actual.replace(old, new)
    need(actual == shape(model, "DispatchResourceOwnerV1"), "complete owner field and payload projection")
    for origin, names in ((base.sources["identity"], ("DeviceKeyV1", "VmKeyV1", "QueueKeyV1")),
                         (base.sources["memory"], ("MemoryAllocationKeyV1", "MemoryMappingKeyV1"))):
        for name in names:
            fields = shape(origin, name)
            for wrapper in base.NUMERIC_IDS:
                fields = fields.replace(":" + wrapper, ":u64")
            need(fields == shape(epoch, name), "existing explicit numeric identity projection")
    publication = shape(base.sources["memory"], "MemoryPublicationKeyV1").replace(":MemoryPublicationIdV1", ":u64")
    need(publication == shape(model, "MemoryPublicationKeyV1"), "publication identity numeric projection")
    need(base.variants(shape(epoch, "Gfx942DispatchBindingErrorV1")) <=
         base.variants(shape(binding, "Gfx942DispatchBindingErrorV1")), "complete actual error variants")


model, epoch = inputs[r["PROOF"]], inputs[r["CANCEL"]]
projections(binding, memory, model, epoch)
canonical_model = compact(code_only(model))
for prefix, method in MODEL_GETTERS:
    for replacement in ("#[cfg(any())]" + method, "fnunrelated(){" + method + "}"):
        rejects(lambda: projections(binding, memory, canonical_model.replace(method, replacement), epoch))
    start = canonical_model.index(prefix)
    end = start + len(prefix) + len(base.balanced_body(canonical_model, start + len(prefix))) + 1
    original = canonical_model[start:end]
    for replacement in ("#[cfg(any())]" + original, "fnunrelated(){" + original + "}"):
        rejects(lambda: projections(binding, memory, canonical_model[:start] + replacement + canonical_model[end:], epoch))
for clause in ("ensures out == self.mapping,", "ensures *out == self.facts,",
               "ensures out == self.vm,", "ensures out == data_vm(*self),"):
    need(clause in model, "hostile projected getter contract")
    rejects(lambda: projections(binding, memory, model.replace(clause, "ensures true,"), epoch))
for method_body in ("{ self.mapping }", "{ &self.facts }", "{ self.vm }",
                    "{ dispatch_template_data_vm_body!(verus_exec_expr, self) }"):
    need(method_body in model, "hostile projected getter body")
    for replacement in ("{ wrong }", "/* " + method_body + " */ { wrong }", 'r###"' + method_body + '"### { wrong }'):
        rejects(lambda: projections(binding, memory, model.replace(method_body, replacement), epoch))
for field in ("credits: C", "lease: D", "persistent_control: T", "data_premises: Vec<P>",
              "conditional_fill: Option<Box<F>>", "conditional_fill: bool",
              "gpu_va_bytes: u64", "publication: MemoryPublicationKeyV1", "generation: DispatchGenerationOwnerV1<E>"):
    need(field in model, "hostile resource projection site")
    for replacement in ("wrong: u64", "/* " + field + " */ wrong: u64", 'r###"' + field + '"### wrong: u64'):
        rejects(lambda: projections(binding, memory, model.replace(field, replacement), epoch))
for old, new in (("ZeroPacketCount", "WrongZeroPacketCount"),
                 ("InvalidKernarg { packet: usize, detail: &'static str }", "InvalidKernarg { packet: u64, detail: &'static str }")):
    need(old in epoch, "hostile actual error projection")
    rejects(lambda: projections(binding, memory, model, epoch.replace(old, new)))
for field in ("recipe_occurrence: u64", "slot_generation: u64", "credits: C", "roster_sha256: [u8; 32]"):
    need(field in epoch, "composed epoch schema site")
    rejects(lambda: projections(binding, memory, model, epoch.replace(field, "wrong: u64")))


def packet_limit(aql, model):
    for source, statement, allow_verus in (
        (aql, "pubconstAQL_MAX_FIXED_BATCH_PACKETS_V2:u32=8192;", False),
        (model, "constAQL_MAX_FIXED_BATCH_PACKETS_V2:u32=8192;", True),
    ):
        code = compact(code_only(source))
        need(code.count(statement) == 1 and code.count("constAQL_MAX_FIXED_BATCH_PACKETS_V2:") == 1,
             "exact active packet limit declaration")
        direct_item(code, code.index(statement), allow_verus)


def packet_limit_import(binding):
    active = code_only(binding)
    code = compact(active)
    statement = ("usefe2o3_aql::{AQL_MAX_FIXED_BATCH_PACKETS_V2,AqlDispatchGeometryV1,AqlDispatchOrderingV1,"
                 "AqlRingCapacityV1,Cov6ImplicitDispatchShapeV1,ObservedGpuAddressV1};")
    need(code.count(statement) == 1, "exact active AQL limit import route")
    direct_item(code, code.index(statement))
    need(not re.search(r"\b(?:const|static|type|fn|struct|enum|mod|as)\s+AQL_MAX_FIXED_BATCH_PACKETS_V2\b", active),
         "no packet limit alias or redefinition")
    need(not re.search(r"\b(?:mod|as)\s+fe2o3_aql\b", active), "no AQL import route shadow")


packet_limit(base.sources["aql"], model)
packet_limit_import(binding)
import_prefix = "use fe2o3_aql::{"
start = binding.index(import_prefix)
end = binding.index("};", start) + 2
import_statement = binding[start:end]
for replacement in (import_statement.replace("fe2o3_aql", "another_aql"), "#[cfg(any())]" + import_statement,
                    "fn unrelated(){" + import_statement + "}", "/* " + import_statement + " */",
                    'r###"' + import_statement + '"###', import_statement + import_statement,
                    import_statement.replace("AQL_MAX_FIXED_BATCH_PACKETS_V2,", "OTHER_LIMIT as AQL_MAX_FIXED_BATCH_PACKETS_V2,")):
    rejects(lambda: packet_limit_import(binding.replace(import_statement, replacement)))
for suffix in ("const AQL_MAX_FIXED_BATCH_PACKETS_V2: u32 = 1;", "use another::LIMIT as AQL_MAX_FIXED_BATCH_PACKETS_V2;",
               "mod fe2o3_aql {}", "use another as fe2o3_aql;"):
    rejects(lambda: packet_limit_import(binding + suffix))
for column, source, statement in (
    (0, base.sources["aql"], "pub const AQL_MAX_FIXED_BATCH_PACKETS_V2: u32 = 8192;"),
    (1, model, "const AQL_MAX_FIXED_BATCH_PACKETS_V2: u32 = 8192;"),
):
    need(source.count(statement) == 1, "packet limit hostile site")
    for replacement in (statement.replace("8192", "4096"), "/* " + statement + " */", 'r###"' + statement + '"###',
                        "#[cfg(any())]" + statement, "fn unrelated(){" + statement + "}", statement + statement):
        texts = [base.sources["aql"], model]
        texts[column] = source.replace(statement, replacement)
        rejects(lambda: packet_limit(*texts))

cases = r["mutations"](inputs[r["BODY"]])
need(len(cases) == 22, "preflight mutation count")
for data, focus in cases.values():
    need(data != inputs[r["BODY"]] and focus[1:] in r["METHODS"], "executable preflight mutation")
rejects(lambda: r["mutations"](inputs[r["BODY"]] * 2))
rejects(lambda: r["mutations"](inputs[r["BODY"]].replace("Ok(generation)", "Ok(0)")))

campaign = r["campaign"]()
classifier, verifier = campaign.inherited(), {"version": "calibration-only"}
leaf = classifier.inherited()
path = "/snapshot/dispatch_template_preflight_v1.rs"
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
need(classifier.proof_positive(0, json.dumps(positive), "", verifier, campaign.EXPECTED, {path}), "exact positive accepted")
need(not classifier.proof_positive(1, json.dumps(positive), "", verifier, campaign.EXPECTED, {path}), "failed positive refused")
print("PASS: dispatch template preflight calibration (5 groups)")
