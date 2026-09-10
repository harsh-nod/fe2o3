#!/usr/bin/env python3
"""Signed R66 retained compute/copy qualification with unchanged host guards."""
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


owner = load("r66_owner_base", "run-r61-owner-mi300x.py")
checker = load("r66_coexistence_checker", "check_r66_coexistence.py")


class CoexistenceRunner(owner.OwnerRunner):
    label = "r66"
    example = "gfx942-runtime-r66-coexistence"
    schema = "fe2o3.r66-retained-coexistence-qualification.v1"
    claim_scope = "one-device-r26-compute-disjoint-directional-sdma-both-publication-orders-retained-native-custody-not-physical-overlap"
    cargo_features = ("fe2o3-runtime/hardware-qualification",)

    def snapshot(self):
        super().snapshot()
        for path in (HERE, pathlib.Path(checker.__file__)):
            relative = str(owner.base.BENCH_DIR / path.name)
            if self.source_hashes.get(relative) != owner.base.sha256_file(path):
                raise owner.base.RunError("R66 runner or checker differs from signed source")

    def validate_qualifier_output(self, output):
        try:
            checker.validate_output(output)
        except checker.ValidationError as error:
            raise owner.base.RunError(f"R66 retained-custody evidence rejected: {error}") from error


if __name__ == "__main__":
    sys.exit(owner.main(CoexistenceRunner, "r66"))
