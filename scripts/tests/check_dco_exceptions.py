"""Exercise approved historical identities without forging Git objects."""

import contextlib
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "dco_checker", Path(__file__).resolve().parents[1] / "check-dco-range.py"
)
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)
APPROVED = (
    "3abb7b18ebe5e3cbc32002738bed0f7d72d4f745",
    "5ed3840a90db3f03a2cded9becffc0459b737f36",
)
UNSIGNED = "a" * 40
SIGNED = "b" * 40


class InheritedExceptions(unittest.TestCase):
    def run_check(self, commits, repo, signed=()):
        def identity(commit):
            body = "Signed-off-by: Test User <test@example.com>" if commit in signed else "unsigned"
            return "Test User", "test@example.com", body

        output = io.StringIO()
        with patch.object(CHECKER, "commits_in_range", return_value=list(commits)), patch.object(
            CHECKER, "commit_identity", side_effect=identity
        ), patch.object(CHECKER, "verified_dependabot", return_value=False), contextlib.redirect_stdout(output):
            count = CHECKER.check("c" * 40, "d" * 40, repo)
        self.assertEqual(count, len(commits))
        return output.getvalue()

    def test_exact_approved_commits_in_both_repositories(self):
        for repo in ("harsh-nod/fe2o3", "powderluv/fe2o3"):
            for commits in ((APPROVED[0],), (APPROVED[1],), APPROVED):
                with self.subTest(repo=repo, commits=commits):
                    output = self.run_check(commits, repo)
                    self.assertIn(f"0 commit(s); {len(commits)} approved inherited exception(s)", output)
                    for commit in commits:
                        self.assertIn(f"{repo}@{commit}", output)

    def test_other_repositories_and_lookalikes_reject(self):
        for repo in ("owner/repository", "harsh-nod/fe2o3-kernels", "powderluv/fe2o3-other", "other/fe2o3"):
            for commit in APPROVED:
                with self.subTest(repo=repo, commit=commit), self.assertRaisesRegex(CHECKER.DcoError, commit):
                    self.run_check((commit,), repo)

    def test_changed_identity_and_third_unsigned_commit_reject(self):
        for repo in ("harsh-nod/fe2o3", "powderluv/fe2o3"):
            for commit in (UNSIGNED, "0" + APPROVED[0][1:], "0" + APPROVED[1][1:]):
                with self.subTest(repo=repo, commit=commit), self.assertRaisesRegex(CHECKER.DcoError, commit):
                    self.run_check((*APPROVED, commit), repo)

    def test_signed_and_excepted_counts_are_distinct(self):
        output = self.run_check((*APPROVED, SIGNED), "harsh-nod/fe2o3", (SIGNED,))
        self.assertIn("1 commit(s); 2 approved inherited exception(s)", output)
        self.assertEqual(output.count("DCO approved inherited exception:"), 2)

    def test_normal_signoff_takes_precedence(self):
        output = self.run_check(APPROVED, "powderluv/fe2o3", APPROVED)
        self.assertIn("2 commit(s); 0 approved inherited exception(s)", output)
        self.assertNotIn("DCO approved inherited exception:", output)


if __name__ == "__main__":
    unittest.main()
