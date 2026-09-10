#!/usr/bin/env python3
"""Copy-DAG correctness with unchanged owner custody and closure guards."""
import importlib.util
import pathlib
import sys

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve()
spec = importlib.util.spec_from_file_location("r63_owner_base", HERE.with_name("run-r61-owner-mi300x.py"))
owner = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = owner
spec.loader.exec_module(owner)


class GraphRunner(owner.OwnerRunner):
    label = "r63"
    example = "gfx942-runtime-r63-async-graph"
    expected_pass = ("PASS schema=fe2o3.runtime.r63-async-graph-copy.v1 bytes=1048832 "
                     "owner_threads=1 streams=4 nodes=12 copies=5 joins=host "
                     "canaries=complete submissions=released cleanup=complete\n")
    schema = "fe2o3.r63-graph-copy-qualification.v1"
    claim_scope = "one-device-four-stream-copy-diamond-host-joins-custody-and-cleanup"

    def snapshot(self):
        super().snapshot()
        relative = str(owner.base.BENCH_DIR / HERE.name)
        if self.source_hashes.get(relative) != owner.base.sha256_file(HERE):
            raise owner.base.RunError("R63 runner differs from signed source")


if __name__ == "__main__":
    sys.exit(owner.main(GraphRunner, "r63"))
