#!/usr/bin/env python3
"""Bind the unchanged strict campaign checker to this fresh evidence packet."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
from contextlib import redirect_stdout
import hashlib
import io
from pathlib import Path
import subprocess
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PRIOR = HERE.parent / "dev-xgmi-hot-batch-mi300x-2026-09-24"
LOCAL = Path("/home/harsh/.codex-tmp/fe2o3-hot-batch-20260924-feYb12D3/native2")
HELPER_COMMIT = "c640f434c407384c1f9f6668c5b82c493b70a704"
HELPERS = {
    "package": "c83d5a05fb8baf8904eb4a9f49c8d5f07906762aa7ea0cfc5c07ad9e51583466",
    "verify": "9e146af4578d82ae148b664c264090977e8d7558665b41a6ec72f6011c74637e",
}
TESTS = {
    "test_package": "fa5f3282ae77f1ea1daefae989f53a145292ddaa89cd186894305d65b03a2fd5",
    "test_verify": "dd0ea3848e9ee6a61263ca9d6df75e809f35d7860084e21fd4311f3b29cbc229",
}
SIGNERS = Path("/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers")
SIGNERS_SHA = "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b"
GIT = ["/usr/bin/git", "--no-replace-objects", "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false"]
GIT_ENV = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C",
           "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_SYSTEM": "/dev/null", "GIT_CONFIG_GLOBAL": "/dev/null"}


def git(*arguments):
    return subprocess.check_output([*GIT, *arguments], cwd=REPO, env=GIT_ENV, timeout=30)


def authenticated(role):
    digest = (HELPERS | TESTS)[role]
    path = PRIOR / (role + ".py")
    if not path.is_file() or path.is_symlink():
        raise RuntimeError("ordinary signed helper")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise RuntimeError("signed helper digest")
    if not SIGNERS.is_file() or SIGNERS.is_symlink() or hashlib.sha256(SIGNERS.read_bytes()).hexdigest() != SIGNERS_SHA:
        raise RuntimeError("trusted helper signer")
    git("-c", "gpg.ssh.allowedSignersFile=" + str(SIGNERS), "verify-commit", HELPER_COMMIT)
    if git("show", HELPER_COMMIT + ":" + str(path.relative_to(REPO))) != raw:
        raise RuntimeError("signed helper source association")
    return path, raw


def load(role):
    if role not in HELPERS:
        raise KeyError(role)
    path, raw = authenticated(role)
    module = ModuleType("strict_hot_batch_" + role)
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def configured(role):
    module = load(role)
    if role == "package":
        module.HERE, module.OWNED = HERE, LOCAL
    else:
        module.ROOT, module.LOCAL = HERE, LOCAL
        # PACKET_PATH remains the original signed protocol/qualification path.
    return module


def verify(module):
    with redirect_stdout(io.StringIO()) as captured:
        module.main()
    for kind in ("local", "remote"):
        for path in (module.ROOT / "raw/native1" / kind).glob("*/receipt.json"):
            row = module.read(path)
            module.need(row["finished_ns"] - row["started_ns"] <= (row["timeout_seconds"] + 15) * 10**9,
                        "bounded strict receipt elapsed time")
    print(captured.getvalue(), end="")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=tuple(HELPERS))
    args = parser.parse_args()
    module = configured(args.action)
    if args.action == "package":
        # The existing packager's CLI defaults to strict, never recovered mode.
        sys.argv = [str(PRIOR / "package.py")]
        module.main()
    else:
        verify(module)


if __name__ == "__main__":
    main()
