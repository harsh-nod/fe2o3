#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from unittest import mock


sys.dont_write_bytecode = True
CHECKER_PATH = Path(__file__).resolve().parents[1] / "runtime_pure_rust_audit.py"
SPEC = importlib.util.spec_from_file_location("runtime_pure_rust_audit", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)
POLICY = CHECKER.load_policy(
    Path(__file__).resolve().parents[1] / "runtime-pure-rust-policy.json"
)


def target(kind: str = "lib") -> dict:
    return {"kind": [kind]}


def package_id(name: str, version: str = "0.1.0") -> str:
    return f"path+file:///workspace/{name}#{version}"


def package(
    name: str,
    *,
    links: str | None = None,
    source: str | None = None,
    targets: list[dict] | None = None,
    version: str = "0.1.0",
) -> dict:
    return {
        "id": package_id(name, version),
        "name": name,
        "links": links,
        "source": source,
        "targets": targets if targets is not None else [target()],
        "version": version,
    }


def dependency(
    name: str,
    kind: str | None = None,
    version: str = "0.1.0",
) -> dict:
    return {
        "pkg": package_id(name, version),
        "dep_kinds": [{"kind": kind, "target": None}],
    }


def node(
    name: str,
    dependencies: list[dict] | None = None,
    version: str = "0.1.0",
) -> dict:
    return {
        "id": package_id(name, version),
        "deps": dependencies or [],
    }


def metadata(packages: list[dict], nodes: list[dict]) -> dict:
    return {"version": 1, "packages": packages, "resolve": {"nodes": nodes}}


def synthetic_elf(
    dependency: str = "libc.so.6",
    symbol: str = "memcpy",
    hidden_literal: bytes = b"",
) -> bytes:
    base_address = 0x400000
    program_offset = 64
    string_offset = 0x100
    symbol_offset = 0x180
    dynamic_offset = 0x1C0
    section_offset = 0x200
    section_count = 4
    file_size = section_offset + section_count * 64

    dependency_bytes = dependency.encode("utf-8")
    symbol_bytes = symbol.encode("utf-8")
    strings = b"\0" + dependency_bytes + b"\0" + symbol_bytes + b"\0"
    dependency_name = 1
    symbol_name = 1 + len(dependency_bytes) + 1

    data = bytearray(file_size)
    data[:16] = b"\x7fELF\x02\x01\x01" + b"\0" * 9
    struct.pack_into(
        "<HHIQQQIHHHHHH",
        data,
        16,
        2,
        62,
        1,
        0,
        program_offset,
        section_offset,
        0,
        64,
        56,
        2,
        64,
        section_count,
        0,
    )
    struct.pack_into(
        "<IIQQQQQQ",
        data,
        program_offset,
        1,
        5,
        0,
        base_address,
        base_address,
        file_size,
        file_size,
        0x1000,
    )
    struct.pack_into(
        "<IIQQQQQQ",
        data,
        program_offset + 56,
        2,
        4,
        dynamic_offset,
        base_address + dynamic_offset,
        base_address + dynamic_offset,
        64,
        64,
        8,
    )
    data[string_offset : string_offset + len(strings)] = strings
    struct.pack_into("<IBBHQQ", data, symbol_offset + 24, symbol_name, 0x12, 0, 0, 0, 0)
    dynamic_entries = (
        (CHECKER.DT_STRTAB, base_address + string_offset),
        (CHECKER.DT_STRSZ, len(strings)),
        (CHECKER.DT_NEEDED, dependency_name),
        (CHECKER.DT_NULL, 0),
    )
    for index, entry in enumerate(dynamic_entries):
        struct.pack_into("<qQ", data, dynamic_offset + index * 16, *entry)
    struct.pack_into(
        "<IIQQQQIIQQ",
        data,
        section_offset + 64,
        0,
        CHECKER.SHT_STRTAB,
        0,
        base_address + string_offset,
        string_offset,
        len(strings),
        0,
        0,
        1,
        0,
    )
    struct.pack_into(
        "<IIQQQQIIQQ",
        data,
        section_offset + 128,
        0,
        CHECKER.SHT_DYNSYM,
        0,
        base_address + symbol_offset,
        symbol_offset,
        48,
        1,
        1,
        8,
        24,
    )
    struct.pack_into(
        "<IIQQQQIIQQ",
        data,
        section_offset + 192,
        0,
        6,
        0,
        base_address + dynamic_offset,
        dynamic_offset,
        64,
        1,
        0,
        8,
        16,
    )
    return bytes(data) + hidden_literal


