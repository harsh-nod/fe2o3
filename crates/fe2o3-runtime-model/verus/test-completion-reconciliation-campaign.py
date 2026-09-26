#!/usr/bin/env python3
"""Controller calibration; these synthetic cases are not solver evidence."""

from collections import Counter
import copy
import json
from pathlib import Path
import re
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-completion-reconciliation-campaign.py")))
leaf = runner["inherited"]()
body = (runner["ROOT"] / runner["BODY"]).read_text()
cases = runner["mutations"](body)


def need(value):
    if not value:
        raise ValueError("planner campaign calibration failed")


names = {
    "omit-ordinary", "omit-custody", "omit-peer-roots", "omit-producer-roots",
    "observe-wrong-backend", "observe-wrong-selected-id", "descend-wrong-id",
    "push-wrong-parent", "pop-wrong-parent", "cache-after-descent", "cache-after-pop",
    "cursor-skips-producer", "cursor-crosses-pending", "quiescent-predecessor-promoted",
    "pending-input-promoted", "no-effect-input-promoted", "failed-physical-observation-promoted",
    "ignore-success-settlement-error", "wrong-local-yield", "short-fuel", "early-yield-return",
}
need(set(cases) == names and len(set(cases.values())) == 21)
hooks = lambda text: Counter(re.findall(r"\$\(\$[a-z_]+\)\*", text))
need(all(hooks(text) == hooks(body) for text in cases.values()))
need(all(text != body and not any(token in text for token in ("assume(", "admit(", "external_body"))
         for text in cases.values()))
for changed in (body + body, body.replace(".cursor += 1;", ".cursor += 3;"), body.replace("$remaining -= 1;", "$remaining -= 2;")):
    try:
        runner["mutations"](changed)
    except ValueError:
        pass
    else:
        raise ValueError("ambiguous or stale mutation site admitted")
need(cases["cursor-crosses-pending"].count("RuntimeCompletionStatusV1::Pending => {") == 1)
need(cases["cursor-crosses-pending"].count("RuntimeCompletionStatusV1::Succeeded => {") == 1)
need(len(runner["FILES"]) == len(set(runner["FILES"])) == 8)
need(all((runner["ROOT"] / path).is_file() for path in runner["FILES"]))
need(len(leaf.MUTANTS) == 4 and sum(path == runner["BODY"] for _, path, *_ in leaf.MUTANTS) == 2)

verifier = {"version": "calibration-only"}
path = "/snapshot/context_completion_reconciliation_planner_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 1,
}}
message = {"$message_type": "diagnostic", "level": "error", "message": "assertion failed", "code": None,
           "children": [], "spans": [{"is_primary": True, "file_name": path}]}
check = lambda status, stdout, messages: runner["logical_negative"](
    leaf, status, stdout, "\n".join(json.dumps(item) for item in messages), verifier, {path})
need(check(1, json.dumps(result), [message]))
need(check(1, json.dumps(result), [dict(message, message="loop invariant not satisfied")]))
need(not check(1, json.dumps(result), [dict(message, message="loop invariant not satisfied: Resource limit (rlimit) exceeded")]))
for status in (0, 2, 124, 137, -9):
    need(not check(status, json.dumps(result), [message]))
for text in ("mismatched types", "Resource limit (rlimit) exceeded", "could not read file", "unexpected token"):
    need(not check(1, json.dumps(result), [dict(message, message=text)]))
need(not check(1, "not JSON", [message]))
need(not check(1, json.dumps(result), [dict(message, spans=[{"is_primary": True, "file_name": "/unauthorized.rs"}])]))
need(not check(1, json.dumps(result), [message, {"level": "warning", "message": "unreachable pattern"}]))
for value in (True, False, None, 0, 1, "false", [], {}):
    malformed = copy.deepcopy(result)
    malformed["verification-results"]["success"] = value
    need(not check(1, json.dumps(malformed), [message]))
