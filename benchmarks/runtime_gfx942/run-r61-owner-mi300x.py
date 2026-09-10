#!/usr/bin/env python3
"""Qualify the signed R61 copy owner using unchanged R60 custody/R26 host guards."""

import importlib.util
import json
import os
import pathlib
import secrets
import shlex
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
HOST_TARGET = "x86_64-unknown-linux-musl"
PASS = ("PASS schema=fe2o3.runtime.r61-async-owner-copy.v1 bytes=1048832 "
        "owner_threads=1 abandoned_upload=completed canaries=complete cleanup=complete\n")


def validate_output(output, expected=PASS):
    if output != expected:
        raise base.RunError("owner qualifier did not emit exact PASS")


def validate_static_symbols(output, policy):
    """Apply the unchanged symbol bans to the full, not merely dynamic, table."""
    symbols = set()
    for line in output.splitlines():
        fields = line.split()
        if len(fields) not in (3, 4) or len(fields[1]) != 1 or not fields[1].isalpha():
            raise base.RunError("missing or malformed full symbol evidence")
        name, kind = fields[:2]
        if kind in "Uwv":
            raise base.RunError(f"static owner has unresolved symbol: {name}")
        try:
            for value in fields[2:]:
                int(value, 16)
        except ValueError as error:
            raise base.RunError("malformed full symbol address") from error
        normalized = name.split("@", 1)[0].lstrip("_").lower()
        if (normalized in policy["forbidden_dynamic_symbols"]
                or any(normalized.startswith(prefix) for prefix in policy["forbidden_dynamic_symbol_prefixes"])
                or normalized == "pthread_get_minstack"):
            raise base.RunError(f"prohibited full symbol: {name}")
        symbols.add(name)
    if not {"main", "pthread_create"}.issubset(symbols):
        raise base.RunError("full owner symbol table lacks required anchors")
    return len(symbols)


def validate_static_headers(output):
    lines = [line.split() for line in output.splitlines() if line.strip()]
    if not any(fields[0] == "LOAD" for fields in lines):
        raise base.RunError("missing static ELF program headers")
    if any(fields[0] == "INTERP" or "(NEEDED)" in fields for fields in lines):
        raise base.RunError("static owner has an interpreter or dynamic dependency")


def validate_link_command(output):
    if len(output.splitlines()) != 1:
        raise base.RunError("missing or ambiguous final link command")
    fields = shlex.split(output)
    while fields and "=" in fields[0] and not fields[0].startswith("-"):
        fields.pop(0)
    if (not fields or fields[0] != "/usr/bin/cc"
            or [field for field in fields if field.startswith("-fuse-ld=")] != ["-fuse-ld=bfd"]
            or fields.count("-static-pie") != 1
            or any(field in ("-shared", "-pie", "-no-pie", "-static", "-r") for field in fields)):
        raise base.RunError("final link did not use the selected static host profile")