class MetadataAuditTests(unittest.TestCase):
    def test_accepts_closed_pure_rust_production_closure(self) -> None:
        value = metadata(
            [package("runtime"), package("model")],
            [node("runtime", [dependency("model")]), node("model")],
        )
        violations, stats = CHECKER.audit_metadata(value, ("runtime",), POLICY)
        self.assertEqual([], violations)
        self.assertEqual(
            {"allowed_build_scripts": (), "packages": 2, "roots": 1}, stats
        )

    def test_dev_only_oracle_is_outside_production_closure(self) -> None:
        value = metadata(
            [package("runtime"), package("fe2o3-hsa-runtime")],
            [
                node("runtime", [dependency("fe2o3-hsa-runtime", "dev")]),
                node("fe2o3-hsa-runtime"),
            ],
        )
        violations, stats = CHECKER.audit_metadata(value, ("runtime",), POLICY)
        self.assertEqual([], violations)
        self.assertEqual(1, stats["packages"])

    def test_rejects_prohibited_runtime_package(self) -> None:
        value = metadata(
            [package("runtime"), package("fe2o3-hip-sys")],
            [
                node("runtime", [dependency("fe2o3-hip-sys")]),
                node("fe2o3-hip-sys"),
            ],
        )
        violations, _ = CHECKER.audit_metadata(value, ("runtime",), POLICY)
        self.assertTrue(any("prohibited package" in item for item in violations))

    def test_rejects_links_and_build_scripts(self) -> None:
        value = metadata(
            [
                package("runtime"),
                package("native", links="private", targets=[target(), target("custom-build")]),
            ],
            [node("runtime", [dependency("native", "build")]), node("native")],
        )
        violations, _ = CHECKER.audit_metadata(value, ("runtime",), POLICY)
        self.assertEqual(2, len(violations))
        self.assertTrue(any("Cargo links" in item for item in violations))
        self.assertTrue(any("unapproved Cargo build script" in item for item in violations))

    def test_rejects_unapproved_build_script_without_links(self) -> None:
        value = metadata(
            [
                package("runtime"),
                package("unreviewed", targets=[target(), target("custom-build")]),
            ],
            [
                node("runtime", [dependency("unreviewed")]),
                node("unreviewed"),
            ],
        )
        violations, stats = CHECKER.audit_metadata(value, ("runtime",), POLICY)
        self.assertEqual(
            [
                "unapproved Cargo build script in production closure: "
                "unreviewed@0.1.0"
            ],
            violations,
        )
        self.assertEqual((), stats["allowed_build_scripts"])

    def test_allows_and_reports_reviewed_rustix_and_libc_build_scripts(self) -> None:
        registry = CHECKER.CRATES_IO_SOURCE
        value = metadata(
            [
                package("runtime"),
                package(
                    "rustix",
                    source=registry,
                    targets=[target(), target("custom-build")],
                    version="1.1.4",
                ),
                package(
                    "libc",
                    source=registry,
                    targets=[target(), target("custom-build")],
                    version="0.2.189",
                ),
            ],
            [
                node(
                    "runtime",
                    [dependency("rustix", version="1.1.4")],
                ),
                node(
                    "rustix",
                    [dependency("libc", version="0.2.189")],
                    version="1.1.4",
                ),
                node("libc", version="0.2.189"),
            ],
        )
        violations, stats = CHECKER.audit_metadata(value, ("runtime",), POLICY)
        self.assertEqual([], violations)
        self.assertEqual(
            ("libc@0.2.189", "rustix@1.1.4"),
            stats["allowed_build_scripts"],
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "metadata.json"
            path.write_text(json.dumps(value), encoding="utf-8")
            output = io.StringIO()
            with redirect_stdout(output):
                status = CHECKER.main(
                    ["metadata", "--input", str(path), "--root", "runtime"]
                )
        self.assertEqual(0, status)
        self.assertIn(
            "allowed_build_scripts=libc@0.2.189,rustix@1.1.4",
            output.getvalue(),
        )

    def test_rejects_allowlisted_build_script_from_another_source(self) -> None:
        value = metadata(
            [
                package("runtime"),
                package(
                    "rustix",
                    targets=[target(), target("custom-build")],
                    version="1.1.4",
                ),
            ],
            [
                node("runtime", [dependency("rustix", version="1.1.4")]),
                node("rustix", version="1.1.4"),
            ],
        )
        violations, _ = CHECKER.audit_metadata(value, ("runtime",), POLICY)
        self.assertEqual(1, len(violations))
        self.assertIn("unapproved source", violations[0])

    def test_missing_resolve_graph_fails_closed(self) -> None:
        with self.assertRaisesRegex(CHECKER.AuditInputError, "resolve"):
            CHECKER.audit_metadata(
                {"version": 1, "packages": [package("runtime")]},
                ("runtime",),
                POLICY,
            )

    def test_cli_reports_deterministic_metadata_summary(self) -> None:
        value = metadata([package("runtime")], [node("runtime")])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "metadata.json"
            path.write_text(json.dumps(value), encoding="utf-8")
            output = io.StringIO()
            with redirect_stdout(output):
                status = CHECKER.main(
                    [
                        "metadata",
                        "--input",
                        str(path),
                        "--root",
                        "runtime",
                    ]
                )
        self.assertEqual(0, status)
        self.assertIn(
            "metadata roots=1 packages=1 allowed_build_scripts=none sha256=",
            output.getvalue(),
        )

    def test_cli_cargo_mode_generates_locked_target_metadata(self) -> None:
        value = metadata([package("runtime")], [node("runtime")])
        completed = mock.Mock(
            returncode=0,
            stdout=json.dumps(value).encode("utf-8"),
            stderr=b"",
        )
        output = io.StringIO()
        with mock.patch.object(
            CHECKER.subprocess, "run", return_value=completed
        ) as run:
            with redirect_stdout(output):
                status = CHECKER.main(
                    ["metadata", "--cargo", "--root", "runtime"]
                )
        self.assertEqual(0, status)
        run.assert_called_once_with(
            [
                "cargo",
                "metadata",
                "--locked",
                "--filter-platform",
                "x86_64-unknown-linux-gnu",
                "--format-version",
                "1",
            ],
            cwd=CHECKER.REPO_ROOT,
            stdout=CHECKER.subprocess.PIPE,
            stderr=CHECKER.subprocess.PIPE,
            check=False,
        )
        self.assertIn("metadata roots=1 packages=1", output.getvalue())


class ElfAuditTests(unittest.TestCase):
    def audit(self, contents: bytes) -> tuple[list[str], dict]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "runtime"
            path.write_bytes(contents)
            return CHECKER.audit_elf(path, POLICY)

    def test_accepts_allowed_system_dependency_and_symbol(self) -> None:
        violations, stats = self.audit(synthetic_elf())
        self.assertEqual([], violations)
        self.assertEqual(1, stats["dynamic_dependencies"])
        self.assertEqual(1, stats["dynamic_symbols"])

    def test_rejects_hip_dependency(self) -> None:
        violations, _ = self.audit(synthetic_elf(dependency="libamdhip64.so.6"))
        self.assertTrue(any("prohibited dynamic dependency" in item for item in violations))

    def test_rejects_unapproved_native_dependency(self) -> None:
        violations, _ = self.audit(synthetic_elf(dependency="libprivate_shim.so"))
        self.assertEqual(
            ["unapproved dynamic dependency: libprivate_shim.so"], violations
        )

    def test_rejects_libdl_dependency(self) -> None:
        violations, _ = self.audit(synthetic_elf(dependency="libdl.so.2"))
        self.assertTrue(
            any("prohibited dynamic dependency" in item for item in violations)
        )

    def test_rejects_hsa_dynamic_symbol(self) -> None:
        violations, _ = self.audit(synthetic_elf(symbol="hsa_init"))
        self.assertTrue(any("prohibited dynamic symbol" in item for item in violations))

    def test_rejects_loader_and_process_spawn_dynamic_symbols(self) -> None:
        for symbol in POLICY["forbidden_dynamic_symbols"]:
            with self.subTest(symbol=symbol):
                violations, _ = self.audit(synthetic_elf(symbol=symbol))
                self.assertIn(
                    f"prohibited dynamic symbol: {symbol} (exact)", violations
                )

    def test_rejects_loader_control_dynamic_tags(self) -> None:
        for tag_name in POLICY["forbidden_dynamic_tags"]:
            with self.subTest(tag=tag_name):
                contents = bytearray(synthetic_elf())
                struct.pack_into(
                    "<qQ",
                    contents,
                    0x1C0 + 2 * 16,
                    CHECKER.DYNAMIC_TAG_VALUES[tag_name],
                    1,
                )
                violations, _ = self.audit(bytes(contents))
                self.assertIn(
                    f"prohibited dynamic tag: {tag_name}", violations
                )

    def test_rejects_hidden_comgr_loader_literal(self) -> None:
        violations, _ = self.audit(
            synthetic_elf(hidden_literal=b"\0libamd_comgr.so.3\0")
        )
        self.assertTrue(any("dynamic-loader literal" in item for item in violations))

    def test_malformed_elf_fails_closed(self) -> None:
        with self.assertRaisesRegex(CHECKER.AuditInputError, "not an ELF"):
            self.audit(b"not-elf")

    def test_dynamic_elf_without_auditable_symbols_fails_closed(self) -> None:
        contents = bytearray(synthetic_elf())
        struct.pack_into("<I", contents, 0x200 + 128 + 4, 1)
        with self.assertRaisesRegex(CHECKER.AuditInputError, "SHT_DYNSYM"):
            self.audit(bytes(contents))

    def test_non_host_elf_fails_closed(self) -> None:
        contents = bytearray(synthetic_elf())
        struct.pack_into("<H", contents, 18, 224)
        with self.assertRaisesRegex(CHECKER.AuditInputError, "x86-64"):
            self.audit(bytes(contents))

    def test_cli_reports_deterministic_elf_summary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "runtime"
            path.write_bytes(synthetic_elf())
            output = io.StringIO()
            with redirect_stdout(output):
                status = CHECKER.main(["elf", "--input", str(path)])
        self.assertEqual(0, status)
        self.assertIn("ELF bytes=768 needed=1 dynsym=1 sha256=", output.getvalue())
        self.assertIn("profile=fe2o3.runtime.pure-rust.gfx942.v1", output.getvalue())


if __name__ == "__main__":
    unittest.main()
