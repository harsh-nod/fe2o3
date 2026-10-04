#!/usr/bin/env python3
"""Packaging source contracts only; no Cargo, service, or authority execution."""

from pathlib import Path
import subprocess
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
COORDINATOR = ROOT / "crates/fe2o3-compiler-execution-coordinator"


def text(path):
    return path.read_text(encoding="utf-8")


def selector(script, boundary, arguments, fields):
    source = text(SCRIPTS / script)
    if source.count(boundary) != 1:
        raise AssertionError("selector boundary is not unique")
    prefix = source.split(boundary)[0]
    report = "printf '%s\\n' " + " ".join(f'"${{{field}}}"' for field in fields)
    return subprocess.run(
        ["/bin/bash", "-c", prefix + report, script, *arguments],
        env={"PATH": "/usr/bin:/bin"}, capture_output=True, text=True, check=False,
    )


def unit_rows(source):
    return [line.removeprefix("OpenFile=") for line in source.splitlines()
            if line.startswith("OpenFile=")]


def expected_rows(family):
    image_suffix = "" if family == "v1" else "-v3"
    issuer_suffix = "" if family == "v1" else "-conditional"
    return [
        "/run/fe2o3:runtime-root:read-only",
        "/var/lib/fe2o3/compiler-execution:supervisor-root:read-only",
        "/var/lib/fe2o3/external-anchor:anchor-root:read-only",
        f"/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor{image_suffix}:supervisor:read-only",
        "/usr/libexec/fe2o3/fe2o3-static-preexec-launcher:launcher:read-only",
        f"/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer{issuer_suffix}:issuer:read-only",
        f"/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper{image_suffix}:anchor-helper:read-only",
        f"/usr/libexec/fe2o3/fe2o3-external-anchor-service{image_suffix}:anchor-daemon:read-only",
        *[f"/etc/fe2o3/compiler-execution/{name}-{family}:{role}:read-only"
          for name, role in [
              ("supervisor-deployment", "supervisor-deployment"),
              ("issuer-policy", "issuer-policy"),
              ("anchor-deployment", "anchor-deployment"),
              ("anchor-provisioning", "anchor-provisioning"),
              ("issuer-signing-key-seed", "issuer-key-seed"),
              ("anchor-signing-key-seed", "anchor-key-seed"),
          ]],
    ]


