#!/usr/bin/env python3
"""Reached shared binder body with explicit std-container and roster/hash trust.

The exact native wrapper has a separately source-reviewed read-only rejecting
guard. This theorem starts after that guard succeeds; it does not prove the
guard's full behavior, allocator/destructor semantics, OS effects or hardware.
Opaque owned conditional-fill storage stays in the unchanged-body frame.
"""
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
LEXER_SHA = "74c83c0776206e6c486ebb0bf9fbf6bcfdf207a1f86c22bed6054b5f7438c5fd"
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
CONDITIONAL_SOURCE = BODY.with_name("conditional_fill.rs")
CONDITIONAL_SOURCE_SHA = "67461e0f16cb8d118727332455e1641ec5174d80d71fda2abd3c6160e97737d6"
COHORT_SOURCE = BODY.with_name("native_fill_cohort.rs")
COHORT_PREMISES = BODY.parent / "native_fill_cohort/premises.rs"
ARENA_SOURCE = BODY.with_name("native_fill_arena.rs")
ARENA_PREMISES = BODY.parent / "native_fill_arena/premises.rs"
PRISTINE_SOURCE = BODY.with_name("pristine_abort.rs")
ACCOUNT_SOURCE = Path("crates/fe2o3-resource-accounting/src/lib.rs")
TABLE_SOURCE = ACCOUNT_SOURCE.with_name("host_table.rs")
# Independent source-review premises for the concrete immutable owner accessors,
# not additional Verus inputs or trusted executable theorem contracts.
CONDITIONAL_READONLY_SOURCES = {
    BODY.parent.parent / "queue_dispatch_binding.rs": "6bd2036fc379a32b5e5ef9d73a187492390e4d908a2f9641fef4e17da4b99ece",
    BODY.parent.parent / "shared_memory.rs": "47e5b54f9a16bb4d726ffb08a995ddafbb217209bb8764bcd9be3fb8905a0400",
    CONDITIONAL_SOURCE: CONDITIONAL_SOURCE_SHA,
    COHORT_SOURCE: "7afca3f847b67cf2ebfb5c61b21f815af40d84b99ea1b95ab868ee144dfaaec0",
    COHORT_PREMISES: "8ac8f7d769fb439369ac90aa45342b6d83d3b87557060b9bb71ba19efe555473",
    ARENA_SOURCE: "b693d44f22b79f0720a32065a1a98fcbe0a5228506dee06fa680a5718f8b26f2",
    ARENA_PREMISES: "73682efada874a1a079a017af13629a73f97766163db8f24eb6a1a94455a4fe7",
    PRISTINE_SOURCE: "207da422c563e2a3426bf5f990173f120d301d9d3d2a76159f065a04b6a78ed1",
    ACCOUNT_SOURCE: "1230f8e658aa54f7b629129172f2924c430adfa68dd9a1074b290cc5d782a777",
    TABLE_SOURCE: "a712150724fa106875d40acd77b8e639a7675e50cd7a7b03e1f7d14e9847d8b9",
}
# These exact edges bind the reached cohort/Arena readonly guards, including the
# original pristine-slot check and table accessor. They do not prove preparation,
# disjoint composition, independent ordering, publication or native execution.
CONDITIONAL_MODULE_EDGES = (
    (BODY.parent.parent / "queue_dispatch_binding.rs", COHORT_SOURCE,
     '#[path = "queue_dispatch_binding/native_fill_cohort.rs"]\nmod native_fill_cohort;',
     "native_fill_cohort"),
    (COHORT_SOURCE, COHORT_PREMISES,
     '#[path = "native_fill_cohort/premises.rs"]\nmod premises;', "premises"),
    (BODY.parent.parent / "queue_dispatch_binding.rs", ARENA_SOURCE,
     '#[path = "queue_dispatch_binding/native_fill_arena.rs"]\nmod native_fill_arena;',
     "native_fill_arena"),
    (ARENA_SOURCE, ARENA_PREMISES,
     '#[path = "native_fill_arena/premises.rs"]\nmod premises;', "premises"),
    (BODY.parent.parent / "queue_dispatch_binding.rs", PRISTINE_SOURCE,
     '#[path = "queue_dispatch_binding/pristine_abort.rs"]\npub(crate) mod pristine_abort;',
     "pristine_abort"),
    (ACCOUNT_SOURCE, TABLE_SOURCE, 'mod host_table;', "host_table"),
)
# Narrow, separately reviewed correspondence inside the full source capture.
# No checksum here establishes semantic or disposal authority. In particular,
# std slice access and the retained opaque owners remain review premises.
ARENA_READONLY_METHODS = (
    (CONDITIONAL_SOURCE, "impl ConditionalFillStorageV1 {",
     "pub(super) fn revalidate(&self, owner: &DispatchResourceOwnerV1) -> Result<(), Gfx942DispatchBindingErrorV1> {", "13a27a85166769a6664f1bfa6735f54b22b00dfbf7145c1c91533fbf9cf78110"),
    (ARENA_PREMISES, "impl ArenaPremisesV1 {",
     "fn require_root(&self, owner: &DispatchResourceOwnerV1) -> Result<Root, Gfx942DispatchBindingErrorV1> {", "ac193616f06f386672ff46a22c99c3a572614e05e19081621d2ddc5ec5b4d6c8"),
    (ARENA_PREMISES, "impl ArenaPremisesV1 {",
     "pub(in crate::queue::dispatch_binding) fn selected(&self, owner: &DispatchResourceOwnerV1, index: usize, queue: Option<QueueKeyV1>) -> Result<CompletedWritableRangeV1, Gfx942DispatchBindingErrorV1> {", "76257fac25545a5f9f1868f5255b6ca1615615a34c835709dd31cfe1324d4684"),
    (ARENA_PREMISES, "impl ArenaPremisesV1 {",
     "pub(in crate::queue::dispatch_binding) fn revalidate(&self, owner: &DispatchResourceOwnerV1) -> Result<(), Gfx942DispatchBindingErrorV1> {", "3a87f49c361582d0ae65f41491e53a5081b009299d4fc9503ee21fb6ba7064b7"),
    (ARENA_PREMISES, "impl Root {",
     "fn check_native(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {", "2637237e44418e42df1781796e02edbece76c96e02962afe632c9db7d1d31d14"),
    (PRISTINE_SOURCE, "impl DispatchGenerationOwnerV1 {",
     "pub(super) fn ensure_pristine(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {", "3baea45f86e90ff2d9904d34f054aa3506e5050785a155a5474fecb245c2fc95"),
    (TABLE_SOURCE, "impl<T> Deref for HostMetadataTableV1<T> {",
     "fn deref(&self) -> &Self::Target {", "bdbe6dc814ea8f74f7e4a0cd6b58de69bdc6ee3b20fbfdb2cae0ebd24194e3f3"),
    (ARENA_SOURCE, "impl ArenaOrderV1 {",
     "pub(in crate::queue) const fn packet_order(self) -> AqlDispatchOrderingV1 {", "c82e9c09bad13263c8265ca4ebe92397fe96f9b30a459fb356c7d4d14b2248c5"),
    (ARENA_SOURCE, "impl ArenaCapacityV1 {",
     "pub(in crate::queue) const fn slots(self) -> usize {", "5565899b2ef41e070f6b61c506633e8390776e2506f87d9a936ada4a23536fec"),
    (ARENA_SOURCE, "impl ArenaCapacityV1 {",
     "pub(in crate::queue) fn permits(self, order: ArenaOrderV1) -> bool {", "e6dc61c5bcf5cefe55e7cd9e55e0a62e69e17a84536f2442fa3c38dc1141f0f0"),
)
# The rejecting guard reaches these exact capacity constants and Copy selector.
# This review does not prove table allocation, 2048 publication or settlement.
ARENA_CAPACITY_ITEMS = (
    "pub const GFX942_NATIVE_FILL_ARENA_SLOTS_V1: usize = 1024;",
    "pub(super) const SLOTS: usize = GFX942_NATIVE_FILL_ARENA_SLOTS_V1;",
    "pub const GFX942_INDEPENDENT_FILL_ARENA2048_SLOTS_V1: usize = 2048;",
    "#[derive(Clone, Copy, Debug, Eq, PartialEq)] pub(in crate::queue) enum ArenaCapacityV1 { Original1024, Independent2048, }",
)
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


