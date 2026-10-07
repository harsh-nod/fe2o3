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

    def check_source(self, source: str, expected: int, path: str = "lib.rs") -> str:
        write(self.repo / "crates/demo/src/lib.rs", "pub fn root() {}\n")
        base = commit(self.repo, "base")
        write(self.repo / "crates/demo/src" / path, source)
        head = commit(self.repo, "head")
        result = self.checker(base, head)
        self.assertEqual(expected, result.returncode, result.stderr)
        return result.stderr

    def test_allows_exact_test_only_functions_methods_and_blocks(self) -> None:
        self.check_source(
            """
#[cfg(test)]
#[allow(dead_code)]
pub(crate) unsafe fn helper(
    value: u32,
) {
    panic!("test helper");
}
struct Demo;
impl Demo {
    #[cfg(test)]
    fn fixture(&self) { todo!(); }
}
#[cfg(test)]
impl Default for Demo {
    fn default() -> Self { unimplemented!(); }
}
fn production() {
    #[cfg(test)]
    if true { panic!(); }
    #[cfg(test)]
    { panic!(); }
}
""",
            0,
        )

    def test_production_panic_after_test_block_on_same_line_is_rejected(self) -> None:
        error = self.check_source(
            '#[cfg(test)] fn helper() { panic!(); } fn bad() { panic!(); }\n',
            1,
        )
        self.assertIn("lib.rs:1: new production panic macro", error)

    def test_test_attribute_does_not_cover_later_production_function(self) -> None:
        self.check_source(
            '#[cfg(test)]\nfn helper() { panic!(); }\nfn bad() { todo!(); }\n',
            1,
        )

    def test_external_test_module_does_not_cover_following_function(self) -> None:
        self.check_source(
            '#[cfg(test)]\nmod tests;\nfn bad() { unimplemented!(); }\n',
            1,
        )

    def test_nonliteral_cfg_expression_is_not_exempted(self) -> None:
        self.check_source(
            '#[cfg(any(test, feature = "production"))]\nfn bad() { panic!(); }\n',
            1,
        )

    def test_unknown_item_does_not_cover_following_production_block(self) -> None:
        self.check_source(
            '#[cfg(test)]\nstruct Fixture;\nfn bad() { panic!(); }\n', 1
        )

    def test_nested_extra_attributes_preserve_exact_test_scope(self) -> None:
        self.check_source(
            '#[cfg(test)]\n#[cfg_attr(feature = "x", allow(dead_code))]\n'
            'fn helper() { panic!(); }\nfn bad() { panic!(); }\n', 1
        )

    def test_array_type_semicolon_does_not_end_test_function_header(self) -> None:
        self.check_source(
            '#[cfg(test)]\nfn helper(value: [u8; 32]) -> [u8; 4] { panic!(); }\n', 0
        )

    def test_array_type_does_not_mask_production_after_external_declaration(self) -> None:
        self.check_source(
            'unsafe extern "C" { #[cfg(test)] fn fixture(value: [u8; 32]); }\n'
            'fn bad() { panic!(); }\n', 1
        )

    def test_closure_in_test_condition_is_not_mistaken_for_statement_body(self) -> None:
        self.check_source(
            'fn production() {\n#[cfg(test)]\n'
            'if [true].iter().any(|value| { *value }) { panic!(); }\n}\n', 0
        )

    def test_production_panic_after_closure_test_condition_is_not_exempted(self) -> None:
        self.check_source(
            'fn production() {\n#[cfg(test)]\n'
            'if [true].iter().any(|value| { *value }) { panic!(); }\npanic!();\n}\n', 1
        )

    def test_fake_attributes_in_comments_and_strings_do_not_mask_panics(self) -> None:
        self.check_source(
            '''
/* #[cfg(test)] */
fn bad() { panic!(); }
const TEXT: &str = r#"#[cfg(test)] fn fake() { panic!(); }"#;
fn also_bad() { todo!(); }
''',
            1,
        )

    def test_inner_test_attribute_exempts_only_test_only_file(self) -> None:
        self.check_source('#![cfg(test)]\nfn helper() { panic!(); }\n', 0)

    def test_nested_test_source_paths_are_exempted(self) -> None:
        write(self.repo / "crates/demo/src/lib.rs", "pub fn root() {}\n")
        base = commit(self.repo, "base")
        for path in ("context/tests/lifecycle.rs", "context/owned_tests/settlement.rs"):
            write(self.repo / "crates/demo/src" / path, "fn fixture() { panic!(); }\n")
        result = self.checker(base, commit(self.repo, "head"))
        self.assertEqual(0, result.returncode, result.stderr)

    def test_similarly_named_production_path_is_not_exempted(self) -> None:
        self.check_source("fn bad() { panic!(); }\n", 1, "context/testing/live.rs")

    def test_size_limit_applies_even_to_test_only_source(self) -> None:
        error = self.check_source(repeated_source(1201), 1, "context/tests/large.rs")
        self.assertIn("new production source file", error)

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
