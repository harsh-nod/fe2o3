#!/usr/bin/env python3
"""R62 cancellation/timeout profile with unchanged owner custody/closure guards."""

import importlib.util
import pathlib
import sys

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve()
spec = importlib.util.spec_from_file_location("r62_owner_base", HERE.with_name("run-r61-owner-mi300x.py"))
owner = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = owner
spec.loader.exec_module(owner)


class ControlRunner(owner.OwnerRunner):
    label = "r62"
    example = "gfx942-runtime-r62-async-control"
    expected_pass = ("PASS schema=fe2o3.runtime.r62-async-control-copy.v1 bytes=1048832 "
                     "owner_threads=1 cancelled_copy=not_submitted timeout_identity=retained "
                     "abandoned_upload=completed canaries=complete cleanup=complete\n")
    schema = "fe2o3.r62-control-copy-qualification.v1"
    claim_scope = "one-device-copy-pre-submit-cancel-recoverable-timeout-drop-custody-and-cleanup"

    def snapshot(self):
        super().snapshot()
        relative = str(owner.base.BENCH_DIR / HERE.name)
        if self.source_hashes.get(relative) != owner.base.sha256_file(HERE):
            raise owner.base.RunError("R62 runner differs from signed source")


if __name__ == "__main__":
    sys.exit(owner.main(ControlRunner, "r62"))
