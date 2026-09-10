#!/usr/bin/env python3
"""Signed copy-only drain capture qualification with unchanged shared-host guards."""
import importlib.util
import pathlib
import sys

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve()


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, HERE.with_name(filename))
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


owner = load("drain_capture_owner_base", "run-r61-owner-mi300x.py")
checker = load("drain_capture_checker", "check_drain_capture.py")


class DrainCaptureRunner(owner.OwnerRunner):
    label = "drn2"
    example = "gfx942-runtime-drain-capture"
    schema = "fe2o3.runtime.drain-capture-copy.v1"
    claim_scope = "copy-only-accepted-prefix-native-retained-host-capture-not-physical-overlap-no-performance-claim"
    cargo_features = ("fe2o3-runtime/hardware-qualification",)

    def snapshot(self):
        super().snapshot()
        for path in (HERE, pathlib.Path(checker.__file__)):
            relative = str(owner.base.BENCH_DIR / path.name)
            if self.source_hashes.get(relative) != owner.base.sha256_file(path):
                raise owner.base.RunError("DRN-2 runner or checker differs from signed source")

    def validate_qualifier_output(self, output):
        try:
            checker.validate_output(output)
        except checker.ValidationError as error:
            raise owner.base.RunError(f"DRN-2 capture evidence rejected: {error}") from error


if __name__ == "__main__":
    sys.exit(owner.main(DrainCaptureRunner, "drn2"))
