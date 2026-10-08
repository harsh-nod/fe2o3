#!/usr/bin/env python3
"""Source-bound writer-lookup reports. Draft policies cannot accept any case."""
import hashlib
import json
import os
import re
from pathlib import Path
import stat
import sys

VERUS = {"profile": "release", "version": "0.2026.08.09.92f466f",
         "platform": {"os": "linux", "arch": "x86_64"},
         "toolchain": "1.97.1-x86_64-unknown-linux-gnu",
         "commit": "92f466f247f45128c630d1c843fd6e27d2115587"}
LIMIT = 4 * 1024 * 1024
TOOL_MANIFEST_SHA = "c0c5a02ae66500676770d5c4d4c13ac316b0e057398896c978ae90e6e37ad199"
LOGICAL = {"assertion failed", "postcondition not satisfied", "precondition not satisfied",
           "invariant not satisfied at end of loop body", "invariant not satisfied before loop"}
INCLUDE_ALIASES = {
    "crates/fe2o3-runtime-model/verus/../../fe2o3-runtime/src/context/versions/submissions/writer_lookup_body.rs":
    "crates/fe2o3-runtime/src/context/versions/submissions/writer_lookup_body.rs",
}
PREFIX = "context_writer_lookup_v1::"
KEYS = {"encountered-error", "encountered-vir-error", "success", "verified", "errors", "is-verifying-entire-crate"}
PROOF = "crates/fe2o3-runtime-model/verus/context_writer_lookup_v1.rs"
BODY = "crates/fe2o3-runtime/src/context/versions/submissions/writer_lookup_body.rs"
PROOF_SHA = "8ca891942dad1dcccf1611d944dbd1ca2dcba0ea07fdca2c4c55999819ad78bd"
BODY_SHA = "fb21d538e2622078336188f9e65a9ae4ec81c6dfa9e320546620191585a4af77"
POSITIVE_SHA = "c2c2b3621d2bddb7c1d7604fb11617941af9b2b887a68c4089b60578fbaeb366"
MUTATIONS_SHA = "d1fd28512dcb0633cbbc305a4880f94204fdcc27e60943eaf499647cb340b42e"
HERE = Path(__file__).resolve().parent
CASES = (
    "expected_writer_erased", "no_versions_accepts_reference", "missing_writer_accepts_reference",
    "empty_absence_rejected", "reader_check_after_missing_writer", "reader_error_is_reference",
    "submission_reader_ignored", "producer_reader_ignored", "record_reader_ignored",
    "record_producer_reader_ignored", "domain_check_omitted", "generated_stream_ignored",
    "generated_hold_ignored", "generated_shell_ignored", "expected_reference_ignored",
    "key_generation_ignored", "key_local_ignored", "key_kind_ignored",
    "return_none_for_writer", "return_slot_zero",
)
SCOPE = "shared-readonly-writer-lookup; one-key-law-premise; opaque-queued-owner/full-Context/native/unwind-excluded"
OBSERVATION_FIELDS = {"inputs", "summary", "functions", "application_declarations",
                      "report_sha256", "diagnostics", "warnings", "logical_sites"}
INTENDED = PREFIX + "preflight_settlement_writer_v1"

def need(value, message):
    if not value:
        raise ValueError(message)

def parse(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, "duplicate JSON key")
            result[key] = value
        return result
    need(type(raw) is str and len(raw.encode()) <= LIMIT, "bounded UTF-8 JSON")
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError("non-finite JSON")))

def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"),
                                    allow_nan=False).encode()).hexdigest()

def diagnostic_member(item, root, files):
    need(type(item) is str, "source diagnostic text")
    path = Path(item)
    need(path.is_absolute() and str(path) == item, "absolute exact diagnostic spelling")
    raw_relative = path.relative_to(root).as_posix()
    normalized = Path(os.path.normpath(item))
    relative = normalized.relative_to(root).as_posix()
    need(relative in files, "exact captured source diagnostic")
    need(path == normalized or INCLUDE_ALIASES.get(raw_relative) == relative,
         "only the guarded shared-body compiler include alias")
    # Check even the directories that lexical '..' will cancel.
    cursor = root
    for part in Path(raw_relative).parts:
        cursor = cursor.parent if part == ".." else cursor / part
        need(cursor.is_relative_to(root), "diagnostic never escapes frozen root")
        need(not cursor.is_symlink(), "no diagnostic symlink component")
    need(path.resolve() == normalized and normalized.is_file(),
         "resolved path is the exact captured regular member")
    return relative

