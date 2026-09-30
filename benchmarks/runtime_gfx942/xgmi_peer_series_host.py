#!/usr/bin/env python3
"""Query-only host identity capture, run inside the native owner's fresh group."""

import argparse
import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
IDENTITY_SHA256 = "6a80769bde37c41787a28c725d9c6eeb04ec0837a752924969bed952af8e036f"


def load_identity():
    path = HERE / "r26-system-identity.py"
    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != IDENTITY_SHA256:
        raise ValueError("reviewed identity helper changed")
    spec = importlib.util.spec_from_file_location("series_system_identity", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def package_source_root(raw):
    roots = {match[1] for line in raw.decode("ascii").splitlines()
             if (match := re.fullmatch(r"/usr/src/(amdgpu-[0-9][A-Za-z0-9.+-]*)(?:/.*)?", line))}
    if len(roots) != 1:
        raise ValueError("package must identify exactly one amdgpu source tree")
    return roots.pop()


def canonical_source_path(root, relative):
    if not root.is_dir() or root.resolve(strict=True) != root:
        raise ValueError("package source root must be a canonical directory")
    name = Path(relative)
    if name.is_absolute() or any(part in ("", ".", "..") for part in relative.split("/")):
        raise ValueError("package source name must be canonical and relative")
    path = root / name
    if not path.is_file() or path.resolve(strict=True) != path:
        raise ValueError("package source file must be canonical without aliases")
    return path


def collect(args, planner):
    identity = load_identity()
    context = identity.collect(args)
    environment = identity.command_environment()
    dpkg = identity.fixed_tool(Path("/usr/bin/dpkg-query"))
    package_raw = identity.run([str(dpkg.path), "-W", "-f=${binary:Package}\t${Version}\n",
                               "amdgpu-dkms"], environment=environment)
    match = re.fullmatch(rb"(amdgpu-dkms)\t([^\s]+)\n", package_raw)
    if match is None:
        raise ValueError("unexpected package observation")
    files_raw = identity.run([str(dpkg.path), "-L", "amdgpu-dkms"], environment=environment)
    tree = package_source_root(files_raw)
    root = Path("/usr/src") / tree
    sources, source_paths = {}, {}
    for relative in planner._SOURCES:
        path = canonical_source_path(root, relative)
        sources[relative] = identity.sha256(identity.read_stable(path))
        source_paths[relative] = str(path)
    identity.require_tools_unchanged((dpkg,))
    package = {"package_query_base64": base64.b64encode(package_raw).decode("ascii"),
               "package_files_base64": base64.b64encode(files_raw).decode("ascii"),
               "source_tree": tree, "source_sha256": sources,
               "source_paths": source_paths,
               "dpkg_query_sha256": dpkg.digest}
    from xgmi_peer_series_observations import digest
    observation = {
        "schema": "fe2o3.xgmi-peer-reviewed-environment-observation.v1",
        "identity": {
            "kernel_release": context["kernel_release"],
            "amdgpu_version": context["amdgpu_version"],
            "amdgpu_srcversion": context["amdgpu_srcversion"],
            "loaded_build_id": context["amdgpu_build_id"],
            "installed_build_id": context["amdgpu_module_build_id"],
            "module_compressed_sha256": context["amdgpu_module_sha256"],
            "module_elf_sha256": context["amdgpu_module_decompressed_sha256"],
            "package": match[1].decode("ascii"), "package_version": match[2].decode("ascii"),
            "source_tree": tree,
        },
        "source_sha256": sources,
        "evidence_sha256": {
            "kernel": digest({key: value for key, value in context.items()
                              if key.startswith("kernel_") or key == "boot_id"}),
            "loaded_module": digest({key: value for key, value in context.items()
                                     if key.startswith("amdgpu_") and not key.startswith("amdgpu_module_")}),
            "installed_module": digest({key: value for key, value in context.items()
                                        if key.startswith("amdgpu_module_")}),
            "package_and_source": digest(package),
        },
        "environment_assumption": planner.ASSUMPTION,
        "excluded_changes": list(planner._EXCLUDED_CHANGES),
    }
    planner.validate_environment(observation)
    # ldd's raw addresses vary; the reviewed helper also records canonical maps.
    continuity = {key: value for key, value in context.items()
                  if key != "observation_edge" and not key.endswith(("_ldd_base64", "_ldd_sha256"))
                  and not key.startswith("rocm_smi_identity_")}
    continuity["package_and_source_sha256"] = digest(package)
    return {"schema": "fe2o3.xgmi-peer-series-host-observation.v1",
            "environment": observation, "context": context, "package_and_source": package,
            "continuity": continuity}


def main():
    if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
        raise RuntimeError("use python3 -I -B")
    sys.path.insert(0, str(HERE))
    import xgmi_peer_series_campaign as planner
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gpu-index", type=int, required=True)
    parser.add_argument("--observation-edge", choices=("start", "end"), required=True)
    parser.add_argument("--kfd-binary", type=Path, required=True)
    parser.add_argument("--hsa-binary", type=Path, required=True)
    parser.add_argument("--hip-binary", type=Path, required=True)
    args = parser.parse_args()
    args.rocm_path = Path("/opt/rocm")
    print(json.dumps(collect(args, planner), sort_keys=True, separators=(",", ":"), allow_nan=False))


if __name__ == "__main__":
    main()
