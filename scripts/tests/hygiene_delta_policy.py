#!/usr/bin/env python3

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "hygiene_delta_policy.py"


def run_git(repo: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def commit(repo: Path, message: str) -> str:
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", message)
    return run_git(repo, "rev-parse", "HEAD")


def repeated_source(lines: int) -> str:
    return "".join(
        f"pub fn generated_{index:04}() -> u32 {{ {index} }}\n"
        for index in range(lines)
    )


class HygieneDeltaPolicyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        run_git(self.repo, "init")
        run_git(self.repo, "config", "user.email", "fe2o3@example.invalid")
        run_git(self.repo, "config", "user.name", "fe2o3 hygiene test")

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def checker(self, base: str, head: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                "-I",
                str(CHECKER),
                "--repo",
                str(self.repo),
                "--base",
                base,
                "--head",
                head,
            ],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_allows_small_production_change(self) -> None:
        write(self.repo / "crates/demo/src/lib.rs", "pub fn one() -> u32 { 1 }\n")
        base = commit(self.repo, "base")
        write(
            self.repo / "crates/demo/src/lib.rs",
            "pub fn one() -> u32 { 1 }\npub fn two() -> u32 { 2 }\n",
        )
        head = commit(self.repo, "head")
        result = self.checker(base, head)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIn("hygiene delta policy: OK", result.stdout)

    def test_rejects_new_large_production_file(self) -> None:
        write(self.repo / "crates/demo/src/lib.rs", "pub fn root() {}\n")
        base = commit(self.repo, "base")
        write(self.repo / "crates/demo/src/large.rs", repeated_source(1201))
        head = commit(self.repo, "head")
        result = self.checker(base, head)
        self.assertEqual(1, result.returncode)
        self.assertIn("new production source file", result.stderr)

    def test_rejects_already_large_file_growth(self) -> None:
        write(self.repo / "crates/demo/src/lib.rs", repeated_source(1201))
        base = commit(self.repo, "base")
        write(self.repo / "crates/demo/src/lib.rs", repeated_source(1453))
        head = commit(self.repo, "head")
        result = self.checker(base, head)
        self.assertEqual(1, result.returncode)
        self.assertIn("already-large production source file grew", result.stderr)

    def test_rejects_added_production_panic_macro(self) -> None:
        write(self.repo / "crates/demo/src/lib.rs", "pub fn root() {}\n")
        base = commit(self.repo, "base")
        write(
            self.repo / "crates/demo/src/lib.rs",
            "pub fn root() {}\npub fn bad() { panic!(\"bad\"); }\n",
        )
        head = commit(self.repo, "head")
        result = self.checker(base, head)
        self.assertEqual(1, result.returncode)
        self.assertIn("new production panic macro", result.stderr)

    def test_allows_marked_test_commented_and_quoted_panic_macros(self) -> None:
        write(self.repo / "crates/demo/src/lib.rs", "pub fn root() {}\n")
        base = commit(self.repo, "base")
        write(
            self.repo / "crates/demo/src/lib.rs",
            """
pub fn marked() {
    // fe2o3-hygiene: allow-panic issue-1
    panic!("intentional fixture");
}

pub fn quoted() {
    let _ = "panic!(not code)";
    // todo!(not code)
}

#[cfg(test)]
mod tests {
    #[test]
    fn allowed_in_tests() {
        unimplemented!("test helper");
    }
}
""",
        )
        head = commit(self.repo, "head")
        result = self.checker(base, head)
        self.assertEqual(0, result.returncode, result.stderr)

    def test_rejects_exact_duplicate_changed_file(self) -> None:
        duplicate = repeated_source(150)
        write(self.repo / "crates/demo/src/a.rs", duplicate)
        write(self.repo / "crates/demo/src/lib.rs", "pub mod a;\n")
        base = commit(self.repo, "base")
        write(self.repo / "crates/demo/src/b.rs", duplicate)
        write(self.repo / "crates/demo/src/lib.rs", "pub mod a;\npub mod b;\n")
        head = commit(self.repo, "head")
        result = self.checker(base, head)
        self.assertEqual(1, result.returncode)
        self.assertIn("exactly duplicates", result.stderr)

    def test_ignores_fixture_and_test_source_paths(self) -> None:
        write(self.repo / "crates/demo/src/lib.rs", "pub fn root() {}\n")
        base = commit(self.repo, "base")
        write(
            self.repo / "crates/demo/tests/fixtures/demo/src/lib.rs",
            repeated_source(1300),
        )
        write(self.repo / "crates/demo/src/tests.rs", "pub fn helper() { panic!(); }\n")
        head = commit(self.repo, "head")
        result = self.checker(base, head)
        self.assertEqual(0, result.returncode, result.stderr)

    def test_malformed_ref_is_input_error(self) -> None:
        write(self.repo / "crates/demo/src/lib.rs", "pub fn root() {}\n")
        head = commit(self.repo, "base")
        result = self.checker("bad ref", head)
        self.assertEqual(2, result.returncode)
        self.assertIn("base ref is malformed", result.stderr)


if __name__ == "__main__":
    unittest.main()