def trusted_context(trusted):
    if trusted is None:
        return None
    need(type(trusted) is tuple and len(trusted) == 2 and type(trusted[1]) is bytes,
         "explicit original tool root and immutable manifest buffer")
    root, raw = Path(trusted[0]), trusted[1]
    need(len(raw) <= LIMIT and hashlib.sha256(raw).hexdigest() == TOOL_MANIFEST_SHA,
         "exact original complete tool manifest")
    records = parse(raw.decode())["verus"]
    need(type(records) is dict and len(records) == 190, "complete pinned Verus roster")
    need(root.is_absolute() and root.resolve() == root, "canonical separate tool root")
    need(all(not p.is_symlink() for p in (root, *root.parents)), "no tool root symlink")
    return root, records

def trusted_span(span, context):
    fields = {"byte_end", "byte_start", "column_end", "column_start", "expansion", "file_name",
              "is_primary", "label", "line_end", "line_start", "suggested_replacement",
              "suggestion_applicability", "text"}
    need(type(span) is dict and set(span) == fields and span["is_primary"] is False
         and span["suggested_replacement"] is None and span["suggestion_applicability"] is None
         and span["expansion"] is None and span["text"] == []
         and (span["label"] is None or type(span["label"]) is str),
         "only exact auxiliary external-contract span, never a primary failure")
    name = span["file_name"]
    need(type(name) is str, "trusted member spelling")
    relative = Path(name)
    root, records = context
    need(not relative.is_absolute() and relative.as_posix() == name
         and bool(relative.parts) and relative.parts[0] == "vstd" and relative.suffix == ".rs"
         and all(part not in (".", "..") for part in relative.parts)
         and name in records, "exact relative pinned vstd member, not a path fallback")
    path = root
    for part in relative.parts:
        path = path / part
        need(not path.is_symlink(), "no trusted member symlink component")
    need(path.resolve() == path, "resolved original trusted member")
    record = records[name]
    need(type(record["bytes"]) is int and 0 < record["bytes"] <= LIMIT, "bounded trusted source")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        observed = os.fstat(fd)
        need(stat.S_ISREG(observed.st_mode) and observed.st_size == record["bytes"],
             "regular original trusted member")
        with os.fdopen(os.dup(fd), "rb") as stream:
            data = stream.read(record["bytes"] + 1)
        after = os.fstat(fd)
        identity = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid,
                              s.st_size, s.st_mtime_ns, s.st_ctime_ns)
        need(identity(observed) == identity(after), "stable trusted member buffer")
    finally:
        os.close(fd)
    need({"mode": stat.S_IMODE(observed.st_mode), "bytes": len(data),
          "sha256": hashlib.sha256(data).hexdigest()} == record,
         "original trusted bytes, mode and extent")
    # The existing recommendation-span checker uses this exact ASCII byte/line join.
    need(data.isascii(), "ASCII trusted span coordinates")
    lines = data.splitlines(keepends=True)
    coordinates = ("byte_start", "byte_end", "line_start", "line_end", "column_start", "column_end")
    need(all(type(span[key]) is int for key in coordinates)
         and 1 <= span["line_start"] <= span["line_end"] <= len(lines)
         and 0 <= span["byte_start"] < span["byte_end"] <= len(data),
         "bounded trusted coordinates")
    first, last = (lines[span[key] - 1].rstrip(b"\r\n") for key in ("line_start", "line_end"))
    need(1 <= span["column_start"] <= len(first) + 1
         and 1 <= span["column_end"] <= len(last) + 1, "bounded trusted columns")
    start = sum(map(len, lines[:span["line_start"] - 1])) + span["column_start"] - 1
    end = sum(map(len, lines[:span["line_end"] - 1])) + span["column_end"] - 1
    need((start, end) == (span["byte_start"], span["byte_end"]), "exact trusted byte/line join")
    return {"namespace": "trusted-verus", "member": name, "sha256": record["sha256"]}

