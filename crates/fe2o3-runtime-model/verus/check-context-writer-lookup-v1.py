"""Portable readonly-selector correspondence; never invokes Cargo or Verus."""
import hashlib
import json
import os
import stat
from pathlib import Path
import re
import types

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PREFIX = "crates/fe2o3-runtime/src/context/"
CONTEXT = "crates/fe2o3-runtime/src/context.rs"
VERSIONS = PREFIX + "versions.rs"
SUBMISSIONS = PREFIX + "versions/submissions.rs"
LOOKUP = PREFIX + "versions/submissions/writer_lookup.rs"
BODY = PREFIX + "versions/submissions/writer_lookup_body.rs"
PROOF = "crates/fe2o3-runtime-model/verus/context_writer_lookup_v1.rs"
PROOF_SHA = "8ca891942dad1dcccf1611d944dbd1ca2dcba0ea07fdca2c4c55999819ad78bd"
LEXER_SHA = "74c83c0776206e6c486ebb0bf9fbf6bcfdf207a1f86c22bed6054b5f7438c5fd"
LEXER = HERE / "check-negative-quality.py"
LIB = "crates/fe2o3-runtime/src/lib.rs"
FILES = (
    CONTEXT, VERSIONS, SUBMISSIONS, LOOKUP, BODY, PROOF, LIB,
    PREFIX + "versions/producer_input_preflight_tests.rs",
    PREFIX + "versions/producer_readers.rs",
    PREFIX + "versions/readers.rs",
    PREFIX + "versions/settlement_bodies.rs",
    PREFIX + "versions/submissions/writer_lookup_tests.rs",
    "crates/fe2o3-runtime-model/src/context_queued_writers/declarations.rs",
    "crates/fe2o3-runtime-model/src/context_version_journal/declarations.rs",
)
REFERENCE = HERE / "pins/CONTEXT_WRITER_LOOKUP_PREDECESSOR_V1.json"
REFERENCE_SHA = "adb0e36676a9a5fcfc59bf2c95739dbb19cdacab2add51ede4666d7cb236793e"
BASE_PINS = {
    SUBMISSIONS: "97325d6fbf523de0b7638354b8c5a347cc35f58a82acafbc9cb61355e83a9857",
    VERSIONS: "cf9540406ce47355154d47ef573a4733b5ff4d264e1d02b794e97fb0b9815faf",
}
MANIFEST = HERE / "pins/CONTEXT_WRITER_LOOKUP_INPUTS_V1.json"
MANIFEST_PIN = HERE / "pins/CONTEXT_WRITER_LOOKUP_INPUTS_V1_SHA256"
SUPPORT = HERE / "pins/CONTEXT_WRITER_LOOKUP_SUPPORT_V1.json"
SUPPORT_PIN = HERE / "pins/CONTEXT_WRITER_LOOKUP_SUPPORT_V1_SHA256"
SUPPORT_FILES = (
    Path(__file__).resolve(),
    HERE / "test-context-writer-lookup-v1.py",
    HERE / "context-writer-lookup-mutations-v1.py",
    HERE / "context-writer-lookup-diagnostics-v1.py",
    HERE / "test-context-writer-lookup-diagnostics-v1.py",
    HERE / "qualify-context-writer-lookup-v1.py",
    HERE / "test-qualify-context-writer-lookup-v1.py",
    HERE / "CONTEXT_WRITER_LOOKUP_BOUNDARY.md",
    HERE / "pins/CONTEXT_WRITER_LOOKUP_MUTATIONS_V1.json",
    HERE / "pins/CONTEXT_WRITER_LOOKUP_POSITIVE_V1.json",
    HERE / "pins/CONTEXT_WRITER_LOOKUP_DIAGNOSTICS_V1.json",
    HERE / "pins/CONTEXT_WRITER_LOOKUP_TOOLCHAIN.toml",
    REFERENCE, MANIFEST, MANIFEST_PIN, LEXER,
)


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def ordinary(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path,
         "ordinary canonical input: " + str(path))
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_size <= 1024**2,
             "bounded regular input")
        with os.fdopen(fd, "rb", closefd=False) as handle:
            raw = handle.read(1024**2 + 1)
        after = os.fstat(fd)
        identity = lambda value: (value.st_dev, value.st_ino, value.st_mode, value.st_size,
                                 value.st_uid, value.st_gid, value.st_mtime_ns, value.st_ctime_ns)
        need(len(raw) == before.st_size and identity(before) == identity(after),
             "stable complete input")
        return raw, stat.S_IMODE(before.st_mode)
    finally:
        os.close(fd)


