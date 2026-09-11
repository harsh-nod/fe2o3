"""Check local Markdown references in the R79 contract, evidence and roadmap."""
from pathlib import Path
import re
from urllib.parse import unquote

repo = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
paths = [
    'docs/runtime-async-generated-preparation-v1.md',
    'docs/runtime-owner-local-operations-v1.md',
    'docs/runtime-a1-a2-next-wave.md',
    'docs/runtime-a1-a2-swarm-plan.md',
    'docs/evidence/local-r79-async-preparation-2026-09-10/README.md',
]
count = 0
for name in paths:
    source = repo / name
    for target in re.findall(r'\]\(([^)]+)\)', source.read_text()):
        if '://' in target or target.startswith('mailto:'):
            continue
        location, _, anchor = unquote(target).partition('#')
        path = (source.parent / location).resolve() if location else source
        assert path.is_file(), (name, target)
        if anchor:
            headings = re.findall(r'^#{1,6}\s+(.+?)\s*$', path.read_text(), re.MULTILINE)
            slugs = [re.sub(r'[^\w\- ]', '', heading.lower()).replace(' ', '-') for heading in headings]
            assert anchor in slugs, (name, target)
        count += 1
result = f'PASS: {count} local Markdown links/anchors'
(repo.parent / 'r79-doc-links.log').write_text(result + '\n')
print(result)
