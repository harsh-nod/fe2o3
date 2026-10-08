#!/usr/bin/env python3
"""Exact source-bound cached-prefix reports; no timeout or warning is proof evidence."""
import hashlib
import json
import os
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
    "crates/fe2o3-runtime-model/verus/../../fe2o3-runtime/src/context/cached_poll_body.rs":
    "crates/fe2o3-runtime/src/context/cached_poll_body.rs",
}
PREFIX = "context_cached_poll_v1::"
KEYS = {"encountered-error", "encountered-vir-error", "success", "verified", "errors", "is-verifying-entire-crate"}
PROOF = "crates/fe2o3-runtime-model/verus/context_cached_poll_v1.rs"
BODY = "crates/fe2o3-runtime/src/context/cached_poll_body.rs"
PROOF_SHA = "6f5eb3581bfa24de99759aad3b59e1d1ba3337675c404d1acf767c41ecaea69f"
BODY_SHA = "2b01dbf17b6a868d787781e31c90e5453a47487f4631a77efc3a625c23d00e76"
HELD_BODY_SHA = "c4e7dbd803246deb010fbdf66a81bab9da27f8893d362d724d093c3af36450f8"
OMISSION = "core::option::impl&%0::is_some"
CASES = (
    "generation_check_inverted", "stream_hold_call_omitted", "cancelled_as_success", "token_id_local_corrupted",
    "backend_identity_omitted", "stream_identity_omitted", "device_identity_omitted",
    "live_stream_device_omitted", "held_stream_accepted", "cached_token_precedes_identity",
    "pending_marked_terminal", "backend_failure_as_success", "quiescent_as_success",
    "token_backend_identity_corrupted", "pending_completion_overwritten", "terminal_completion_omitted",
)
SCOPE = "shared-cached-prefix; two-key-law-premises; opaque-owner/full-Context/native-excluded"

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
    observed = path.lstat()
    need(stat.S_ISREG(observed.st_mode), "regular original trusted member")
    record = records[name]
    need(type(record["bytes"]) is int and 0 < record["bytes"] <= LIMIT, "bounded trusted source")
    with path.open("rb") as stream:
        data = stream.read(record["bytes"] + 1)
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
               and v['verified']>=0 and v['errors']>=0 and v['verified']+v['errors']==21
               and (v['errors']>0 if negative else v['errors']==0), 'whole twenty-one obligations')
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


