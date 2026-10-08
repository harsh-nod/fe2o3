"""Pure CPU result/orchestration controls; no real proof or service qualification."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "native_cpu", ROOT / "scripts/qualify_native_conditional_cpu_chain.py")
cpu = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cpu)


def valid():
    value = {"schema": cpu.SCHEMA, **cpu.PROFILE}
    value.update({name: {"sha256": "12" * 32, "bytes": 17} for name in cpu.application.BLOBS})
    value.update({name: "34" * 32 for name in cpu.application.IDENTITIES})
    return value


class ProofOnly(unittest.TestCase):
    def test_private_cgroup_plan_has_same_order_without_systemd_observation(self):
        plan = cpu.startup_plan()
        self.assertEqual(plan["mode"], "joined-native-manager-v3")
        self.assertEqual(plan["compilerRequests"], 1)
        self.assertFalse(plan["applicationListenerRequiredBeforeCompiler"])
        self.assertFalse(plan["standaloneCoordinatorAllowed"])
        self.assertFalse(plan["grantsAdmissionAuthority"])
        self.assertNotIn("systemd", str(plan))

    def test_exact_profile_has_no_runtime_or_root_cleanup_claim(self):
        self.assertEqual(cpu.validate_result(valid()), valid())
        for field in ("shutdown", "result_credits", "settled_native_launches", "devices",
                      "manager_cleanup", "cgroup_empty", "production_qualified"):
            changed = valid()
            changed[field] = True
            with self.assertRaises(ValueError):
                cpu.validate_result(changed)

    def test_fields_types_and_content_axes_are_closed(self):
        for field in cpu.FIELDS:
            changed = valid()
            del changed[field]
            with self.assertRaises(ValueError):
                cpu.validate_result(changed)
        for field, expected in cpu.PROFILE.items():
            for replacement in [None, [], {}, "", 1 if expected is False else False]:
                changed = valid()
                changed[field] = replacement
                if type(replacement) is type(expected) and replacement == expected:
                    continue
                with self.assertRaises(ValueError):
                    cpu.validate_result(changed)
        for field in cpu.application.BLOBS:
            for replacement in [0, True, -1, 2**64, 1.0]:
                changed = valid()
                changed[field]["bytes"] = replacement
                with self.assertRaises(ValueError):
                    cpu.validate_result(changed)
            changed = valid()
            changed[field]["sha256"] = "0" * 64
            with self.assertRaises(ValueError):
                cpu.validate_result(changed)
        for field in cpu.application.IDENTITIES:
            changed = valid()
            changed[field] = "0" * 64
            with self.assertRaises(ValueError):
                cpu.validate_result(changed)

    def test_log_refuses_duplicate_malformed_legacy_gpu_and_roster_records(self):
        raw = json.dumps(valid()).encode()
        self.assertEqual(cpu.result_from_log(b"compiler output\n" + raw), valid())
        for bad in [b"", b"\xff", raw + b"\n" + raw,
                    raw.replace(b'"schema":', b'"schema":"duplicate","schema":'),
                    b'{"schema":"fe2o3.native-conditional-proof-only.v1",broken}',
                    b"x" * (cpu.MAX_LOG + 1)]:
            with self.assertRaises((ValueError, UnicodeError)):
                cpu.result_from_log(bad)
        for schema in [cpu.application.SCHEMA, cpu.application.ROSTER_SCHEMA,
                       "fe2o3.genuine-application.v1", "fe2o3.native-conditional-proof-only.v2"]:
            changed = valid()
            changed["schema"] = schema
            with self.assertRaises(ValueError):
                cpu.result_from_log(json.dumps(changed).encode())
        with self.assertRaises(ValueError):
            cpu.application.result_from_log(raw, "0x0000000000000001")

    def test_capture_needs_exact_reaped_success_and_unchanged_log(self):
        raw = json.dumps(valid()).encode()
        result = {"status": "cargo-completed-unqualified", "exitCode": 0,
                  "logComplete": True, "directChildReaped": True, "logBytes": len(raw),
                  "logSha256": hashlib.sha256(raw).hexdigest()}
        self.assertEqual(cpu.reconcile_application(result, raw), valid())
        for field, bad in [("status", "passed"), ("exitCode", False), ("exitCode", 1),
                           ("logComplete", False), ("directChildReaped", False),
                           ("logBytes", float(len(raw))), ("logSha256", "0" * 64)]:
            changed = copy.deepcopy(result)
            changed[field] = bad
            with self.assertRaises(ValueError):
                cpu.reconcile_application(changed, raw)

    def test_native_command_has_exact_source_and_no_device_or_fallback(self):
        command = cpu.command("/pinned/cargo-fe2o3", "/source", "/target", "./src/lib.rs")
        self.assertEqual(command[:6], ["/pinned/cargo-fe2o3", "authority", "release", "--native", "run",
                                       "--native-application-proof-custodian"])
        self.assertEqual(command.count("--native"), 1)
        self.assertEqual(command[-5:], ["native-conditional-proof-only", "--",
                                        "--native-v5-proof-only", "--producer-source", "./src/lib.rs"])
        for forbidden in ["--device", "--devices", "--transport", "--application-proof-custodian"]:
            self.assertNotIn(forbidden, command)
        for source in ["", "a\0b", "a\nb", "a\rb", "a" * 4097, "\u00e9" * 2049]:
            with self.assertRaises(ValueError):
                cpu.command("/cargo", "/source", "/target", source)


if __name__ == "__main__":
    unittest.main()