def unique_json(raw):
    def pairs(items):
        result = {}
        for name, value in items:
            need(name not in result, "duplicate JSON key")
            result[name] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs)


def record(raw, mode):
    return dict(bytes=len(raw), mode=mode, sha256=digest(raw))


def validate_input_roster(rows):
    need(set(rows) == set(FILES), "closed fourteen-file source roster")
    return rows


def inputs():
    raw, mode = ordinary(MANIFEST)
    sidecar, sidecar_mode = ordinary(MANIFEST_PIN)
    need(mode == sidecar_mode == 0o644 and sidecar == (digest(raw) + "\n").encode(),
         "input manifest byte pin")
    rows = validate_input_roster(unique_json(raw))
    values = {}
    for name in FILES:
        contents, file_mode = ordinary(ROOT / name)
        need(record(contents, file_mode) == rows[name], "exact source bytes/mode: " + name)
        values[name] = contents.decode()
    return values


def support_check():
    raw, mode = ordinary(SUPPORT)
    sidecar, sidecar_mode = ordinary(SUPPORT_PIN)
    need(mode == sidecar_mode == 0o644 and sidecar == (digest(raw) + "\n").encode(),
         "support manifest byte pin")
    actual = {}
    for path in SUPPORT_FILES:
        contents, file_mode = ordinary(path)
        actual[str(path.relative_to(ROOT))] = record(contents, file_mode)
    need(unique_json(raw) == actual, "closed support roster and transitive pins")
    return actual


raw, _ = ordinary(LEXER)
need(digest(raw) == LEXER_SHA, "pinned existing Rust lexical helper")
lexical = types.ModuleType("writer_lookup_lexical")
lexical.__file__ = str(LEXER)
exec(compile(raw, lexical.__file__, "exec"), lexical.__dict__)
CODE = lexical.code_only


def compact(source):
    return re.sub(r",([)}])", r"\1", re.sub(r"\s+", "", CODE(source)))


def block(source, start):
    depth, end = 1, start
    while depth and end < len(source):
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    need(depth == 0, "balanced declaration")
    return source[start:end - 1], end


def fields(source, name):
    source = re.sub(r"#\[cfg\(test\)\]\s*(?:mixed_input_fault|completion_fault):[^,\n]+,", "", source)
    code = compact(source)
    hits = list(re.finditer(r"struct" + name + r"(?:<[^{};]+>)?\{", code))
    need(len(hits) == 1, "unique field declaration " + name)
    body, _ = block(code, hits[0].end())
    parts, start, depth = [], 0, 0
    for index, char in enumerate(body):
        depth += (char in "(<[{") - (char in ")>]}")
        if char == "," and depth == 0:
            parts.append(body[start:index])
            start = index + 1
    parts.append(body[start:])
    result = {}
    for part in parts:
        name, kind = re.sub(r"^pub(?:\([^)]*\))?", "", part).split(":", 1)
        need(name not in result, "distinct fields")
        result[name] = kind
    return result


def originals():
    raw, mode = ordinary(REFERENCE)
    need(mode == 0o644 and digest(raw) == REFERENCE_SHA,
         "exact authenticated predecessor capsule")
    reference = unique_json(raw)
    need(reference["commit"] == "e516fdda72ac7e4b9ea906bff9ed567b841a8f00"
         and reference["tree"] == "71d4f4a59c0da029087435ddb54e4cbd89315b50"
         and reference["original_sha256"] == BASE_PINS,
         "exact signed predecessor identities")
    values = reference["originals"]
    need(set(values) == set(BASE_PINS), "closed original source roster")
    for name, text in values.items():
        need(digest(text.encode()) == BASE_PINS[name], "original bytes " + name)
    return values


