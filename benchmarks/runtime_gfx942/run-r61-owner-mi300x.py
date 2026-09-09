#!/usr/bin/env python3
"""Qualify the signed R61 copy owner using unchanged R60 custody/R26 host guards."""

import importlib.util
import json
import os
import pathlib
import secrets
import shutil
import signal
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve()
spec = importlib.util.spec_from_file_location("r61_guarded_base", HERE.with_name("run-r60-pipeline-mi300x.py"))
base = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = base
spec.loader.exec_module(base)
EXAMPLE = "gfx942-runtime-r61-async-owner"
PASS = ("PASS schema=fe2o3.runtime.r61-async-owner-copy.v1 bytes=1048832 "
        "owner_threads=1 abandoned_upload=completed canaries=complete cleanup=complete\n")


def validate_output(output):
    if output != PASS:
        raise base.RunError("owner qualifier did not emit exact PASS")


class OwnerRunner(base.Runner):
    def snapshot(self):
        super().snapshot()
        relative = str(base.BENCH_DIR / HERE.name)
        if self.source_hashes.get(relative) != base.sha256_file(HERE):
            raise base.RunError("R61 runner differs from signed source")

    def build(self):
        home = self.args.build_home
        for name in ("config", "config.toml"):
            if (home / ".cargo" / name).exists():
                raise base.RunError("ambient Cargo configuration is not admitted")
        rust = home / ".cargo/bin"
        temporary = self.stage / "tmp"
        temporary.mkdir(mode=0o700)
        env = dict(base.CLEAN_ENV, HOME=str(home), PATH=f"{rust}:{base.SYSTEM_PATH}",
                   CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2",
                   CARGO_TARGET_DIR=str(self.stage / "target"), TMPDIR=str(temporary))
        tools = [rust / "cargo", rust / "rustc", rust / "rustup", self.args.rocm_path / "bin/rocm-smi",
                 *(pathlib.Path("/usr/bin") / name for name in
                   ("git", "ssh-keygen", "python3", "numactl", "taskset", "timeout", "readelf", "nm"))]
        for name in ("cargo", "rustc"):
            self.run([rust / name, "--version"], label=f"{name}-version", env=env, cwd=self.source)
            tools.append(pathlib.Path(self.run([rust / "rustup", "which", name],
                label=f"resolved-{name}", env=env, cwd=self.source).strip()))
        self.tool_hashes = {str(path): {"resolved": str(path.resolve(strict=True)),
                            "sha256": base.sha256_file(path)} for path in tools}
        self.run([rust / "cargo", "build", "--offline", "--locked", "--release",
                  "--no-default-features", "-p", "fe2o3-runtime", "--example", EXAMPLE],
                 label="build-owner", timeout=1200, env=env, cwd=self.source)
        binary = self.stage / "target/release/examples" / EXAMPLE
        binary.chmod(0o500)
        self.binaries = {"kfd": binary}
        self.binary_hashes = {"kfd": base.sha256_file(binary)}
        audit = self.source / "scripts/runtime_pure_rust_audit.py"
        self.run(["/usr/bin/python3", audit, "metadata", "--cargo", "--root", "fe2o3-runtime"],
                 label="cargo-closure", timeout=180, env=env, cwd=self.source)
        self.run(["/usr/bin/python3", audit, "elf", "--input", binary], label="elf-closure")
        self.run(["/usr/bin/uname", "-a"], label="kernel")
        self.verify_source()

    def telemetry(self, label):
        raw = self.run([self.args.rocm_path / "bin/rocm-smi", "--showuniqueid", "--showbus",
                       "--showuse", "--showmemuse", "--json"], label=f"telemetry-{label}")
        card = json.loads(raw)[f"card{base.GPU_INDEX}"]
        if (card["Unique ID"] != base.UNIQUE_ID or card["PCI Bus"].lower() != "0000:26:00.0"
                or int(card["GPU use (%)"]) > 5 or int(card["GPU Memory Allocated (VRAM%)"]) != 0):
            raise base.RunError("selected GPU identity changed or GPU is busy")

    def qualify_and_measure(self):
        self.identity_start = {"pci_bdf": "0000:26:00.0"}
        self.topology_raw = self.topology_record("initial")
        topology = base.sealed_fields(self.topology_raw, "topology", self.retained.R26_TOPOLOGY_SEALED_FIELDS)
        identity = {"pci_bdf": "0000:26:00.0", "pci_numa_node": topology["numa_node"],
                    "gpu_node_id": topology["kfd_node"], "gpu_guid": topology["kfd_gpu_id"]}
        self.topology = base.validate_topology(self.topology_raw, identity, self.retained)
        self.run(["/usr/bin/taskset", "--cpu-list", self.topology["measurement_cpu_list"],
                  "/usr/bin/numactl", f"--physcpubind={self.topology['measurement_cpu_list']}",
                  f"--membind={self.topology['numa_node']}", "/usr/bin/true"], label="placement-probe")
        for ordinal in range(2):
            output = self.phase(f"owner-{ordinal}", "kfd", [base.UNIQUE_ID])
            validate_output(output.read_text())
        self.verify_source()

    def publish(self):
        destination = self.args.output_dir / f"r61-owner-{secrets.token_hex(16)}"
        base.write_json(self.evidence / "commands.json", self.commands)
        base.write_json(self.evidence / "provenance.json", {
            "schema": "fe2o3.r61-owner-copy-qualification.v1", "source_commit": self.commit,
            "claim_scope": "one-device-one-stream-h2d-d2h-abandoned-observer-custody-and-cleanup",
            "source_archive_sha256": base.sha256_file(self.evidence / "source.tar"),
            "snapshot_input_sha256": self.snapshot_input_hashes,
            "tool_identity": self.tool_hashes, "binary_sha256": self.binary_hashes,
            "topology": self.topology, "qualification_runs": 2,
            "census_retry_policy": "abort-set", "performance_claim": False,
        })
        shutil.copy2(self.binaries["kfd"], self.evidence / "owner-binary")
        base.write_json(self.evidence / "sha256.json", base.tree_hashes(self.evidence))
        pending = pathlib.Path(tempfile.mkdtemp(prefix=".r61-publish.", dir=self.args.output_dir))
        try:
            shutil.copytree(self.evidence, pending, dirs_exist_ok=True)
            actual = base.tree_hashes(pending)
            del actual["sha256.json"]
            if actual != json.loads((pending / "sha256.json").read_text()):
                raise base.RunError("copied owner evidence differs from manifest")
            base.readonly_tree(pending)
            pending.rename(destination)
        finally:
            base.cleanup_tree(pending)
        return destination


