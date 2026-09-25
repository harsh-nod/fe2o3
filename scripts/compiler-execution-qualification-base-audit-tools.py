#!/usr/bin/env python3
"""Build diagnostic for audit-installed's command/ELF closure, not runtime authority.

Requires Linux host Python 3/readelf, plus unsquashfs for --image. Inspects pinned
build inputs without executing them; not an adversarial filesystem admission API.
"""
import argparse
import os
from pathlib import Path
import posixpath
import re
import stat
import subprocess
import unittest
from unittest import mock


# Includes the inventory-failure diagnostics, not just the successful path.
COMMANDS = "awk bash cat cmp diff dirname find mawk mktemp od readlink rm sha256sum sort stat tail tr wc".split()
LOADER = "/lib64/ld-linux-x86-64.so.2"
LINKS = {
    "/bin": "usr/bin",
    "/lib64": "usr/lib64",
    "/usr/lib64/ld-linux-x86-64.so.2": "../lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
    "/usr/bin/awk": "mawk",
}
LIBRARY_DIRS = ("/lib/x86_64-linux-gnu", "/usr/lib/x86_64-linux-gnu", "/lib", "/usr/lib")
MAX_BYTES = 16 * 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def command(argv, pass_fds=()):
    result = subprocess.run(argv, check=True, capture_output=True, timeout=30,
                            env={**os.environ, "LC_ALL": "C"}, pass_fds=pass_fds)
    # This is a post-capture size check, not a process-output allocation limit.
    require(len(result.stdout) <= MAX_BYTES, "inspection output exceeds bound")
    return result.stdout


class Tree:
    def resolve(self, path):
        # Resolve every link against the image root, never the host filesystem.
        for _ in range(40):
            parts = path.strip("/").split("/")
            for index in range(len(parts)):
                prefix = "/" + "/".join(parts[:index + 1])
                entry = self.entry(prefix)
                if entry is None:
                    return None
                mode, _, link = entry
                if stat.S_ISLNK(mode):
                    path = posixpath.normpath(posixpath.join(
                        posixpath.dirname(prefix), link, *parts[index + 1:]))
                    break
                if index < len(parts) - 1:
                    require(stat.S_ISDIR(mode) and mode & 0o555 == 0o555,
                            "inaccessible image ancestor: " + prefix)
            else:
                return path
        raise ValueError("image symlink cycle: " + path)

    def read(self, path):
        mode, size, _ = self.entry(path)
        require(stat.S_ISREG(mode) and 0 < size <= MAX_BYTES,
                "invalid ELF file type/size: " + path)
        data = self.contents(path)
        require(len(data) == size, "ELF size changed: " + path)
        return data


class RootTree(Tree):
    def __init__(self, root):
        self.root = Path(root)
        require(self.root.is_dir() and not self.root.is_symlink(), "expected extracted root directory")

    def entry(self, path):
        path = self.root / path.lstrip("/")
        try:
            info = path.lstat()
        except FileNotFoundError:
            return None
        return info.st_mode, info.st_size, os.readlink(path) if stat.S_ISLNK(info.st_mode) else None

    def contents(self, path):
        return (self.root / path.lstrip("/")).read_bytes()


class ImageTree(Tree):
    def __init__(self, image):
        self.image = str(image)
        self.entries = {}
        listing = command(["unsquashfs", "-lln", "-processors", "1", "-no-progress", self.image])
        for line in listing.decode("utf-8").splitlines():
            metadata, separator, name = line.partition(" squashfs-root")
            if not separator:
                continue
            fields = metadata.split()
            permissions = fields[0]
            require(len(permissions) == 10 and permissions[0] in "-dl", "unsupported image entry")
            mode = {"-": stat.S_IFREG, "d": stat.S_IFDIR, "l": stat.S_IFLNK}[permissions[0]]
            for index, expected in enumerate("rwxrwxrwx"):
                actual = permissions[index + 1]
                if actual == expected or (expected == "x" and actual in "st"):
                    mode |= 1 << (8 - index)
            path, separator, target = name.partition(" -> ")
            path = path or "/"
            require(path not in self.entries, "duplicate image path")
            self.entries[path] = mode, int(fields[2]), target if separator else None
        require("/" in self.entries, "missing image root")

    def entry(self, path):
        return self.entries.get(path)

    def contents(self, path):
        return command(["unsquashfs", "-cat", "-processors", "1", "-no-progress",
                        self.image, path.lstrip("/")])


