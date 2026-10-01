#!/usr/bin/env python3
"""Read evidence without extracting it; report locations, never secret values."""
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import sys
import tarfile

SENSITIVE = re.compile(r'(TOKEN|SECRET|PASSWORD|CREDENTIAL|API_KEY|CUSTOM_HEADERS|AUTHORIZATION)', re.I)
JSON_FIELD = re.compile(rb'"((?:[A-Za-z_][A-Za-z0-9_]*)?(?:TOKEN|SECRET|PASSWORD|CREDENTIAL|API_KEY|CUSTOM_HEADERS|AUTHORIZATION)[A-Za-z0-9_]*)"\s*:\s*("(?:[^"\\]|\\.)*")', re.I)


def needles():
    result = {}
    for key, value in os.environ.items():
        if SENSITIVE.search(key) and len(value) >= 12 and value.lower() not in ('not-a-real-key',):
            variants = {value.encode(), json.dumps(value)[1:-1].encode()}
            if 'HEADER' in key:
                for line in value.splitlines():
                    if ':' in line:
                        part = line.split(':', 1)[1].strip()
                        if len(part) >= 12:
                            variants.update((part.encode(), json.dumps(part)[1:-1].encode()))
            for variant in variants:
                result.setdefault(variant, set()).add(key)
    return result


def scan(paths, output):
    patterns = needles()
    if not patterns:
        raise ValueError('expected local sensitive-value match controls')
    hits = []
    stats = {'files': 0, 'archive_members': 0, 'bytes': 0}
    errors = []
    overlap = max(max(map(len, patterns)), 8192)

    def consume(stream, location):
        tail = b''
        found = set()
        suspicious = set()
        while chunk := stream.read(1024 * 1024):
            stats['bytes'] += len(chunk)
            data = tail + chunk
            for needle, keys in patterns.items():
                if needle in data:
                    found.update(keys)
            for match in JSON_FIELD.finditer(data):
                value = json.loads(match[2])
                if len(value) >= 12 and not any(word in value.lower() for word in ('redacted', 'placeholder', 'dummy', 'not-a-real', '${')):
                    suspicious.add(match[1].decode())
            tail = data[-overlap:]
        if found or suspicious:
            hits.append({'location': location, 'matching_environment_keys': sorted(found),
                         'nonempty_sensitive_json_keys': sorted(suspicious)})

    def archive(stream, location, depth):
        if depth > 4:
            raise ValueError('nested archive depth exceeded')
        with tarfile.open(fileobj=stream, mode='r|*') as tar:
            for member in tar:
                if not member.isfile():
                    continue
                stats['archive_members'] += 1
                name = location + '::' + member.name
                child = tar.extractfile(member)
                if member.name.endswith(('.tar.gz', '.tgz', '.tar')):
                    archive(child, name, depth + 1)
                elif member.name.endswith('.gz'):
                    with gzip.GzipFile(fileobj=child) as decoded:
                        consume(decoded, name)
                else:
                    consume(child, name)

    for root in paths:
        if not root.exists() or root.is_symlink() or not (root.is_file() or root.is_dir()):
            errors.append({'path': str(root), 'error_type': 'InvalidRequestedRoot'})
            continue
        entries = sorted(root.rglob('*')) if root.is_dir() else [root]
        for path in entries:
            if path.is_symlink():
                errors.append({'path': str(path), 'error_type': 'UnscannedSymlink'})
                continue
            if not path.is_file():
                continue
            stats['files'] += 1
            try:
                with path.open('rb') as stream:
                    if path.name.endswith(('.tar.gz', '.tgz', '.tar')):
                        archive(stream, str(path), 0)
                    elif path.name.endswith('.gz'):
                        with gzip.GzipFile(fileobj=stream) as decoded:
                            consume(decoded, str(path))
                    else:
                        consume(stream, str(path))
            except Exception as error:
                errors.append({'path': str(path), 'error_type': type(error).__name__})
    result = {'scope': [str(path) for path in paths], 'stats': stats, 'hits': hits,
              'errors': errors, 'accepted': not hits and not errors,
              'limitations': ['Known current environment values and JSON credential fields only.',
                              'JSON keys must be literal ASCII; very long split values may exceed overlap.',
                              'Git pack/bundle compression is not decoded by this scanner.'],
              'scanner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    with output.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, indent=2, sort_keys=True)
        stream.write('\n')
    os.chmod(output, 0o600)
    print(json.dumps({'report': str(output), 'stats': stats, 'hit_locations': len(hits),
                      'errors': errors, 'accepted': result['accepted']}), flush=True)
    return 0 if result['accepted'] else 1


if __name__ == '__main__':
    raise SystemExit(scan([Path(value) for value in sys.argv[2:]], Path(sys.argv[1])))
