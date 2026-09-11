// Check local references in the R91 contract, evidence and dispatch documents.
const fs = require('fs');
const path = require('path');
const repo = '/home/harsh/.codex-tmp/fe2o3-r61-execution';
const files = [
  'docs/runtime-queue-construction-handoffs-v1.md',
  'docs/evidence/local-r91-queue-handoffs-2026-09-11/README.md',
  'docs/runtime-a1-a2-next-wave.md',
  'docs/runtime-a1-a2-swarm-plan.md',
  'docs/runtime-a1-a2-swarm-dispatch-r83.md',
];
let count = 0;
for (const file of files) {
  const source = path.join(repo, file);
  for (const match of fs.readFileSync(source, 'utf8').matchAll(/\]\(([^)]+)\)/g)) {
    const target = match[1];
    if (target.includes('://') || target.startsWith('mailto:')) continue;
    const [location, anchor] = decodeURIComponent(target).split('#');
    const resolved = location ? path.resolve(path.dirname(source), location) : source;
    if (!fs.statSync(resolved).isFile()) throw Error(file + ': ' + target);
    if (anchor) {
      const slugs = [...fs.readFileSync(resolved, 'utf8').matchAll(/^#{1,6}\s+(.+?)\s*$/gm)]
        .map(m => m[1].toLowerCase().replace(/[^\w\- ]/g, '').replace(/ /g, '-'));
      if (!slugs.includes(anchor)) throw Error(file + ': ' + target);
    }
    count++;
  }
}
console.log('PASS: ' + count + ' local Markdown links/anchors in ' + files.length + ' R91 documents');