def elf_requirements(data):
    require(data[:6] == b"\x7fELF\x02\x01" and int.from_bytes(data[18:20], "little") == 62,
            "expected little-endian x86-64 ELF")
    fd = os.memfd_create("qualification-base-elf-inspection", os.MFD_CLOEXEC)
    try:
        require(os.write(fd, data) == len(data), "short ELF inspection copy")
        output = command(["readelf", "--wide", "--program-headers", "--dynamic",
                          f"/proc/self/fd/{fd}"], pass_fds=(fd,)).decode("ascii")
    finally:
        os.close(fd)
    return readelf_requirements(output)


def readelf_requirements(output):
    require("Dynamic section at offset" in output, "missing ELF dynamic section")
    require("(RPATH)" not in output and "(RUNPATH)" not in output, "unexpected ELF search path")
    interpreter = re.search(r"Requesting program interpreter: ([^\]]+)", output)
    needed = re.findall(r"\(NEEDED\).*?\[([^\]]+)\]", output)
    require(all(re.fullmatch(r"[A-Za-z0-9_+.-]+", name) for name in needed), "invalid ELF dependency name")
    return interpreter.group(1) if interpreter else None, needed


def check(tree, inspect=elf_requirements):
    for path, target in LINKS.items():
        entry = tree.entry(path)
        require(entry is not None and stat.S_ISLNK(entry[0]) and entry[2] == target,
                "audit link differs: " + path)
    pending = []
    for name in COMMANDS:
        path = tree.resolve("/usr/bin/" + name)
        require(path is not None, "missing audit command: " + name)
        mode = tree.entry(path)[0]
        require(stat.S_ISREG(mode) and mode & 0o555 == 0o555, "audit command is not executable: " + name)
        pending.append((path, True))
    seen = set()
    while pending:
        path, executable = pending.pop()
        if path in seen:
            continue
        seen.add(path)
        require(len(seen) <= 64, "audit ELF closure exceeds bound")
        interpreter, needed = inspect(tree.read(path))
        require(not executable or interpreter == LOADER, "audit command interpreter differs: " + path)
        if interpreter:
            require(interpreter == LOADER, "unexpected ELF interpreter: " + interpreter)
            loader = tree.resolve(interpreter)
            require(loader is not None and tree.entry(loader)[0] & 0o555 == 0o555, "missing executable loader")
            pending.append((loader, False))
        for name in needed:
            library = next((resolved for directory in LIBRARY_DIRS
                            if (resolved := tree.resolve(directory + "/" + name))), None)
            require(library is not None, "missing ELF dependency: " + name + " required by " + path)
            require(tree.entry(library)[0] & 0o444 == 0o444, "unreadable ELF dependency: " + library)
            pending.append((library, False))
    return len(seen)