def source_records(root, files):
    root = Path(root)
    need(root.is_absolute() and root.resolve() == root, "canonical captured source")
    need(set(files) == {PROOF, BODY, "rust-toolchain.toml"}, "exact three-file compiled source")
    result = {}
    for name in sorted(files):
        diagnostic_member(str(root / name), root, files)
        fd = os.open(root / name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            before = os.fstat(fd)
            need(stat.S_ISREG(before.st_mode) and before.st_size <= LIMIT, "bounded regular source")
            with os.fdopen(os.dup(fd), "rb") as stream:
                raw = stream.read(LIMIT + 1)
            after = os.fstat(fd)
            identity = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
            need(identity(before) == identity(after) and len(raw) == before.st_size, "stable source buffer")
            result[name] = {"bytes": len(raw), "mode": stat.S_IMODE(before.st_mode), "sha256": hashlib.sha256(raw).hexdigest()}
        finally:
            os.close(fd)
    return result


def policy_check(policy, *, active=True):
    need(type(policy) is dict and set(policy) == {"schema", "state", "scope", "proof_sha256", "positive", "cases"}, "closed policy schema")
    need(policy["schema"] == "fe2o3.context-cached-poll.diagnostics.v1" and policy["scope"] == SCOPE
         and policy["proof_sha256"] == PROOF_SHA, "exact conditional policy scope")
    need(policy["state"] == ("reviewed-calibrated" if active else "draft-review-required"), "explicit policy activation after review")
    need(type(policy["cases"]) is dict and set(policy["cases"]) == set(CASES), "exact sixteen policies")
    positive = policy["positive"]
    baseline = positive["functions"]
    need(type(baseline) is list and len(baseline) == 197 and baseline == sorted(set(baseline)), "exact positive roster")
    need(positive["inputs"][BODY]["sha256"] == BODY_SHA, "original executable body")
    applications = sorted(n for n in baseline if n.startswith(PREFIX))
    need(len(applications) == 51 and positive["application_declarations"] == applications, "all application declarations")
    fields = {"inputs", "summary", "functions", "application_declarations", "report_sha256", "diagnostics", "warnings", "logical_sites"}
    for name, row in (("positive", positive), *policy["cases"].items()):
        negative = name != "positive"
        need(type(row) is dict and set(row) == fields, "closed named policy entry")
        need(set(row["inputs"]) == {PROOF, BODY, "rust-toolchain.toml"}
             and row["inputs"][PROOF]["sha256"] == PROOF_SHA, "unchanged theorem and source roster")
        need(row["application_declarations"] == applications, "no missing application declaration")
        expected = [n for n in baseline if n != OMISSION] if name == "held_stream_accepted" else baseline
        need(row["functions"] == expected, "only the exact source-bound held-stream omission")
        if name == "held_stream_accepted":
            need(OMISSION in baseline and row["inputs"][BODY]["sha256"] == HELD_BODY_SHA, "named executable is_some removal")
        need(row["summary"] == {"encountered-error": negative, "encountered-vir-error": False,
             "success": not negative, "verified": 20 if negative else 21,
             "errors": 1 if negative else 0, "is-verifying-entire-crate": True}, "all twenty-one obligations")
        need(type(row["warnings"]) is list and len(row["warnings"]) == (2 if negative else 5), "closed phase warning sequence")
        if negative:
            need(row["warnings"] == positive["warnings"][:2] and row["logical_sites"], "exact calibrated negative warnings and sites")
        else:
            need(row["diagnostics"] == [] and row["logical_sites"] == [], "positive has no logical diagnostics")
    return True


def classify(status, stdout, stderr, root, policy, case):
    try:
        policy_check(policy)
        need(case == "positive" or case in CASES, "closed named case")
        negative = case != "positive"
        need(type(status) is int and status == (1 if negative else 0), "exact logical exit")
        expected = policy["cases"][case] if negative else policy["positive"]
        files = expected["inputs"]
        need(source_records(root, files) == files, "exact named actual source bytes and modes")
        need(type(stdout) is str and type(stderr) is str and len(stderr.encode()) <= LIMIT, "bounded output strings")
        paths = sys.modules[__name__]
        value = report(paths, stdout.encode(), negative)
        need(value["verification-results"] == expected["summary"]
             and sorted(value["func-details"]) == expected["functions"]
             and digest(value) == expected["report_sha256"], "complete calibrated report, not count-only evidence")
        rows = [parse(line) for line in stderr.splitlines() if line.strip()]
        warnings, logical = [], []
        for row in rows:
            need(type(row) is dict, "structured diagnostic row")
            if row.get("level") == "warning":
                warnings.append(warning(paths, row, root, files, None))
            else:
                logical.append(row)
        # These calibrated failures have no external logical spans. Virtual compiler
        # paths remain exact warning data only, never logical-source authority.
        normalized = normalized_diagnostics("\n".join(json.dumps(row) for row in logical), root, files)
        sites = []
        for row in normalized:
            if row["level"] == "error" and row.get("message") in LOGICAL:
                primary = [span for span in row.get("spans", []) if span.get("is_primary") is True]
                need(primary, "located logical error")
                for span in primary:
                    need(type(span.get("file_name")) is str and span["file_name"] in files
                         and type(span.get("line_start")) is int and span["line_start"] > 0, "actual application primary")
                    sites.append({"message": row["message"], "file": span["file_name"], "line": span["line_start"]})
            elif row["level"] == "error":
                need(row.get("spans") == [] and type(row.get("message")) is str
                     and row["message"].startswith("aborting due to "), "no frontend substitution")
        need(warnings == expected["warnings"] and normalized == expected["diagnostics"]
             and sites == expected["logical_sites"], "complete calibrated warnings and logical diagnostics")
        need(bool(sites) == negative, "logical negative or clean positive")
        return True
    except (ValueError, TypeError, KeyError, AttributeError, OverflowError, OSError, RecursionError):
        return False