def main():
    os.umask(0o077)
    args = base.parse_args()
    for signum in (signal.SIGHUP, signal.SIGINT, signal.SIGQUIT, signal.SIGTERM):
        signal.signal(signum, base.interrupted)
    stage = runner = destination = None
    completed = False
    try:
        stage = pathlib.Path(tempfile.mkdtemp(prefix="fe2o3-r61-owner.", dir=args.staging_parent))
        runner = OwnerRunner(args, stage)
        runner.snapshot()
        runner.build()
        runner.qualify_and_measure()
        destination = runner.publish()
        base.cleanup_tree(stage)
        stage = None
        print(f"PASS R61 owner qualification: {destination}")
        completed = True
        return 0
    except Exception as error:
        rejected = None
        if runner is not None and runner.evidence.exists():
            commands = runner.evidence / "commands.json"
            if not commands.exists():
                base.write_json(commands, runner.commands)
            base.write_json(runner.evidence / "rejection.json", {"error": str(error)})
            rejected = args.output_dir / f"r61-rejected-{secrets.token_hex(16)}"
            shutil.copytree(runner.evidence, rejected)
        print(f"R61 rejected: {error}; diagnostics={rejected}", file=sys.stderr)
        return 2
    finally:
        try:
            if stage is not None:
                base.cleanup_tree(stage)
        finally:
            if destination is not None and not completed:
                base.cleanup_tree(destination)


if __name__ == "__main__":
    sys.exit(main())