def root_item_prefix(prefix, message):
    stack = []
    closing = {")": "(", "]": "[", "}": "{"}
    for token in prefix:
        if token in "([{":
            stack.append(token)
        elif token in closing:
            need(stack and stack.pop() == closing[token], "balanced module prefix")
    need(not stack and (not prefix.strip() or prefix.rstrip()[-1] in ";}"), message)


def root_module(source, name):
    code = CODE(source)
    hits = list(re.finditer(r"\bmod\s+" + re.escape(name) + r"\s*;", code))
    need(len(hits) == 1, "one active module " + name)
    prefix = code[:hits[0].start()]
    prefix = re.sub(r"\bpub(?:\s*\([^()]*\))?\s*$", "", prefix)
    root_item_prefix(prefix, "root module has no hidden attributes/path " + name)


def root_literal_item(source, item, marker):
    need(source.count(item) == 1, "one exact literal-bearing item")
    need(marker not in source, "reserved literal-item marker")
    active = CODE(source.replace(item, marker + ";"))
    hits = list(re.finditer(r"\b" + re.escape(marker) + r"\s*;", active))
    need(len(hits) == 1, "active literal-bearing item")
    root_item_prefix(active[:hits[0].start()], "root item has no additional attributes")


def root_include(source, include):
    root_literal_item(source, include, "__writer_lookup_include__")


TEST_MODULE = '#[cfg(test)]\n#[path = "writer_lookup_tests.rs"]\nmod tests;'


def root_test_module(source):
    root_literal_item(source, TEST_MODULE, "__writer_lookup_test_module__")


