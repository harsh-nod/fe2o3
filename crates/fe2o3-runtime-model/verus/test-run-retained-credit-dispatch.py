#!/usr/bin/env python3
"""Synthetic and source-only recorder controls; never launch a compiler or solver."""
import contextlib
import copy
import hashlib
import io
import json
from pathlib import Path
import signal
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

path = Path(__file__).resolve().with_name("run-retained-credit-dispatch.py")
run = types.ModuleType("portable_dispatch_tests")
run.__file__ = str(path)
sys.modules[run.__name__] = run
exec(compile(path.read_bytes(), str(path), "exec"), run.__dict__)
check, classifier, owner, identity, leaf = run.helpers()


def span(path, line, text=None, primary=False):
    return {"file_name": str(path), "line_start": line, "line_end": line, "is_primary": primary,
            "text": [] if text is None else [{"text": text}]}


def diagnostic(family, root=run.ROOT):
    site = run.sites(check)[family]
    primary = span(root / run.PROOF, site["contract_line"], site["contract_text"], True)
    lines = (root / site["body"]).read_text().splitlines()
    body = span(root / site["body"], 2, lines[1])
    body["expansion"] = {"macro_decl_name": site["macro"],
        "span": span(root / run.PROOF, site["call_line"], site["call_text"]),
        "def_site_span": span(root / site["body"], site["definition_line"])}
    return {"$message_type": "diagnostic", "message": "postcondition not satisfied", "level": "error",
            "code": None, "children": [], "spans": [primary, body]}


def negative_summary():
    return {"verus": identity.VERIFIER, "verification-results": {
        "encountered-error": True, "encountered-vir-error": False, "verified": 0,
        "errors": 1, "is-verifying-entire-crate": False}}


