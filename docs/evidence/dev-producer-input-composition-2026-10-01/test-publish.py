#!/usr/bin/env python3
"""Small generated fixtures only; never load qualification components or proofs."""
import copy
import hashlib
import io
from pathlib import Path
import subprocess
import tarfile
import tempfile
import types
import unittest

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / "publish_composition_v2.py"
if not SOURCE.exists():
    SOURCE = ROOT / "publish.py"
p = types.ModuleType("publication_controls")
p.__file__ = str(SOURCE)
exec(compile(SOURCE.read_bytes(), str(SOURCE), "exec"), p.__dict__)


class PublicationControls(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="fe2o3-publication-controls-")
        self.root = Path(self.tmp.name).resolve()
        self.addCleanup(self.tmp.cleanup)
        self.first = self.root / "first"
        self.first.write_bytes(b"exact source bytes\n")
        self.second = self.root / "second"
        self.second.write_bytes(self.first.read_bytes())
        self.entries = p.inventory({"a/source": self.first, "b/duplicate": self.second})

    def path(self, name="archive.tar.xz"):
        return self.root / name

    def reject(self, fn, *args):
        with self.assertRaises(ValueError):
            fn(*args)

    def custom_archive(self, members):
        path = self.path()
        with tarfile.open(path, "x:xz", format=tarfile.PAX_FORMAT) as archive:
            for info, content in members:
                archive.addfile(info, io.BytesIO(content) if content is not None else None)
        return path

    def info(self, name, link=None):
        row = self.entries[name]
        info = tarfile.TarInfo(name)
        info.mode = row["mode"]
        if link is None:
            info.size = row["bytes"]
        else:
            info.type, info.linkname = tarfile.LNKTYPE, link
        return info

    def test_inventory_sorted_and_deduplicated(self):
        self.assertEqual(list(self.entries), ["a/source", "b/duplicate"])
        self.assertIsNone(self.entries["a/source"]["hardlink"])
        self.assertEqual(self.entries["b/duplicate"]["hardlink"], "a/source")
        self.assertEqual(self.entries["a/source"]["sha256"], hashlib.sha256(self.first.read_bytes()).hexdigest())

    def test_executable_mode_is_not_collapsed(self):
        self.second.chmod(0o755)
        entries = p.inventory({"a/source": self.first, "b/executable": self.second})
        self.assertEqual(entries["b/executable"]["mode"], 0o755)
        self.assertIsNone(entries["b/executable"]["hardlink"])
        p.create_archive(self.path(), entries)
        p.verify_archive(self.path(), entries)

    def test_archive_paths_fail_closed(self):
        for name in ("", ".", "../x", "a/../x", "/absolute", "a//b", "a/./b", "a/", "a\0b", "caf\u00e9", None):
            with self.subTest(name=name):
                self.reject(p.archive_name, name)

    def test_symlink_sources_fail_closed(self):
        link = self.root / "link"
        link.symlink_to(self.first)
        self.reject(p.inventory, {"a/source": link})
        self.reject(p.tree, self.root)

    def test_roundtrip_hardlinks_long_pax_paths_and_exact_bytes(self):
        name = "long/" + "x" * 180 + "/source"
        entries = p.inventory({name: self.first, "z/duplicate": self.second})
        p.create_archive(self.path(), entries)
        p.verify_archive(self.path(), entries)
        self.reject(p.create_archive, self.path(), entries)

    def test_archive_is_deterministic(self):
        p.create_archive(self.path("a.tar.xz"), self.entries)
        p.create_archive(self.path("b.tar.xz"), self.entries)
        self.assertEqual(self.path("a.tar.xz").read_bytes(), self.path("b.tar.xz").read_bytes())

    def test_wrong_hash_and_mode_are_rejected(self):
        p.create_archive(self.path(), self.entries)
        for field, value in (("sha256", "0" * 64), ("bytes", 1), ("mode", 0o755)):
            modified = copy.deepcopy(self.entries)
            modified["a/source"][field] = value
            with self.subTest(field=field):
                self.reject(p.verify_archive, self.path(), modified)

    def test_missing_extra_and_reordered_members_are_rejected(self):
        p.create_archive(self.path(), self.entries)
        self.reject(p.verify_archive, self.path(), {"a/source": self.entries["a/source"]})
        extra = self.entries | {"c/extra": self.entries["a/source"]}
        self.reject(p.verify_archive, self.path(), extra)
        reversed_entries = dict(reversed(list(self.entries.items())))
        self.reject(p.verify_archive, self.path(), reversed_entries)

    def test_wrong_link_and_forward_link_are_rejected(self):
        for target in ("/host/file", "../outside", "b/duplicate"):
            members = [(self.info("a/source"), self.first.read_bytes()),
                       (self.info("b/duplicate", target), None)]
            path = self.custom_archive(members)
            self.reject(p.verify_archive, path, self.entries)
            path.unlink()
        path = self.custom_archive([(self.info("b/duplicate", "a/source"), None),
                                    (self.info("a/source"), self.first.read_bytes())])
        self.reject(p.verify_archive, path, self.entries)

    def test_duplicate_and_symlink_members_are_rejected(self):
        info = self.info("a/source")
        path = self.custom_archive([(info, self.first.read_bytes()), (info, self.first.read_bytes())])
        self.reject(p.verify_archive, path, self.entries)
        path.unlink()
        link = self.info("b/duplicate", "a/source")
        link.type = tarfile.SYMTYPE
        path = self.custom_archive([(info, self.first.read_bytes()), (link, None)])
        self.reject(p.verify_archive, path, self.entries)

    def test_nonnormalized_metadata_is_rejected(self):
        for field, value in (("uid", 1), ("mtime", 1), ("uname", "owner"), ("pax_headers", {"comment": "extra"})):
            info = self.info("a/source")
            setattr(info, field, value)
            path = self.custom_archive([(info, self.first.read_bytes()),
                                        (self.info("b/duplicate", "a/source"), None)])
            with self.subTest(field=field):
                self.reject(p.verify_archive, path, self.entries)
            path.unlink()

    def bundle(self, version=2, prerequisite=None, head=None, capabilities=""):
        path = self.path("fixture.bundle")
        path.write_bytes((f"# v{version} git bundle\n" + capabilities +
                          "-" + (prerequisite or p.COMMON) + " public ancestor\n" +
                          (head or p.COMMITS[-1]) + " HEAD\n\nPACK").encode())
        return path

    def test_bundle_header_exact_ancestry(self):
        self.assertEqual(p.bundle_header(self.bundle())["prerequisite"], p.COMMON)
        self.assertEqual(p.bundle_header(self.bundle(3, capabilities="@object-format=sha1\n"))["heads"],
                         [[p.COMMITS[-1], "HEAD"]])

    def test_bundle_header_wrong_ancestry_and_capabilities_rejected(self):
        for kwargs in ({"prerequisite": p.COMMITS[0]}, {"head": p.COMMITS[0]}, {"version": 4},
                       {"version": 3}, {"capabilities": "@filter=blob:none\n"},
                       {"version": 3, "capabilities": "@object-format=sha256\n"}):
            with self.subTest(kwargs=kwargs):
                self.reject(p.bundle_header, self.bundle(**kwargs))

    def test_incomplete_or_oversized_bundle_header_rejected(self):
        path = self.path("fixture.bundle")
        for content in (b"", b"# v2 git bundle\n", b"x" * 4096, b"# v2 git bundle\n" + b"-x\n" * 6000):
            path.write_bytes(content)
            self.reject(p.bundle_header, path)

    def test_bundle_object_helper_uses_exact_data_keyword(self):
        calls = []
        def git(*args, data=None):
            calls.append((args, data))
            if args[0] == "rev-list":
                return ("1" * 40 + " path\n" + "2" * 40 + "\n").encode()
            return ("1" * 40 + " blob 7\n" + "2" * 40 + " commit 9\n").encode()
        self.assertEqual(p.bundle_objects(git), (2, 16))
        self.assertEqual(calls, [(("rev-list", "--objects", p.COMMON + ".." + p.COMMITS[-1]), None),
            (("cat-file", "--batch-check=%(objectname) %(objecttype) %(objectsize)"),
             ("1" * 40 + "\n" + "2" * 40 + "\n").encode())])

    def test_bundle_object_bound_and_complete_identity(self):
        for row in ("", "2" * 40 + " blob 1\n", "1" * 40 + " tag 1\n",
                    "1" * 40 + " blob -1\n", "1" * 40 + " blob 33554433\n",
                    ("1" * 40 + " blob 1\n") * 2):
            def git(*args, data=None):
                return (("1" * 40 + "\n") if args[0] == "rev-list" else row).encode()
            with self.subTest(row=row):
                self.reject(p.bundle_objects, git)

    def test_exact_head_and_source_drift_controls(self):
        for head, dirty, accepted in ((p.COMMITS[-1], b"", True),
                                      (p.COMMITS[0], b"", False),
                                      (p.COMMITS[-1], b" M source.rs\n", False)):
            calls = []
            def git(*args, data=None):
                calls.append(args)
                return ((head + "\n").encode() if args[0] == "rev-parse" else dirty)
            with self.subTest(head=head, dirty=dirty):
                if accepted:
                    p.bundle_head(git)
                    self.assertEqual(calls, [("rev-parse", "HEAD"),
                        ("status", "--porcelain=v1", "--untracked-files=all")])
                else:
                    self.reject(p.bundle_head, git)

    def test_named_head_bundle_with_two_actual_temporary_git_commits(self):
        repo = self.root / "git-fixture"
        repo.mkdir()
        env = {"HOME": str(self.root), "PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C",
               "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
        def git(*args, data=None):
            command = ["/usr/bin/git", "--no-replace-objects", "--no-pager", "-c", "gc.auto=0",
                       "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "-c", "user.name=Fixture",
                       "-c", "user.email=fixture@example.invalid", *args]
            return subprocess.run(command, cwd=repo, env=env, input=data, check=True,
                                  capture_output=True, timeout=15).stdout
        git("init", "--quiet")
        git("commit", "--allow-empty", "--quiet", "-m", "Public ancestor fixture")
        common = git("rev-parse", "HEAD").decode().strip()
        commits = []
        for message in ("First side commit fixture", "Second side commit fixture"):
            git("commit", "--allow-empty", "--quiet", "-m", message)
            commits.append(git("rev-parse", "HEAD").decode().strip())
        self.addCleanup(setattr, p, "COMMON", p.COMMON)
        self.addCleanup(setattr, p, "COMMITS", p.COMMITS)
        p.COMMON, p.COMMITS = common, commits
        git("checkout", "--quiet", "--detach", commits[-1])
        p.bundle_head(git)
        self.assertEqual(git("rev-list", "--reverse", common + "..HEAD").decode().splitlines(), commits)
        bundle = self.path("real.bundle")
        git("bundle", "create", str(bundle), "HEAD", "^" + common)
        git("bundle", "verify", str(bundle))
        self.assertEqual(git("bundle", "list-heads", str(bundle)).decode(), commits[-1] + " HEAD\n")
        self.assertEqual(p.bundle_header(bundle)["heads"], [[commits[-1], "HEAD"]])
        self.assertGreater(p.bundle_objects(git)[1], 0)
        p.bundle_head(git)

    def test_scope_and_prior_attempts_remain_explicit(self):
        for text in ("NOT self-contained full replay", "33-stage calibration remains incomplete",
                     "All 81 qualified negatives are fresh", "not a fresh", "Concrete journal forwarding",
                     "BOTH unpublished signed side commits", "HIP/HSA parity remain outside"):
            self.assertIn(text, p.README)


if __name__ == "__main__":
    unittest.main(verbosity=2)