def normalized_diagnostics(raw, root, files, trusted=None):
    root = Path(root)
    need(root.is_absolute() and root.resolve() == root, "canonical source root")
    files = {str(p) for p in files}
    context = trusted_context(trusted)
    if context is not None:
        need(not root.is_relative_to(context[0]) and not context[0].is_relative_to(root),
             "application and trusted-tool namespaces are disjoint")
    tool_spans = []

    def visit(value):
        if isinstance(value, list):
            return [visit(item) for item in value]
        if not isinstance(value, dict):
            return value
        result = {}
        for key, item in value.items():
            if key == "rendered":
                need(type(item) is str, "text human rendering")
            elif key == "file_name":
                if type(item) is str and Path(item).is_absolute():
                    result[key] = diagnostic_member(item, root, files)
                else:
                    need(context is not None, "relative diagnostic requires authenticated tool context")
                    result[key] = trusted_span(value, context)
                    tool_spans.append(result[key])
            else:
                result[key] = visit(item)
        return result

    need(type(raw) is str and len(raw.encode()) <= LIMIT, "bounded stderr")
    rows = [parse(line) for line in raw.splitlines() if line.strip()]
    normalized = []
    for row in rows:
        need(type(row) is dict and row.get("$message_type") == "diagnostic"
             and row.get("code") is None and row.get("level") in ("error", "note"),
             "only reviewed logical diagnostics, never frontend codes")
        count = len(tool_spans)
        selected = visit(row)
        if len(tool_spans) != count:
            need(any(span.get("is_primary") is True and type(span.get("file_name")) is str
                     and span["file_name"] in files for span in selected.get("spans", [])),
                 "trusted auxiliary requires a real captured application primary in the same diagnostic")
        normalized.append(selected)
    return normalized

def report(paths, raw, negative):
    value=paths.parse(raw.decode())
    paths.need(type(value) is dict and set(value)=={'func-details','verification-results','verus'},
               'complete report schema')
    paths.need(value['verus']==paths.VERUS,'original Verus identity')
    v=value['verification-results']
    paths.need(type(v) is dict and set(v)==KEYS,'complete counters')
    for name,expected in [('encountered-error',negative),('encountered-vir-error',False),
                          ('success',not negative),('is-verifying-entire-crate',True)]:
        paths.need(type(v[name]) is bool and v[name] is expected,'exact logical outcome flags')
    paths.need(type(v['verified']) is int and type(v['errors']) is int
               and v['verified']>=0 and v['errors']>=0 and v['verified']+v['errors']==16
               and (v['errors']>0 if negative else v['errors']==0), 'whole sixteen obligations')
    functions=value['func-details']
    paths.need(type(functions) is dict and 0<len(functions)<=10000,'bounded complete function report')
    for name,notes in functions.items():
        paths.need(type(name) is str and notes=={'obligation_proof_notes':[],'failed_proof_notes':[]},
                   'no selected or trusted proof notes')
    return value

def warning(paths, row, root, files, trusted):
    paths.need(row.get('$message_type')=='diagnostic' and row.get('level')=='warning',
               'original warning schema')
    code=row.get('code')
    paths.need(code is None or code=={'code':'unused_macros','explanation':None},
               'no unreviewed warning code')
    root=Path(root)
    paths.need(root.is_absolute() and root.resolve()==root,'canonical warning source root')
    def relocate(value):
        if isinstance(value,list):return [relocate(item) for item in value]
        if not isinstance(value,dict):
            return value.replace(str(root),'<CAPTURED_SOURCE>') if type(value) is str else value
        result={}
        for key,item in value.items():
            if key=='rendered':
                paths.need(item is None or type(item) is str,'warning rendering is text or null')
            if key=='file_name':
                paths.need(type(item) is str,'warning path spelling')
                if item.startswith(str(root)+'/'):
                    paths.diagnostic_member(item,root,files)
            result[key]=relocate(item)
        return result
    # External compiler spellings are compared as exact warning data, not source authority.
    return relocate(row)


def source_snapshot(root, files):
    root = Path(root)
    need(root.is_absolute() and root.resolve() == root, "canonical captured source")
    need(set(files) == {PROOF, BODY, "rust-toolchain.toml"}, "exact three-file compiled source")
    result, buffers = {}, {}
    for name in sorted(files):
        diagnostic_member(str(root / name), root, files)
        fd = os.open(root / name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
        try:
            before = os.fstat(fd)
            need(stat.S_ISREG(before.st_mode) and before.st_size <= LIMIT, "bounded regular source")
            with os.fdopen(os.dup(fd), "rb") as stream:
                raw = stream.read(LIMIT + 1)
            after = os.fstat(fd)
            identity = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
            need(identity(before) == identity(after) and len(raw) == before.st_size, "stable source buffer")
            result[name] = {"bytes": len(raw), "mode": stat.S_IMODE(before.st_mode), "sha256": hashlib.sha256(raw).hexdigest()}
            buffers[name] = raw
        finally:
            os.close(fd)
    return result, buffers


def source_records(root, files):
    return source_snapshot(root, files)[0]


def pinned_json(name, expected):
    path = HERE / "pins" / name
    need(path.resolve() == path and not path.is_symlink(), "canonical pinned fixture")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and stat.S_IMODE(before.st_mode) == 0o644
             and 0 < before.st_size <= LIMIT, "bounded regular pinned fixture")
        with os.fdopen(os.dup(fd), "rb") as stream:
            raw = stream.read(LIMIT + 1)
        after = os.fstat(fd)
        identity = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid,
                              s.st_size, s.st_mtime_ns, s.st_ctime_ns)
        need(identity(before) == identity(after) and len(raw) == before.st_size
             and hashlib.sha256(raw).hexdigest() == expected, "exact stable fixture buffer")
        return parse(raw.decode())
    finally:
        os.close(fd)


