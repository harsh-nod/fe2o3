#!/usr/bin/env python3
"""Emit fail-closed R26 MI300X host and loader identity evidence."""

from __future__ import annotations

import argparse
import base64
import hashlib
import os
import pathlib
import re
import resource
import shlex
import struct
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from typing import Mapping


SCHEMA = "fe2o3.r26-system-identity.v1"
EXECUTION_ENVIRONMENT = "env-i-lang-c-lc-all-c-path-usr-sbin-usr-bin-sbin-bin-v1"
HSA_SONAME = "libhsa-runtime64.so.1"
HIP_SONAME = "libamdhip64.so.7"
ROCM_SMI_SONAME = "librocm_smi64.so.1"
ROCM_SMI_PACKAGE_FILES = ("rocm_smi.py", "rsmiBindings.py", "rsmiBindingsInit.py")
ROCM_SMI_SHEBANG = b"#!/usr/bin/env python3\n"
LOADER_RESOLUTION = "fixed-ldd-transitive-observed-to-canonical-v1"
BUILD_ID = re.compile(r"[0-9a-f]{16,128}")
UNIQUE_ID = re.compile(r"0x[0-9a-f]{16}")
BDF = re.compile(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]")
FORBIDDEN_KFD_RUNTIME = re.compile(r"lib(?:hsa-runtime64|amdhip64)\.so(?:\..*)?")
MAX_DECOMPRESSED_MODULE_BYTES = 256 * 1024 * 1024
MAX_DECOMPRESSOR_ADDRESS_SPACE_BYTES = 512 * 1024 * 1024
MODINFO_PATH = pathlib.Path("/usr/sbin/modinfo")
LDD_PATH = pathlib.Path("/usr/bin/ldd")
READELF_PATH = pathlib.Path("/usr/bin/readelf")
ZSTD_PATH = pathlib.Path("/usr/bin/zstd")
PYTHON3_PATH = pathlib.Path("/usr/bin/python3")
BOOT_ID = re.compile(
    r"[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}"
)
MAX_LDD_BYTES = 1 << 20
MAX_LOADER_DEPENDENCIES = 4096
MAX_LOADER_EVIDENCE_BYTES = 4 << 20


class IdentityError(RuntimeError):
    pass


@dataclass(frozen=True)
class ProductIdentity:
    unique_id: str
    serial: str
    pci_bdf: str
    series: str
    model: str
    vendor: str
    sku: str
    subsystem_id: str
    revision: str
    node_id: str
    guid: str
    gfx_version: str


@dataclass(frozen=True)
class PciIdentity:
    unique_id: str
    serial: str
    product_name: str
    product_number: str
    vendor: str
    device: str
    subsystem_vendor: str
    subsystem_device: str
    revision: str
    device_class: str
    numa_node: str
    driver: str


@dataclass(frozen=True)
class FixedTool:
    path: pathlib.Path
    digest: str


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def b64(data: bytes) -> str:
    return base64.b64encode(data).decode("ascii")


def read_stable(path: pathlib.Path) -> bytes:
    try:
        before = path.stat()
        data = path.read_bytes()
        after = path.stat()
    except OSError as error:
        raise IdentityError(f"cannot read identity source {path}: {error}") from error
    identity_before = (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
    )
    identity_after = (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
    )
    # procfs/sysfs report synthetic sizes (commonly zero or one page), so only
    # compare their metadata snapshots rather than equating st_size with bytes.
    if identity_before != identity_after:
        raise IdentityError(f"identity source changed while reading: {path}")
    return data


def scalar(path: pathlib.Path, *, allow_empty: bool = False) -> str:
    try:
        value = read_stable(path).decode("utf-8", "strict").strip()
    except UnicodeDecodeError as error:
        raise IdentityError(f"identity scalar is not UTF-8: {path}") from error
    if (not value and not allow_empty) or any(
        character in value for character in ("\x00", "\n", "\r")
    ):
        raise IdentityError(f"identity scalar is malformed: {path}")
    return value


def command_environment() -> dict[str, str]:
    return {
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/sbin:/usr/bin:/sbin:/bin",
    }


