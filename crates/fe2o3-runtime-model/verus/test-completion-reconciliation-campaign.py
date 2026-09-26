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
    "ignore-success-settlement-error", "wrong-local-yield", "short-fuel", "early-yield",
}
need(set(cases) == names and len(set(cases.values())) == 21)
hooks = lambda text: Counter(re.findall(r"\$\(\$[a-z_]+\)\*", text))
need(all(hooks(text) == hooks(body) for text in cases.values()))
need(all(text != body and not any(token in text for token in ("assume(", "admit(", "external_body"))
         for text in cases.values()))
for changed in (body + body, body.replace(".cursor += 1;", ".cursor += 3;"), body.replace("while $remaining > 0", "loop")):
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
message = {"level": "error", "message": "assertion failed", "spans": [{"is_primary": True, "file_name": path}]}
check = lambda status, stdout, messages: runner["logical_negative"](
    leaf, status, stdout, "\n".join(json.dumps(item) for item in messages), verifier, {path})
need(check(1, json.dumps(result), [message]))
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
print("PASS: production planner campaign calibration (7 groups)")
