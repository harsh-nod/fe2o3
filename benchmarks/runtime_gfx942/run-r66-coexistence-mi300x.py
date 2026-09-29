#!/usr/bin/env python3
"""Development-only isolated R66 cells with unchanged census and strict endpoints.

The legacy monitor does not export target T0. These sequential endpoint checks
are not modern immediate/20-second acceptance; that needs an external pinned
campaign. No physical overlap, reservation or performance acceptance is claimed.
"""
import hashlib
import importlib.util
import json
import pathlib
import sys
from types import ModuleType

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
ENDPOINT_PATH = HERE.parents[2] / "docs/evidence/dev-native-producer-mi300x-2026-09-24/protocol.py"
ENDPOINT_SHA = "384cf35d5f3471c4fe0b2228e39963f647383fbbb63e2475d64d643be2d40be1"
OBSERVER_PATH = HERE.with_name("copy-host-observe.py")
OBSERVER_SHA = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"


def pinned_module(name, path, expected):
    if path.is_symlink() or not path.is_file():
        raise owner.base.RunError("ordinary pinned endpoint helper required")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != expected:
        raise owner.base.RunError("pinned endpoint helper changed")
    module = ModuleType(name)
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


endpoint = pinned_module("r66_strict_endpoint", ENDPOINT_PATH, ENDPOINT_SHA)
observer = pinned_module("r66_strict_observer", OBSERVER_PATH, OBSERVER_SHA)


