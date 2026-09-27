#!/usr/bin/env python3
"""Qualify shared byte-prefix arithmetic, not native completion/identity refinement.

Reuse the pinned gate campaign's signed-source, verifier-closure, process-owner
and diagnostic controls. Its two classifier calibration phases are unchanged.
"""

import hashlib
from pathlib import Path
import sys
import types

DRIVER_SHA = "361815b696234ca917b924da976b9fe5421aa5bd4256d91e28076c0dda806881"


def load_driver():
    path = Path(__file__).with_name("check-compute-peer-gate.py")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != DRIVER_SHA:
        raise ValueError("authenticated shared-body campaign driver")
    module = types.ModuleType("prefix_campaign")
    module.__file__ = str(path)
    sys.modules[module.__name__] = module
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def mutations(body):
    cases = {}

    def add(name, old, new, function):
        if body.count(old) != 1:
            raise ValueError("unique mutation site: " + name)
        cases[name] = (body.replace(old, new), "*" + function + "*")

    add("covers-foreign-extent", "$prefix <= $extent && ", "", "covers")
    add("covers-empty", "$len != 0 && ", "", "covers")
    add("covers-misses-last-byte", "$len <= $prefix - $offset", "$len < $prefix - $offset", "covers")
    add("write-invalid-prefix", "$prefix > $extent || ", "", "after_write")
    add("write-empty", "$len == 0 || ", "", "after_write")
    add("write-outside-extent", "$len > $extent - $offset", "false", "after_write")
    add("bridge-gap", "$offset <= $prefix && end > $prefix", "end > $prefix", "after_write")
    add("known-becomes-unknown", "if $known {", "if false {", "after_write")
    add("unknown-becomes-known", "if $known {", "if true {", "after_write")
    add("unknown-keeps-prefix", "Some($offset)", "Some($prefix)", "after_write")
    add("known-promotes-entire-extent", "Some(end)", "Some($extent)", "after_write")
    return cases


def selection_notes(leaf):
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function initialized_prefix_v1::" + name + " (selected functions)"
          for name in ("covers", "after_write")},
    })


def main():
    driver = load_driver()
    driver.__doc__ = __doc__
    driver.BODY = Path("crates/fe2o3-kfd/src/initialized_prefix_body.rs")
    driver.PROOF = driver.V / "initialized_prefix_v1.rs"
    driver.FILES = [driver.BODY, driver.PROOF]
    driver.EXPECTED = {"encountered-error": False, "encountered-vir-error": False,
                       "errors": 0, "is-verifying-entire-crate": True,
                       "success": True, "verified": 8}
    driver.mutations = mutations
    driver.selection_notes = selection_notes
    driver.main()


if __name__ == "__main__":
    main()
