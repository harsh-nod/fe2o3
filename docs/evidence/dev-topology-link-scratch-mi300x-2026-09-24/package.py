#!/usr/bin/env python3
"""Package owned records without build caches or duplicate extracted source trees."""
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import tarfile

HERE = Path(__file__).resolve().parent
DEST = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution/docs/evidence/dev-topology-link-scratch-mi300x-2026-09-24')

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def main():
    index = {}
    for name in ('native1', 'native2', 'protocol8'):
        root = HERE / name
        files = {}
        for path in sorted(root.rglob('*')):
            relative = path.relative_to(root)
            if relative.parts[0] in ('source-baseline', 'source-candidate', 'target', 'target-baseline', 'target-candidate'):
                continue
            if path.is_symlink():
                raise RuntimeError('symlink in owned records')
            if path.is_file():
                files[relative.as_posix()] = digest(path)
        target = DEST / (name + '.tar.gz')
        with target.open('xb') as stream, gzip.GzipFile(fileobj=stream, mode='wb', filename='', mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode='w|') as archive:
                for relative in files:
                    path = root / relative
                    info = archive.gettarinfo(path, arcname=relative)
                    info.uid = info.gid = info.mtime = 0
                    info.uname = info.gname = ''
                    with path.open('rb') as source:
                        archive.addfile(info, source)
                    if digest(path) != files[relative]:
                        raise RuntimeError('records changed while packaging')
        index[name] = {'archive_sha256': digest(target), 'files': files}
    (DEST / 'archive-index.json').write_text(json.dumps(index, sort_keys=True, indent=2) + '\n')
    for name in ('campaign.py', 'native.py', 'test_campaign.py', 'test_cpu_binding.py', 'validate_protocol.py', 'package.py'):
        shutil.copy2(HERE / name, DEST / name)
    print(json.dumps({name: len(row['files']) for name, row in index.items()}, sort_keys=True))

if __name__ == '__main__':
    main()
