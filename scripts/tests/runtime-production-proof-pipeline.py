#!/usr/bin/env python3
"""Exercise the real production dispatch script without executing a proof."""

from __future__ import annotations

from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/qualify-runtime-production-proofs.sh"
COMMANDS = (
    ("check-dispatch-template-prepare.py", "template-prepare", ()),
    ("check-dispatch-template-preflight.py", "template-preflight", ()),
    ("check-dispatch-template-bind.py", "template-bind", ()),
    ("check-dispatch-template-bind.py", "template-roster", ("--roster",)),
    ("check-producer-input-preflight.py", "producer-preflight", ()),
    ("check-retained-pair-routing.py", "retained-routing", ()),
    ("check-retained-credit-dispatch.py", "retained-credit", ("--campaign",)),
    ("qualify-graph-version-ledger-v1.py", "graph-version-ledger", ()),
    ("qualify-graph-reservation-retirement-v1.py", "graph-reservation-retirement", ()),
)
CACHED_COMMANDS = (
    ("qualify-context-cached-poll-v1.py", "context-cached-poll", ()),
)
RECORDER = r'''#!/bin/bash
set -Eeuo pipefail
printf '%s\0' "$PWD" "$@" >> "$LOG"
printf '\n' >> "$LOG"
count=$(wc -l < "$LOG")
[[ "$count" != "$FAIL_AT" ]] || exit 37
'''


class ProductionProofPipelineTests(unittest.TestCase):
    def test_reviewed_host_routes_only_the_commit_without_relaxing_event_gates(self):
        workflow = (ROOT / ".github/workflows/runtime-model-verus.yml").read_text()
        job_marker = "  producer-live-composition:\n"
        self.assertEqual(workflow.count(job_marker), 1)
        header = workflow.partition(job_marker)[2].partition("    steps:\n")[0]
        self.assertIn(
            "    runs-on:\n"
            "      - self-hosted\n"
            "      - linux\n"
            "      - x64\n"
            "      - fe2o3-verus-reviewed-host-v1\n"
            '      - "fe2o3-verus-commit-${{ github.sha }}"\n',
            header,
        )
        self.assertEqual(header.count("    runs-on:"), 1)
        self.assertEqual(header.count("    if:"), 1)
        self.assertIn(
            "    if: ${{ github.repository == 'harsh-nod/fe2o3' && "
            "github.ref == 'refs/heads/main' && "
            "github.event_name != 'pull_request' }}\n",
            header,
        )
        self.assertIn("commit label is routing, not an authorization boundary", header)

    def test_workflow_and_local_policy_run_the_maintained_entrypoints(self):
        workflow = (ROOT / ".github/workflows/runtime-model-verus.yml").read_text()
        self.assertEqual(workflow.count(
            "        run: bash scripts/qualify-runtime-production-proofs.sh\n"), 1)
        self.assertEqual(workflow.count(
            "        run: python3 -I -B scripts/tests/runtime-production-proof-pipeline.py\n"), 1)
        for path in ("scripts/qualify-runtime-production-proofs.sh",
                     "scripts/tests/runtime-production-proof-pipeline.py"):
            self.assertEqual(workflow.count(f'      - "{path}"\n'), 2)
        for _, output, _ in COMMANDS:
            self.assertEqual(workflow.count(
                f"            ${{{{ env.A2_CAMPAIGN_ROOT }}}}/{output}\n"), 1)
        self.assertIn(
            "  run_step runtime-production-proof-pipeline-tests \\\n"
            "    python3 -I -B scripts/tests/runtime-production-proof-pipeline.py\n",
            (ROOT / "scripts/ci-local.sh").read_text(),
        )

    def test_workflow_retains_cached_prefix_diagnostics_in_existing_upload(self):
        workflow = (ROOT / ".github/workflows/runtime-model-verus.yml").read_text()
        marker = "      - name: Retain complete campaign diagnostics\n"
        self.assertEqual(workflow.count(marker), 1)
        upload = workflow.partition(marker)[2].partition(
            "      - name: Remove private campaign directory\n")[0]
        for _, output, _ in CACHED_COMMANDS:
            path = f"            ${{{{ env.A2_CAMPAIGN_ROOT }}}}/{output}\n"
            self.assertEqual(workflow.count(path), 1)
            self.assertEqual(upload.count(path), 1)

    def invoke(self, fail_at: int = 0, invalid: str = ""):
        with tempfile.TemporaryDirectory(prefix="fe2o3-production-proof-wiring-") as name:
            root = Path(name)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            recorder = bin_dir / "python3"
            recorder.write_text(RECORDER)
            recorder.chmod(0o700)
            verifier = root / "pinned verifier"
            verifier.write_text("#!/bin/sh\nexit 99\n")
            verifier.chmod(0o700)
            campaign = root / "private campaign"
            campaign.mkdir(mode=0o700)
            log = root / "calls"
            environment = {
                "PATH": f"{bin_dir}:/usr/bin:/bin",
                "LOG": str(log),
                "FAIL_AT": str(fail_at),
                "VERUS": str(verifier),
                "A2_CAMPAIGN_ROOT": str(campaign),
            }
            if invalid == "missing-verus":
                environment.pop("VERUS")
            elif invalid == "empty-verus":
                environment["VERUS"] = ""
            elif invalid == "relative-verus":
                environment["VERUS"] = verifier.name
            elif invalid == "nonexecutable-verus":
                verifier.chmod(0o600)
            elif invalid == "directory-verus":
                environment["VERUS"] = str(campaign)
            elif invalid == "missing-root":
                environment.pop("A2_CAMPAIGN_ROOT")
            elif invalid == "empty-root":
                environment["A2_CAMPAIGN_ROOT"] = ""
            elif invalid == "relative-root":
                environment["A2_CAMPAIGN_ROOT"] = campaign.name
            elif invalid == "absent-root":
                campaign.rmdir()
            elif invalid == "symlink-root":
                link = root / "campaign-link"
                link.symlink_to(campaign, target_is_directory=True)
                environment["A2_CAMPAIGN_ROOT"] = str(link)
            result = subprocess.run(
                ["bash", str(SCRIPT)], cwd=root, env=environment,
                capture_output=True, check=False, timeout=10,
            )
            rows = [tuple(field.decode() for field in line.split(b"\0")[:-1])
                    for line in log.read_bytes().splitlines()] if log.exists() else []
            expected = [
                (str(ROOT), "-I", "-B", f"crates/fe2o3-runtime-model/verus/{script}",
                 *options, "--verus", str(verifier), "--output", str(campaign / output))
                for script, output, options in (*COMMANDS, *CACHED_COMMANDS)
            ]
            return result, rows, expected

    def test_every_campaign_uses_the_exact_options_order_and_private_output(self):
        result, rows, expected = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(rows, expected)

    def test_each_campaign_failure_stops_before_the_next_campaign(self):
        for position in range(1, len(COMMANDS) + len(CACHED_COMMANDS) + 1):
            with self.subTest(position=position):
                result, rows, expected = self.invoke(fail_at=position)
                self.assertEqual(result.returncode, 37, result.stderr.decode())
                self.assertEqual(rows, expected[:position])

    def test_invalid_prerequisites_refuse_before_any_controller(self):
        for invalid in (
            "missing-verus", "empty-verus", "relative-verus", "nonexecutable-verus",
            "directory-verus", "missing-root", "empty-root", "relative-root",
            "absent-root", "symlink-root",
        ):
            with self.subTest(invalid=invalid):
                result, rows, _ = self.invoke(invalid=invalid)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(rows, [])


if __name__ == "__main__":
    unittest.main()
