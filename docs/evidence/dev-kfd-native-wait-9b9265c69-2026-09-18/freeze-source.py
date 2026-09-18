#!/usr/bin/env python3
"""Derive exact regular-file identities from the committed source archive."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import tarfile

root = Path(__file__).resolve().parent
rows = {}
with tarfile.open(root / "source.tar") as archive:
    for member in archive.getmembers():
        path = PurePosixPath(member.name)
        assert not path.is_absolute() and ".." not in path.parts
        if member.isdir():
            continue
        assert member.isfile() and member.name not in rows
        data = archive.extractfile(member).read()
        rows[member.name] = hashlib.sha256(data).hexdigest()
manifest = "".join(f"{value}  {name}\n" for name, value in sorted(rows.items()))
(root / "source-files.sha256").write_text(manifest)
print(json.dumps({"commit": "9b9265c6919cb8dff9506f2c6ffa7b7f2538905f", "files": len(rows), "tar_sha256": hashlib.sha256((root / "source.tar").read_bytes()).hexdigest(), "manifest_sha256": hashlib.sha256(manifest.encode()).hexdigest()}, indent=2))
