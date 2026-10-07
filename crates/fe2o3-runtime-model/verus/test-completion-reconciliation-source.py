#!/usr/bin/env python3
"""Calibrate the closed syntax comparison independently of Verus outcomes."""

from pathlib import Path
import os
import sys
import types

if not sys.flags.isolated or not sys.flags.dont_write_bytecode or sys.flags.optimize:
    raise RuntimeError("use python3 -I -B")

path = Path(__file__).with_name("check-completion-reconciliation-source.py")
check = types.ModuleType("completion_reconciliation_source")
check.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)
candidate = (check.ROOT / check.BODY).read_text()
context = (check.ROOT / check.CONTEXT).read_text()


previous, prior_context = check.baseline(check.BODY), check.baseline(check.CONTEXT)


def rejected(body=candidate, production=context):
    try:
        check.compare(previous, body, production, prior_context)
    except ValueError:
        return
    raise AssertionError("invalid syntax correspondence accepted")


check.compare(previous, candidate, context, prior_context)

# Wrapper/production domains are closed, including the default empty hooks.
rejected(candidate.replace("[], [$($on_step)*]", "[injected()], [$($on_step)*]"))
rejected(production=context.replace(check.INVOCATION, check.INVOCATION.replace("[]", "[changed()]")))
rejected(candidate + "\n")

# The counter normalization does not erase changed control flow or fuel use.
for before, after in (("let mut $length = 0usize", "let mut $length = 1usize"),
                      ("2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1", "2 * MAX_RUNTIME_DEPENDENCIES_V1 + 2"),
                      ("$remaining -= 1", "$remaining -= 2"),
                      ("while $remaining > 0", "while $remaining > 1")):
    rejected(candidate.replace(before, after))

# Correct counts alone cannot authorize a moved proof hook.
for hook in check.HOOKS:
    token = "$($" + hook + ")*"
    marker = candidate.index("        $syntax!({")
    prefix, body = candidate[:marker], candidate[marker:]
    rejected(prefix + body.replace(token, "", 1))
    rejected(prefix + body.replace(token, token + "\n                " + token, 1))
    rejected(prefix + body.replace(token, "", 1).replace("let mut $path", token + "\n            let mut $path", 1))

# Identifier substitution is finite; changed lookups remain visible.
rejected(candidate.replace(".get(&$id)", ".get(&$requested)"))
rejected(candidate.replace(".get(&$id)", ".get(&$unreviewed)"))
rejected(candidate.replace("dependencies.get($state.cursor)", "dependencies.get(0)"))

# Result propagation and Observe fields cannot be silently discarded/substituted.
rejected(candidate.replace("                            result?;", "                            let _ = result;", 1))
rejected(candidate.replace("backend: $record.backend_submission", "backend: 0", 1))
rejected(candidate.replace("id: $id,", "id: $requested,", 1))

# Other algorithm changes must survive normalization and fail parsed comparison.
rejected(candidate.replace(".cursor += 1", ".cursor += 2"))
rejected(candidate.replace("$path[$length] = $id", "$path[$length] = $dependency.submission"))
rejected(candidate.replace("$context.require_ordinary_submission_v1($id)?;", ""))

# Ambient command lookup and Git injection cannot change authenticated baseline bytes.
saved = dict(os.environ)
try:
    os.environ.update(PATH="/nonexistent", GIT_CONFIG_COUNT="1", GIT_CONFIG_KEY_0="core.bare",
                      GIT_CONFIG_VALUE_0="true", GIT_DIR="/nonexistent", LD_PRELOAD="/nonexistent")
    assert check.baseline(check.BODY) == previous
    check.compare(previous, candidate, context, prior_context)
finally:
    os.environ.clear()
    os.environ.update(saved)

print("PASS: completion planner source calibration (8 groups)")
