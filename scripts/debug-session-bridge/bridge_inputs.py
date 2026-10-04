"""Owner-selected file identities; no browser-controlled paths or authentication claims."""
import hashlib
import os
from pathlib import Path
import re
import stat

MIB = 1024 * 1024
HASH_WORK_CAP = 6 * 1024 * MIB


class CustodyError(ValueError):
    """A selected file no longer satisfies its explicit local custody contract."""


def require(condition):
    if not condition:
        raise CustodyError("selected input custody refused")


def metadata(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


class HashBudget:
    def __init__(self):
        self.total = 0

    def charge(self, size):
        require(self.total + size <= HASH_WORK_CAP)
        self.total += size


class FilePin:
    def __init__(self, path, size, sha256, cap, budget, executable=False):
        self.fd = None
        self.budget = budget
        require(type(path) is str and os.path.isabs(path))
        require(len(os.fsencode(path)) <= 4096 and
                not any(ord(c) < 32 or ord(c) == 127 for c in path))
        require(str(Path(path).resolve(strict=True)) == path)
        require(type(size) is int and 0 < size <= cap)
        require(type(sha256) is str and re.fullmatch(r"[0-9a-f]{64}", sha256))
        self.path, self.size, self.sha256 = path, size, sha256
        try:
            self.fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
            info = os.fstat(self.fd)
            require(stat.S_ISREG(info.st_mode) and info.st_size == size)
            require(not executable or os.access(path, os.X_OK))
            self.initial = metadata(info)
            self.check(full=True)
        except BaseException:
            self.close()
            raise

    def check(self, full=False):
        require(self.fd is not None)
        require(str(Path(self.path).resolve(strict=True)) == self.path)
        require(metadata(os.fstat(self.fd)) == self.initial)
        require(metadata(os.stat(self.path, follow_symlinks=False)) == self.initial)
        if full:
            self.budget.charge(self.size + 1)
            digest = hashlib.sha256()
            offset = 0
            while offset < self.size:
                piece = os.pread(self.fd, min(MIB, self.size - offset), offset)
                require(bool(piece))
                digest.update(piece)
                offset += len(piece)
            require(os.pread(self.fd, 1, self.size) == b"")
            require(digest.hexdigest() == self.sha256)
            require(str(Path(self.path).resolve(strict=True)) == self.path)
            require(metadata(os.fstat(self.fd)) == self.initial)
            require(metadata(os.stat(self.path, follow_symlinks=False)) == self.initial)

    def close(self):
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None


class InputPins:
    def __init__(self, specs):
        self.pins = []
        budget = HashBudget()
        try:
            for path, size, digest, cap, executable in specs:
                self.pins.append(FilePin(path, size, digest, cap, budget, executable))
        except BaseException:
            self.close()
            raise

    def check(self, full=False):
        for pin in self.pins:
            pin.check(full)

    def close(self):
        for pin in self.pins:
            pin.close()


class TokenFile:
    """A pre-created owner-only secret, never printed or returned to a browser."""
    def __init__(self, path):
        self.fd = None
        require(type(path) is str and os.path.isabs(path) and
                str(Path(path).resolve(strict=True)) == path)
        require(len(os.fsencode(path)) <= 4096 and
                not any(ord(c) < 32 or ord(c) == 127 for c in path))
        try:
            self.fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
            info = os.fstat(self.fd)
            require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid() and
                    stat.S_IMODE(info.st_mode) == 0o600 and info.st_nlink == 1 and
                    info.st_size in (64, 65))
            self.path, self.initial = path, metadata(info)
            raw = os.pread(self.fd, 66, 0)
            require(re.fullmatch(rb"[0-9a-f]{64}\n?", raw))
            require(raw[:64] != b"0" * 64)
            self.value = raw[:64].decode("ascii")
            self.raw = raw
            self.check()
        except BaseException:
            self.close()
            raise

    def check(self):
        require(self.fd is not None and str(Path(self.path).resolve(strict=True)) == self.path)
        require(metadata(os.fstat(self.fd)) == self.initial)
        require(metadata(os.stat(self.path, follow_symlinks=False)) == self.initial)
        require(os.pread(self.fd, 66, 0) == self.raw)

    def close(self):
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None
