#!/usr/bin/env python3
"""Compare the fixed Git export, Git blob identities, and remote build sources."""
import hashlib
import json
from pathlib import Path
import re
import tarfile

root = Path(__file__).resolve().parent
commit = "5d70cb0a6e16fb265fe224690274fdb0be2b0055"
expected = {}
for line in (root / "source-tree.txt").read_text().splitlines():
    match = re.fullmatch(r"(100644|100755) blob ([0-9a-f]{40})\t(.+)", line)
    assert match, line
    mode, blob, name = match.groups()
    assert name not in expected
    expected[name] = (mode, blob)
actual = {}
with tarfile.open(root / "source.tar", "r:") as archive:
    assert archive.pax_headers["comment"] == commit
    for entry in archive:
        if entry.isdir():
            continue
        assert entry.isfile() and entry.name not in actual
        mode, blob = expected[entry.name]
        data = archive.extractfile(entry).read()
        assert len(data) == entry.size
        assert hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest() == blob
        assert bool(entry.mode & 0o111) == (mode == "100755")
        actual[entry.name] = hashlib.sha256(data).hexdigest()
assert set(actual) == set(expected) and len(actual) == 5538
for name in ("source-files.sha256", "source-build-after.sha256"):
    observed = {}
    for line in (root / "results-before" / name).read_text().splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  \./(.+)", line)
        assert match and match[2] not in observed
        observed[match[2]] = match[1]
    assert actual == observed, name
print(json.dumps({"commit": commit, "source_files": len(actual), "git_blob_identities": True, "remote_build_source_equal": True}, indent=2))