class ContractTests(unittest.TestCase):
    def setUp(self):
        class Fixture(Tree):
            def entry(self, path):
                return self.entries.get(path)

            def contents(self, path):
                return path.encode("ascii")

        self.tree = Fixture()
        self.tree.entries = {}
        for path in ("/usr", "/usr/bin", "/usr/lib", "/usr/lib64", "/usr/lib/x86_64-linux-gnu"):
            self.tree.entries[path] = stat.S_IFDIR | 0o755, 0, None
        for name in COMMANDS:
            path = "/usr/bin/" + name
            self.tree.entries[path] = stat.S_IFREG | 0o755, len(path), None
        for name in ("ld-linux-x86-64.so.2", "libc.so.6", "libcrypto.so.3", "libselinux.so.1", "libpcre2-8.so.0"):
            path = "/usr/lib/x86_64-linux-gnu/" + name
            self.tree.entries[path] = stat.S_IFREG | 0o755, len(path), None
        for path, target in LINKS.items():
            self.tree.entries[path] = stat.S_IFLNK | 0o777, len(target), target

    @staticmethod
    def inspect(data):
        path = data.decode("ascii")
        if path.startswith("/usr/bin/"):
            needed = {"sort": "libcrypto.so.3", "sha256sum": "libcrypto.so.3", "stat": "libselinux.so.1"}
            return LOADER, [needed.get(posixpath.basename(path), "libc.so.6")]
        return None, ["libpcre2-8.so.0"] if path.endswith("libselinux.so.1") else []

    def test_complete_closure(self):
        self.assertGreater(check(self.tree, self.inspect), len(COMMANDS))

    def test_each_missing_command(self):
        for name in COMMANDS:
            with self.subTest(command=name):
                path = "/usr/bin/" + name
                entry = self.tree.entries.pop(path)
                with self.assertRaises(ValueError):
                    check(self.tree, self.inspect)
                self.tree.entries[path] = entry

    def test_wrong_awk_link(self):
        self.tree.entries["/usr/bin/awk"] = stat.S_IFLNK | 0o777, 4, "bash"
        with self.assertRaisesRegex(ValueError, "audit link differs: /usr/bin/awk"):
            check(self.tree, self.inspect)

    def test_nonexecutable_command(self):
        path = "/usr/bin/find"
        self.tree.entries[path] = stat.S_IFREG | 0o644, len(path), None
        with self.assertRaisesRegex(ValueError, "not executable: find"):
            check(self.tree, self.inspect)

    def test_missing_direct_and_transitive_dependencies(self):
        for name in ("libcrypto.so.3", "libpcre2-8.so.0", "ld-linux-x86-64.so.2"):
            with self.subTest(library=name):
                path = "/usr/lib/x86_64-linux-gnu/" + name
                entry = self.tree.entries.pop(path)
                with self.assertRaisesRegex(ValueError, "missing"):
                    check(self.tree, self.inspect)
                self.tree.entries[path] = entry

    def test_no_host_fallback_or_symlink_cycle(self):
        path = "/usr/lib/x86_64-linux-gnu/libcrypto.so.3"
        for target in ("/host/libcrypto.so.3", "libcrypto.so.3"):
            with self.subTest(target=target):
                self.tree.entries[path] = stat.S_IFLNK | 0o777, len(target), target
                with self.assertRaises(ValueError):
                    check(self.tree, self.inspect)

    def test_real_elf_reader(self):
        interpreter, needed = elf_requirements(Path("/usr/bin/true").read_bytes())
        self.assertEqual(interpreter, LOADER)
        self.assertIn("libc.so.6", needed)
        with self.assertRaisesRegex(ValueError, "expected.*ELF"):
            elf_requirements(b"#!/bin/bash\n")

    def test_readelf_output_parser(self):
        output = """  [Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]
Dynamic section at offset 0x1000 contains 3 entries:
  0x0000000000000001 (NEEDED)             Shared library: [libcrypto.so.3]
  0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
  0x0000000000000000 (NULL)               0x0
"""
        self.assertEqual(readelf_requirements(output), (LOADER, ["libcrypto.so.3", "libc.so.6"]))
        library_output = output.split("\n", 1)[1]
        self.assertEqual(readelf_requirements(library_output)[0], None)
        for invalid in ("There is no dynamic section in this file.",
                        output + "  0x1d (RUNPATH) Library runpath: [/host/lib]\n",
                        output + "  0x0f (RPATH) Library rpath: [/host/lib]\n",
                        output.replace("libcrypto.so.3", "../host/libcrypto.so.3")):
            with self.subTest(output=invalid), self.assertRaises(ValueError):
                readelf_requirements(invalid)

    def test_image_listing_parser(self):
        listing = b"""drwxr-xr-x 0/0 199 2026-09-25 08:38 squashfs-root
lrwxrwxrwx 0/0 7 2026-09-25 08:38 squashfs-root/bin -> usr/bin
drwxr-xr-x 0/0 153 2026-09-25 08:38 squashfs-root/usr
drwxr-xr-x 0/0 3745 2026-09-25 08:38 squashfs-root/usr/bin
-rwxr-xr-x 0/0 1446024 2026-09-25 08:38 squashfs-root/usr/bin/bash
-rw-r--r-- 0/0 123456 2026-09-25 08:38 squashfs-root/usr/bin/mawk
lrwxrwxrwx 0/0 4 2026-09-25 08:38 squashfs-root/usr/bin/awk -> mawk
"""
        with mock.patch(__name__ + ".command", return_value=listing) as run:
            tree = ImageTree("fixture.squashfs")
        self.assertEqual(run.call_args.args[0],
                         ["unsquashfs", "-lln", "-processors", "1", "-no-progress", "fixture.squashfs"])
        self.assertEqual(tree.resolve("/bin/bash"), "/usr/bin/bash")
        self.assertEqual(tree.resolve("/bin/awk"), "/usr/bin/mawk")
        self.assertEqual(tree.entry("/usr/bin/bash"), (stat.S_IFREG | 0o755, 1446024, None))
        self.assertEqual(tree.entry("/usr/bin/mawk")[0], stat.S_IFREG | 0o644)
        self.assertIsNone(tree.resolve("/bin/find"))
        with mock.patch(__name__ + ".command", return_value=listing + listing):
            with self.assertRaisesRegex(ValueError, "duplicate image path"):
                ImageTree("duplicate.squashfs")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--root")
    mode.add_argument("--image")
    mode.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        unittest.main(argv=[__file__])
    else:
        try:
            count = check(RootTree(args.root) if args.root else ImageTree(args.image))
        except (ValueError, OSError, subprocess.SubprocessError) as error:
            parser.exit(1, f"qualification-base audit tools: {error}\n")
        print(f"qualification-base audit command closure: {len(COMMANDS)} commands, {count} ELF objects")
