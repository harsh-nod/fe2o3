#!/usr/bin/env python3

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location(
    "xgmi_peer_hot_campaign", Path(__file__).with_name("campaign.py")
)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("cannot load campaign.py")
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)


class InjectedFailure(RuntimeError):
    pass


class FakeSteps:
    def __init__(self, failures=None):
        self.calls = []
        self.failures = failures or {}

    def __call__(self, name):
        self.calls.append(name)
        error = self.failures.get(name)
        if error is not None:
            raise error
        return name


def collect_exact(step, digest_failure=None):
    step("remote-inventory")
    step("collect")
    if digest_failure is not None:
        raise digest_failure


def check_absence(step, validation_failure=None):
    step("absence")
    if validation_failure is not None:
        raise validation_failure


def receipt_folder(root, mutation=None):
    folder = root / "phase"
    folder.mkdir()
    stdout = b"complete\n"
    stderr = b""
    (folder / "stdout").write_bytes(stdout)
    (folder / "stderr").write_bytes(stderr)
    row = {
        "command": ["command", "argument"],
        "cwd": "/owned/worktree",
        "started_ns": 100,
        "timeout_seconds": 60,
        "pid": 123,
        "exit": 0,
        "error": None,
        "group_absent": True,
        "environment": None,
        "stdin_sha256": None,
        "finished_ns": 200,
        "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
        "stderr_sha256": hashlib.sha256(stderr).hexdigest(),
    }
    if mutation is not None:
        mutation(row)
    (folder / "receipt.json").write_text(
        json.dumps(row, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return folder


class CampaignLifecycleTests(unittest.TestCase):
    def exercise(
        self,
        failures=None,
        *,
        digest_failure=None,
        absence_validation_failure=None,
    ):
        steps = FakeSteps(failures)
        state = C.new_state()

        def collect(step):
            collect_exact(step, digest_failure)

        def absence(step):
            check_absence(step, absence_validation_failure)

        error = None
        try:
            C.remote_lifecycle(steps, collect, absence, state)
        except BaseException as caught:
            error = caught
        return steps.calls, state, error

    def test_success_has_exact_complete_order_and_replayable_state(self):
        calls, state, error = self.exercise()
        self.assertIsNone(error)
        payload = Path(tempfile.mkdtemp(prefix="peer-hot-success-"))
        try:
            self.assertIsNone(C.finalize_local(payload, state, error))
            self.assertTrue(C.path_absent(payload))
        finally:
            if payload.exists() and not payload.is_symlink():
                shutil.rmtree(payload)
        self.assertEqual(
            calls,
            [
                "create",
                "upload",
                "native",
                "remote-inventory",
                "collect",
                "cleanup",
                "absence",
            ],
        )
        self.assertEqual(
            state,
            {
                "schema": "fe2o3.xgmi-peer-hot-controller-state.v1",
                "create_attempted": True,
                "created": True,
                "uploaded": True,
                "native_attempted": True,
                "native_succeeded": True,
                "collected": True,
                "cleanup_attempted": True,
                "cleaned": True,
                "absence_attempted": True,
                "remote_absent": True,
                "local_payload_absent": True,
                "failure": None,
                "secondary_failures": [],
            },
        )

    def test_ambiguous_create_failure_still_cleans_and_checks_absence(self):
        original = InjectedFailure("ambiguous create")
        calls, state, error = self.exercise({"create": original})
        self.assertIs(error, original)
        self.assertEqual(calls, ["create", "cleanup", "absence"])
        self.assertFalse(state["created"])
        self.assertFalse(state["native_attempted"])
        self.assertTrue(state["cleaned"])
        self.assertTrue(state["remote_absent"])
        self.assertEqual(state["failure"]["stage"], "create")

    def test_upload_failure_uses_pre_native_cleanup(self):
        original = InjectedFailure("upload")
        calls, state, error = self.exercise({"upload": original})
        self.assertIs(error, original)
        self.assertEqual(calls, ["create", "upload", "cleanup", "absence"])
        self.assertTrue(state["created"])
        self.assertFalse(state["uploaded"])
        self.assertFalse(state["native_attempted"])

    def test_native_failure_is_collected_before_cleanup(self):
        original = InjectedFailure("native")
        calls, state, error = self.exercise({"native": original})
        self.assertIs(error, original)
        self.assertEqual(
            calls,
            [
                "create",
                "upload",
                "native",
                "remote-inventory",
                "collect",
                "cleanup",
                "absence",
            ],
        )
        self.assertFalse(state["native_succeeded"])
        self.assertTrue(state["collected"])
        self.assertTrue(state["remote_absent"])
        self.assertEqual(state["failure"]["stage"], "native")

    def test_inventory_collect_and_digest_failures_never_delete_remote(self):
        cases = [
            ({"remote-inventory": InjectedFailure("inventory")}, None),
            ({"collect": InjectedFailure("collect")}, None),
            ({}, InjectedFailure("digest")),
        ]
        for failures, digest_failure in cases:
            with self.subTest(failures=failures, digest=digest_failure):
                calls, state, error = self.exercise(
                    failures, digest_failure=digest_failure
                )
                self.assertIsNotNone(error)
                self.assertFalse(state["collected"])
                self.assertFalse(state["cleanup_attempted"])
                self.assertFalse(state["absence_attempted"])
                self.assertNotIn("cleanup", calls)
                self.assertNotIn("absence", calls)
                self.assertEqual(state["failure"]["stage"], "collection")

    def test_cleanup_failure_does_not_skip_absence(self):
        original = InjectedFailure("cleanup")
        calls, state, error = self.exercise({"cleanup": original})
        self.assertIs(error, original)
        self.assertEqual(calls[-2:], ["cleanup", "absence"])
        self.assertTrue(state["collected"])
        self.assertFalse(state["cleaned"])
        self.assertTrue(state["remote_absent"])
        self.assertEqual(state["failure"]["stage"], "post-collection-cleanup")

    def test_absence_command_and_validation_failures_are_not_promoted(self):
        for failures, validation in (
            ({"absence": InjectedFailure("absence command")}, None),
            ({}, InjectedFailure("absence payload")),
        ):
            with self.subTest(failures=failures, validation=validation):
                calls, state, error = self.exercise(
                    failures, absence_validation_failure=validation
                )
                self.assertIsNotNone(error)
                self.assertEqual(calls[-2:], ["cleanup", "absence"])
                self.assertTrue(state["cleaned"])
                self.assertFalse(state["remote_absent"])
                self.assertEqual(state["failure"]["stage"], "post-collection-absence")

    def test_original_failure_wins_and_all_secondary_failures_are_retained(self):
        native = InjectedFailure("native")
        cleanup = InjectedFailure("cleanup")
        absence = InjectedFailure("absence")
        calls, state, error = self.exercise(
            {"native": native, "cleanup": cleanup, "absence": absence}
        )
        self.assertIs(error, native)
        self.assertEqual(calls[-2:], ["cleanup", "absence"])
        self.assertEqual(state["failure"]["stage"], "native")
        self.assertEqual(
            [row["stage"] for row in state["secondary_failures"]],
            ["post-collection-cleanup", "post-collection-absence"],
        )

    def test_collection_failure_is_secondary_to_native_and_preserves_remote(self):
        native = InjectedFailure("native")
        digest = InjectedFailure("digest")
        calls, state, error = self.exercise({"native": native}, digest_failure=digest)
        self.assertIs(error, native)
        self.assertEqual(calls[-2:], ["remote-inventory", "collect"])
        self.assertEqual(state["failure"]["stage"], "native")
        self.assertEqual(
            [row["stage"] for row in state["secondary_failures"]], ["collection"]
        )
        self.assertFalse(state["cleanup_attempted"])

    def test_pre_native_cleanup_and_absence_failures_are_independent(self):
        create = InjectedFailure("create")
        cleanup = InjectedFailure("cleanup")
        absence = InjectedFailure("absence")
        calls, state, error = self.exercise(
            {"create": create, "cleanup": cleanup, "absence": absence}
        )
        self.assertIs(error, create)
        self.assertEqual(calls, ["create", "cleanup", "absence"])
        self.assertEqual(
            [row["stage"] for row in state["secondary_failures"]],
            ["pre-native-cleanup", "pre-native-absence"],
        )

    def test_local_payload_is_deleted_only_when_remote_absence_is_proven(self):
        preserved = Path(tempfile.mkdtemp(prefix="peer-hot-preserved-"))
        removable = Path(tempfile.mkdtemp(prefix="peer-hot-removable-"))
        precreate = Path(tempfile.mkdtemp(prefix="peer-hot-precreate-"))
        try:
            state = C.new_state()
            state["create_attempted"] = True
            state["collected"] = False
            self.assertIsNone(C.finalize_local(preserved, state, None))
            self.assertTrue(preserved.is_dir())
            self.assertFalse(state["local_payload_absent"])

            state = C.new_state()
            state["create_attempted"] = True
            state["remote_absent"] = True
            self.assertIsNone(C.finalize_local(removable, state, None))
            self.assertTrue(C.path_absent(removable))
            self.assertTrue(state["local_payload_absent"])

            state = C.new_state()
            self.assertIsNone(C.finalize_local(precreate, state, None))
            self.assertTrue(C.path_absent(precreate))
            self.assertTrue(state["local_payload_absent"])
        finally:
            for path in (preserved, removable, precreate):
                if path.exists() and not path.is_symlink():
                    shutil.rmtree(path)

    def test_local_absence_rejects_a_dangling_symlink(self):
        root = Path(tempfile.mkdtemp(prefix="peer-hot-symlink-"))
        link = root / "payload"
        link.symlink_to(root / "missing", target_is_directory=True)
        try:
            self.assertFalse(C.path_absent(link))
        finally:
            link.unlink()
            root.rmdir()

    def test_receipt_accepts_only_the_exact_typed_recorder_schema(self):
        with tempfile.TemporaryDirectory(prefix="peer-hot-receipt-") as temporary:
            folder = receipt_folder(Path(temporary))
            row = C.verify_receipt(folder)
            self.assertEqual(set(row), C.RECEIPT_KEYS)
            self.assertEqual(len(row), 13)

        valid_variants = [
            lambda row: row.update(
                environment={"PATH": "/usr/bin"}, stdin_sha256="a" * 64
            ),
        ]
        for mutation in valid_variants:
            with self.subTest(valid=mutation):
                with tempfile.TemporaryDirectory(
                    prefix="peer-hot-receipt-"
                ) as temporary:
                    C.verify_receipt(receipt_folder(Path(temporary), mutation))

        invalid_mutations = [
            lambda row: row.update(extra=True),
            lambda row: row.pop("cwd"),
            lambda row: row.update(command="command"),
            lambda row: row.update(command=[]),
            lambda row: row.update(command=["command", 1]),
            lambda row: row.update(cwd=""),
            lambda row: row.update(timeout_seconds=True),
            lambda row: row.update(timeout_seconds=0),
            lambda row: row.update(pid=True),
            lambda row: row.update(pid=0),
            lambda row: row.update(exit=False),
            lambda row: row.update(exit=1),
            lambda row: row.update(error="failure"),
            lambda row: row.update(group_absent=1),
            lambda row: row.update(group_absent=False),
            lambda row: row.update(started_ns=True),
            lambda row: row.update(started_ns=0),
            lambda row: row.update(finished_ns=True),
            lambda row: row.update(finished_ns=99),
            lambda row: row.update(environment=[]),
            lambda row: row.update(environment={"PATH": 1}),
            lambda row: row.update(stdin_sha256="A" * 64),
            lambda row: row.update(stdin_sha256="a" * 63),
            lambda row: row.update(stdout_sha256="0" * 64),
            lambda row: row.update(stderr_sha256=True),
        ]
        for index, mutation in enumerate(invalid_mutations):
            with self.subTest(invalid=index):
                with tempfile.TemporaryDirectory(
                    prefix="peer-hot-receipt-"
                ) as temporary:
                    folder = receipt_folder(Path(temporary), mutation)
                    with self.assertRaises((RuntimeError, KeyError)):
                        C.verify_receipt(folder)

    def test_receipt_rejects_extra_and_symlinked_files_or_directory(self):
        with tempfile.TemporaryDirectory(prefix="peer-hot-receipt-") as temporary:
            folder = receipt_folder(Path(temporary))
            (folder / "extra").write_bytes(b"")
            with self.assertRaises(RuntimeError):
                C.verify_receipt(folder)

        with tempfile.TemporaryDirectory(prefix="peer-hot-receipt-") as temporary:
            root = Path(temporary)
            folder = receipt_folder(root)
            (folder / "stdout").unlink()
            (folder / "stdout").symlink_to(folder / "stderr")
            with self.assertRaises(RuntimeError):
                C.verify_receipt(folder)

        with tempfile.TemporaryDirectory(prefix="peer-hot-receipt-") as temporary:
            root = Path(temporary)
            folder = receipt_folder(root)
            alias = root / "alias"
            alias.symlink_to(folder, target_is_directory=True)
            with self.assertRaises(RuntimeError):
                C.verify_receipt(alias)


if __name__ == "__main__":
    unittest.main()