def run(
    command: list[str],
    *,
    environment: Mapping[str, str],
    pass_fds: tuple[int, ...] = (),
) -> bytes:
    try:
        result = subprocess.run(
            command,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=dict(environment),
            pass_fds=pass_fds,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise IdentityError(
            f"identity command failed to execute: {command[0]}"
        ) from error
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise IdentityError(
            f"identity command failed ({result.returncode}): {command[0]}: {detail}"
        )
    if result.stderr:
        raise IdentityError(f"identity command emitted stderr: {command[0]}")
    return result.stdout


def parse_rocm_smi_identity(text: str, gpu_index: int) -> ProductIdentity:
    rows: dict[int, dict[str, str]] = {}
    line_pattern = re.compile(r"^GPU\[([0-9]+)\]\s*:\s*([^:]+):\s*(.*?)\s*$")
    for line in text.splitlines():
        match = line_pattern.fullmatch(line.strip())
        if match is None:
            continue
        index = int(match.group(1))
        key = " ".join(match.group(2).split())
        value = match.group(3).strip()
        fields = rows.setdefault(index, {})
        if key in fields:
            raise IdentityError(f"duplicate rocm-smi field for GPU {index}: {key}")
        fields[key] = value

    required = {
        "Unique ID",
        "Serial Number",
        "PCI Bus",
        "Card Series",
        "Card Model",
        "Card Vendor",
        "Card SKU",
        "Subsystem ID",
        "Device Rev",
        "Node ID",
        "GUID",
        "GFX Version",
    }
    fields = rows.get(gpu_index)
    if fields is None:
        raise IdentityError(f"rocm-smi omitted GPU {gpu_index}")
    missing = required - fields.keys()
    if missing:
        raise IdentityError(f"rocm-smi omitted field: {sorted(missing)[0]}")
    unique_id = fields["Unique ID"].lower()
    pci_bdf = fields["PCI Bus"].lower()
    if UNIQUE_ID.fullmatch(unique_id) is None or BDF.fullmatch(pci_bdf) is None:
        raise IdentityError("rocm-smi unique ID or PCI BDF is not canonical")
    if unique_id == "0x0000000000000000":
        raise IdentityError("rocm-smi unique ID is zero")
    return ProductIdentity(
        unique_id=unique_id,
        serial=fields["Serial Number"],
        pci_bdf=pci_bdf,
        series=fields["Card Series"],
        model=fields["Card Model"].lower(),
        vendor=fields["Card Vendor"],
        sku=fields["Card SKU"],
        subsystem_id=fields["Subsystem ID"].lower(),
        revision=fields["Device Rev"].lower(),
        node_id=fields["Node ID"],
        guid=fields["GUID"],
        gfx_version=fields["GFX Version"],
    )


def validate_product(product: ProductIdentity, pci: PciIdentity) -> None:
    expected_product = {
        "series": "AMD Instinct MI300X",
        "model": "0x74a1",
        "vendor": "Advanced Micro Devices, Inc. [AMD/ATI]",
        "sku": "M3000100",
        "subsystem_id": "0x74a1",
        "revision": "0x00",
        "gfx_version": "gfx942",
    }
    expected_pci = {
        "product_name": "AMD Instinct MI300X OAM",
        "product_number": "102-G30211-00",
        "vendor": "0x1002",
        "device": "0x74a1",
        "subsystem_vendor": "0x1002",
        "subsystem_device": "0x74a1",
        "revision": "0x00",
        "device_class": "0x120000",
        "driver": "amdgpu",
    }
    for field, expected in expected_product.items():
        if getattr(product, field) != expected:
            raise IdentityError(f"selected GPU has unsupported product {field}")
    for field, expected in expected_pci.items():
        if getattr(pci, field) != expected:
            raise IdentityError(f"selected GPU has unsupported PCI {field}")
    if re.fullmatch(r"[0-9a-f]{16}", pci.unique_id) is None:
        raise IdentityError("PCI sysfs unique ID is not canonical")
    if product.unique_id.removeprefix("0x") != pci.unique_id:
        raise IdentityError("rocm-smi and PCI sysfs unique IDs differ")
    if product.serial != pci.serial:
        raise IdentityError("rocm-smi and PCI sysfs serial numbers differ")
    if not product.serial.isdigit() or product.serial == "0":
        raise IdentityError("selected GPU serial number is not canonical")
    if product.model != pci.device or product.subsystem_id != pci.subsystem_device:
        raise IdentityError("rocm-smi and PCI sysfs device identities differ")
    if (
        not product.node_id.isdigit()
        or not product.guid.isdigit()
        or product.guid == "0"
    ):
        raise IdentityError("rocm-smi node or GUID is not canonical")
    if re.fullmatch(r"-?[0-9]+", pci.numa_node) is None:
        raise IdentityError("PCI NUMA node is not canonical")


def read_pci_identity(root: pathlib.Path) -> PciIdentity:
    driver_path = root / "driver"
    try:
        driver = driver_path.resolve(strict=True).name
    except OSError as error:
        raise IdentityError("selected PCI device has no bound driver") from error
    return PciIdentity(
        unique_id=scalar(root / "unique_id"),
        serial=scalar(root / "serial_number"),
        product_name=scalar(root / "product_name"),
        product_number=scalar(root / "product_number"),
        vendor=scalar(root / "vendor").lower(),
        device=scalar(root / "device").lower(),
        subsystem_vendor=scalar(root / "subsystem_vendor").lower(),
        subsystem_device=scalar(root / "subsystem_device").lower(),
        revision=scalar(root / "revision").lower(),
        device_class=scalar(root / "class").lower(),
        numa_node=scalar(root / "numa_node"),
        driver=driver,
    )


def parse_os_release(data: bytes) -> dict[str, str]:
    try:
        text = data.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise IdentityError("os-release is not UTF-8") from error
    fields: dict[str, str] = {}
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise IdentityError("os-release contains a malformed line")
        key, encoded = line.split("=", 1)
        if re.fullmatch(r"[A-Z][A-Z0-9_]*", key) is None or key in fields:
            raise IdentityError("os-release contains an invalid or duplicate key")
        try:
            decoded = shlex.split(encoded, posix=True)
        except ValueError as error:
            raise IdentityError(f"os-release field {key} is malformed") from error
        if len(decoded) != 1:
            raise IdentityError(f"os-release field {key} is not one value")
        fields[key] = decoded[0]
    if fields.get("ID") != "ubuntu" or fields.get("VERSION_ID") != "24.04":
        raise IdentityError("R26 requires Ubuntu 24.04")
    return fields


def parse_ldd(text: str) -> dict[str, pathlib.Path]:
    if len(text.encode("utf-8")) > MAX_LDD_BYTES:
        raise IdentityError("loader output exceeds the retained evidence bound")
    resolved: dict[str, pathlib.Path] = {}
    lines = text.splitlines()
    if len(lines) > MAX_LOADER_DEPENDENCIES:
        raise IdentityError("loader output has excessive dependency cardinality")
    for line in lines:
        stripped = line.strip()
        if not stripped:
            raise IdentityError("loader output contains an empty row")
        if "=> not found" in stripped:
            raise IdentityError(f"loader dependency is unresolved: {stripped}")
        mapped = re.fullmatch(r"(\S+)\s+=>\s+(/\S+)\s+\(0x[0-9a-fA-F]+\)", stripped)
        direct = re.fullmatch(r"(/\S+)\s+\(0x[0-9a-fA-F]+\)", stripped)
        virtual = re.fullmatch(
            r"linux-(?:vdso|gate)\.so\.[0-9]+\s+\(0x[0-9a-fA-F]+\)",
            stripped,
        )
        if virtual is not None or stripped == "statically linked":
            continue
        if mapped is not None:
            soname, path_text = mapped.groups()
        elif direct is not None:
            path_text = direct.group(1)
            soname = pathlib.PurePosixPath(path_text).name
        else:
            raise IdentityError(f"loader output contains an unknown row: {stripped}")
        lexical_path = pathlib.PurePosixPath(path_text)
        if (
            re.fullmatch(r"[^/=\s]+", soname) is None
            or not lexical_path.is_absolute()
            or str(lexical_path) != path_text
            or ".." in lexical_path.parts
            or "=" in path_text
        ):
            raise IdentityError("loader output contains a noncanonical dependency")
        if soname in resolved:
            raise IdentityError(f"loader output contains duplicate dependency {soname}")
        resolved[soname] = pathlib.Path(path_text)
    return resolved


def canonical_loader_evidence(
    resolved: Mapping[str, pathlib.Path],
) -> tuple[bytes, bytes]:
    if not resolved or len(resolved) > MAX_LOADER_DEPENDENCIES:
        raise IdentityError("loader map has invalid dependency cardinality")
    map_rows: list[str] = []
    resolution_rows: list[str] = []
    for soname in sorted(resolved):
        observed = resolved[soname]
        if (
            re.fullmatch(r"[^/=\s]+", soname) is None
            or not observed.is_absolute()
            or any(
                character.isspace() or character in "=\t" for character in str(observed)
            )
            or pathlib.PurePosixPath(observed).as_posix() != str(observed)
            or ".." in observed.parts
        ):
            raise IdentityError("loader map contains a noncanonical dependency")
        try:
            canonical = observed.resolve(strict=True)
        except OSError as error:
            raise IdentityError(
                f"loader dependency does not exist: {soname}"
            ) from error
        if any(
            character.isspace() or character in "=\t" for character in str(canonical)
        ):
            raise IdentityError("canonical loader path is not representable")
        map_rows.append(f"{soname}={canonical}\n")
        resolution_rows.append(
            f"soname={soname}\tobserved={observed}\tresolved={canonical}\n"
        )
    canonical_map = "".join(map_rows).encode("utf-8", "strict")
    resolution = "".join(resolution_rows).encode("utf-8", "strict")
    if (
        len(canonical_map) > MAX_LOADER_EVIDENCE_BYTES
        or len(resolution) > MAX_LOADER_EVIDENCE_BYTES
    ):
        raise IdentityError("canonical loader evidence exceeds its retained bound")
    return canonical_map, resolution


def canonical_loader_map(resolved: Mapping[str, pathlib.Path]) -> bytes:
    return canonical_loader_evidence(resolved)[0]


def validate_loader_maps(
    hsa: Mapping[str, pathlib.Path], hip: Mapping[str, pathlib.Path]
) -> tuple[pathlib.Path, pathlib.Path]:
    if HIP_SONAME in hsa:
        raise IdentityError("raw HSA comparator resolves the HIP runtime")
    if HSA_SONAME not in hsa:
        raise IdentityError("raw HSA comparator does not resolve the HSA runtime")
    if HIP_SONAME not in hip or HSA_SONAME not in hip:
        raise IdentityError("HIP comparator lacks its HIP/HSA runtime dependencies")
    try:
        hsa_direct = hsa[HSA_SONAME].resolve(strict=True)
        hsa_via_hip = hip[HSA_SONAME].resolve(strict=True)
        hip_runtime = hip[HIP_SONAME].resolve(strict=True)
    except OSError as error:
        raise IdentityError("resolved runtime library does not exist") from error
    if hsa_direct != hsa_via_hip:
        raise IdentityError("HSA and HIP comparators resolve different HSA runtimes")
    if hsa_direct == hip_runtime:
        raise IdentityError("HSA and HIP SONAMEs resolve to the same file")
    return hsa_direct, hip_runtime


def validate_kfd_loader_map(kfd: Mapping[str, pathlib.Path]) -> None:
    forbidden = sorted(
        soname for soname in kfd if FORBIDDEN_KFD_RUNTIME.fullmatch(soname)
    )
    if forbidden:
        raise IdentityError(f"KFD benchmark resolves forbidden runtime {forbidden[0]}")


def parse_build_id_note(data: bytes) -> str:
    if len(data) < 16:
        raise IdentityError("GNU build-ID note is truncated")
    namesz, descsz, note_type = struct.unpack_from("<III", data)
    name_end = 12 + namesz
    desc_start = (name_end + 3) & ~3
    desc_end = desc_start + descsz
    padded_end = (desc_end + 3) & ~3
    if (
        namesz != 4
        or note_type != 3
        or data[12:name_end] != b"GNU\x00"
        or descsz < 8
        or descsz > 64
        or padded_end != len(data)
        or any(data[desc_end:padded_end])
    ):
        raise IdentityError("GNU build-ID note has an unsupported envelope")
    return data[desc_start:desc_end].hex()


def parse_readelf_build_id(text: str, description: str) -> str:
    build_ids = re.findall(r"Build ID:\s*([0-9a-fA-F]+)", text)
    if (
        len(build_ids) != 1
        or len(build_ids[0]) % 2 != 0
        or BUILD_ID.fullmatch(build_ids[0].lower()) is None
    ):
        raise IdentityError(f"{description} lacks one canonical build ID")
    return build_ids[0].lower()


def readelf_identity(
    readelf: pathlib.Path,
    path: pathlib.Path,
    expected_soname: str,
    environment: Mapping[str, str],
) -> str:
    dynamic = run([str(readelf), "-dW", str(path)], environment=environment).decode(
        "utf-8", "strict"
    )
    sonames = re.findall(r"\(SONAME\).*Shared library: \[([^]]+)\]", dynamic)
    if sonames != [expected_soname]:
        raise IdentityError(f"runtime library has the wrong SONAME: {path}")
    return readelf_build_id(readelf, path, f"runtime library {path}", environment)


def readelf_build_id(
    readelf: pathlib.Path,
    path: pathlib.Path,
    description: str,
    environment: Mapping[str, str],
) -> str:
    notes = run([str(readelf), "-nW", str(path)], environment=environment).decode(
        "utf-8", "strict"
    )
    return parse_readelf_build_id(notes, description)


def limit_decompressed_output() -> None:
    resource.setrlimit(
        resource.RLIMIT_FSIZE,
        (MAX_DECOMPRESSED_MODULE_BYTES, MAX_DECOMPRESSED_MODULE_BYTES),
    )
    resource.setrlimit(
        resource.RLIMIT_AS,
        (
            MAX_DECOMPRESSOR_ADDRESS_SPACE_BYTES,
            MAX_DECOMPRESSOR_ADDRESS_SPACE_BYTES,
        ),
    )


def decompressed_module_identity(
    zstd: pathlib.Path,
    readelf: pathlib.Path,
    module_path: pathlib.Path,
    environment: Mapping[str, str],
) -> tuple[str, str, int]:
    if module_path.suffix != ".zst":
        raise IdentityError("on-disk amdgpu module is not a zstd frame")
    with tempfile.TemporaryFile() as output:
        try:
            result = subprocess.run(
                [str(zstd), "--decompress", "--stdout", "--quiet", str(module_path)],
                check=False,
                stdout=output,
                stderr=subprocess.PIPE,
                env=dict(environment),
                timeout=30,
                preexec_fn=limit_decompressed_output,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise IdentityError(
                "amdgpu module decompression failed to execute"
            ) from error
        if result.returncode != 0:
            detail = result.stderr.decode("utf-8", "replace").strip()
            raise IdentityError(
                f"amdgpu module decompression failed ({result.returncode}): {detail}"
            )
        size = output.tell()
        if size <= 0 or size >= MAX_DECOMPRESSED_MODULE_BYTES:
            raise IdentityError("decompressed amdgpu module size is outside policy")
        output.seek(0)
        digest = hashlib.file_digest(output, "sha256").hexdigest()
        output.seek(0)
        descriptor = f"/proc/self/fd/{output.fileno()}"
        notes = run(
            [str(readelf), "-nW", descriptor],
            environment=environment,
            pass_fds=(output.fileno(),),
        ).decode("utf-8", "strict")
    return parse_readelf_build_id(notes, "on-disk amdgpu module"), digest, size


def validate_module_build_ids(loaded: str, on_disk: str) -> None:
    if on_disk != loaded:
        raise IdentityError("loaded and on-disk amdgpu build IDs differ")


def executable(path: pathlib.Path) -> pathlib.Path:
    try:
        resolved = path.resolve(strict=True)
        mode = resolved.stat().st_mode
    except OSError as error:
        raise IdentityError(f"benchmark executable is unavailable: {path}") from error
    if not resolved.is_file() or mode & 0o111 == 0:
        raise IdentityError(f"benchmark path is not executable: {path}")
    return resolved


def rocm_smi_package_identity(
    invocation: pathlib.Path, rocm_path: pathlib.Path
) -> tuple[pathlib.Path, bytes, bytes, pathlib.Path]:
    script = executable(invocation)
    try:
        script.relative_to(rocm_path)
    except ValueError as error:
        raise IdentityError(
            "ROCm SMI entry point resolves outside the ROCm tree"
        ) from error
    if script.name != "rocm_smi.py":
        raise IdentityError("ROCm SMI entry point is not the expected Python script")
    source = read_stable(script)
    if not source.startswith(ROCM_SMI_SHEBANG):
        raise IdentityError("ROCm SMI script has an unexpected interpreter contract")
    manifest_rows: list[str] = []
    for name in ROCM_SMI_PACKAGE_FILES:
        path = (script.parent / name).resolve(strict=True)
        if path.parent != script.parent or not path.is_file():
            raise IdentityError("ROCm SMI package file escapes its package directory")
        manifest_rows.append(f"file={name}\tsha256={sha256(read_stable(path))}\n")
    package_manifest = "".join(manifest_rows).encode("ascii")
    native_library = (script.parent / "../../lib" / ROCM_SMI_SONAME).resolve(
        strict=True
    )
    try:
        native_library.relative_to(rocm_path)
    except ValueError as error:
        raise IdentityError(
            "ROCm SMI native library resolves outside the ROCm tree"
        ) from error
    return script, source, package_manifest, native_library


def fixed_tool(path: pathlib.Path) -> FixedTool:
    try:
        is_executable = path.is_file() and os.access(path, os.X_OK)
    except OSError as error:
        raise IdentityError(f"cannot inspect fixed identity tool {path}") from error
    if not is_executable:
        raise IdentityError(f"fixed identity tool is unavailable: {path}")
    return FixedTool(path=path, digest=sha256(read_stable(path)))


def require_tools_unchanged(tools: tuple[FixedTool, ...]) -> None:
    for fixed in tools:
        if sha256(read_stable(fixed.path)) != fixed.digest:
            raise IdentityError(
                f"identity tool changed during collection: {fixed.path}"
            )


def render_context(fields: Mapping[str, str]) -> str:
    tokens = ["context", f"schema={SCHEMA}"]
    for key in sorted(fields):
        value = fields[key]
        if key == "schema" or re.fullmatch(r"[a-z][a-z0-9_]*", key) is None:
            raise IdentityError(f"context key is not canonical: {key}")
        if not value or any(character.isspace() for character in value):
            raise IdentityError(f"context value is not canonical: {key}")
        tokens.append(f"{key}={value}")
    return " ".join(tokens)


def collect(args: argparse.Namespace) -> dict[str, str]:
    for variable in ("LD_PRELOAD", "LD_AUDIT", "LD_LIBRARY_PATH"):
        if variable in os.environ:
            raise IdentityError(
                f"{variable} must be absent for deterministic resolution"
            )
    if args.gpu_index < 0:
        raise IdentityError("GPU index must be nonnegative")

    rocm_path = args.rocm_path.resolve(strict=True)
    rocm_smi_invocation = rocm_path / "bin" / "rocm-smi"
    rocm_smi, rocm_smi_source, rocm_smi_package, rocm_smi_library = (
        rocm_smi_package_identity(rocm_smi_invocation, rocm_path)
    )
    environment = command_environment()
    modinfo_tool = fixed_tool(MODINFO_PATH)
    ldd_tool = fixed_tool(LDD_PATH)
    readelf_tool = fixed_tool(READELF_PATH)
    zstd_tool = fixed_tool(ZSTD_PATH)
    python_path = executable(PYTHON3_PATH)
    python_tool = fixed_tool(python_path)
    fixed_tools = (modinfo_tool, ldd_tool, readelf_tool, zstd_tool, python_tool)
    product_snapshot = run(
        [
            str(python_path),
            str(rocm_smi),
            "--showproductname",
            "--showserial",
            "--showuniqueid",
            "--showbus",
        ],
        environment=environment,
    )
    try:
        product_text = product_snapshot.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise IdentityError("rocm-smi identity output is not UTF-8") from error
    product = parse_rocm_smi_identity(product_text, args.gpu_index)
    pci_root = pathlib.Path("/sys/bus/pci/devices") / product.pci_bdf
    pci = read_pci_identity(pci_root)
    validate_product(product, pci)

    os_release = read_stable(pathlib.Path("/etc/os-release"))
    parse_os_release(os_release)
    kernel_version = read_stable(pathlib.Path("/proc/version"))
    uname = os.uname()
    if uname.sysname != "Linux" or uname.machine != "x86_64":
        raise IdentityError("R26 requires Linux x86_64")
    expected_kernel = f"Linux version {uname.release} ".encode()
    if not kernel_version.startswith(expected_kernel):
        raise IdentityError("uname and /proc/version kernel releases differ")
    boot_id = scalar(pathlib.Path("/proc/sys/kernel/random/boot_id")).lower()
    if BOOT_ID.fullmatch(boot_id) is None:
        raise IdentityError("boot ID is not canonical")

    modinfo = modinfo_tool.path
    module_path = pathlib.Path(
        run([str(modinfo), "-F", "filename", "amdgpu"], environment=environment)
        .decode("utf-8", "strict")
        .strip()
    ).resolve(strict=True)
    module_bytes = read_stable(module_path)
    modinfo_version = (
        run([str(modinfo), "-F", "version", "amdgpu"], environment=environment)
        .decode("utf-8", "strict")
        .strip()
    )
    modinfo_srcversion = (
        run([str(modinfo), "-F", "srcversion", "amdgpu"], environment=environment)
        .decode("utf-8", "strict")
        .strip()
    )
    vermagic = run(
        [str(modinfo), "-F", "vermagic", "amdgpu"], environment=environment
    ).strip()
    sysfs_version = scalar(pathlib.Path("/sys/module/amdgpu/version"))
    sysfs_srcversion = scalar(pathlib.Path("/sys/module/amdgpu/srcversion"))
    if (
        not modinfo_version
        or not modinfo_srcversion
        or sysfs_version != modinfo_version
        or sysfs_srcversion != modinfo_srcversion
    ):
        raise IdentityError("loaded and on-disk amdgpu module identities differ")
    if not vermagic.startswith(f"{uname.release} ".encode()):
        raise IdentityError("amdgpu vermagic does not match the running kernel")
    expected_module_root = (pathlib.Path("/lib/modules") / uname.release).resolve(
        strict=True
    )
    try:
        module_path.relative_to(expected_module_root)
    except ValueError as error:
        raise IdentityError(
            "amdgpu module is outside the running kernel tree"
        ) from error
    build_note = read_stable(
        pathlib.Path("/sys/module/amdgpu/notes/.note.gnu.build-id")
    )
    module_build_id = parse_build_id_note(build_note)
    disk_module_build_id, decompressed_module_sha256, decompressed_module_bytes = (
        decompressed_module_identity(
            zstd_tool.path,
            readelf_tool.path,
            module_path,
            environment,
        )
    )
    validate_module_build_ids(module_build_id, disk_module_build_id)
    taint = scalar(pathlib.Path("/sys/module/amdgpu/taint"), allow_empty=True)

    kfd_binary = executable(args.kfd_binary)
    hsa_binary = executable(args.hsa_binary)
    hip_binary = executable(args.hip_binary)
    ldd = ldd_tool.path
    kfd_ldd = run([str(ldd), str(kfd_binary)], environment=environment)
    hsa_ldd = run([str(ldd), str(hsa_binary)], environment=environment)
    hip_ldd = run([str(ldd), str(hip_binary)], environment=environment)
    kfd_map = parse_ldd(kfd_ldd.decode("utf-8", "strict"))
    hsa_map = parse_ldd(hsa_ldd.decode("utf-8", "strict"))
    hip_map = parse_ldd(hip_ldd.decode("utf-8", "strict"))
    validate_kfd_loader_map(kfd_map)
    hsa_library, hip_library = validate_loader_maps(hsa_map, hip_map)
    kfd_loader_map, kfd_loader_resolution = canonical_loader_evidence(kfd_map)
    hsa_loader_map, hsa_loader_resolution = canonical_loader_evidence(hsa_map)
    hip_loader_map, hip_loader_resolution = canonical_loader_evidence(hip_map)
    for library in (hsa_library, hip_library):
        try:
            library.relative_to(rocm_path)
        except ValueError as error:
            raise IdentityError(
                "runtime library resolved outside the selected ROCm tree"
            ) from error
    readelf = readelf_tool.path
    hsa_build_id = readelf_identity(readelf, hsa_library, HSA_SONAME, environment)
    hip_build_id = readelf_identity(readelf, hip_library, HIP_SONAME, environment)
    python_ldd = run([str(ldd), str(python_path)], environment=environment)
    python_map = parse_ldd(python_ldd.decode("utf-8", "strict"))
    python_loader_map, python_loader_resolution = canonical_loader_evidence(python_map)
    python_build_id = readelf_build_id(
        readelf, python_path, "ROCm SMI Python interpreter", environment
    )
    rocm_smi_library_ldd = run(
        [str(ldd), str(rocm_smi_library)], environment=environment
    )
    rocm_smi_library_map = parse_ldd(rocm_smi_library_ldd.decode("utf-8", "strict"))
    rocm_smi_library_loader_map, rocm_smi_library_loader_resolution = (
        canonical_loader_evidence(rocm_smi_library_map)
    )
    rocm_smi_library_build_id = readelf_identity(
        readelf, rocm_smi_library, ROCM_SMI_SONAME, environment
    )
    hsa_library_bytes = read_stable(hsa_library)
    hip_library_bytes = read_stable(hip_library)
    python_bytes = read_stable(python_path)
    rocm_smi_library_bytes = read_stable(rocm_smi_library)
    hsa_binary_bytes = read_stable(hsa_binary)
    hip_binary_bytes = read_stable(hip_binary)
    kfd_binary_bytes = read_stable(kfd_binary)
    final_rocm_smi, final_source, final_package, final_library = (
        rocm_smi_package_identity(rocm_smi_invocation, rocm_path)
    )
    if (
        final_rocm_smi != rocm_smi
        or final_source != rocm_smi_source
        or final_package != rocm_smi_package
        or final_library != rocm_smi_library
        or read_stable(rocm_smi_library) != rocm_smi_library_bytes
    ):
        raise IdentityError("ROCm SMI provenance changed during collection")
    require_tools_unchanged(fixed_tools)

    return {
        "amdgpu_build_id": module_build_id,
        "amdgpu_build_note_base64": b64(build_note),
        "amdgpu_build_note_sha256": sha256(build_note),
        "amdgpu_module_path_base64": b64(os.fsencode(module_path)),
        "amdgpu_module_build_id": disk_module_build_id,
        "amdgpu_module_decompressed_bytes": str(decompressed_module_bytes),
        "amdgpu_module_decompressed_sha256": decompressed_module_sha256,
        "amdgpu_module_sha256": sha256(module_bytes),
        "amdgpu_srcversion": sysfs_srcversion,
        "amdgpu_taint": taint if taint else "none",
        "amdgpu_vermagic_base64": b64(vermagic),
        "amdgpu_version": sysfs_version,
        "boot_id": boot_id,
        "execution_environment": EXECUTION_ENVIRONMENT,
        "gfx_version": product.gfx_version,
        "gpu_guid": product.guid,
        "gpu_index": str(args.gpu_index),
        "gpu_node_id": product.node_id,
        "gpu_serial": product.serial,
        "hip_binary_sha256": sha256(hip_binary_bytes),
        "hip_ldd_base64": b64(hip_ldd),
        "hip_ldd_sha256": sha256(hip_ldd),
        "hip_loader_map_base64": b64(hip_loader_map),
        "hip_loader_map_sha256": sha256(hip_loader_map),
        "hip_loader_resolution_base64": b64(hip_loader_resolution),
        "hip_loader_resolution_sha256": sha256(hip_loader_resolution),
        "hip_library_build_id": hip_build_id,
        "hip_library_path_base64": b64(os.fsencode(hip_library)),
        "hip_library_sha256": sha256(hip_library_bytes),
        "hip_library_soname": HIP_SONAME,
        "hsa_binary_sha256": sha256(hsa_binary_bytes),
        "hsa_ldd_base64": b64(hsa_ldd),
        "hsa_ldd_sha256": sha256(hsa_ldd),
        "hsa_loader_map_base64": b64(hsa_loader_map),
        "hsa_loader_map_sha256": sha256(hsa_loader_map),
        "hsa_loader_resolution_base64": b64(hsa_loader_resolution),
        "hsa_loader_resolution_sha256": sha256(hsa_loader_resolution),
        "hsa_library_build_id": hsa_build_id,
        "hsa_library_path_base64": b64(os.fsencode(hsa_library)),
        "hsa_library_sha256": sha256(hsa_library_bytes),
        "hsa_library_soname": HSA_SONAME,
        "kernel_machine": uname.machine,
        "kernel_release": uname.release,
        "kernel_sysname": uname.sysname,
        "kernel_version_base64": b64(kernel_version),
        "kernel_version_sha256": sha256(kernel_version),
        "kfd_binary_sha256": sha256(kfd_binary_bytes),
        "kfd_ldd_base64": b64(kfd_ldd),
        "kfd_ldd_sha256": sha256(kfd_ldd),
        "kfd_loader_map_base64": b64(kfd_loader_map),
        "kfd_loader_map_sha256": sha256(kfd_loader_map),
        "kfd_loader_resolution_base64": b64(kfd_loader_resolution),
        "kfd_loader_resolution_sha256": sha256(kfd_loader_resolution),
        "ldd_path_base64": b64(os.fsencode(ldd_tool.path)),
        "ldd_sha256": ldd_tool.digest,
        "ld_audit": "absent",
        "ld_library_path": "absent",
        "ld_preload": "absent",
        "loader_resolution": LOADER_RESOLUTION,
        "modinfo_path_base64": b64(os.fsencode(modinfo_tool.path)),
        "modinfo_sha256": modinfo_tool.digest,
        "observation_edge": args.observation_edge,
        "os_release_base64": b64(os_release),
        "os_release_sha256": sha256(os_release),
        "pci_bdf": product.pci_bdf,
        "pci_class": pci.device_class,
        "pci_device": pci.device,
        "pci_driver": pci.driver,
        "pci_numa_node": pci.numa_node,
        "pci_revision": pci.revision,
        "pci_serial": pci.serial,
        "pci_subsystem_device": pci.subsystem_device,
        "pci_subsystem_vendor": pci.subsystem_vendor,
        "pci_unique_id": pci.unique_id,
        "pci_vendor": pci.vendor,
        "product_model": product.model,
        "product_name_base64": b64(pci.product_name.encode()),
        "product_number": pci.product_number,
        "product_series_base64": b64(product.series.encode()),
        "product_sku": product.sku,
        "readelf_path_base64": b64(os.fsencode(readelf_tool.path)),
        "readelf_sha256": readelf_tool.digest,
        "rocm_path_base64": b64(os.fsencode(rocm_path)),
        "rocm_smi_entrypoint_path_base64": b64(os.fsencode(rocm_smi)),
        "rocm_smi_entrypoint_sha256": sha256(rocm_smi_source),
        "rocm_smi_identity_base64": b64(product_snapshot),
        "rocm_smi_identity_sha256": sha256(product_snapshot),
        "rocm_smi_interpreter_invocation_path_base64": b64(os.fsencode(PYTHON3_PATH)),
        "rocm_smi_interpreter_build_id": python_build_id,
        "rocm_smi_interpreter_ldd_base64": b64(python_ldd),
        "rocm_smi_interpreter_ldd_sha256": sha256(python_ldd),
        "rocm_smi_interpreter_loader_map_base64": b64(python_loader_map),
        "rocm_smi_interpreter_loader_map_sha256": sha256(python_loader_map),
        "rocm_smi_interpreter_loader_resolution_base64": b64(python_loader_resolution),
        "rocm_smi_interpreter_loader_resolution_sha256": sha256(
            python_loader_resolution
        ),
        "rocm_smi_interpreter_path_base64": b64(os.fsencode(python_path)),
        "rocm_smi_interpreter_sha256": sha256(python_bytes),
        "rocm_smi_library_build_id": rocm_smi_library_build_id,
        "rocm_smi_library_ldd_base64": b64(rocm_smi_library_ldd),
        "rocm_smi_library_ldd_sha256": sha256(rocm_smi_library_ldd),
        "rocm_smi_library_loader_map_base64": b64(rocm_smi_library_loader_map),
        "rocm_smi_library_loader_map_sha256": sha256(rocm_smi_library_loader_map),
        "rocm_smi_library_loader_resolution_base64": b64(
            rocm_smi_library_loader_resolution
        ),
        "rocm_smi_library_loader_resolution_sha256": sha256(
            rocm_smi_library_loader_resolution
        ),
        "rocm_smi_library_path_base64": b64(os.fsencode(rocm_smi_library)),
        "rocm_smi_library_sha256": sha256(rocm_smi_library_bytes),
        "rocm_smi_library_soname": ROCM_SMI_SONAME,
        "rocm_smi_invocation_path_base64": b64(os.fsencode(rocm_smi_invocation)),
        "rocm_smi_package_manifest_base64": b64(rocm_smi_package),
        "rocm_smi_package_manifest_sha256": sha256(rocm_smi_package),
        "rocm_smi_shebang_base64": b64(ROCM_SMI_SHEBANG),
        "unique_id": product.unique_id,
        "uuid": f"GPU-{product.unique_id.removeprefix('0x')}",
        "zstd_path_base64": b64(os.fsencode(zstd_tool.path)),
        "zstd_sha256": zstd_tool.digest,
    }


def parse_arguments(arguments: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--gpu-index", type=int, default=0)
    parser.add_argument(
        "--rocm-path", type=pathlib.Path, default=pathlib.Path("/opt/rocm")
    )
    parser.add_argument("--observation-edge", choices=("start", "end"), required=True)
    parser.add_argument("--kfd-binary", type=pathlib.Path, required=True)
    parser.add_argument("--hsa-binary", type=pathlib.Path, required=True)
    parser.add_argument("--hip-binary", type=pathlib.Path, required=True)
    return parser.parse_args(arguments)


def main() -> int:
    arguments = parse_arguments()
    try:
        print(render_context(collect(arguments)))
    except (IdentityError, OSError, UnicodeDecodeError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