def references():
    positive = pinned_json("CONTEXT_WRITER_LOOKUP_POSITIVE_V1.json", POSITIVE_SHA)
    mutants = pinned_json("CONTEXT_WRITER_LOOKUP_MUTATIONS_V1.json", MUTATIONS_SHA)
    need(positive["maintained_qualification"] is False and positive["negative_credit"] is False,
         "historical component positive is not maintained acceptance")
    need(mutants["state"] == "NOT_RUN" and mutants["accepted"] is False
         and mutants["proof_sha256"] == PROOF_SHA and mutants["body_before_sha256"] == BODY_SHA
         and set(mutants["cases"]) == set(CASES), "exact twenty unrun constructions")
    base = positive["observation"]
    value = report(sys.modules[__name__], json.dumps(positive["report"]).encode(), False)
    need(base["summary"] == value["verification-results"] and digest(value) == base["report_sha256"]
         and base["functions"] == sorted(value["func-details"]) and len(base["functions"]) == 182
         and base["application_declarations"] == sorted(n for n in value["func-details"] if n.startswith(PREFIX))
         and len(base["application_declarations"]) == 51, "complete original positive")
    need(base["inputs"][PROOF]["sha256"] == PROOF_SHA and base["inputs"][BODY]["sha256"] == BODY_SHA,
         "original theorem and body")
    return positive, mutants


def named_inputs(case, positive, mutants):
    need(case == "positive" or case in CASES, "closed named source")
    result = {name: dict(row) for name, row in positive["observation"]["inputs"].items()}
    if case != "positive":
        row = mutants["cases"][case]
        result[BODY] = {key: row[key] for key in ("bytes", "mode", "sha256")}
    return result


def warning_frame(warnings, diagnostics, case, positive):
    original = positive["observation"]["warnings"]
    if case == "positive":
        need(warnings == original, "exact complete positive warning rows")
        return
    need(case in CASES and len(original) == 2 and type(warnings) is list
         and len(warnings) == 1 and warnings[0] == original[0],
         "exact complete original Clone warning on negative")
    errors = [row for row in diagnostics
              if row.get("level") == "error" and row.get("message") in LOGICAL]
    need(errors, "located negative errors before abort summary")
    count = len(errors)
    message = ("aborting due to " + str(count) + " previous "
               + ("error" if count == 1 else "errors") + "; 1 warning emitted")
    summary = {"$message_type": "diagnostic", "children": [], "code": None,
               "level": "error", "message": message, "spans": []}
    need(diagnostics[-1] == summary
         and [row for row in diagnostics if row.get("level") == "error"
              and row.get("message") not in LOGICAL] == [summary],
         "one exact final negative error and warning-count summary")


def observation_check(row, case, positive, mutants):
    need(type(row) is dict and set(row) == OBSERVATION_FIELDS, "closed complete observation")
    base = positive["observation"]
    need(row["inputs"] == named_inputs(case, positive, mutants), "exact case bytes and modes")
    need(row["functions"] == base["functions"]
         and row["application_declarations"] == base["application_declarations"],
         "exact complete roster; no standard-library or application omissions")
    warning_frame(row["warnings"], row["diagnostics"], case, positive)
    summary = row["summary"]
    value = {"verus": VERUS, "verification-results": summary,
             "func-details": {name: {"obligation_proof_notes": [], "failed_proof_notes": []}
                              for name in row["functions"]}}
    report(sys.modules[__name__], json.dumps(value).encode(), case != "positive")
    need(digest(value) == row["report_sha256"], "exact whole-report fingerprint")
    if case == "positive":
        need(row == base, "exact retained positive observation")
    else:
        need(type(row["diagnostics"]) is list and row["diagnostics"]
             and type(row["logical_sites"]) is list and row["logical_sites"],
             "calibrated located logical failures required")
        for site in row["logical_sites"]:
            need(type(site) is dict and set(site) == {"message", "file", "line", "declaration"}
                 and site["message"] in LOGICAL and site["file"] in (PROOF, BODY)
                 and type(site["line"]) is int and site["line"] > 0
                 and site["declaration"] == INTENDED, "only the intended actual declaration")
    return True


