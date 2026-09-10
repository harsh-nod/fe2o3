"""Retain generated evidence and independently inspect its actual ELF and censuses."""
import hashlib
import importlib.util
import json
import pathlib
import subprocess
import sys
import tarfile
import tempfile

sys.dont_write_bytecode = True
capture, repo, root = map(pathlib.Path, sys.argv[1:])
prefix = "output/r65-owner-5f946e71cb14c3b46415705434482a6b/"
with tarfile.open(capture) as archive:
    payload = {member.name[len(prefix):]: archive.extractfile(member).read()
               for member in archive.getmembers()
               if member.isfile() and member.name.startswith(prefix)}
    outer = {name: archive.extractfile(name).read() for name in ("run.log", "clone.log", "exit-status.txt")}
assert outer["exit-status.txt"] == b"0\n"
manifest = json.loads(payload["sha256.json"])
assert manifest == {name: hashlib.sha256(value).hexdigest() for name, value in payload.items() if name != "sha256.json"}
retained = root / "raw/musl-accepted"
retained.mkdir(parents=True, exist_ok=True)
for name, value in payload.items():
    assert pathlib.PurePosixPath(name).name == name
    if name not in ("owner-binary", "source.tar"):
        (retained / name).write_bytes(value)
(root / "outer").mkdir(exist_ok=True)
for name, value in outer.items():
    (root / "outer" / name).write_bytes(value)
source_files = json.loads(payload["source-files.json"])
for name in ("benchmarks/runtime_gfx942/run-r61-owner-mi300x.py",
             "benchmarks/runtime_gfx942/run-r60-pipeline-mi300x.py",
             "benchmarks/runtime_gfx942/check-parity.py",
             "scripts/runtime-pure-rust-policy.json", "scripts/runtime_pure_rust_audit.py"):
    assert hashlib.sha256((repo / name).read_bytes()).hexdigest() == source_files[name]
spec = importlib.util.spec_from_file_location("r65_independent_owner", repo / "benchmarks/runtime_gfx942/run-r61-owner-mi300x.py")
owner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(owner)
checker = owner.base.load_module(repo / "benchmarks/runtime_gfx942/check-parity.py", "r65_independent_retained")
policy = json.loads((repo / "scripts/runtime-pure-rust-policy.json").read_text())
with tempfile.TemporaryDirectory(prefix="fe2o3-r65-elf-audit-") as directory:
    binary = pathlib.Path(directory) / "owner-binary"
    binary.write_bytes(payload["owner-binary"])
    symbols = subprocess.check_output(["/usr/bin/nm", "--format=posix", "--no-demangle", binary], text=True)
    headers = subprocess.check_output(["/usr/bin/readelf", "--program-headers", "--dynamic", "--wide", binary], text=True)
    count = owner.validate_static_symbols(symbols, policy)
    owner.validate_static_headers(headers)
    assert symbols == payload["018-full-symbols.stdout"].decode()
    assert headers == payload["017-static-headers.stdout"].decode()
    assert b"__pthread_get_minstack" not in payload["owner-binary"]
    assert b"tokio" not in payload["owner-binary"]
    closure = json.loads(payload["static-closure.json"])
    assert count == closure["full_symbol_names"]
    audit = repo / "scripts/runtime_pure_rust_audit.py"
    elf = subprocess.check_output([sys.executable, audit, "--policy", repo / "scripts/runtime-pure-rust-policy.json",
                                   "elf", "--input", binary], text=True)
metadata = subprocess.check_output([sys.executable, audit, "--policy", repo / "scripts/runtime-pure-rust-policy.json",
    "metadata", "--input", retained / "cargo-metadata.json", "--root", "fe2o3-runtime"], text=True)
provenance = json.loads(payload["provenance.json"])
topology = provenance["topology"]
monitors = [owner.base.validate_monitor(payload[f"{label}-monitor-owner-{ordinal}.stdout"].decode(),
    retained / f"owner-{ordinal}.jsonl", topology, checker) for ordinal, label in [(0, "024"), (1, "029")]]
identity = {"pci_bdf": "0000:26:00.0", "pci_numa_node": topology["numa_node"],
            "gpu_node_id": topology["kfd_node"], "gpu_guid": topology["kfd_gpu_id"]}
topologies = [name for name in payload if "-topology-" in name and name.endswith(".stdout")]
assert len(topologies) == 5
for name in topologies:
    assert owner.base.validate_topology(payload[name].decode(), identity, checker) == topology
telemetries = [name for name in payload if "-telemetry-" in name and name.endswith(".stdout")]
assert len(telemetries) == 4
for name in telemetries:
    card = json.loads(payload[name])["card1"]
    assert card["Unique ID"] == "0xab83d2ffef0d3cdf"
    assert card["PCI Bus"].lower() == "0000:26:00.0"
    assert int(card["GPU use (%)"]) <= 5
    assert int(card["GPU Memory Allocated (VRAM%)"]) == 0
print(json.dumps({"full_symbol_names": count, "actual_symbols_match_capture": True,
    "actual_headers_match_capture": True, "elf_audit": elf.strip(), "metadata_audit": metadata.strip(),
    "source_archive_sha256": provenance["source_archive_sha256"], "monitors": monitors,
    "matching_topology_records": len(topologies), "idle_boundary_telemetries": len(telemetries)}, sort_keys=True))