def correspondence(inputs, before):
    old = before[SUBMISSIONS]
    start = old.index("            let record = self.submissions.get(&id);", old.index("fn settle_writer_v1("))
    end = old.index("            #[cfg(test)]", start)
    original_prefix = old[start:end]
    code = compact(inputs[BODY])
    opening = ("macro_rules!completion_writer_lookup_body_v1{"
               "($syntax:ident,$submissions:ident,$versions:ident,$id:ident,$domain:ident)"
               "=>{$syntax!({")
    need(code.startswith(opening) and code.endswith("})};}"), "one exact shared macro arm")
    body = code[len(opening):-len("})};}")]
    need(body.endswith("Ok(Some(writer))"), "selected original reference result")
    body = body[:-len("Ok(Some(writer))")]
    for left, right in (("$submissions", "self.submissions"),
                        ("$versions", "self.versions.as_mut()"),
                        ("$id", "id"), ("$domain", "domain")):
        body = body.replace(left, right)
    for field in ("journal_read", "journal_producer_read"):
        matched = ("matchrecord{Some(record)=>record." + field
                   + ".is_some(),None=>false}")
        original = "record.is_some_and(|record|record." + field + ".is_some())"
        need(body.count(matched) == 1, "exact explicit Option predicate " + field)
        body = body.replace(matched, original, 1)
    expected_match = "matchrecord{Some(record)=>record.journal_writer,None=>None}"
    need(body.count(expected_match) == 1, "exact explicit original writer Option")
    body = body.replace(expected_match, "record.and_then(|record|record.journal_writer)", 1)
    need(body.replace("Ok(None)", "Ok(())") == compact(original_prefix),
         "actual original lookup predicates and order")

    expected = """            let Some(writer) = writer_lookup::preflight_settlement_writer_v1(
                &self.submissions, self.versions.as_ref(), id, domain,
            )? else { return Ok::<(), ContextVersionJournalErrorV1>(()); };
            let versions = self.versions.as_mut().ok_or(ContextVersionJournalErrorV1::InvalidReference)?;
"""
    replaced = old[:start] + expected + old[end:]
    forwarding = """mod writer_lookup;
#[cfg(test)]
pub(super) fn preflight_settlement_writer_for_test_v1(
    submissions: &HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1>,
    versions: Option<&ContextVersionsV1>, id: RuntimeSubmissionIdV1,
    domain: SubmissionWriterDomainV1,
) -> Result<Option<ContextWriterReferenceV1>, ContextVersionJournalErrorV1> {
    writer_lookup::preflight_settlement_writer_v1(submissions, versions, id, domain)
}
"""
    replaced = replaced.replace("// Context owns this root", forwarding + "// Context owns this root", 1)
    need(compact(inputs[SUBMISSIONS]) == compact(replaced),
         "only lookup extraction; unchanged catch, effects, retirement and quarantine")
    wrapper = inputs[LOOKUP]
    include = 'include!("writer_lookup_body.rs");'
    root_include(wrapper, include)
    root_test_module(wrapper)
    expected_wrapper = """use super::*;
include!("writer_lookup_body.rs");
pub(super) fn preflight_settlement_writer_v1(
    submissions: &HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1>,
    versions: Option<&ContextVersionsV1>, id: RuntimeSubmissionIdV1,
    domain: SubmissionWriterDomainV1,
) -> Result<Option<ContextWriterReferenceV1>, ContextVersionJournalErrorV1> {
    completion_writer_lookup_body_v1!(completion_journal_rust_syntax, submissions, versions, id, domain)
}
#[cfg(test)]
#[path = "writer_lookup_tests.rs"]
mod tests;
"""
    need(compact(wrapper) == compact(expected_wrapper), "closed runtime helper and test module")
    need(inputs[VERSIONS] == before[VERSIONS], "existing identity syntax adapter unchanged")
    need(digest(inputs[PROOF].encode()) == PROOF_SHA, "reviewed semantic contracts and proof bodies")
    root_include(inputs[PROOF],
                 'include!("../../fe2o3-runtime/src/context/versions/submissions/writer_lookup_body.rs");')
    need("completion_writer_lookup_body_v1!(verus_exec_expr,submissions,versions,id,domain)"
         in compact(inputs[PROOF]), "actual proof specialization")
    producer_tests = inputs[PREFIX + "versions/producer_input_preflight_tests.rs"]
    need("completion_writer_lookup_body_v1!" not in producer_tests,
         "producer fixtures do not use a separate macro specialization")
    need(compact(producer_tests).count(
         "submissions::preflight_settlement_writer_for_test_v1(&owner.submissions,owner.versions.as_ref(),id(),SubmissionWriterDomainV1::Ordinary)") == 1,
         "producer fixtures reach actual private runtime helper through cfg(test) forwarder")
    specs = (
        (CONTEXT, "SubmissionRecordV1", 15,
         {"journal_writer": "Option<ContextWriterReferenceV1>", "journal_read": "Option<R>",
          "journal_producer_read": "Option<P>"}),
        (SUBMISSIONS, "RetainedSubmissionWriterV1", 10,
         {"allocations": "O", "members": "O", "queued": "Option<O>"}),
        (VERSIONS, "ContextVersionsV1", 7,
         {"journal": "O", "phases": "O",
          "submission_writers": "HashMap<RuntimeSubmissionIdV1,RetainedSubmissionWriterV1<O>>",
          "submission_readers": "HashMap<RuntimeSubmissionIdV1,O>",
          "producer_readers": "HashMap<RuntimeSubmissionIdV1,O>",
          "disposal_groups": "HashMap<RuntimeSubmissionIdV1,O>",
          "disposal_allocations": "HashMap<RuntimeAllocationIdV1,O>"}),
    )
    for path, name, count, substitutions in specs:
        actual = fields(inputs[path], name)
        projected = dict(actual)
        need(set(substitutions) <= set(actual), "explicit existing projection fields")
        projected.update(substitutions)
        need(len(actual) == count and fields(inputs[PROOF], name) == projected,
             "full field names/types with explicit opaque projection: " + name)
    return {"shared_predicates": True, "field_counts": [15, 10, 7],
            "runtime_catch_and_effect_suffix_unchanged": True,
            "queued_journal": "opaque, not flat JournalContents",
            "cargo_runs": 0, "solver_runs": 0}


def audit(inputs):
    need(set(inputs) == set(FILES), "exact source correspondence input roster")
    root_module(inputs[LIB], "context")
    root_module(inputs[CONTEXT], "versions")
    root_module(inputs[VERSIONS], "submissions")
    root_module(inputs[SUBMISSIONS], "writer_lookup")
    return correspondence(inputs, originals())


if __name__ == "__main__":
    support_check()
    print(json.dumps(audit(inputs()), sort_keys=True))