def policy_check(policy, *, active=True):
    need(type(policy) is dict and set(policy) ==
         {"schema", "state", "scope", "proof_sha256", "positive", "cases"}, "closed policy schema")
    need(policy["schema"] == "fe2o3.context-writer-lookup.diagnostics.v1"
         and policy["scope"] == SCOPE and policy["proof_sha256"] == PROOF_SHA, "exact conditional scope")
    need(policy["state"] == ("reviewed-calibrated" if active else "draft-review-required"),
         "explicit policy activation after separate review")
    positive, mutants = references()
    observation_check(policy["positive"], "positive", positive, mutants)
    need(type(policy["cases"]) is dict and set(policy["cases"]) == set(CASES), "exact twenty policies")
    for name, row in policy["cases"].items():
        need(type(row) is dict and set(row) ==
             {"state", "inputs", "intended_declarations", "observation"}, "closed named policy")
        need(row["inputs"] == named_inputs(name, positive, mutants)
             and row["intended_declarations"] == [INTENDED], "source-bound intended failure")
        if active:
            need(row["state"] == "reviewed-calibrated", "each case independently calibrated and reviewed")
            observation_check(row["observation"], name, positive, mutants)
        else:
            need(row["state"] == "NOT_RUN" and row["observation"] is None, "draft has no negative credit")
    return True


def logical_declaration(buffers, file, line):
    need(type(line) is int and line > 0, "positive source line")
    need(file in buffers and type(buffers[file]) is bytes, "original source snapshot member")
    text = buffers[file].decode()
    need(line <= len(text.splitlines()), "line in captured member")
    if file == BODY:
        return INTENDED
    need(file == PROOF, "proof or actual shared executable body")
    starts = [(index, re.match(r"^(?:proof )?fn ([A-Za-z0-9_]+)", value))
              for index, value in enumerate(text.splitlines(), 1)]
    names = [(index, match.group(1)) for index, match in starts if match]
    selected = [name for index, name in names if index <= line]
    need(selected and selected[-1] == "preflight_settlement_writer_v1", "failure in intended function")
    return PREFIX + selected[-1]


def inspect(status, stdout, stderr, root, case, trusted=None):
    positive, mutants = references()
    files = named_inputs(case, positive, mutants)
    negative = case != "positive"
    need(type(status) is int and status == (1 if negative else 0), "exact logical exit")
    actual, buffers = source_snapshot(root, files)
    need(actual == files, "exact named actual source bytes and modes")
    need(type(stdout) is str and type(stderr) is str and len(stderr.encode()) <= LIMIT, "bounded output")
    paths = sys.modules[__name__]
    value = report(paths, stdout.encode(), negative)
    base = positive["observation"]
    need(sorted(value["func-details"]) == base["functions"], "exact original full function roster")
    warnings, logical = [], []
    for row in [parse(line) for line in stderr.splitlines() if line.strip()]:
        need(type(row) is dict, "structured diagnostic row")
        if row.get("level") == "warning":
            need(not logical, "complete warnings precede logical diagnostics")
            warnings.append(warning(paths, row, root, files, trusted))
        else:
            logical.append(row)
    normalized = normalized_diagnostics("\n".join(json.dumps(row) for row in logical), root, files, trusted)
    sites = []
    for row in normalized:
        if row["level"] == "error" and row.get("message") in LOGICAL:
            primary = [span for span in row.get("spans", []) if span.get("is_primary") is True]
            need(primary, "located logical error")
            for span in primary:
                file, line = span.get("file_name"), span.get("line_start")
                need(type(file) is str and file in (PROOF, BODY), "actual application primary")
                sites.append({"message": row["message"], "file": file, "line": line,
                              "declaration": logical_declaration(buffers, file, line)})
        elif row["level"] == "error":
            need(row.get("spans") == [] and type(row.get("message")) is str
                 and row["message"].startswith("aborting due to "), "no frontend substitution")
    need(bool(sites) == negative, "intended result")
    observed = {"inputs": files, "summary": value["verification-results"],
                "functions": sorted(value["func-details"]), "application_declarations": base["application_declarations"],
                "report_sha256": digest(value), "warnings": warnings, "diagnostics": normalized, "logical_sites": sites}
    observation_check(observed, case, positive, mutants)
    need(source_records(root, files) == files, "closing original source readback")
    return {"case": case, "accepted": False, "observation": observed}


def classify(status, stdout, stderr, root, policy, case, trusted=None):
    try:
        policy_check(policy)
        expected = policy["positive"] if case == "positive" else policy["cases"][case]["observation"]
        return inspect(status, stdout, stderr, root, case, trusted)["observation"] == expected
    except (ValueError, TypeError, KeyError, AttributeError, OverflowError, OSError, RecursionError):
        return False
