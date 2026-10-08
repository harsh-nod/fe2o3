#!/usr/bin/env python3
"""Inert fixtures for full-inventory finalizer sharding; no Cargo or test binary runs."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("finalizer_shards", ROOT / "scripts/finalizer-test-shards.py")
S = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(S)


def target(kind, name, test=True, required=()):
    return dict(kind=[kind], name=name, test=test, src_path=f"/source/{name}.rs", **{"required-features": list(required)})


def fixtures():
    targets = [target("lib", "fe2o3_hsaco_finalize"), target("test", "worker_v3_hsaco_admission"),
               target("bin", "fixture", False), target("example", "ordinary", False),
               target("example", "test_example"), target("bin", "optional", required=("opt",))]
    package = dict(name=S.PACKAGE, id="original-package", manifest_path=str(ROOT / "crates" / S.PACKAGE / "Cargo.toml"), targets=targets)
    messages = [dict(reason="compiler-artifact", package_id=package["id"], target=t,
                     profile={"test": t["test"]}, executable=f"/target/{t['name']}", features=[])
                for t in targets[:-1]]
    messages.append(dict(reason="build-finished", success=True))
    return dict(version=1, packages=[package]), messages


class ShardTests(unittest.TestCase):
    def test_build_inventory_is_complete_and_default_feature_exact(self):
        metadata, messages = fixtures()
        self.assertEqual(set(S.inventory(metadata, messages)), {("lib", "fe2o3_hsaco_finalize"),
            ("test", "worker_v3_hsaco_admission"), ("example", "test_example")})
        for mutate in (lambda m, e: e.pop(0), lambda m, e: e.insert(0, copy.deepcopy(e[0])),
                       lambda m, e: e[-1].update(success=False),
                       lambda m, e: m["packages"][0]["targets"].append(target("test", "new_test")),
                       lambda m, e: e[0].update(features=["opt"]),
                       lambda m, e: e[0]["target"].update(kind=["custom-harness"]),
                       lambda m, e: e[0].update(executable=None)):
            m, e = copy.deepcopy((metadata, messages))
            mutate(m, e)
            with self.assertRaises(S.ShardError):
                S.inventory(m, e)

    def test_list_requires_complete_counts_unique_names_and_known_harness_format(self):
        self.assertEqual(S.listing("a::one: test\nb: benchmark\n\n1 test, 1 benchmark\n"),
                         {"a::one": "test", "b": "benchmark"})
        self.assertEqual(S.listing("0 tests, 0 benchmarks\n"), {})
        for raw in ("one: test\n", "one: test\n0 tests, 0 benchmarks\n",
                    "one: test\none: test\n2 tests, 0 benchmarks\n", "hello custom harness\n",
                    "--ignored: test\n1 test, 0 benchmarks\n", "one: test\n1 test, 0 benchmarks\n0 tests, 0 benchmarks\n"):
            with self.assertRaises(S.ShardError):
                S.listing(raw)

    def test_partition_is_deterministic_complete_and_balanced_for_new_names_and_targets(self):
        roster = {("test", "heavy"): {f"test_{i:03}": "test" for i in range(71)},
                  ("lib", "root"): {"shared_name": "test"}, ("test", "new"): {"shared_name": "test"}}
        shards = S.partition(roster)
        self.assertEqual(shards, S.partition(dict(reversed(list(roster.items())))))
        flattened = [row for shard in shards for row in shard]
        self.assertEqual(len(flattened), len(set(flattened)))
        self.assertEqual(len(flattened), 73)
        self.assertLessEqual(max(map(len, shards)) - min(map(len, shards)), 1)
        roster[("example", "later")] = {"additional": "test"}
        self.assertEqual(sum(map(len, S.partition(roster))), 74)

    def test_cargo_remains_launcher_with_original_target_and_nonempty_exact_filters(self):
        for kind, selector in S.KINDS.items():
            command = S.test_command((kind, "original"), ["a", "a::nested"])
            self.assertEqual(command[:6], ["cargo", "test", "--locked", "-p", S.PACKAGE, selector])
            self.assertEqual(command[command.index("--") + 1:], ["--exact", "--format=pretty", "--color=never", "a", "a::nested"])
            self.assertNotIn("--ignored", command)
        for names in ([], ["a", "a"], ["--include-ignored"]):
            with self.assertRaises(S.ShardError):
                S.test_command(("test", "original"), names)

    def test_exact_test_outcomes_do_not_turn_ignored_or_missing_tests_into_success(self):
        raw = "running 2 tests\ntest a ... ok\ntest ignored ... ignored, opt-in hardware\n\ntest result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 1 filtered out; finished in 0.00s\n"
        cases = dict(a="test", ignored="test", unselected="test")
        self.assertEqual(S.outcome(raw, cases, {"ignored"}, ["a", "ignored"])["ignored"], 1)
        self.assertEqual(S.outcome(raw.replace("test a ...", "test a - should panic ..."), cases,
                                   {"ignored"}, ["a", "ignored"])["passed"], 1)
        for bad in (raw.replace("test a ... ok", "test another ... ok"), raw.replace("1 filtered", "0 filtered"),
                    raw.replace("1 ignored", "0 ignored"), raw.replace("ignored, opt-in hardware", "ok"),
                    raw + raw):
            with self.assertRaises(S.ShardError):
                S.outcome(bad, cases, {"ignored"}, ["a", "ignored"])

    def receipts(self, directory):
        cases = {f"case_{i}": "test" for i in range(9)}
        cases["ignored"] = "test"
        shards = S.partition({("test", "original"): cases})
        for shard in range(4):
            folder = directory / str(shard)
            folder.mkdir()
            manifest = dict(schema=1, package=S.PACKAGE, commit="1" * 40, shards=shards,
                targets=[dict(kind="test", name="original", executable=f"/different-shard-{shard}/test",
                              cases=cases, ignored=["ignored"])])
            raw = json.dumps(manifest).encode()
            (folder / "inventory.json").write_bytes(raw)
            selected = [name for _, _, name in shards[shard]]
            skipped = int("ignored" in selected)
            result = dict(shard=shard, inventory_sha256=hashlib.sha256(raw).hexdigest(), discovery_only=False,
                selected=shards[shard], docs=shard == 0, complete=True,
                targets=[dict(kind="test", name="original", selected=selected, passed=len(selected) - skipped,
                              ignored=skipped, measured=0, filtered=len(cases) - len(selected))])
            (folder / "result.json").write_text(json.dumps(result))

    def test_aggregate_requires_four_actual_shards_and_exact_once_docs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.receipts(root)
            with mock.patch("builtins.print"):
                S.aggregate(root, "1" * 40)
            path = root / "1/result.json"
            original = path.read_text()
            for change in (dict(shard=0), dict(complete=False), dict(discovery_only=True), dict(docs=True),
                           dict(selected=[]), dict(targets=[]), dict(inventory_sha256="0" * 64)):
                value = json.loads(original)
                value.update(change)
                path.write_text(json.dumps(value))
                with self.assertRaises(S.ShardError):
                    S.aggregate(root, "1" * 40)
            path.write_text(original)
            with self.assertRaises(S.ShardError):
                S.aggregate(root, "2" * 40)
            path.unlink()
            with self.assertRaises(S.ShardError):
                S.aggregate(root, "1" * 40)

    def test_aggregate_rejects_different_inventory_even_if_its_local_partition_is_complete(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.receipts(root)
            path = root / "1/inventory.json"
            manifest = json.loads(path.read_text())
            manifest["targets"][0]["ignored"] = []
            raw = json.dumps(manifest).encode()
            path.write_bytes(raw)
            result_path = path.with_name("result.json")
            result = json.loads(result_path.read_text())
            result["inventory_sha256"] = hashlib.sha256(raw).hexdigest()
            result_path.write_text(json.dumps(result))
            with self.assertRaises(S.ShardError):
                S.aggregate(root, "1" * 40)

    def test_partial_rerun_keeps_three_prior_successes_and_replaces_only_its_shard(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.receipts(root)
            for shard in range(4):
                (root / str(shard)).rename(root / f"finalizer-shard-cpu-finalize-{shard}-receipt")
            previous = {path: path.read_bytes() for path in root.rglob("*.json")
                        if "finalize-3-receipt" not in str(path)}
            rerun = root / "finalizer-shard-cpu-finalize-3-receipt"
            inventory = json.loads((rerun / "inventory.json").read_text())
            inventory["targets"][0]["executable"] = "/rerun/original-target"
            raw = json.dumps(inventory).encode()
            (rerun / "inventory.json").write_bytes(raw)
            result = json.loads((rerun / "result.json").read_text())
            result["inventory_sha256"] = hashlib.sha256(raw).hexdigest()
            (rerun / "result.json").write_text(json.dumps(result))
            with mock.patch("builtins.print"):
                S.aggregate(root, "1" * 40)
            self.assertTrue(all(path.read_bytes() == raw for path, raw in previous.items()))
            result["complete"] = False
            (rerun / "result.json").write_text(json.dumps(result))
            with self.assertRaises(S.ShardError):
                S.aggregate(root, "1" * 40)

    def test_duplicate_json_fields_refuse(self):
        with self.assertRaises(S.ShardError):
            S.unique_json('{"schema":1,"schema":1}')

    def test_mocked_four_shard_execution_keeps_cargo_env_docs_and_ignored_semantics(self):
        metadata, messages = fixtures()
        cases = {key: {"first": "test", "second": "test"}
                 for key in S.inventory(metadata, messages)}
        ignored = {key: {"second"} for key in cases}
        calls = []

        def run(_commands, label, argv):
            calls.append(argv)
            if label == "commit":
                return "1" * 40 + "\n"
            if label == "metadata":
                return json.dumps(metadata)
            if label == "build":
                return "\n".join(map(json.dumps, messages))
            if label == "docs":
                return "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
            key = next(key for key in cases if argv[:len(S.cargo_target(key))] == S.cargo_target(key))
            if "--list" in argv:
                names = sorted(ignored[key] if "--ignored" in argv else cases[key])
                return "".join(name + ": test\n" for name in names) + f"\n{len(names)} tests, 0 benchmarks\n"
            self.assertIn("--exact", argv)
            names = argv[argv.index("--color=never") + 1:]
            self.assertTrue(names)
            skipped = len(set(names) & ignored[key])
            return "".join(f"test {name} ... {'ignored' if name in ignored[key] else 'ok'}\n" for name in names) + (
                f"\ntest result: ok. {len(names) - skipped} passed; 0 failed; {skipped} ignored; 0 measured; "
                f"{len(cases[key]) - len(names)} filtered out; finished in 0.00s\n")

        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(S.Commands, "run", run), \
             mock.patch.object(S.subprocess, "run", side_effect=AssertionError("no subprocess in inert test")), \
             mock.patch("builtins.print"):
            root = Path(temporary)
            for shard in range(4):
                S.execute(shard, root / str(shard))
            S.aggregate(root, "1" * 40)
            self.assertEqual(sum("--doc" in argv for argv in calls), 1)
            self.assertEqual(sum("--no-run" in argv for argv in calls), 4)
            executed = [argv for argv in calls if "--exact" in argv]
            self.assertEqual(sum(len(argv) - argv.index("--color=never") - 1 for argv in executed), 6)
            self.assertTrue(all(argv[:2] == ["cargo", "test"] for argv in executed))

    def test_failed_build_cannot_emit_a_complete_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "failure"
            with mock.patch.object(S.Commands, "run", side_effect=["1" * 40, ShardErrorFixture()]), \
                 self.assertRaises(ShardErrorFixture):
                S.execute(0, output)
            self.assertFalse((output / "result.json").exists())


class ShardErrorFixture(Exception):
    pass


if __name__ == "__main__":
    unittest.main(verbosity=2)