class DeploymentFamiliesTests(unittest.TestCase):
    def test_bundle_selector_keeps_default_v3_and_binds_complete_v1_family(self):
        fields = ["family", "schema_version", "content_file_count", "entrypoint_suffix",
                  "service_suffix", "image_suffix", "issuer_suffix", "issuer_arguments[*]",
                  "manifest_arguments[*]", "cache_suffix"]
        for arguments, expected in [
            (["/unused"], ["v3", "3", "12", "", "", "-v3", "-conditional",
                           "--conditional", "--v3", ""]),
            (["--v3", "/unused"], ["v3", "3", "12", "", "", "-v3", "-conditional",
                                    "--conditional", "--v3", ""]),
            (["--v1", "/unused"], ["v1", "1", "13", "-v1", "-v1", "", "", "", "", "-v1"]),
        ]:
            with self.subTest(arguments=arguments):
                result = selector("build-static-compiler-execution-deployment.sh",
                                  "readonly jobs=", arguments, fields)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.splitlines(), expected)

    def test_selectors_refuse_unknown_or_mixed_families_before_building(self):
        for script, boundary, invalid in [
            ("build-static-compiler-execution-deployment.sh", "readonly jobs=",
             [[], ["--v2", "/unused"], ["--v1", "--v3", "/unused"], ["--v1"], [""]]),
            *[(f"build-static-compiler-execution-{role}.sh", "repo_root=",
               [["v2"], ["v1", "v3"], ["--v1"], [""]])
              for role in ["coordinator", "provisioner"]],
        ]:
            for arguments in invalid:
                with self.subTest(script=script, arguments=arguments):
                    result = selector(script, boundary, arguments, ["family"])
                    self.assertEqual(result.returncode, 2)
                    self.assertEqual(result.stdout, "")
                    self.assertIn("usage:", result.stderr)

    def test_entrypoint_builders_select_distinct_bins_without_changing_defaults(self):
        bins = {row["name"]: row["path"] for row in
                tomllib.loads(text(COORDINATOR / "Cargo.toml"))["bin"]}
        for role, entrypoint in [
            ("coordinator", "run_inherited_compiler_execution_coordinator"),
            ("provision", "run_compiler_execution_reference_provisioner"),
        ]:
            script_role = "provisioner" if role == "provision" else role
            source = text(SCRIPTS / f"build-static-compiler-execution-{script_role}.sh")
            self.assertIn('--bin "${binary}"', source)
            for arguments, family, suffix in [([], "v3", ""), (["v3"], "v3", ""),
                                               (["v1"], "v1", "-v1")]:
                result = selector(f"build-static-compiler-execution-{script_role}.sh",
                                  "repo_root=", arguments, ["family", "suffix"])
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.splitlines(), [family, suffix])
                main = text(COORDINATOR / bins[f"fe2o3-compiler-execution-{role}{suffix}"])
                self.assertIn(f"::{entrypoint}_{family}()", main)
                other = "v1" if family == "v3" else "v3"
                self.assertNotIn(f"::{entrypoint}_{other}()", main)

    def test_units_bind_exact_descriptor_roles_and_refuse_cross_family_rows(self):
        units = {family: text(ROOT / "deployment/systemd" /
                             f"fe2o3-compiler-execution{suffix}.service")
                 for family, suffix in [("v1", "-v1"), ("v3", "")]}
        for family, source in units.items():
            self.assertEqual(unit_rows(source), expected_rows(family))
            for line in ["Type=notify", "NotifyAccess=main", "User=root", "Group=root",
                         "ExecStart=/usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator",
                         "KillMode=mixed", "TimeoutStartSec=300", "TimeoutStopSec=30"]:
                self.assertEqual(source.splitlines().count(line), 1)
            other = "v3" if family == "v1" else "v1"
            for index, row in enumerate(expected_rows(other)):
                if row != expected_rows(family)[index]:
                    mutant = unit_rows(source)
                    mutant[index] = row
                    with self.assertRaises(AssertionError):
                        self.assertEqual(mutant, expected_rows(family))
        roles = ":".join(row.split(":")[1] for row in expected_rows("v1"))
        self.assertIn(f'"{roles}"', text(COORDINATOR / "src/entrypoint.rs"))
        provisioner = text(COORDINATOR / "src/provisioning_entrypoint.rs")
        for row in expected_rows("v1")[3:8]:
            self.assertIn(f'"{row.split(":")[0]}"', provisioner)
        for row in expected_rows("v1")[8:]:
            self.assertIn(f'"{Path(row.split(":")[0]).name}"', provisioner)

    def test_builder_consumes_family_selection_at_each_packaging_boundary(self):
        source = text(SCRIPTS / "build-static-compiler-execution-deployment.sh")
        for role in ["coordinator", "supervisor", "provisioner"]:
            self.assertIn(f'build-static-compiler-execution-{role}.sh" "${{family}}"', source)
        for role in ["provisioning-helper", "service"]:
            self.assertIn(f'build-static-external-anchor-{role}.sh" "${{family}}"', source)
        for role in ["coordinator", "provision"]:
            self.assertIn(f'/release/fe2o3-compiler-execution-{role}${{entrypoint_suffix}}"', source)
        self.assertIn('build-static-compiler-execution-issuer.sh" "${issuer_arguments[@]}"', source)
        self.assertIn('fe2o3-compiler-execution${service_suffix}.service"', source)
        for tool in ["manifest_generator", "deployment_verifier"]:
            self.assertIn(f'"${{{tool}}}" "${{manifest_arguments[@]}}" "${{partial}}"', source)
        self.assertIn('"${schema_version}" "${commit}" "${source_epoch}" "${target}"', source)
        self.assertIn('"${commit}" "${target}" "${manifest_sha256}" "${content_file_count}"', source)
        self.assertEqual(source.count('if [[ ${family} == v1 ]]; then'), 3)
        client_build = '''if [[ ${family} == v1 ]]; then
  FE2O3_STATIC_CLIENT_CHECK_TARGET_DIR="${target_root}/client-check" \\
    "${repo_root}/scripts/build-static-compiler-execution-client-check.sh"
fi'''
        client_install = '''if [[ ${family} == v1 ]]; then
  install -m 0555 -- \\
    "${target_root}/client-check/${target}/release/fe2o3-compiler-execution-client-check" \\
    "${image_dir}/fe2o3-compiler-execution-client-check"
fi'''
        self.assertEqual(source.count(client_build), 1)
        self.assertEqual(source.count(client_install), 1)
        self.assertNotIn("client-check", source.replace(client_build, "").replace(client_install, ""))


if __name__ == "__main__":
    unittest.main()
