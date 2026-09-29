#!/usr/bin/env python3
"""Host binder composition with explicit std-container and roster/hash trust."""
import functools
import hashlib
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
LEXER = V / "check-negative-quality.py"
LEXER_SHA = "7fadaf2f0b1b155ae8b0038e17fc01471a648319f4c2d59dc4adea25ec59cd5f"
PROOF = V / "dispatch_template_bind_v1.rs"
PREFLIGHT = V / "dispatch_template_preflight_v1.rs"
RESERVE = V / "dispatch_epoch_reserve_v1.rs"
CANCEL = V / "dispatch_epoch_cancel_v1.rs"
ROSTER = V / "dispatch_template_roster_v1.rs"
HASH = V / "dispatch_template_hash_contract_v1.rs"
BOX = V / "completion_box_contracts_v1.rs"
BODY = Path("crates/fe2o3-kfd/src/queue_dispatch_binding/template_bind_body.rs")
PREPARE_BODY = BODY.with_name("template_prepare_body.rs")
PREFLIGHT_BODY = BODY.with_name("template_preflight_body.rs")
RESERVE_BODY = BODY.with_name("epoch_reserve_body.rs")
CANCEL_BODY = BODY.with_name("epoch_cancel_body.rs")
ROSTER_BODY = Path("crates/fe2o3-kfd/src/queue_completion/dispatch_roster_body.rs")
FILES = [PROOF, PREFLIGHT, RESERVE, CANCEL, ROSTER, HASH, BOX, BODY,
         PREPARE_BODY, PREFLIGHT_BODY, RESERVE_BODY, CANCEL_BODY, ROSTER_BODY]
EDGES = {
    PROOF: [(f'include!("{path.name}");', path) for path in (PREFLIGHT, BOX, HASH, ROSTER)]
        + [(f'include!("../../fe2o3-kfd/src/queue_dispatch_binding/{path.name}");', path)
           for path in (PREPARE_BODY, BODY)],
    PREFLIGHT: [('include!("dispatch_epoch_reserve_v1.rs");', RESERVE),
                ('include!("../../fe2o3-kfd/src/queue_dispatch_binding/template_preflight_body.rs");', PREFLIGHT_BODY)],
    RESERVE: [('include!("dispatch_epoch_cancel_v1.rs");', CANCEL),
              ('include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_reserve_body.rs");', RESERVE_BODY)],
    CANCEL: [('include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_cancel_body.rs");', CANCEL_BODY)],
    ROSTER: [('include!("../../fe2o3-kfd/src/queue_completion/dispatch_roster_body.rs");', ROSTER_BODY)],
}
TRUSTED = {
    BOX: "5822ed2870b5c5572d0a97045bcb78b0a8b8966f123ee4267a121fca6a8eceea",
    HASH: "3877751bfc917bbfe1a74cf2a61438145ff2af5e09d0b4821b6e465a2131bd98",
}
BODY_SHA = "40242c2cd87fb8afcbe24ea3a327a7fd72e549e57a5f00bba939ec0feb0043b1"
ROSTER_BODY_SHA = "dab4466fd7d4f5944facb9221d81891ffc2b5c54090568ab0b58fb1fc5c41433"
# Filled only from the complete positive solver result, never guessed.
EXPECTED_VERIFIED = 64


def need(value, message):
    if not value:
        raise ValueError(message)


@functools.lru_cache(maxsize=1)
def lexer():
    raw = (ROOT / LEXER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == LEXER_SHA, "authenticated Rust lexical helper")
    module = types.ModuleType("binder_rust_lexer")
    module.__file__ = str(ROOT / LEXER)
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module.code_only


def compact(source):
    return re.sub(r",([)}])", r"\1", re.sub(r"\s+", "", lexer()(source)))


def block(source, start):
    depth, end = 1, start
    while depth and end < len(source):
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    need(depth == 0, "complete source block")
    return source[start:end - 1]


def direct_item(code, position):
    before = code[:position]
    need(before.count("{") == before.count("}") and not before.endswith("]"), "direct non-attributed item")


def owned_method(code, prefix, expected):
    need(code.count(prefix) == 1, "unique method owner")
    direct_item(code, code.index(prefix))
    owner = block(code, code.index(prefix) + len(prefix))
    need(code.count(expected) == owner.count(expected) == 1, "exact method belongs to actual owner")
    direct_item(owner, owner.index(expected))


