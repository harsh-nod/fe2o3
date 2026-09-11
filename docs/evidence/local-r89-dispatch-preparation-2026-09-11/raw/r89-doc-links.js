// Check local references in the R89 contract, evidence and current roadmap.
const fs = require('fs');
const path = require('path');
const repo = '/home/harsh/.codex-tmp/fe2o3-r61-execution';
const files = [
  'docs/runtime-fixed-dispatch-preparation-custody-v1.md',
  'docs/evidence/local-r89-dispatch-preparation-2026-09-11/README.md',
  'docs/runtime-session-transition-custody-v1.md',
  'docs/evidence/local-r88-session-transitions-2026-09-11/README.md',
  'docs/runtime-pending-gtt-allocation-v1.md',
  'docs/evidence/local-r87-pending-allocation-2026-09-11/README.md',
  'docs/runtime-dispatch-data-retention-v1.md',
  'docs/evidence/local-r86-dispatch-retention-2026-09-11/README.md',
  'docs/runtime-charged-readback-completion-v1.md',
  'docs/evidence/local-r85-charged-readback-2026-09-11/README.md',
  'docs/runtime-generated-allocation-shells-v1.md',
  'docs/runtime-unpublished-operation-lifecycle-v1.md',
  'docs/runtime-async-generated-preparation-v1.md',
  'docs/runtime-owner-local-operations-v1.md',
  'docs/runtime-a1-a2-next-wave.md',
  'docs/runtime-a1-a2-swarm-plan.md',
  'docs/runtime-a1-a2-swarm-dispatch-r83.md',
  'docs/evidence/local-r83-unpublished-lifecycle-2026-09-10/README.md',
  'docs/evidence/local-r84-generated-shells-2026-09-11/README.md',
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
const result = 'PASS: ' + count + ' local Markdown links/anchors';
if (process.argv[2]) fs.writeFileSync(process.argv[2], result + '\n');
console.log(result);
