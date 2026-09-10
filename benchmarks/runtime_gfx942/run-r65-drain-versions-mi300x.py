#!/usr/bin/env python3
"""Repeated copy-graph lineage and idle drain with unchanged owner guards."""
import importlib.util
import pathlib
import sys

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve()
spec = importlib.util.spec_from_file_location("r65_owner_base", HERE.with_name("run-r61-owner-mi300x.py"))
owner = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = owner
spec.loader.exec_module(owner)


class DrainVersionsRunner(owner.OwnerRunner):
    label = "r65"
    example = "gfx942-runtime-r65-drain-versions"
    expected_pass = ("PASS schema=fe2o3.runtime.r65-graph-versions-idle-drain.v1 bytes=1048832 "
                     "owner_threads=1 streams=4 nodes=12 copies=5 executions=2 "
                     "versions=21 version_inputs=10 current_versions=13 occurrences=distinct "
                     "joins=host canaries=complete submissions=released drain=idle-quiescent "
                     "admission=closed cleanup=complete\n")
    schema = "fe2o3.r65-graph-versions-idle-drain-qualification.v1"
    claim_scope = "one-device-repeated-copy-diamond-graph-local-lineage-idle-drain-admission-closure-and-cleanup"

    def snapshot(self):
        super().snapshot()
        relative = str(owner.base.BENCH_DIR / HERE.name)
        if self.source_hashes.get(relative) != owner.base.sha256_file(HERE):
            raise owner.base.RunError("R65 runner differs from signed source")


if __name__ == "__main__":
    sys.exit(owner.main(DrainVersionsRunner, "r65"))