CONDITIONAL_GUARD = """
    match (
        &self.conditional_fill,
        self.packets.iter().any(|packet| packet.conditional_fill),
    ) {
        (Some(premises), true) => premises.revalidate(self)?,
        (None, false) => {}
        _ => return Err(Gfx942DispatchBindingErrorV1::ResourcePhase),
    }
"""
BINDER = """
pub(super) fn bind_templates<const N: usize>(&mut self, queue: QueueKeyV1)
 -> Result<(Box<[CompletionPacketTemplateV1; N]>, DispatchEpochIdentityV1,), Gfx942DispatchBindingErrorV1,> {
""" + CONDITIONAL_GUARD + """
    dispatch_bind_templates_body!(dispatch_rust_expr, self, N, queue)
}
"""


def conditional_readonly_sources(sources):
    need(set(sources) == set(CONDITIONAL_READONLY_SOURCES), "exact reviewed conditional source roster")
    for path, expected in CONDITIONAL_READONLY_SOURCES.items():
        need(hashlib.sha256(sources[path].encode()).hexdigest() == expected,
             "reviewed immutable conditional accessor source: " + str(path))
    conditional_cohort_wiring(sources)
    conditional_arena_accessors(sources)


def readonly_method(source, owner, method):
    code, owner, method = compact(source), compact(owner), compact(method)
    candidates = []
    for match in re.finditer(re.escape(owner), code):
        contents = block(code, match.end())
        if method in contents:
            direct_item(code, match.start())
            candidates.append(contents)
    need(len(candidates) == 1, "unique actual immutable method owner")
    contents = candidates[0]
    name = re.search(r"fn([a-zA-Z0-9_]+)\(", method).group(1)
    need(contents.count(method) == 1
         and len(re.findall(r"fn" + name + r"[<(]", contents)) == 1,
         "unique immutable method in actual owner")
    start = contents.index(method)
    direct_item(contents, start)
    return method + block(contents, start + len(method)) + "}"