class CoexistenceRunner(owner.OwnerRunner):
    label = "r66"
    example = "gfx942-runtime-r66-coexistence"
    schema = "fe2o3.r66-isolated-coexistence-development.v1"
    claim_scope = ("development-only-two-eight-cell-process-isolated-matrices-retained-native-custody-"
                   "strict-sequential-endpoints-no-modern-t0-20s-acceptance-not-physical-overlap")
    cargo_features = ("fe2o3-runtime/hardware-qualification",)
    qualification_runs = 16
    repetitions = 2
    cells_per_repetition = 8

    def qualification_shape(self):
        shape = {"qualification_runs": self.qualification_runs, "repetitions": self.repetitions,
                 "cells_per_repetition": self.cells_per_repetition}
        if (any(type(value) is not int for value in shape.values())
                or list(shape.values()) != [16, 2, len(checker.PROFILES)] or len(checker.PROFILES) != 8):
            raise owner.base.RunError("R66 requires exactly sixteen processes in two eight-cell repetitions")
        return shape

    def qualification_metadata(self):
        shape = self.qualification_shape()
        path = self.evidence / "isolated-cells.json"
        try:
            document = json.loads(checker.read_output(path), object_pairs_hook=checker.unique_object,
                                  parse_constant=checker.invalid_constant)
            fixed = {"schema": self.schema, **shape, "process_scope": "one-cell-per-monitored-process",
                     "occurrence_scope": "process-local", "strict_endpoint_timing": "sequential-not-target-t0-relative",
                     "modern_endpoint_acceptance": False, "physical_overlap": "unmeasured",
                     "exclusive_reservation": False, "performance_acceptance": False}
            checker.exact_keys(document, {*fixed, "cells"})
            checker.require(endpoint.same({key: document[key] for key in fixed}, fixed), "exact development matrix claims")
            cells = document["cells"]
            checker.require(type(cells) is list and len(cells) == self.qualification_runs, "complete isolated-cell roster")
            monitors = [row for row in self.commands
                        if row["argv"][:3] == ["/usr/bin/python3", str(self.guard), "monitor"]]
            checker.require(len(monitors) == self.qualification_runs
                            and len({row["stdout"] for row in monitors}) == self.qualification_runs,
                            "one separately recorded monitor per isolated cell")
            monitor_seals = set()
            for index, (cell, monitor) in enumerate(zip(cells, monitors)):
                repetition, ordinal = divmod(index, self.cells_per_repetition)
                label = f"owner-{repetition}-cell-{ordinal}"
                observed = self.evidence / (label + ".jsonl")
                expected = {"repetition": repetition, "ordinal": ordinal, "phase": label,
                            "output": observed.name, "output_sha256": owner.base.sha256_file(observed)}
                checker.require(endpoint.same(cell, expected), "exact ordered isolated cell and output identity")
                checker.validate_cell_output(checker.read_output(observed), ordinal)
                argv = ["/usr/bin/python3", self.guard, "monitor", "--gpu-id", self.topology["kfd_gpu_id"],
                        "--observer-cpu", self.topology["observer_cpu"], "--target-output", observed,
                        "--", *self.phase_command("kfd", [owner.base.UNIQUE_ID, "--case", str(ordinal)])]
                checker.require(monitor["argv"] == [str(part) for part in argv]
                                and type(monitor["returncode"]) is int and monitor["returncode"] == 0,
                                "exact successful isolated monitor command")
                record = owner.base.validate_monitor((self.stage / monitor["stdout"]).read_text(), observed,
                                                     self.topology, self.retained)
                checker.require(record["monitor_sha256"] not in monitor_seals,
                                "distinct sealed monitor records for every isolated process")
                monitor_seals.add(record["monitor_sha256"])
        except (OSError, ValueError, KeyError, TypeError) as error:
            raise owner.base.RunError(f"R66 publication roster rejected: {error}") from error
        return {**shape, "isolated_cells_sha256": owner.base.sha256_file(path),
                "modern_endpoint_acceptance": False}

    def snapshot(self):
        super().snapshot()
        for path in (HERE, pathlib.Path(checker.__file__), ENDPOINT_PATH, OBSERVER_PATH):
            relative = str(path.relative_to(HERE.parents[2]))
            if self.source_hashes.get(relative) != owner.base.sha256_file(path):
                raise owner.base.RunError("R66 runner or checker differs from signed source")

    def validate_qualifier_output(self, output, ordinal):
        try:
            return checker.validate_cell_output(output, ordinal)
        except checker.ValidationError as error:
            raise owner.base.RunError(f"R66 retained-custody evidence rejected: {error}") from error

    def strict_endpoint(self, label):
        observed = self.bench / OBSERVER_PATH.name
        if observed.is_symlink() or owner.base.sha256_file(observed) != OBSERVER_SHA:
            raise owner.base.RunError("captured strict observer changed")
        raw = self.run(["/usr/bin/python3", "-I", "-B", observed,
                        "--gpu-index", str(endpoint.GPU), "--pci-bdf", endpoint.BDF,
                        "--unique-id", endpoint.UID], label=f"strict-{label}", timeout=100,
                       limit=4 * 1024 * 1024)
        if (self.stage / self.commands[-1]["stderr"]).read_bytes():
            raise owner.base.RunError("strict endpoint emitted diagnostics")
        try:
            endpoint.endpoint(raw.encode("ascii"), observer)
        except (ValueError, KeyError, TypeError) as error:
            raise owner.base.RunError(f"strict R66 endpoint rejected: {error}") from error

    def phase(self, label, backend, args):
        self.strict_endpoint(label + "-before")
        failure = None
        output = None
        try:
            output = super().phase(label, backend, args)
        except Exception as error:
            failure = error
        try:
            self.strict_endpoint(label + "-after")
        except Exception as error:
            if failure is not None:
                raise owner.base.RunError(f"R66 monitored phase failed: {failure}; strict postflight failed: {error}") from error
            raise
        if failure is not None:
            raise failure
        return output

    def qualify_and_measure(self):
        shape = self.qualification_shape()
        self.identity_start = {"pci_bdf": endpoint.BDF}
        self.topology_raw = self.topology_record("initial")
        topology = owner.base.sealed_fields(self.topology_raw, "topology", self.retained.R26_TOPOLOGY_SEALED_FIELDS)
        identity = {"pci_bdf": endpoint.BDF, "pci_numa_node": topology["numa_node"],
                    "gpu_node_id": topology["kfd_node"], "gpu_guid": topology["kfd_gpu_id"]}
        self.topology = owner.base.validate_topology(self.topology_raw, identity, self.retained)
        self.run(["/usr/bin/taskset", "--cpu-list", self.topology["measurement_cpu_list"],
                  "/usr/bin/numactl", f"--physcpubind={self.topology['measurement_cpu_list']}",
                  f"--membind={self.topology['numa_node']}", "/usr/bin/true"], label="placement-probe")
        cells = []
        for repetition in range(self.repetitions):
            for ordinal in range(self.cells_per_repetition):
                label = f"owner-{repetition}-cell-{ordinal}"
                output = self.phase(label, "kfd", [owner.base.UNIQUE_ID, "--case", str(ordinal)])
                self.validate_qualifier_output(checker.read_output(output), ordinal)
                cells.append({"repetition": repetition, "ordinal": ordinal, "phase": label,
                              "output": str(output.relative_to(self.evidence)),
                              "output_sha256": owner.base.sha256_file(output)})
        self.verify_source()
        owner.base.write_json(self.evidence / "isolated-cells.json", {
            "schema": self.schema, **shape, "cells": cells, "process_scope": "one-cell-per-monitored-process",
            "occurrence_scope": "process-local", "strict_endpoint_timing": "sequential-not-target-t0-relative",
            "modern_endpoint_acceptance": False, "physical_overlap": "unmeasured",
            "exclusive_reservation": False, "performance_acceptance": False,
        })


if __name__ == "__main__":
    sys.exit(owner.main(CoexistenceRunner, "r66"))