class OwnerRunner(base.Runner):
    label = "r61"
    example = EXAMPLE
    expected_pass = PASS
    schema = "fe2o3.r61-owner-copy-qualification.v1"
    claim_scope = "one-device-one-stream-h2d-d2h-abandoned-observer-custody-and-cleanup"
    cargo_features = ()

    def cargo_feature_arguments(self):
        if self.cargo_features not in ((), ("fe2o3-runtime/hardware-qualification",)):
            raise base.RunError("unadmitted owner Cargo feature profile")
        return ["--features", ",".join(self.cargo_features)] if self.cargo_features else []

    def validate_qualifier_output(self, output):
        validate_output(output, self.expected_pass)

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
        tools.append(pathlib.Path("/usr/bin/cc"))
        for name in ("ld.bfd", "collect2"):
            program = self.run(["/usr/bin/cc", f"-print-prog-name={name}"], label=f"resolved-{name}", env=env)
            resolved = shutil.which(program.strip(), path=base.SYSTEM_PATH)
            if resolved is None:
                raise base.RunError(f"selected linker tool is missing: {name}")
            tools.append(pathlib.Path(resolved))
        for name in ("cargo", "rustc"):
            self.run([rust / name, "--version"], label=f"{name}-version", env=env, cwd=self.source)
            tools.append(pathlib.Path(self.run([rust / "rustup", "which", name],
                label=f"resolved-{name}", env=env, cwd=self.source).strip()))
        self.run([rust / "rustc", "-vV"], label="rustc-verbose-version", env=env, cwd=self.source)
        self.tool_hashes = {str(path): {"resolved": str(path.resolve(strict=True)),
                            "sha256": base.sha256_file(path)} for path in tools}
        target_libdir = pathlib.Path(self.run([rust / "rustc", "--print", "target-libdir", "--target", HOST_TARGET],
                                              label="target-libdir", env=env, cwd=self.source).strip())
        target_libraries = base.tree_hashes(target_libdir)
        if not target_libraries or not any(name.startswith("libstd-") for name in target_libraries):
            raise base.RunError("target standard-library closure is missing")
        base.write_json(self.evidence / "target-libraries.json", target_libraries)
        link_command = self.run([rust / "cargo", "rustc", "--offline", "--locked", "--release",
                  "--target", HOST_TARGET, "--no-default-features", *self.cargo_feature_arguments(), "-p", "fe2o3-runtime", "--example", self.example,
                  "--", "--print=link-args", "-C", "linker=/usr/bin/cc", "-C", "link-arg=-fuse-ld=bfd"],
                 label="build-owner", timeout=1200, env=env, cwd=self.source)
        validate_link_command(link_command)
        binary = self.stage / "target" / HOST_TARGET / "release/examples" / self.example
        binary.chmod(0o500)
        self.binaries = {"kfd": binary}
        self.binary_hashes = {"kfd": base.sha256_file(binary)}
        audit = self.source / "scripts/runtime_pure_rust_audit.py"
        metadata = self.evidence / "cargo-metadata.json"
        self.run([rust / "cargo", "metadata", "--offline", "--locked", "--format-version", "1",
                  "--filter-platform", HOST_TARGET, "--no-default-features", *self.cargo_feature_arguments()],
                 label="cargo-metadata", timeout=180, env=env, cwd=self.source, output=metadata)
        self.run(["/usr/bin/python3", audit, "metadata", "--input", metadata, "--root", "fe2o3-runtime"],
                 label="cargo-closure", timeout=180, env=env, cwd=self.source)
        self.run(["/usr/bin/python3", audit, "elf", "--input", binary], label="elf-closure")
        headers = self.run(["/usr/bin/readelf", "--program-headers", "--dynamic", "--wide", binary], label="static-headers")
        validate_static_headers(headers)
        symbols = self.run(["/usr/bin/nm", "--format=posix", "--no-demangle", binary], label="full-symbols")
        count = validate_static_symbols(symbols, json.loads((self.source / "scripts/runtime-pure-rust-policy.json").read_text()))
        if binary.stat().st_size > (64 << 20) or b"__pthread_get_minstack" in binary.read_bytes():
            raise base.RunError("owner binary exceeds its bound or retains the GNU stack lookup")
        if base.tree_hashes(target_libdir) != target_libraries:
            raise base.RunError("target standard-library closure changed during build")
        for path, identity in self.tool_hashes.items():
            if str(pathlib.Path(path).resolve(strict=True)) != identity["resolved"] or base.sha256_file(pathlib.Path(path)) != identity["sha256"]:
                raise base.RunError("selected build or audit tool changed during qualification")
        base.write_json(self.evidence / "static-closure.json", {
            "host_target": HOST_TARGET, "full_symbol_names": count, "undefined_symbols": 0,
            "interpreter": False, "dynamic_dependencies": 0, "gnu_minstack_lookup": False,
            "binary_sha256": base.sha256_file(binary),
            "target_libdir": str(target_libdir), "target_libraries_rechecked": True,
        })
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
            self.validate_qualifier_output(output.read_text())
        self.verify_source()

    def publish(self):
        destination = self.args.output_dir / f"{self.label}-owner-{secrets.token_hex(16)}"
        base.write_json(self.evidence / "commands.json", self.commands)
        base.write_json(self.evidence / "provenance.json", {
            "schema": self.schema, "source_commit": self.commit,
            "host_target": HOST_TARGET,
            "claim_scope": self.claim_scope,
            "source_archive_sha256": base.sha256_file(self.evidence / "source.tar"),
            "snapshot_input_sha256": self.snapshot_input_hashes,
            "tool_identity": self.tool_hashes, "binary_sha256": self.binary_hashes,
            "topology": self.topology, "qualification_runs": 2,
            "census_retry_policy": "abort-set", "performance_claim": False,
        })
        shutil.copy2(self.binaries["kfd"], self.evidence / "owner-binary")
        base.write_json(self.evidence / "sha256.json", base.tree_hashes(self.evidence))
        pending = pathlib.Path(tempfile.mkdtemp(prefix=f".{self.label}-publish.", dir=self.args.output_dir))
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


def main(runner_type=None, label="r61"):
    os.umask(0o077)
    args = base.parse_args()
    for signum in (signal.SIGHUP, signal.SIGINT, signal.SIGQUIT, signal.SIGTERM):
        signal.signal(signum, base.interrupted)
    stage = runner = destination = None
    completed = False
    try:
        stage = pathlib.Path(tempfile.mkdtemp(prefix=f"fe2o3-{label}-owner.", dir=args.staging_parent))
        runner = (OwnerRunner if runner_type is None else runner_type)(args, stage)
        runner.snapshot()
        runner.build()
        runner.qualify_and_measure()
        destination = runner.publish()
        base.cleanup_tree(stage)
        stage = None
        print(f"PASS {label.upper()} owner qualification: {destination}")
        completed = True
        return 0
    except Exception as error:
        rejected = None
        if runner is not None and runner.evidence.exists():
            commands = runner.evidence / "commands.json"
            if not commands.exists():
                base.write_json(commands, runner.commands)
            base.write_json(runner.evidence / "rejection.json", {"error": str(error)})
            rejected = args.output_dir / f"{label}-rejected-{secrets.token_hex(16)}"
            shutil.copytree(runner.evidence, rejected)
        print(f"{label.upper()} rejected: {error}; diagnostics={rejected}", file=sys.stderr)
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