def conditional_arena_accessors(sources):
    need(set(sources) == set(CONDITIONAL_READONLY_SOURCES), "complete readonly accessor source roster")
    for path, owner, method, expected in ARENA_READONLY_METHODS:
        actual = readonly_method(sources[path], owner, method)
        need(hashlib.sha256(actual.encode()).hexdigest() == expected,
             "reviewed exact immutable guard body: " + method)
    conditional_arena_capacity(sources[ARENA_SOURCE])
    for path, statement, name in (
        (BODY.parent.parent / "queue_dispatch_binding.rs",
         "use fe2o3_resource_accounting::{HostMetadataTableV1, ResourceCreditAccountV1};", "HostMetadataTableV1"),
        (ACCOUNT_SOURCE,
         "pub use host_table::{HostMetadataTableV1, host_metadata_table_payload_bytes_v1};", "HostMetadataTableV1"),
        (ARENA_SOURCE, "pub(super) use premises::ArenaPremisesV1;", "ArenaPremisesV1"),
        (TABLE_SOURCE, "use core::ops::{Deref, DerefMut};", "Deref"),
    ):
        source = sources[path]
        sentinel = "__readonly_accessor_import__"
        need(sentinel not in source and source.count(statement) == 1,
             "one exact readonly accessor import")
        active = compact(source.replace(statement, sentinel))
        need(active.count(sentinel) == 1, "active readonly accessor import")
        direct_item(active, active.index(sentinel))
        need(not re.search(r"\b(?:mod|as|type|struct|enum)\s+" + name + r"\b",
                           lexer()(source.replace(statement, ""))),
             "no readonly accessor shadow: " + name)


def conditional_arena_capacity(source):
    code = compact(source)
    for item in ARENA_CAPACITY_ITEMS:
        exact = compact(item)
        need(code.count(exact) == 1, "one exact closed Arena capacity item")
        direct_item(code, code.index(exact))
    # Preserve token separation for alias/shadow checks; compacting can hide `as`.
    residual = lexer()(source)
    for item in ARENA_CAPACITY_ITEMS:
        pattern = r"\s*".join(re.escape(token) for token in re.findall(r"\w+|[^\w\s]", item))
        residual, count = re.subn(pattern, "", residual)
        need(count == 1, "one active capacity declaration")
    for name in ("SLOTS", "GFX942_NATIVE_FILL_ARENA_SLOTS_V1",
                 "GFX942_INDEPENDENT_FILL_ARENA2048_SLOTS_V1", "ArenaCapacityV1"):
        need(not re.search(r"\b(?:as|mod|type|struct|enum|const|static)\s+" + name + r"\b", residual),
             "no closed capacity shadow: " + name)


def conditional_cohort_wiring(sources):
    need(set(sources) == set(CONDITIONAL_READONLY_SOURCES), "complete conditional module source roster")
    for owner, target, declaration, name in CONDITIONAL_MODULE_EDGES:
        need(target in sources, "retained complete cohort module")
        source = sources[owner]
        sentinel = "__conditional_cohort_module_" + name + "__"
        need(sentinel not in source and source.count(declaration) == 1,
             "one exact cohort module: " + name)
        active = compact(source.replace(declaration, sentinel))
        need(active.count(sentinel) == 1, "active cohort module: " + name)
        direct_item(active, active.index(sentinel))
        need(not re.search(r"\b(?:mod|as)\s+" + name + r"\b",
                           lexer()(source.replace(declaration, ""))),
             "no cohort module shadow: " + name)


def conditional_guard_wiring(binding, source):
    # Every revalidate branch borrows its exact owners; the pinned implementation
    # only compares retained fields and numeric ranges. No owner is moved/dropped
    # by this guard. This source review is not a theorem about opaque F's Drop.
    need(hashlib.sha256(source.encode()).hexdigest() == CONDITIONAL_SOURCE_SHA,
         "reviewed read-only conditional guard implementation")
    declaration = '#[path = "queue_dispatch_binding/conditional_fill.rs"]\nmod conditional_fill;'
    sentinel = "__conditional_guard_module__"
    need(sentinel not in binding and binding.count(declaration) == 1,
         "one exact conditional guard module")
    active = compact(binding.replace(declaration, sentinel))
    need(active.count(sentinel) == 1, "active conditional guard module")
    direct_item(active, active.index(sentinel))
    need(not re.search(r"\b(?:mod|as)\s+conditional_fill\b",
                       lexer()(binding.replace(declaration, ""))),
         "no conditional guard module shadow")


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
    conditional_readonly_sources({path: (ROOT / path).read_text() for path in CONDITIONAL_READONLY_SOURCES})
    binding = (ROOT / BODY.parent.parent / "queue_dispatch_binding.rs").read_text()
    conditional_guard_wiring(binding, (ROOT / CONDITIONAL_SOURCE).read_text())
    binder_wiring(binding, (ROOT / BODY).read_text())
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