for key in result["verification-results"]:
    malformed = copy.deepcopy(result)
    del malformed["verification-results"][key]
    need(not check(1, json.dumps(malformed), [message]))
for key, values in (("verified", (True, -1, "1", None)), ("errors", (True, 0, -1, "1", None))):
    for value in values:
        malformed = copy.deepcopy(result)
        malformed["verification-results"][key] = value
        need(not check(1, json.dumps(malformed), [message]))
for value in ([], None, 0, "object"):
    need(not check(1, json.dumps(value), [message]))
    need(not check(1, json.dumps(dict(result, **{"verification-results": value})), [message]))
    need(not check(1, json.dumps(result), [value]))
need(not check(1, json.dumps(result).replace('"errors": 1', '"errors": 0, "errors": 1'), [message]))
need(not check(1, json.dumps(result), [dict(message, spans=[None])]))
need(not check(1, json.dumps(result), [message, dict(message, message="while loop: Resource limit (rlimit) exceeded")]))
root = Path("/snapshot")
verus = Path("/verifier/verus")
positive = runner["proof_command"](root, verus)
negative = runner["proof_command"](root, verus, "*plan_completion_step_v1*")
need(negative == positive[:-1] + ["--verify-function", "*plan_completion_step_v1*", "--verify-root", positive[-1]])
need(positive[:7] == ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120", str(verus), "--crate-type"])
need(positive[positive.index("--multiple-errors") + 1] == "0" and positive.count("--multiple-errors") == 1)
need("--no-cheating" in positive and "--rlimit" not in positive and "--smt-option" not in positive)
need(positive[-1] == str(root / runner["FILES"][1]))
success = {"verus": verifier, "verification-results": leaf.PROOF_RESULT}
note = dict(message, **{"$message_type": "diagnostic", "level": "note", "message": sorted(runner["ENUMERATION_NOTES"])[0],
                       "code": None, "children": []})
positive_check = lambda status, result, notes: runner["proof_positive"](
    status, result, "\n".join(json.dumps(item) for item in notes), verifier, leaf.PROOF_RESULT, {path})
need(positive_check(0, json.dumps(success), []))
need(positive_check(0, json.dumps(success), [note]))
for field, values in (("level", ("warning", "error")), ("message", ("unrecognized note", "Resource limit (rlimit) exceeded")),
                      ("children", ([message], None)), ("spans", ([], [None], [{"file_name": "/foreign.rs", "is_primary": True}]))):
    for value in values:
        need(not positive_check(0, json.dumps(success), [dict(note, **{field: value})]))
need(not positive_check(1, json.dumps(success), [note]))
need(not positive_check(0, json.dumps(result), [note]))
need(not positive_check(0, json.dumps(success).replace('"errors": 0', '"errors": 1, "errors": 0'), []))
for malformed in ("not JSON", "[]", "null"):
    need(not positive_check(0, malformed, []))
need(not positive_check(0, json.dumps(success), [None]))
include_path = "/snapshot/verus/../../snapshot/context_completion_reconciliation_planner_v1.rs"
need(positive_check(0, json.dumps(success), [dict(note, spans=[{"file_name": include_path, "is_primary": True}])]))
need(check(1, json.dumps(result), [dict(message, spans=[{"file_name": include_path, "is_primary": True}])]))
for classifier, data in ((positive_check, success), (check, result)):
    status = 0 if classifier is positive_check else 1
    base = note if classifier is positive_check else message
    need(not classifier(status, json.dumps(data), [dict(base, children=[dict(message, message="Resource limit (rlimit) exceeded")])]))
    need(not classifier(status, json.dumps(data), [dict(base, spans=[{"file_name": "relative.rs", "is_primary": True}])]))
need(not check(1, json.dumps(result), [message, dict(note, message=note["message"] + "; Resource limit (rlimit) exceeded")]))
print("PASS: production planner campaign calibration (8 groups)")
