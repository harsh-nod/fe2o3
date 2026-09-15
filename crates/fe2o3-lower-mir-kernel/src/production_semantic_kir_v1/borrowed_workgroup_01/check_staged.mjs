// Read-only overlay validation. This never installs hooks or invokes Cargo.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const directory = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(directory, '../../../../..');
const manifest = JSON.parse(fs.readFileSync(path.join(directory, 'integration.edits.json'), 'utf8'));
const files = new Map();
for (const [index, edit] of manifest.edits.entries()) {
  const absolute = path.resolve(root, edit.path);
  if (!absolute.startsWith(root + path.sep)) throw new Error('Edit escapes repository');
  let file = files.get(edit.path);
  if (!file) {
    file = { original: fs.readFileSync(absolute, 'utf8'), edits: [] };
    files.set(edit.path, file);
  }
  const start = file.original.indexOf(edit.before);
  if (start < 0 || file.original.indexOf(edit.before, start + 1) >= 0) {
    throw new Error(`Nonunique or stale anchor ${index}: ${edit.path}`);
  }
  file.edits.push({ ...edit, start, end: start + edit.before.length });
}

const patch = [];
for (const [filename, file] of files) {
  const edits = file.edits.sort((a, b) => a.start - b.start);
  for (let i = 1; i < edits.length; i++) {
    if (edits[i - 1].end > edits[i].start) throw new Error(`Overlapping edits: ${filename}`);
  }
  let overlay = file.original;
  for (const edit of [...edits].reverse()) {
    overlay = overlay.slice(0, edit.start) + edit.after + overlay.slice(edit.end);
  }
  if (process.argv.includes('--parse')) {
    const parsed = spawnSync('rustfmt', ['--edition', '2024', '--emit', 'stdout', '--config', 'skip_children=true'], {
      input: overlay, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, timeout: 10000,
    });
    if (parsed.status !== 0) throw new Error(`Rust syntax check ${filename}: ${parsed.stderr || parsed.error}`);
  }
  const lines = file.original.split('\n');
  const starts = [0];
  for (let i = 0; i < lines.length - 1; i++) starts.push(starts.at(-1) + lines[i].length + 1);
  const lineAt = offset => {
    let low = 0, high = starts.length;
    while (low + 1 < high) {
      const mid = (low + high) >>> 1;
      if (starts[mid] <= offset) low = mid; else high = mid;
    }
    return low;
  };
  const groups = [];
  for (const edit of edits) {
    const first = Math.max(0, lineAt(edit.start) - 3);
    const last = Math.min(lines.length - 1, lineAt(Math.max(edit.start, edit.end - 1)) + 4);
    const previous = groups.at(-1);
    if (previous && first <= previous.last) {
      previous.last = Math.max(previous.last, last);
      previous.edits.push(edit);
    } else groups.push({ first, last, edits: [edit] });
  }
  patch.push(`--- a/${filename}`, `+++ b/${filename}`);
  let delta = 0;
  for (const group of groups) {
    const offset = starts[group.first];
    const end = starts[group.last];
    const before = file.original.slice(offset, end);
    let after = before;
    for (const edit of [...group.edits].reverse()) {
      const start = edit.start - offset;
      after = after.slice(0, start) + edit.after + after.slice(edit.end - offset);
    }
    if (!before.endsWith('\n') || !after.endsWith('\n')) throw new Error('Expected complete newline-terminated hunk');
    const oldLines = before.slice(0, -1).split('\n');
    const newLines = after.slice(0, -1).split('\n');
    const columns = newLines.length + 1;
    const table = new Uint32Array((oldLines.length + 1) * columns);
    for (let i = oldLines.length - 1; i >= 0; i--) {
      for (let j = newLines.length - 1; j >= 0; j--) {
        table[i * columns + j] = oldLines[i] === newLines[j]
          ? 1 + table[(i + 1) * columns + j + 1]
          : Math.max(table[(i + 1) * columns + j], table[i * columns + j + 1]);
      }
    }
    const steps = [];
    let i = 0, j = 0;
    while (i < oldLines.length || j < newLines.length) {
      if (i < oldLines.length && j < newLines.length && oldLines[i] === newLines[j]) {
        steps.push(' ' + oldLines[i++]); j++;
      } else if (i < oldLines.length && (j === newLines.length || table[(i + 1) * columns + j] >= table[i * columns + j + 1])) {
        steps.push('-' + oldLines[i++]);
      } else steps.push('+' + newLines[j++]);
    }
    const spans = [];
    steps.forEach((step, index) => {
      if (step[0] === ' ') return;
      const begin = Math.max(0, index - 3), end = Math.min(steps.length, index + 4);
      const previous = spans.at(-1);
      if (previous && begin <= previous.end) previous.end = end;
      else spans.push({ begin, end });
    });
    for (const span of spans) {
      const prefix = steps.slice(0, span.begin), hunk = steps.slice(span.begin, span.end);
      const oldCount = hunk.filter(step => step[0] !== '+').length;
      const newCount = hunk.filter(step => step[0] !== '-').length;
      const oldStart = group.first + 1 + prefix.filter(step => step[0] !== '+').length;
      const newStart = group.first + 1 + delta + prefix.filter(step => step[0] !== '-').length;
      patch.push(`@@ -${oldStart},${oldCount} +${newStart},${newCount} @@`, ...hunk);
    }
    delta += newLines.length - oldLines.length;
  }
}
if (process.argv.includes('--patch')) process.stdout.write(patch.join('\n') + '\n');
else console.log(JSON.stringify({ status: 'unmounted', edits: manifest.edits.length, files: files.size, rustSyntax: process.argv.includes('--parse') ? 'parsed' : 'not run', typeCheck: 'not run', tests: 'not run' }));
