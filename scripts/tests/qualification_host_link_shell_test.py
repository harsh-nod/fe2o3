#!/usr/bin/python3
"""Opt-in harness controls; no production admission or device execution."""
import hashlib
from pathlib import Path
import subprocess
import tempfile
import unittest

HELPER = Path(__file__).resolve().parents[1] / "qualify-host-link.sh"


class ShellTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-host-link-shell-")
        self.root = Path(self.temporary.name)
        self.proxy = self.root / "proxy"
        self.proxy.write_bytes(b"not executed by configuration tests")
        self.pin = hashlib.sha256(self.proxy.read_bytes()).hexdigest()

    def tearDown(self):
        self.temporary.cleanup()

    def run_shell(self, script, **values):
        return subprocess.run(["/bin/bash", "-c", 'set -euo pipefail; source "$1"; ' + script,
                               "controls", str(HELPER)], env={"PATH": "/usr/bin:/bin", **values},
                              capture_output=True, text=True, timeout=10)

    def enabled(self, **extra):
        return {"campaign": "genuine", "FE2O3_GENUINE_HOST_LINK_PROXY": str(self.proxy),
                "FE2O3_GENUINE_HOST_LINK_PROXY_SHA256": self.pin, **extra}

    def test_disabled_preserves_empty_mounts(self):
        result = self.run_shell('prepare_host_link_input; [[ ${#FE2O3_HOST_LINK_INPUT_MOUNTS[@]} == 0 ]]', campaign="resources")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_valid_configuration_mounts_fixed_input(self):
        result = self.run_shell('prepare_host_link_input; printf "%s\\n" "${FE2O3_HOST_LINK_INPUT_MOUNTS[@]}"', **self.enabled())
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["--ro-bind", str(self.proxy),
                         "/run/qualification-host-link-input", "--setenv",
                         "FE2O3_GENUINE_HOST_LINK_PROXY", "/run/qualification-host-link-input"])

    def test_missing_empty_invalid_or_unpaired_options_reject(self):
        for values in ({"FE2O3_GENUINE_HOST_LINK_PROXY": str(self.proxy)},
                       {"FE2O3_GENUINE_HOST_LINK_PROXY_SHA256": self.pin},
                       {"FE2O3_GENUINE_HOST_LINK_SHA256": self.pin},
                       {"FE2O3_GENUINE_HOST_LINK_PROXY": ""},
                       self.enabled(FE2O3_GENUINE_HOST_LINK_SHA256="bad")):
            with self.subTest(values=values):
                self.assertNotEqual(self.run_shell("prepare_host_link_input", **{"campaign": "genuine", **values}).returncode, 0)

    def test_wrong_proxy_pin_and_resource_campaign_reject(self):
        for extra in ({"FE2O3_GENUINE_HOST_LINK_PROXY_SHA256": "0" * 64}, {"campaign": "resources"}):
            self.assertNotEqual(self.run_shell("prepare_host_link_input", **self.enabled(**extra)).returncode, 0)

    def test_symlink_proxy_rejects(self):
        alias = self.root / "alias"
        alias.symlink_to(self.proxy)
        self.assertNotEqual(self.run_shell("prepare_host_link_input", **self.enabled(FE2O3_GENUINE_HOST_LINK_PROXY=str(alias))).returncode, 0)

    def test_original_campaign_and_capabilities_are_retained(self):
        for campaign, name in (("genuine", "root_genuine_application_campaign"),
                               ("genuine-two-gpu", "root_genuine_two_gpu_application_campaign")):
            result = self.run_shell('set_genuine_campaign_command; printf "%s\\n" "${FE2O3_GENUINE_COMMAND[@]}"', campaign=campaign)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.splitlines(), ["/usr/bin/setpriv",
                "--bounding-set=-all,+chown,+dac_override,+kill,+setgid,+setpcap,+setuid,+sys_ptrace",
                "--inh-caps=-all", "--ambient-caps=-all", "/usr/libexec/fe2o3/resource-qualification",
                "--exact", "provisioning::tests::genuine_application::" + name,
                "--ignored", "--nocapture", "--test-threads=1"])

    def test_campaign_failure_preserves_exit_and_skips_audit(self):
        result = self.run_shell("FE2O3_GENUINE_COMMAND=(/bin/bash -c 'exit 17'); run_host_link_postflight", repo="/absent")
        self.assertEqual(result.returncode, 17)
        self.assertEqual(result.stderr, "")

    def test_postflight_runs_and_failure_is_not_hidden(self):
        scripts = self.root / "scripts"
        scripts.mkdir()
        (scripts / "qualification_host_link.py").write_text(
            "import sys; assert sys.argv[1:] == ['audit']; print('postflight'); sys.exit(23)\n")
        result = self.run_shell("FE2O3_GENUINE_COMMAND=(/bin/true); run_host_link_postflight", repo=str(self.root))
        self.assertEqual(result.returncode, 23)
        self.assertEqual(result.stdout, "postflight\n")


if __name__ == "__main__":
    unittest.main()