class Controls(unittest.TestCase):
    def test_exact_source_roster(self):
        sources = check.snapshot()
        check.audit(sources)
        self.assertEqual(len(check.FILES), 14)
        rows = run.mutations(check)
        self.assertEqual(len(rows), 25)
        self.assertEqual(len(run.stages(rows)), 33)
        self.assertEqual(set(run.sites(check)), set(check.BODIES))

    def test_no_cheating_and_focus(self):
        paths = {"timeout": Path("/usr/bin/timeout")}
        for focus in (None, *check.SELECTORS.values()):
            command = run.command(paths, Path("/verifier/verus"), run.ROOT, focus)
            self.assertIn("--no-cheating", command)
            self.assertEqual(command[command.index("--multiple-errors") + 1], "1")
            self.assertEqual("--verify-function" in command, focus is not None)
            self.assertEqual(command[-1], str(run.ROOT / run.PROOF))

    def test_positive_exact_count_types_and_status(self):
        expected = {"encountered-error": False, "encountered-vir-error": False, "errors": 0,
                    "is-verifying-entire-crate": True, "success": True, "verified": 41}
        def accepts(status=0, result=expected, stderr=""):
            return run.positive(classifier, identity, check, run.ROOT, status,
                                json.dumps({"verus": identity.VERIFIER, "verification-results": result}), stderr)
        self.assertTrue(accepts())
        for count in (0, 40, 42, True, "41"):
            self.assertFalse(accepts(result=dict(expected, verified=count)))
        for status in (True, 1, 101, 124, -9):
            self.assertFalse(accepts(status=status))
        self.assertFalse(accepts(stderr="not JSON"))

    def test_negative_all_families(self):
        for family in check.BODIES:
            self.assertTrue(run.negative(classifier, identity, check, leaf, run.ROOT,
                check.BODIES[family], check.SELECTORS[family], 1,
                json.dumps(negative_summary()), json.dumps(diagnostic(family))))

    def test_foreign_family_is_not_logical_success(self):
        # The account wildcard also suffix-matches the runtime method.
        self.assertFalse(run.negative(classifier, identity, check, leaf, run.ROOT,
            check.BODIES["account"], check.SELECTORS["account"], 1,
            json.dumps(negative_summary()), json.dumps(diagnostic("runtime"))))
        self.assertFalse(run.negative(classifier, identity, check, leaf, run.ROOT,
            check.BODIES["account"], check.SELECTORS["runtime"], 1,
            json.dumps(negative_summary()), json.dumps(diagnostic("account"))))

    def test_family_span_and_expansion_integrity(self):
        original = diagnostic("account")
        site = run.sites(check)["account"]
        def accepts(value):
            return run.family_error_join(classifier, run.ROOT, site, json.dumps(value))
        self.assertTrue(accepts(original))
        for target, key, value in ((0, "is_primary", False), (0, "line_start", 1),
                                   (0, "file_name", "/foreign.rs"), (0, "text", [{"text": "foreign"}]),
                                   (1, "is_primary", True), (1, "line_end", 99999),
                                   (1, "text", []), (1, "expansion", None)):
            changed = copy.deepcopy(original)
            changed["spans"][target][key] = value
            self.assertFalse(accepts(changed))
        for key, value in (("macro_decl_name", "wrong!"), ("span", {}), ("def_site_span", {})):
            changed = copy.deepcopy(original)
            changed["spans"][1]["expansion"][key] = value
            self.assertFalse(accepts(changed))

    def test_frontend_timeout_and_malformed_negative(self):
        def accepts(status=1, report=None, error=None):
            return run.negative(classifier, identity, check, leaf, run.ROOT, check.BODIES["domain"],
                check.SELECTORS["domain"], status,
                json.dumps(negative_summary()) if report is None else report,
                json.dumps(diagnostic("domain")) if error is None else error)
        self.assertTrue(accepts())
        for status in (0, True, 101, 124, -9):
            self.assertFalse(accepts(status=status))
        for value in ('{"x":1,"x":2}', '{"x":NaN}', '{}', 'not JSON'):
            self.assertFalse(accepts(report=value))
            self.assertFalse(accepts(error=value))
        for delta in ({"code": {"code": "E0308"}}, {"message": "mismatched types"},
                      {"level": "warning"}, {"message": "timed out"}):
            self.assertFalse(accepts(error=json.dumps(dict(diagnostic("domain"), **delta))))

    def test_generated_tree_exact_one_change(self):
        expected = {str(path): run.sha(run.ROOT / path) for path in check.FILES}
        self.assertTrue(run.generated_tree_valid(expected, expected, None))
        for path, body, _focus in run.mutations(check).values():
            measured = {**expected, str(path): check.sha(body)}
            self.assertTrue(run.generated_tree_valid(expected, measured, (path, body)))
            self.assertFalse(run.generated_tree_valid(expected, expected, (path, body)))
            self.assertFalse(run.generated_tree_valid(expected, dict(measured, extra="x"), (path, body)))
            self.assertFalse(run.generated_tree_valid(expected, {**measured, str(run.PROOF): "x"}, (path, body)))

    def test_receipt_and_complete_roster(self):
        argv = ["fixture"]
        receipt = {"command": argv, "status": 0, "group_absent": True, "process_group": 10}
        self.assertTrue(run.receipt_valid(receipt, argv, 0))
        for delta in ({"command": []}, {"status": True}, {"group_absent": False},
                      {"process_group": True}, {"exception": "TimeoutExpired"}):
            self.assertFalse(run.receipt_valid(dict(receipt, **delta), argv, 0))
        expected = run.stages(run.mutations(check))
        rows = [{"name": name, **dict.fromkeys(run.FLAGS, True)} for name in expected]
        counts = {"full_positive": 3, "logical_negative": 25}
        self.assertTrue(run.complete(rows, expected, counts))
        self.assertFalse(run.complete(rows[:-1], expected, counts))
        self.assertFalse(run.complete(rows[::-1], expected, counts))
        self.assertFalse(run.complete(rows, expected, dict(counts, logical_negative=24)))
        for flag in run.FLAGS:
            changed = copy.deepcopy(rows)
            changed[8][flag] = False
            self.assertFalse(run.complete(changed, expected, counts))

    def test_census_and_fresh_arguments(self):
        census = {"recorded_groups": [10, 11], "members": [], "uncertain": [],
                  "all_recorded_groups_absent": True, "host_wide_absence_claimed": False,
                  "historical_groups_probed": False}
        self.assertTrue(run.census_valid(census, ["a", "b"]))
        for delta in ({"recorded_groups": [10, 10]}, {"members": [10]}, {"uncertain": [10]},
                      {"historical_groups_probed": True}, {"host_wide_absence_claimed": True}):
            self.assertFalse(run.census_valid(dict(census, **delta), ["a", "b"]))
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            verus = root / "release/verus"
            verus.parent.mkdir()
            verus.write_text("fixture")
            good = ["--campaign", "--verus", str(verus), "--output", str(root / "fresh")]
            self.assertEqual(run.arguments(good).output, root / "fresh")
            for bad in (good[1:], good + ["--calibrate"], good + ["--unknown"], ["--camp", *good[1:]]):
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                    run.arguments(bad)
            for output in (root, run.ROOT / "fresh", verus.parent / "fresh", Path("relative")):
                with self.assertRaises(ValueError):
                    run.arguments(good[:-1] + [str(output)])

    def test_signed_blob_framing_and_roster(self):
        value = b"signed fixture\n"
        oid = hashlib.sha1(b"blob " + str(len(value)).encode() + b"\0" + value).hexdigest()
        before = {"commit": "1" * 40, "files": {"crates/fixture.rs": {
            "sha256": hashlib.sha256(value).hexdigest(), "bytes": len(value)}}}
        listing = ("100644 blob " + oid + "\tcrates/fixture.rs\0").encode()
        batch = (oid + " blob " + str(len(value)) + "\n").encode() + value + b"\n"
        def accepts(tree=listing, blobs=batch, inputs=before):
            def git(_paths, _env, operation, *_args, **_kwargs):
                return tree if operation == "ls-tree" else blobs
            with patch.object(run, "git", git):
                return run.bind_blobs({}, {}, inputs)
        self.assertEqual(accepts()["files"]["crates/fixture.rs"]["git_blob"], oid)
        for tree, blobs in ((listing + listing, batch), (listing.replace(b"100644", b"120000"), batch),
                            (b"", batch), (listing, batch + b"extra"),
                            (listing, batch.replace(value, b"x" * len(value))),
                            (listing, batch[:-1])):
            with self.assertRaises(ValueError):
                accepts(tree, blobs)
        changed = copy.deepcopy(before)
        changed["files"]["crates/fixture.rs"]["sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            accepts(inputs=changed)

    def test_new_host_profile_measures_but_pins_release(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            manifest = root / run.MANIFEST
            manifest.parent.mkdir(parents=True)
            release = root / "release"
            release.mkdir()
            records = []
            for name in ("verus", "rust_verify", "z3"):
                executable = release / name
                executable.write_text("synthetic " + name)
                records.append(f"required={name}|{executable.stat().st_mode & 0o7777:o}|{executable.stat().st_size}|{run.sha(executable)}")
            manifest.write_text("\n".join(records) + "\n")
            host = root / "host-tool"
            host.write_text("measured host fixture")
            paths = {"git": host}
            with patch.object(run, "ROOT", root), patch.object(run, "tool_paths", lambda _env: paths):
                first = run.tools_inventory(paths, release / "verus", {})
                host.write_text("updated host fixture")
                second = run.tools_inventory(paths, release / "verus", {})
                self.assertNotEqual(first, second)
                (release / "z3").write_text("wrong release")
                with self.assertRaises(ValueError):
                    run.tools_inventory(paths, release / "verus", {})
            with patch.object(run, "tool_paths", lambda _env: {"git": root / "other"}):
                with self.assertRaises(ValueError):
                    run.tools_inventory(paths, release / "verus", {})

    def simulated_campaign(self, fail_stage=None, save_failure=None):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder).resolve() / "campaign"
            calls = []
            def owned(argv, seconds, directory, env):
                name = directory.parent.name
                calls.append(name)
                directory.mkdir()
                status, stdout, stderr = 0, "", ""
                if name == "00-signature":
                    stderr = run.SIGNATURE
                elif name == "01-controls":
                    stderr = f"\nRan {run.CONTROL_COUNT} tests in 0.001s\n\nOK\n"
                elif name == "02-source-calibration":
                    stdout = "retained dispatch calibration: 4 groups passed; 25 logical mutants constructed, not executed\n"
                elif "release-" in name:
                    stdout = run.CLOSURE_OK
                elif "-negative-" in name:
                    status = 1
                if name == fail_stage:
                    status = 124
                record = {"command": argv, "status": status, "group_absent": True, "process_group": 100 + len(calls)}
                run.save(directory / "record.json", record)
                (directory / "stdout.log").write_text(stdout)
                (directory / "stderr.log").write_text(stderr)
                return status, stdout, stderr
            fake_owner = types.SimpleNamespace(run_owned=owned, SIGNALS=(signal.SIGINT,))
            fake_owner.interrupted = lambda *_: None
            paths = dict.fromkeys((*run.UTILITY_NAMES, "python", "rustc"), Path("/fixture/tool"))
            before = {"commit": "1" * 40, "files": {}}
            namespace = {"fixture": "not live evidence"}
            def census(_output, _namespace, attempted, _classifier):
                return {"recorded_groups": list(range(101, 101 + len(attempted))), "members": [], "uncertain": [],
                        "all_recorded_groups_absent": True, "host_wide_absence_claimed": False,
                        "historical_groups_probed": False}
            original_save = run.save
            def save(path, value):
                if path == output / str(save_failure):
                    raise OSError("synthetic disk write failure")
                original_save(path, value)
            previous_cwd = Path.cwd()
            previous_handler = signal.getsignal(signal.SIGINT)
            with contextlib.ExitStack() as stack:
                for name, replacement in (
                    ("helpers", lambda: (check, classifier, fake_owner, identity, leaf)),
                    ("tool_paths", lambda _env: paths), ("tools_inventory", lambda *_: {"synthetic": True}),
                    ("inventory", lambda *_: before), ("bind_blobs", lambda *_: {}),
                    ("namespace_identity", lambda: namespace), ("independent_absence", census),
                    ("save", save),
                    ("positive", lambda *args: args[-3] == 0), ("negative", lambda *args: args[-3] == 1)):
                    stack.enter_context(patch.object(run, name, replacement))
                stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                args = types.SimpleNamespace(output=output, verus=Path("/fixture/verus"))
                if save_failure is not None:
                    with self.assertRaisesRegex(OSError, "synthetic disk write failure"):
                        run.campaign(args)
                else:
                    status = run.campaign(args)
            self.assertEqual(Path.cwd(), previous_cwd)
            self.assertIs(signal.getsignal(signal.SIGINT), previous_handler)
            if save_failure is not None:
                self.assertEqual(calls[-1], "32-release-after")
                self.assertFalse((output / "results.json").exists())
                return
            result = json.loads((output / "results.json").read_text())
            self.assertEqual(result["accepted"], fail_stage is None)
            self.assertEqual(status, int(fail_stage is not None))
            for closing in ("source", "tools", "raw", "namespace", "census", "trees"):
                self.assertTrue((output / (closing + "-after.json")).is_file())
            return calls, result

    def test_successful_campaign_exact_order(self):
        calls, result = self.simulated_campaign()
        self.assertEqual(calls, list(run.stages(run.mutations(check))))
        self.assertEqual(result["solver_attempts"], {"full_positive": 3, "logical_negative": 25})

    def test_rejected_campaign_still_closes(self):
        for failure in ("03-release-before", "11-negative-device-ignored"):
            calls, result = self.simulated_campaign(failure)
            self.assertEqual(calls[-1], "32-release-after")
            self.assertIn(failure, calls)
            self.assertNotIn("31-positive-after", calls)
            self.assertTrue(result["errors"])

    def test_evidence_write_failure_restores_process_state(self):
        previous_handler = signal.getsignal(signal.SIGINT)
        try:
            with tempfile.TemporaryDirectory() as folder, contextlib.chdir(folder):
                self.simulated_campaign("11-negative-device-ignored", save_failure="results.json")
                self.simulated_campaign(save_failure="raw-results.json")
        finally:
            signal.signal(signal.SIGINT, previous_handler)


if __name__ == "__main__":
    run.need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    unittest.main()
