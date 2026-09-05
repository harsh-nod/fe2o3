#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import hashlib
import pathlib
import struct
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from unittest import mock


MODULE_PATH = pathlib.Path(__file__).with_name("r26-system-identity.py")
SPEC = importlib.util.spec_from_file_location("fe2o3_r26_system_identity", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
IDENTITY = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = IDENTITY
SPEC.loader.exec_module(IDENTITY)


ROCM_SMI_IDENTITY = """\
======================= ROCm System Management Interface =======================
GPU[0]          : Unique ID: 0x6ced1647a296545c
GPU[0]          : Serial Number: 692424017146
GPU[0]          : PCI Bus: 0000:05:00.0
GPU[0]          : Card Series: AMD Instinct MI300X
GPU[0]          : Card Model: 0x74a1
GPU[0]          : Card Vendor: Advanced Micro Devices, Inc. [AMD/ATI]
GPU[0]          : Card SKU: M3000100
GPU[0]          : Subsystem ID: 0x74a1
GPU[0]          : Device Rev: 0x00
GPU[0]          : Node ID: 2
GPU[0]          : GUID: 7848959733474673756
GPU[0]          : GFX Version: gfx942
================================================================================
"""


def valid_pci(**overrides: str) -> object:
    fields = {
        "unique_id": "6ced1647a296545c",
        "serial": "692424017146",
        "product_name": "AMD Instinct MI300X OAM",
        "product_number": "102-G30211-00",
        "vendor": "0x1002",
        "device": "0x74a1",
        "subsystem_vendor": "0x1002",
        "subsystem_device": "0x74a1",
        "revision": "0x00",
        "device_class": "0x120000",
        "numa_node": "0",
        "driver": "amdgpu",
    }
    fields.update(overrides)
    return IDENTITY.PciIdentity(**fields)


class R26SystemIdentityTests(unittest.TestCase):
    def test_accepts_exact_mi300x_product_and_sysfs_identity(self) -> None:
        product = IDENTITY.parse_rocm_smi_identity(ROCM_SMI_IDENTITY, 0)
        IDENTITY.validate_product(product, valid_pci())
        self.assertEqual(product.pci_bdf, "0000:05:00.0")
        self.assertEqual(product.gfx_version, "gfx942")

    def test_rejects_duplicate_missing_zero_or_mismatched_gpu_identity(self) -> None:
        duplicate = ROCM_SMI_IDENTITY.replace(
            "GPU[0]          : Serial Number: 692424017146\n",
            "GPU[0]          : Serial Number: 692424017146\n"
            "GPU[0]          : Serial Number: 692424017146\n",
        )
        with self.assertRaisesRegex(IDENTITY.IdentityError, "duplicate"):
            IDENTITY.parse_rocm_smi_identity(duplicate, 0)
        with self.assertRaisesRegex(IDENTITY.IdentityError, "omitted field"):
            IDENTITY.parse_rocm_smi_identity(
                ROCM_SMI_IDENTITY.replace(
                    "GPU[0]          : GFX Version: gfx942\n", ""
                ),
                0,
            )
        with self.assertRaisesRegex(IDENTITY.IdentityError, "zero"):
            IDENTITY.parse_rocm_smi_identity(
                ROCM_SMI_IDENTITY.replace("0x6ced1647a296545c", "0x0000000000000000"),
                0,
            )
        product = IDENTITY.parse_rocm_smi_identity(ROCM_SMI_IDENTITY, 0)
        with self.assertRaisesRegex(IDENTITY.IdentityError, "serial numbers differ"):
            IDENTITY.validate_product(product, valid_pci(serial="692424017147"))
        with self.assertRaisesRegex(
            IDENTITY.IdentityError, "unique ID is not canonical"
        ):
            IDENTITY.validate_product(product, valid_pci(unique_id="6CED1647A296545C"))
        with self.assertRaisesRegex(IDENTITY.IdentityError, "unsupported PCI driver"):
            IDENTITY.validate_product(product, valid_pci(driver="vfio-pci"))

    def test_os_release_parser_requires_ubuntu_2404_and_unique_keys(self) -> None:
        parsed = IDENTITY.parse_os_release(
            b'ID=ubuntu\nVERSION_ID="24.04"\nPRETTY_NAME="Ubuntu 24.04.4 LTS"\n'
        )
        self.assertEqual(parsed["PRETTY_NAME"], "Ubuntu 24.04.4 LTS")
        with self.assertRaisesRegex(IDENTITY.IdentityError, "Ubuntu 24.04"):
            IDENTITY.parse_os_release(b'ID=ubuntu\nVERSION_ID="22.04"\n')
        with self.assertRaisesRegex(IDENTITY.IdentityError, "duplicate"):
            IDENTITY.parse_os_release(b"ID=ubuntu\nID=ubuntu\nVERSION_ID=24.04\n")

    def test_loader_maps_resolve_exact_shared_objects_and_are_canonical(self) -> None:
        retained_resolution = b""
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            hsa_real = root / "libhsa-runtime64.so.1.18.70204"
            hip_real = root / "libamdhip64.so.7.2.70204"
            libc_real = root / "libc.so.6"
            loader_real = root / "ld-linux-x86-64.so.2"
            hsa_real.write_bytes(b"hsa")
            hip_real.write_bytes(b"hip")
            libc_real.write_bytes(b"libc")
            loader_real.write_bytes(b"loader")
            hsa_link = root / IDENTITY.HSA_SONAME
            hip_link = root / IDENTITY.HIP_SONAME
            hsa_link.symlink_to(hsa_real.name)
            hip_link.symlink_to(hip_real.name)
            hsa_text = (
                "linux-vdso.so.1 (0x00007f00)\n"
                f"libc.so.6 => {libc_real} (0x00007f10)\n"
                f"{IDENTITY.HSA_SONAME} => {hsa_link} (0x00007f20)\n"
                f"{loader_real} (0x00007f25)\n"
            )
            hip_text = (
                f"{IDENTITY.HIP_SONAME} => {hip_link} (0x00007f30)\n"
                f"{IDENTITY.HSA_SONAME} => {hsa_link} (0x00007f40)\n"
                f"libc.so.6 => {libc_real} (0x00007f50)\n"
            )
            hsa_map = IDENTITY.parse_ldd(hsa_text)
            hip_map = IDENTITY.parse_ldd(hip_text)
            self.assertEqual(
                IDENTITY.validate_loader_maps(hsa_map, hip_map),
                (hsa_real, hip_real),
            )
            expected_hsa = (
                f"ld-linux-x86-64.so.2={loader_real}\n"
                f"libc.so.6={libc_real}\n{IDENTITY.HSA_SONAME}={hsa_real}\n"
            ).encode()
            expected_resolution = (
                "soname=ld-linux-x86-64.so.2\t"
                f"observed={loader_real}\tresolved={loader_real}\n"
                f"soname=libc.so.6\tobserved={libc_real}\t"
                f"resolved={libc_real}\n"
                f"soname={IDENTITY.HSA_SONAME}\tobserved={hsa_link}\t"
                f"resolved={hsa_real}\n"
            ).encode()
            canonical, retained_resolution = IDENTITY.canonical_loader_evidence(hsa_map)
            self.assertEqual(canonical, expected_hsa)
            different_addresses = IDENTITY.parse_ldd(
                hsa_text.replace("0x00007f10", "0xabcd").replace("0x00007f20", "0xef01")
            )
            self.assertEqual(
                IDENTITY.canonical_loader_map(different_addresses), expected_hsa
            )
        self.assertEqual(retained_resolution, expected_resolution)
        self.assertIn(f"observed={hsa_link}".encode(), retained_resolution)
        self.assertIn(f"resolved={hsa_real}".encode(), retained_resolution)

    def test_loader_parser_rejects_unresolved_duplicates_and_hsa_hip_leakage(
        self,
    ) -> None:
        with self.assertRaisesRegex(IDENTITY.IdentityError, "unresolved"):
            IDENTITY.parse_ldd(f"{IDENTITY.HSA_SONAME} => not found\n")
        with self.assertRaisesRegex(IDENTITY.IdentityError, "duplicate"):
            IDENTITY.parse_ldd(
                f"{IDENTITY.HSA_SONAME} => /one (0x1)\n"
                f"{IDENTITY.HSA_SONAME} => /two (0x2)\n"
            )
        with self.assertRaisesRegex(IDENTITY.IdentityError, "unknown row"):
            IDENTITY.parse_ldd("warning: ambiguous loader evidence\n")
        for row in (
            "directory/libc.so.6 => /lib/libc.so.6 (0x1)\n",
            "libc.so.6 => /lib/lib=c.so.6 (0x1)\n",
            "libc.so.6 => /lib/../usr/libc.so.6 (0x1)\n",
            "libc.so.6 => /lib//libc.so.6 (0x1)\n",
        ):
            with (
                self.subTest(row=row),
                self.assertRaisesRegex(
                    IDENTITY.IdentityError, "noncanonical dependency"
                ),
            ):
                IDENTITY.parse_ldd(row)
        with self.assertRaisesRegex(IDENTITY.IdentityError, "resolves the HIP"):
            IDENTITY.validate_loader_maps(
                {
                    IDENTITY.HSA_SONAME: pathlib.Path("/hsa"),
                    IDENTITY.HIP_SONAME: pathlib.Path("/hip"),
                },
                {},
            )
        IDENTITY.validate_kfd_loader_map({"libc.so.6": pathlib.Path("/libc")})
        with self.assertRaisesRegex(IDENTITY.IdentityError, "forbidden runtime"):
            IDENTITY.validate_kfd_loader_map({"libamdhip64.so": pathlib.Path("/hip")})
        with self.assertRaisesRegex(IDENTITY.IdentityError, "retained evidence bound"):
            IDENTITY.parse_ldd("x" * (IDENTITY.MAX_LDD_BYTES + 1))
        excessive_rows = "\n".join(
            "linux-vdso.so.1 (0x1)" for _ in range(IDENTITY.MAX_LOADER_DEPENDENCIES + 1)
        )
        with self.assertRaisesRegex(IDENTITY.IdentityError, "cardinality"):
            IDENTITY.parse_ldd(excessive_rows)
        with self.assertRaisesRegex(IDENTITY.IdentityError, "cardinality"):
            IDENTITY.canonical_loader_evidence({})

    def test_rocm_smi_package_identity_is_complete_and_stable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            rocm = root / "rocm-7.2.4"
            package = rocm / "libexec" / "rocm_smi"
            binary_directory = rocm / "bin"
            library_directory = rocm / "lib"
            package.mkdir(parents=True)
            binary_directory.mkdir()
            library_directory.mkdir()
            contents = {
                "rocm_smi.py": IDENTITY.ROCM_SMI_SHEBANG + b"print('identity')\n",
                "rsmiBindings.py": b"bindings = 1\n",
                "rsmiBindingsInit.py": b"initialized = True\n",
            }
            for name, data in contents.items():
                path = package / name
                path.write_bytes(data)
                path.chmod(0o755 if name == "rocm_smi.py" else 0o644)
            invocation = binary_directory / "rocm-smi"
            invocation.symlink_to("../libexec/rocm_smi/rocm_smi.py")
            native = library_directory / IDENTITY.ROCM_SMI_SONAME
            native.write_bytes(b"native-rocm-smi-library")

            observed = IDENTITY.rocm_smi_package_identity(invocation, rocm)
            expected_manifest = "".join(
                f"file={name}\tsha256={hashlib.sha256(contents[name]).hexdigest()}\n"
                for name in IDENTITY.ROCM_SMI_PACKAGE_FILES
            ).encode()
            self.assertEqual(
                observed,
                (
                    package / "rocm_smi.py",
                    contents["rocm_smi.py"],
                    expected_manifest,
                    native,
                ),
            )
            self.assertEqual(
                IDENTITY.rocm_smi_package_identity(invocation, rocm), observed
            )

            entrypoint = package / "rocm_smi.py"
            bindings = package / "rsmiBindings.py"
            bindings.write_bytes(b"bindings = 2\n")
            changed = IDENTITY.rocm_smi_package_identity(invocation, rocm)
            self.assertNotEqual(changed[2], observed[2])
            bindings.write_bytes(contents["rsmiBindings.py"])
            entrypoint.write_bytes(b"#!/bin/python3\n")
            with self.assertRaisesRegex(IDENTITY.IdentityError, "interpreter contract"):
                IDENTITY.rocm_smi_package_identity(invocation, rocm)
            entrypoint.write_bytes(contents["rocm_smi.py"])
            outside = root / IDENTITY.ROCM_SMI_SONAME
            outside.write_bytes(b"foreign")
            native.unlink()
            native.symlink_to(outside)
            with self.assertRaisesRegex(
                IDENTITY.IdentityError, "outside the ROCm tree"
            ):
                IDENTITY.rocm_smi_package_identity(invocation, rocm)

    def test_parses_one_little_endian_gnu_build_id_note(self) -> None:
        descriptor = bytes.fromhex("4cd22e1f91450b8d9da1fc7bbbc02ee412e202")
        note = (
            struct.pack("<III", 4, len(descriptor), 3)
            + b"GNU\x00"
            + descriptor
            + b"\x00"
        )
        self.assertEqual(
            IDENTITY.parse_build_id_note(note),
            "4cd22e1f91450b8d9da1fc7bbbc02ee412e202",
        )
        with self.assertRaisesRegex(IDENTITY.IdentityError, "unsupported envelope"):
            IDENTITY.parse_build_id_note(note + b"\x00")

    def test_readelf_identity_accepts_the_ubuntu_2404_soname_dialect(self) -> None:
        build_id = "ab" * 20

        def fake_run(command: list[str], **_kwargs: object) -> bytes:
            if "-dW" in command:
                return (
                    b" 0x000000000000000e (SONAME)             "
                    b"Library soname: [libhsa-runtime64.so.1]\n"
                )
            return f"Build ID: {build_id}\n".encode()

        with mock.patch.object(IDENTITY, "run", side_effect=fake_run):
            self.assertEqual(
                IDENTITY.readelf_identity(
                    pathlib.Path("/usr/bin/readelf"),
                    pathlib.Path("/opt/rocm/lib/libhsa-runtime64.so.1"),
                    IDENTITY.HSA_SONAME,
                    IDENTITY.command_environment(),
                ),
                build_id,
            )

    def test_uses_fixed_tools_and_an_explicit_minimal_environment(self) -> None:
        self.assertEqual(IDENTITY.MODINFO_PATH, pathlib.Path("/usr/sbin/modinfo"))
        self.assertEqual(IDENTITY.LDD_PATH, pathlib.Path("/usr/bin/ldd"))
        self.assertEqual(IDENTITY.READELF_PATH, pathlib.Path("/usr/bin/readelf"))
        self.assertEqual(IDENTITY.ZSTD_PATH, pathlib.Path("/usr/bin/zstd"))
        self.assertEqual(IDENTITY.PYTHON3_PATH, pathlib.Path("/usr/bin/python3"))
        self.assertEqual(
            IDENTITY.LOADER_RESOLUTION,
            "fixed-ldd-transitive-observed-to-canonical-v1",
        )
        with mock.patch.dict("os.environ", {"UNRELATED": "ambient"}, clear=True):
            self.assertEqual(
                IDENTITY.command_environment(),
                {
                    "LANG": "C",
                    "LC_ALL": "C",
                    "PATH": "/usr/sbin:/usr/bin:/sbin:/bin",
                },
            )
        completed = subprocess.CompletedProcess(
            ["/fixed/tool"], 0, stdout=b"identity", stderr=b"warning"
        )
        with (
            mock.patch.object(IDENTITY.subprocess, "run", return_value=completed),
            self.assertRaisesRegex(IDENTITY.IdentityError, "emitted stderr"),
        ):
            IDENTITY.run(["/fixed/tool"], environment=IDENTITY.command_environment())

    def test_requires_start_or_end_observation_edge(self) -> None:
        common = [
            "--kfd-binary",
            "/kfd",
            "--hsa-binary",
            "/hsa",
            "--hip-binary",
            "/hip",
        ]
        self.assertEqual(
            IDENTITY.parse_arguments(
                ["--observation-edge", "start", *common]
            ).observation_edge,
            "start",
        )
        with redirect_stderr(StringIO()), self.assertRaises(SystemExit):
            IDENTITY.parse_arguments(["--observation-edge", "middle", *common])
        with redirect_stderr(StringIO()), self.assertRaises(SystemExit):
            IDENTITY.parse_arguments(common)

    def test_decompresses_with_a_size_limit_and_extracts_module_build_id(self) -> None:
        decompressed = b"ELF module bytes"
        build_id = "ab" * 20

        def fake_run(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess:
            if command[0] == str(IDENTITY.ZSTD_PATH):
                output = kwargs["stdout"]
                output.write(decompressed)
                self.assertIs(kwargs["preexec_fn"], IDENTITY.limit_decompressed_output)
                return subprocess.CompletedProcess(command, 0, stderr=b"")
            self.assertEqual(command[0], str(IDENTITY.READELF_PATH))
            self.assertTrue(kwargs["pass_fds"])
            return subprocess.CompletedProcess(
                command, 0, stdout=f"Build ID: {build_id}\n".encode(), stderr=b""
            )

        with mock.patch.object(IDENTITY.subprocess, "run", side_effect=fake_run):
            observed = IDENTITY.decompressed_module_identity(
                IDENTITY.ZSTD_PATH,
                IDENTITY.READELF_PATH,
                pathlib.Path("/module/amdgpu.ko.zst"),
                IDENTITY.command_environment(),
            )
        self.assertEqual(
            observed,
            (build_id, hashlib.sha256(decompressed).hexdigest(), len(decompressed)),
        )
        IDENTITY.validate_module_build_ids(build_id, build_id)
        with self.assertRaisesRegex(IDENTITY.IdentityError, "build IDs differ"):
            IDENTITY.validate_module_build_ids(build_id, "cd" * 20)

    def test_context_renderer_sorts_and_rejects_ambiguous_tokens(self) -> None:
        self.assertEqual(
            IDENTITY.render_context({"z": "last", "a": "first"}),
            "context schema=fe2o3.r26-system-identity.v1 a=first z=last",
        )
        for fields in (
            {"schema": "duplicate"},
            {"Bad": "key"},
            {"good": "two words"},
            {"good": ""},
        ):
            with self.subTest(fields=fields):
                with self.assertRaises(IDENTITY.IdentityError):
                    IDENTITY.render_context(fields)

    def test_cli_main_emits_exactly_one_lf_terminated_record(self) -> None:
        arguments = object()
        stdout = StringIO()
        stderr = StringIO()
        with (
            mock.patch.object(
                IDENTITY, "parse_arguments", return_value=arguments
            ) as parse_arguments,
            mock.patch.object(
                IDENTITY,
                "collect",
                return_value={"observation_edge": "start"},
            ) as collect,
            redirect_stdout(stdout),
            redirect_stderr(stderr),
        ):
            self.assertEqual(IDENTITY.main(), 0)
        self.assertEqual(
            stdout.getvalue(),
            "context schema=fe2o3.r26-system-identity.v1 observation_edge=start\n",
        )
        self.assertEqual(stdout.getvalue().count("\n"), 1)
        self.assertEqual(stderr.getvalue(), "")
        parse_arguments.assert_called_once_with()
        collect.assert_called_once_with(arguments)


if __name__ == "__main__":
    unittest.main()