def audit(inputs):
    need(set(inputs) == set(FILES), "exact thirteen-file binder closure")
    need(inputs[PROOF].count("#![allow(unused_macros)]") == 1,
         "one explicit proof-root unused production macro lint")
    need(compact(inputs[PROOF]).startswith("#![feature(allocator_api)]#![allow(unused_macros)]macro_rules!debug_assert_eq"),
         "active exact proof-root lint only")
    for path, source in inputs.items():
        if path == PROOF:
            source = source.replace("#![allow(unused_macros)]", "")
        if path == PREPARE_BODY:
            need(source.count("#[allow(unused_macros)]") == 4,
                 "four inherited prepare macro lints")
            source = source.replace("#[allow(unused_macros)]", "")
        need(not re.search(r"#\s*!?\s*\[\s*(?:allow|expect)\b", lexer()(source)),
             "no other warning suppression: " + str(path))
    for path, digest in TRUSTED.items():
        need(hashlib.sha256(inputs[path].encode()).hexdigest() == digest,
             "exact declared trusted adapter: " + str(path))
    reached, pending = set(), [PROOF]
    while pending:
        path = pending.pop()
        if path in reached:
            continue
        reached.add(path)
        source = inputs[path]
        if path not in TRUSTED:
            need(not re.search(r"\b(?:assume\w*|admit\w*|axiom|external\w*|verifier|unsafe|extern)\b", source),
                 "no undeclared trust: " + str(path))
        for statement, target in EDGES.get(path, []):
            need(source.count(statement) == 1, "exact closure edge")
            sentinel = "__binder_include_guard__"
            need(sentinel not in source, "reserved include marker")
            active = compact(source.replace(statement, sentinel))
            need(active.count(sentinel) == 1, "active closure edge")
            prefix = active[:active.index(sentinel)]
            need(prefix.count("{") == prefix.count("}") and not prefix.endswith("]"),
                 "root non-attributed closure edge")
            source = source.replace(statement, "")
            pending.append(target)
        need(not re.search(r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b", source), "no extra closure input")
    need(reached == set(FILES), "reachable complete binder closure")


BINDER = """
pub(super) fn bind_templates<const N: usize>(&mut self, queue: QueueKeyV1)
 -> Result<(Box<[CompletionPacketTemplateV1; N]>, DispatchEpochIdentityV1,), Gfx942DispatchBindingErrorV1,> {
    dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)
}
"""


def binder_wiring(binding, body):
    need(hashlib.sha256(body.encode()).hexdigest() == BODY_SHA, "exact shared binder sequence")
    include = 'include!("queue_dispatch_binding/template_bind_body.rs");'
    need(binding.count(include) == 1, "one exact binder include")
    sentinel = "__binder_production_include__"
    need(sentinel not in binding, "reserved production marker")
    active = compact(binding.replace(include, sentinel))
    need(active.count(sentinel) == 1, "active binder include")
    before = active[:active.index(sentinel)]
    need(before.count("{") == before.count("}") and not before.endswith("]"), "root non-attributed binder include")
    code = compact(binding)
    need(len(re.findall(r"fnbind_templates[<(]", code)) == 1, "unique real binder method")
    owners = []
    for match in re.finditer(re.escape("implDispatchResourceOwnerV1{"), code):
        content = block(code, match.end())
        if re.search(r"fnbind_templates[<(]", content):
            owners.append((match.start(), content))
    need(len(owners) == 1, "unique actual resource owner binder")
    position, owner = owners[0]
    before = code[:position]
    need(before.count("{") == before.count("}") and not before.endswith("]"), "non-attributed root owner implementation")
    expected = compact(BINDER)
    need(owner.count(expected) == 1, "exact direct binder wrapper")
    before = owner[:owner.index(expected)]
    need(before.count("{") == before.count("}") and not before.endswith("]"), "direct non-attributed owner method")
    need(code.count("dispatch_bind_templates_body!(") == 1, "one active production binder body call")
    need("macro_rules!dispatch_bind_templates_body" not in code, "no production binder macro shadow")
    imports = list(re.finditer(r"usesuper::completion::\{([^}]+)\};", code))
    need(len(imports) == 1 and "completion_template_dispatch_roster_v1" in imports[0].group(1).split(",")
         and len(re.findall(r"\bcompletion_template_dispatch_roster_v1\b", code)) == 1,
         "actual sibling roster helper import without alias")
    direct_item(code, imports[0].start())
    conversion = "implFrom<Gfx942CompletionErrorV1>forGfx942DispatchBindingErrorV1{fnfrom(value:Gfx942CompletionErrorV1)->Self{Self::Completion(value)}}"
    need(code.count(conversion) == 1, "explicit binder conversion equals production From")


HASH_WRAPPERS = {
    "completion_dispatch_roster_v1": """pub(super) fn completion_dispatch_roster_v1(
        dispatches: &[CompletionDispatchGenerationBindingV1]) -> Result<CompletionDispatchRosterV1, Gfx942CompletionErrorV1> {
        completion_dispatch_roster_with_visits_v1(dispatches).map(|(roster, _)| roster)
    }""",
    "completion_dispatch_roster_with_visits_v1": """fn completion_dispatch_roster_with_visits_v1(
        dispatches: &[CompletionDispatchGenerationBindingV1]) -> Result<(CompletionDispatchRosterV1, usize), Gfx942CompletionErrorV1> {
        let mut visits = 0;
        let roster = completion_dispatch_roster_projected_v1(dispatches, |dispatch| {visits += 1; *dispatch})?;
        Ok((roster, visits))
    }""",
    "completion_template_dispatch_roster_v1": """pub(super) fn completion_template_dispatch_roster_v1(
        templates: &[CompletionPacketTemplateV1]) -> Result<CompletionDispatchRosterV1, Gfx942CompletionErrorV1> {
        completion_dispatch_roster_projected_v1(templates, |template| template.generations())
    }""",
    "completion_dispatch_roster_projected_v1": """fn completion_dispatch_roster_projected_v1<T>(values: &[T],
        project: impl FnMut(&T) -> CompletionDispatchGenerationBindingV1)
        -> Result<CompletionDispatchRosterV1, Gfx942CompletionErrorV1> {
        let mut hasher = CompletionOccurrenceHasherV1(Sha256::new());
        let (queue, dispatch_generation) = hash_completion_dispatch_roster_projected_v1(values, project, &mut hasher)?;
        Ok(CompletionDispatchRosterV1 {queue, packet_count: values.len(), dispatch_generation,
            roster_sha256: hasher.0.finalize().into()})
    }""",
    "hash_completion_dispatch_roster_projected_v1": """fn hash_completion_dispatch_roster_projected_v1<T, H: Hasher>(
        values: &[T], mut project: impl FnMut(&T) -> CompletionDispatchGenerationBindingV1, hasher: &mut H)
        -> Result<(QueueKeyV1, u64), Gfx942CompletionErrorV1> {
        completion_hash_roster_body!(completion_rust_expr, values, project, hasher)
    }""",
}


def hash_imports(completion):
    active = compact(completion)
    protected = r"(?:Hash|Hasher|Digest|Sha256|core|sha2)"
    expected = {"usecore::hash::{Hash,Hasher};", "usesha2::{Digest,Sha256};"}
    seen = set()
    for match in re.finditer(r"(?:pub(?:\([^)]*\))?)?use[^;]+;", active):
        before = active[:match.start()]
        if before.count("{") != before.count("}"):
            continue
        if before and before[-1] not in ";}]":
            continue
        statement = match.group()
        need("*" not in statement, "no root wildcard can shadow a trusted Hash adapter")
        if re.search(protected, statement):
            exact = statement
            need(not before.endswith("]") and exact in expected | {"usecore::fmt;"},
                 "root non-attributed trusted Hash import route without alias")
            need(exact not in seen, "unique trusted Hash import")
            seen.add(exact)
    need(expected <= seen, "both actual core Hash and sha2 adapter imports")
    shadow = re.compile(r"(?:struct|enum|union|type|trait|mod|fn|const|static|macro_rules!|externcrate)"
                        r"(?:Hash|Hasher|Digest|Sha256|core|sha2)\b|as(?:Hash|Hasher|Digest|Sha256|core|sha2)\b")
    for match in shadow.finditer(active):
        before = active[:match.start()]
        need(before.count("{") != before.count("}"), "no root trusted Hash name shadow")


def hash_wiring(completion, body=None):
    if body is None:
        body = (ROOT / ROSTER_BODY).read_text()
    need(hashlib.sha256(body.encode()).hexdigest() == ROSTER_BODY_SHA, "exact shared roster and Hash adapter bodies")
    include = 'include!("queue_completion/dispatch_roster_body.rs");'
    need(completion.count(include) == 1, "one exact roster include")
    sentinel = "__roster_production_include__"
    need(sentinel not in completion, "reserved roster marker")
    active = compact(completion.replace(include, sentinel))
    need(active.count(sentinel) == 1, "active roster include")
    before = active[:active.index(sentinel)]
    need(before.count("{") == before.count("}") and not before.endswith("]"), "root non-attributed roster include")
    hash_imports(completion)
    code = compact(completion)
    need("macro_rules!completion_hash_roster_body" not in code
         and "macro_rules!completion_roster_" not in code, "no roster or Hash macro shadow")
    for name, expected in HASH_WRAPPERS.items():
        expected = compact(expected)
        need(len(re.findall(r"fn" + name + r"[<(]", code)) == 1 and code.count(expected) == 1,
             "exact active projected hashing helper: " + name)
        before = code[:code.index(expected)]
        need(before.count("{") == before.count("}") and not before.endswith("]"), "root hashing helper")
    for expected in (
        "structCompletionOccurrenceHasherV1(Sha256);",
        "implHasherforCompletionOccurrenceHasherV1{fnfinish(&self)->u64{0}fnwrite(&mutself,bytes:&[u8]){self.0.update(bytes);}}",
        "#[derive(Clone,Copy,Debug,Eq,Hash,PartialEq)]pub(crate)structCompletionDispatchGenerationBindingV1{queue:QueueKeyV1,code:MemoryMappingKeyV1,kernarg:MemoryMappingKeyV1,dispatch_generation:u64}",
    ):
        need(code.count(expected) == 1, "exact Hash/accessor adapter")
        direct_item(code, code.index(expected))
    owned_method(code, "implCompletionPacketTemplateV1{",
                 "pub(super)constfngenerations(self)->CompletionDispatchGenerationBindingV1{self.generations}")
    need(len(re.findall(r"fngenerations[<(]", code)) == 1, "unique generation projection")


def mutations(body, roster=False):
    cases = {}
    def add(name, old, new):
        need(body.count(old) == 1, "unique binder mutation: " + name)
        cases[name] = (body.replace(old, new), "*hash_completion_dispatch_roster_projected_v1" if roster else "*bind_templates")
    if roster:
        for name, old, new in (
            ("roster-empty-error", ".ok_or(Gfx942CompletionErrorV1::ZeroPacketCount)?", ".ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?"),
            ("roster-zero-generation", "if $first.dispatch_generation == 0", "if false"),
            ("roster-first-projection", "$projection!($project, $first)", "$projection!($project, &$values[$values.len() - 1])"),
            ("roster-start-zero", "let mut $index = 1;", "let mut $index = 0;"),
            ("roster-drop-last", "while $index < $values.len() $($invariants)*", "while $index < $values.len() && $index != $values.len() - 1 $($invariants)*"),
            ("roster-ignore-queue", "$dispatch.queue != $first.queue", "false"),
            ("roster-ignore-generation", "$dispatch.dispatch_generation != $first.dispatch_generation", "false"),
            ("roster-wrong-length", "$hash_length!($values.len(), $hasher);", "$hash_length!(1, $hasher);"),
            ("roster-wrong-binding", "$hash_binding!($dispatch, $hasher);", "$hash_binding!($first, $hasher);"),
            ("roster-wrong-result", "Ok(($first.queue, $first.dispatch_generation))", "Ok(($first.queue, 0))"),
            ("roster-hash-order", "$hash_length!($values.len(), $hasher);\n            $hash_binding!($first, $hasher);",
             "$hash_binding!($first, $hasher);\n            $hash_length!($values.len(), $hasher);"),
        ):
            add(name, old, new)
        old = "                $hash_binding!($dispatch, $hasher);\n"
        need(body.count(old) == 1, "late binding hash site")
        changed = body.replace(old, "").replace("                if $dispatch.queue", old + "                if $dispatch.queue")
        cases["roster-hash-before-refusal"] = (changed, "*hash_completion_dispatch_roster_projected_v1")
        need(len(cases) == len(set(cases.values())) == 12, "distinct roster mutation roster")
        return cases
    preflight = "let $generation = $owner.preflight_templates::<$n>($queue)?;"
    add("skip-preflight", preflight, "let $generation = $owner.generation.next_generation;")
    add("rewrite-preflight-error", preflight,
        "let $generation = match $owner.preflight_templates::<$n>($queue) { Ok(value) => value, Err(_error) => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase) };")
    add("substitute-prepared-generation", "                $generation,", "                0,")
    preparation = """let $templates = prepare_dispatch_templates_v1(
                &$owner.packets,
                &$owner.code_identity,
                $queue,
                $generation,
            )?;"""
    add("rewrite-preparation-error", preparation, preparation.replace(
        "= prepare_dispatch_templates_v1", "= match prepare_dispatch_templates_v1").replace(
        ")?;", ") { Ok(value) => value, Err(_error) => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase) };"))
    add("rewrite-roster-error", "Err(error) => return Err(Gfx942DispatchBindingErrorV1::Completion(error)),",
        "Err(error) => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase),")
    add("substitute-roster-count", "$($hashed)*", "$($hashed)*\n            let $roster = CompletionDispatchRosterV1 { packet_count: 0, ..$roster };")
    add("substitute-roster-generation", "$($hashed)*", "$($hashed)*\n            let $roster = CompletionDispatchRosterV1 { dispatch_generation: 0, ..$roster };")
    add("reserve-before-roster", "$($boxed)*", "$($boxed)*\n            let _early = $owner.generation.reserve($queue, CompletionDispatchRosterV1 { queue: $queue, packet_count: $n, dispatch_generation: $generation, roster_sha256: [0; 32] })?;")
    add("reserve-before-preparation", preflight, preflight + "\n            let _early = $owner.generation.reserve($queue, CompletionDispatchRosterV1 { queue: $queue, packet_count: $n, dispatch_generation: $generation, roster_sha256: [0; 32] })?;")
    add("rewrite-reservation-error", "$owner.generation.reserve($queue, $roster)?;",
        "match $owner.generation.reserve($queue, $roster) { Ok(value) => value, Err(_error) => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase) };")
    add("poison-after-reserve", "$($reserved)*", "$($reserved)*\n            $owner.generation.poisoned = true;")
    add("substitute-returned-identity", "Ok(($templates, $identity))",
        "Ok(($templates, DispatchEpochIdentityV1 { dispatch_generation: 0, ..$identity }))")
    need(len(cases) == len(set(cases.values())) == 12, "distinct binder mutation roster")
    return cases


def selection_notes(leaf, focus=None):
    need(focus is None or focus in {"*bind_templates", "*hash_completion_dispatch_roster_projected_v1"}, "exact binder selector")
    name = "DispatchResourceOwnerV1::bind_templates" if focus != "*hash_completion_dispatch_roster_projected_v1" else "hash_completion_dispatch_roster_projected_v1"
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | ({
        "precondition not met: index in bounds for this access", "possible arithmetic underflow/overflow"
        } if focus == "*hash_completion_dispatch_roster_projected_v1" else set()),
        SELECTION_NOTES={"verifying root module (selected functions)",
            "verifying root module, function dispatch_template_bind_v1::" + name + " (selected functions)"})


def campaign(roster=False):
    audit({path: (ROOT / path).read_text() for path in FILES})
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated inherited controller")
    source = raw.decode()
    need(source.count('"--no-cheating", ') == 1, "explicit std/hash trusted campaign")
    need(source.count('"--multiple-errors", "0"') == 1, "exact diagnostic enumeration option")
    source = source.replace('"--multiple-errors", "0"', '"--multiple-errors", "1"')
    module = types.ModuleType("template_bind_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(source.replace('"--no-cheating", ', ''), module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, ROSTER_BODY if roster else BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.MULTIPLE_ERRORS = "1"
    module.mutations, module.selection_notes = lambda body: mutations(body, roster), selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-dispatch-template-bind.py"))
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, "measured full positive proof count not configured")
    roster = "--roster" in sys.argv
    if roster:
        sys.argv.remove("--roster")
    campaign(roster).main()
