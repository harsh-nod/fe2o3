const assert = require('assert');
const crypto = require('crypto');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const digest = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
function exactKeys(value, keys, label) {
  assert(value && typeof value === 'object' && !Array.isArray(value), label);
  assert.deepStrictEqual(Object.keys(value).sort(), [...keys].sort(), label);
}
function uniqueIndex(text, anchor, label) {
  assert(typeof anchor === 'string' && anchor.length > 0, label + ': nonempty');
  const index = text.indexOf(anchor);
  assert(index >= 0 && text.indexOf(anchor, index + 1) < 0, label + ': unique');
  return index;
}
function mutatedSource(original, patch) {
  assert.strictEqual(typeof original, 'string');
  exactKeys(patch, ['path', 'edits', ...(Object.hasOwn(patch, 'scope') ? ['scope'] : [])], 'patch schema');
  let start = 0;
  let end = original.length;
  if (Object.hasOwn(patch, 'scope')) {
    exactKeys(patch.scope, ['start', 'end'], 'scope schema');
    start = uniqueIndex(original, patch.scope.start, 'scope start');
    end = uniqueIndex(original, patch.scope.end, 'scope end');
    assert(end > start + patch.scope.start.length, 'ordered nonempty method scope');
  }
  assert(Array.isArray(patch.edits) && patch.edits.length > 0, 'nonempty edits');
  let changed = original.slice(start, end);
  for (const edit of patch.edits) {
    assert(Array.isArray(edit) && edit.length === 2, 'edit pair');
    const [from, to] = edit;
    assert.strictEqual(typeof to, 'string');
    assert.notStrictEqual(from, to, 'nontrivial edit');
    const index = uniqueIndex(changed, from, 'mutation anchor');
    changed = changed.slice(0, index) + to + changed.slice(index + from.length);
  }
  const result = original.slice(0, start) + changed + original.slice(end);
  assert.notStrictEqual(result, original, 'nontrivial patch');
  return result;
}
function originalSources(map, originals, paths) {
  assert(Array.isArray(paths) && paths.length > 0, 'declared mutation paths');
  assert.deepStrictEqual([...new Set(paths)].sort(), paths, 'unique sorted mutation paths');
  assert(Array.isArray(originals), 'original-source collection');
  assert.deepStrictEqual(originals.map(source => source.path), paths, 'exact original-source paths');
  for (const source of originals) {
    exactKeys(source, ['path', 'text'], 'original-source schema');
    assert(source.path.startsWith('crates/') && !source.path.split('/').some(p => !p || p === '.' || p === '..'), 'relative crate source');
    assert(Object.hasOwn(map, source.path) && digest(map[source.path]), 'pinned source path');
    assert.strictEqual(typeof source.text, 'string');
    assert.strictEqual(hash(source.text), map[source.path], 'pinned original bytes');
  }
  return Object.fromEntries(originals.map(source => [source.path, source.text]));
}
function mutationSources(map, originals, mutation, paths) {
  const sources = originalSources(map, originals, paths);
  assert(!Object.hasOwn(mutation, 'path') && !Object.hasOwn(mutation, 'edits'), 'no legacy singular mutation');
  assert(Array.isArray(mutation.patches) && mutation.patches.length > 0, 'nonempty patches');
  const selected = mutation.patches.map(patch => patch.path);
  assert.strictEqual(new Set(selected).size, selected.length, 'one patch per file');
  return mutation.patches.map(patch => {
    assert(paths.includes(patch.path), 'declared mutation path');
    const original = sources[patch.path];
    const changed = mutatedSource(original, patch);
    return {path: patch.path, original, changed, original_sha256: hash(original), changed_sha256: hash(changed)};
  });
}
function mutationMap(map, files) {
  assert(Array.isArray(files) && files.length > 0, 'nonempty mutation files');
  assert.strictEqual(new Set(files.map(file => file.path)).size, files.length, 'unique mutation files');
  const result = {...map};
  for (const file of files) {
    assert.strictEqual(map[file.path], hash(file.original), 'original full-map identity');
    assert.strictEqual(file.original_sha256, map[file.path]);
    assert.strictEqual(file.changed_sha256, hash(file.changed));
    assert.notStrictEqual(file.changed_sha256, file.original_sha256);
    result[file.path] = file.changed_sha256;
  }
  return Object.fromEntries(Object.entries(result).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0));
}
function recoverySources(manifest, files) {
  exactKeys(manifest, ['name', 'sha256'], 'manifest identity');
  assert(/^r119-integrated-[a-z0-9-]+\.json$/.test(manifest.name) && digest(manifest.sha256), 'pinned manifest');
  assert(files.length > 0 && new Set(files.map(file => file.path)).size === files.length);
  return {manifest: {...manifest}, sources: files.map(file => {
    assert.strictEqual(file.original_sha256, hash(file.original));
    assert.strictEqual(file.changed_sha256, hash(file.changed));
    return {path: file.path, original_sha256: file.original_sha256, mutated_sha256: file.changed_sha256};
  })};
}
function validateRecovery(recovery, manifest, files) {
  assert.deepStrictEqual(recovery, recoverySources(manifest, files), 'exact manifest-bound recovery roster');
}
function applyOwnedMutation(files, io, attempts) {
  // The caller owns try/finally and sets runnerAttempted before invoking any child.
  assert(Array.isArray(attempts) && attempts.length === 0, 'fresh write history');
  for (const file of files) assert.strictEqual(hash(io.read(file.path)), file.original_sha256, 'application preflight');
  for (const file of files) {
    assert.strictEqual(hash(io.read(file.path)), file.original_sha256, 'immediate pre-write identity');
    const attempt = {path: file.path, verified: false};
    attempts.push(attempt);
    io.write(file.path, file.changed);
    assert.strictEqual(hash(io.read(file.path)), file.changed_sha256, 'complete mutation write');
    attempt.verified = true;
  }
}
function canRestore(record) {
  if (!record || record.child_closed !== true) return false;
  const cleanup = record.process_group_cleanup?.close;
  if (cleanup?.status === 'not_spawned') return typeof record.spawn_error === 'string' &&
    Number.isSafeInteger(record.child_returncode) && record.child_returncode < 0 && !Object.hasOwn(cleanup, 'pgid');
  return record.spawn_error === null && ['absent', 'signaled'].includes(cleanup?.status) &&
    Number.isSafeInteger(cleanup.pgid) && cleanup.pgid > 1 &&
    Array.isArray(cleanup.live_members) && cleanup.live_members.length === 0;
}
function restoreOwnedMutation(map, files, state, io) {
  assert.strictEqual(typeof state.runnerAttempted, 'boolean');
  assert.strictEqual(typeof state.recovery, 'boolean');
  if (!state.runnerAttempted) assert.strictEqual(state.record, null, 'no invented child record');
  mutationMap(map, files);
  assert(Array.isArray(state.attempts) && state.attempts.length <= files.length, 'bounded application attempts');
  for (const [index, attempt] of state.attempts.entries()) {
    exactKeys(attempt, ['path', 'verified'], 'application attempt schema');
    assert.strictEqual(attempt.path, files[index].path, 'ordered application attempt prefix');
    assert.strictEqual(typeof attempt.verified, 'boolean');
    if (index + 1 < state.attempts.length) assert.strictEqual(attempt.verified, true, 'only the final write may be incomplete');
  }
  if (state.runnerAttempted) {
    assert.strictEqual(state.attempts.length, files.length, 'child requires all files applied');
    assert(state.attempts.every(attempt => attempt.verified), 'child requires verified application writes');
  }
  if (state.recovery) validateRecovery(state.recovery_record, state.manifest, files);
  const result = {restored: false, quiescent: false, all_mutant_before_restore: false,
    live_members: null, external_edit: false, error: null, files: files.map(file => ({
      path: file.path, before_sha256: null, after_sha256: null, write_attempted: false,
    }))};
  try {
    result.quiescent = !state.runnerAttempted || canRestore(state.record);
    const pgid = state.record?.process_group_cleanup?.close?.pgid;
    if (result.quiescent && pgid) {
      result.quiescent = false;
      result.live_members = io.live(pgid);
      assert(Array.isArray(result.live_members), 'live-member observation');
      result.quiescent = result.live_members.length === 0;
    }
    if (!result.quiescent) return result;
    // Validate the entire target set after the process scan and before any restoration.
    for (const [index, file] of files.entries()) {
      const row = result.files[index];
      row.before_sha256 = hash(io.read(file.path));
      const allowedOriginal = (!state.runnerAttempted || state.recovery) && row.before_sha256 === file.original_sha256;
      const allowedMutant = (index < state.attempts.length || state.recovery) && row.before_sha256 === file.changed_sha256;
      if (!allowedMutant && !allowedOriginal) result.external_edit = true;
    }
    result.all_mutant_before_restore = result.files.every((row, index) => row.before_sha256 === files[index].changed_sha256);
    if (result.external_edit) return result;
    for (const [index, file] of files.entries()) {
      const row = result.files[index];
      if (hash(io.read(file.path)) !== row.before_sha256) {
        result.external_edit = true;
        break;
      }
      if (row.before_sha256 === file.original_sha256) continue;
      row.write_attempted = true;
      io.write(file.path, file.original);
      assert.strictEqual(hash(io.read(file.path)), file.original_sha256, 'complete restoration write');
    }
  } catch (error) {result.error = String(error);}
  finally {
    for (const [index, file] of files.entries()) {
      try {result.files[index].after_sha256 = hash(io.read(file.path));}
      catch (error) {result.error ??= String(error);}
    }
    if (result.quiescent) {
      try {
        assert.deepStrictEqual(io.identities(), map, 'entire frozen map restored');
        result.restored = true;
      } catch (error) {result.error ??= String(error);}
    }
  }
  return result;
}
module.exports = {hash, mutatedSource, originalSources, mutationSources, mutationMap,
  recoverySources, validateRecovery, applyOwnedMutation, canRestore, restoreOwnedMutation};
