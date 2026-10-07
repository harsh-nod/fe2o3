"""Inert startup observation controls, never root admission or live service tests."""
import hashlib
import importlib.util
from pathlib import Path
import socket
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("startup", ROOT / "scripts/native_application_startup.py")
startup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(startup)


def records():
    result = {name: {"Id": name, "LoadState": "loaded", "ActiveState": "inactive",
                     "SubState": "dead", "MainPID": "0", "InvocationID": "", "Job": ""}
              for name in startup.UNITS}
    result[startup.MANAGER].update(ActiveState="active", SubState="running", MainPID="123",
                                   InvocationID="ab" * 16)
    return result


def encode(value):
    return ("\n\n".join("\n".join(f"{key}={item}" for key, item in record.items())
                         for record in value.values()) + "\n").encode("ascii")


def outcome(raw):
    return {"status": "cargo-completed-unqualified", "exitCode": 0, "logComplete": True,
            "directChildReaped": True, "logBytes": len(raw), "logSha256": hashlib.sha256(raw).hexdigest()}


class StartupObservation(unittest.TestCase):
    def test_plan_separates_unsuccessful_compiler_and_keeps_private_cgroup_option(self):
        plan = startup.phase_plan()
        self.assertEqual(plan["compilerRequests"], 1)
        self.assertFalse(plan["standaloneCoordinatorAllowed"])
        self.assertFalse(plan["applicationListenerRequiredBeforeCompiler"])
        self.assertFalse(plan["grantsAdmissionAuthority"])
        self.assertEqual(plan["unsuccessfulCompilerCompletionGate"],
                         "original-retirement-without-application-startup")
        self.assertNotIn("systemd", str(plan))

    def test_exact_manager_only_startup(self):
        value = records()
        self.assertEqual(startup.parse_units(encode(value)), value)
        value[startup.STANDALONE[0]]["LoadState"] = "not-found"
        self.assertEqual(startup.parse_units(encode(value)), value)

    def test_conflicting_or_queued_standalone_startup_refuses(self):
        for unit in startup.STANDALONE:
            for updates in ({"ActiveState": "active", "SubState": "running", "MainPID": "124"},
                            {"ActiveState": "activating", "SubState": "start"},
                            {"MainPID": "123"}, {"Job": "42"}, {"LoadState": "masked"}):
                value = records()
                value[unit].update(updates)
                with self.subTest(unit=unit, updates=updates), self.assertRaises(ValueError):
                    startup.parse_units(encode(value))

    def test_inactive_substituted_or_noncanonical_manager_refuses(self):
        for key, wrong in (("ActiveState", "inactive"), ("SubState", "start"), ("MainPID", "0"),
                           ("MainPID", "01"), ("MainPID", "2147483648"), ("Job", "queued"),
                           ("InvocationID", "0" * 32), ("InvocationID", "AB" * 16),
                           ("LoadState", "not-found"), ("Id", startup.STANDALONE[0])):
            value = records()
            value[startup.MANAGER][key] = wrong
            with self.subTest(key=key, wrong=wrong), self.assertRaises(ValueError):
                startup.parse_units(encode(value))

    def test_missing_extra_duplicate_and_unbounded_observations_refuse(self):
        raw = encode(records())
        cases = [b"", b"x" * (startup.MAX_LOG + 1), raw + b"\xff",
                 raw.replace(b"Job=", b"Other=", 1),
                 raw.replace(b"Job=", b"Job=\nJob=", 1),
                 raw.split(b"\n\n")[0], raw + b"\n" + raw]
        for candidate in cases:
            with self.subTest(candidate=candidate[:50]), self.assertRaises(ValueError):
                startup.parse_units(candidate)

    def test_socket_preflight_never_connects_and_rejects_early_app_or_alias(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-startup-") as directory:
            root = Path(directory)
            compiler, application = root / "compiler", root / "application"
            with socket.socket(socket.AF_UNIX, socket.SOCK_SEQPACKET) as listener:
                listener.bind(str(compiler))
                result = startup.socket_phase(compiler, application)
                self.assertFalse(result["socketCustodyAuthenticated"])
                application.symlink_to(root / "missing")
                with self.assertRaises(ValueError):
                    startup.socket_phase(compiler, application)
                application.unlink()
                alias = root / "alias"
                alias.symlink_to(compiler)
                with self.assertRaises(ValueError):
                    startup.socket_phase(alias, application)
                application.write_bytes(b"not a listener")
                with self.assertRaises(ValueError):
                    startup.socket_phase(compiler, application)

    def test_observer_is_bounded_no_mutation_and_validates_capture(self):
        raw = encode(records())
        with tempfile.TemporaryDirectory(prefix="fe2o3-startup-capture-") as directory:
            output = Path(directory)
            calls = []
            def run(arguments, cwd, environment, log, timeout, maximum):
                calls.append((arguments, cwd, environment, timeout, maximum))
                log.write_bytes(raw)
                return outcome(raw)
            with mock.patch.object(startup, "socket_phase", return_value={"fixture": True}):
                result = startup.observe_installed_systemd(SimpleNamespace(run_command=run), output)
            self.assertEqual(result["scope"], "systemd-observation-only-not-admission")
            self.assertEqual(len(calls), 1)
            arguments, cwd, environment, timeout, maximum = calls[0]
            self.assertEqual(arguments[:4], ["/usr/bin/systemctl", "--no-pager", "--all", "show"])
            self.assertEqual(arguments[-3:], list(startup.UNITS))
            self.assertEqual((cwd, timeout, maximum), (output, 5, 16384))
            self.assertEqual(environment, {"PATH": "/usr/bin:/bin", "LC_ALL": "C"})
            for key, wrong in (("status", "timeout"), ("exitCode", 1), ("exitCode", False),
                               ("logComplete", False), ("directChildReaped", False),
                               ("logBytes", len(raw) + 1), ("logSha256", "0" * 64)):
                def fail(*args):
                    return {**outcome(raw), key: wrong}
                with self.subTest(key=key), self.assertRaises(ValueError):
                    startup.observe_installed_systemd(SimpleNamespace(run_command=fail), output)


if __name__ == "__main__":
    unittest.main()
