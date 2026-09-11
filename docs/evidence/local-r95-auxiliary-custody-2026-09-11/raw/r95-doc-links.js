const fs = require('fs');
const path = require('path');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp/fe2o3-r61-execution';
const documents = [
  'docs/runtime-a1-a2-swarm-dispatch-r83.md',
  'docs/runtime-a1-a2-next-wave.md',
  'docs/runtime-a1-a2-swarm-plan.md',
  'docs/runtime-a1-a2-swarm-current.md',
  'docs/runtime-auxiliary-queue-construction-custody-v1.md',
  'docs/evidence/local-r95-auxiliary-custody-2026-09-11/README.md',
];
let checked = 0;
for (const file of documents) {
  const absolute = path.join(root, file);
  const source = fs.readFileSync(absolute, 'utf8');
  for (const match of source.matchAll(/\[[^\]]*\]\(([^\s)]+)\)/g)) {
    const href = match[1];
    if (/^[a-z]+:/i.test(href)) continue;
    const [relative, anchor] = href.split('#');
    const target = relative ? path.resolve(path.dirname(absolute), decodeURIComponent(relative)) : absolute;
    assert(fs.existsSync(target), file + ': missing ' + href);
    if (anchor && target.endsWith('.md')) {
      const body = fs.readFileSync(target, 'utf8');
      const headings = Array.from(body.matchAll(/^#{1,6}\s+(.+)$/gm), match =>
        match[1].toLowerCase().replace(/[^\p{L}\p{N}_\- ]/gu, '').replace(/ /g, '-'));
      assert(headings.includes(anchor), file + ': missing anchor ' + href);
    }
    checked++;
  }
}
console.log('PASS: ' + checked + ' local documentation links and anchors');
