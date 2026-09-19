#!/usr/bin/env node
// One actual ordinary Rust export, immutable queries, diagnostic attribution only.
// Importing this file performs no reads/writes/process launches.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { measureNativeBuildInput, requireDiskReserve } from './assembly-region-worker-prototype.mjs';
import { runNavigationCommand } from './authoring-navigation-v1-process.mjs';
import { MiB, FIXTURE, TARGET, RUSTC_COMMIT, LIMITS, AUTHORITY, AVAILABILITY,
  demand, exact, same, uint, list, text, digest, decimal, absolute, artifact, sha256,
  validateNavigationData, parseNavigationCapture } from './authoring-navigation-v1-data.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SCOPE = 'retained actual ordinary-source read-only navigation; not compiler or source authority';
const inside = (parent, child) => {
  const rel = path.relative(parent, child);
  return rel === '' || (rel !== '..' && !rel.startsWith('../') && !path.isAbsolute(rel));
};
export function sourceTarget(c) {
  return path.join(c.cache_roots[0], 'navigation-' + path.basename(c.output));
}
export function exportArguments(c) {
  return ['--crate', 'fe2o3_ordinary_bitwise_promotion_v1_fixture', '--output',
    path.join(c.output, 'ordinary.fe2sim'), '--bundle-version', '6', '--target', 'gfx942',
    '--target-dir', sourceTarget(c), '--', '--manifest-path', path.join(c.repo, FIXTURE, 'Cargo.toml'), '--lib', '--offline'];
}
export function measurementRoster(c) {
  return [
    ['source', path.join(c.repo, FIXTURE, 'src/lib.rs')], ['manifest', path.join(c.repo, FIXTURE, 'Cargo.toml')],
    ['lock', path.join(c.repo, FIXTURE, 'Cargo.lock')],
    ...[['workspace_manifest', 'Cargo.toml'], ['workspace_lock', 'Cargo.lock'], ['toolchain', 'rust-toolchain.toml'],
      ['runner', 'scripts/authoring-navigation-v1-smoke.mjs'], ['data', 'scripts/authoring-navigation-v1-data.mjs'],
      ['process', 'scripts/authoring-navigation-v1-process.mjs'], ['controls', 'scripts/authoring-navigation-v1-smoke.test.mjs'],
      ['shared_json', 'scripts/ordered-program-source-native.mjs'], ['shared_ordered', 'scripts/ordered-program-worker-prototype.mjs'],
      ['shared_measure', 'scripts/assembly-region-worker-prototype.mjs']].map(([role, file]) => [role, path.join(c.repo, file)]),
    ...[['exporter', 'fe2o3-export-sim'], ['author', 'fe2o3-author'], ['extract', 'fe2o3-rustc-extract'],
      ['backend', 'librustc_codegen_fe2o3.so']].map(([role, file]) => [role, path.join(c.bin_dir, file)]),
    ['rustc', c.rustc], ['cargo', c.cargo], ['rustc_driver', c.rustc_driver],
    ['node', c.node], ['git', '/usr/bin/git'], ['du', '/usr/bin/du'],
  ];
}
export function validateConfiguration(c) {
  exact(c, ['repo', 'output', 'bin_dir', 'rustc', 'cargo', 'rustc_driver', 'cargo_home', 'node', 'cache_roots'], 'configuration');
  for (const [key, value] of Object.entries(c)) if (key !== 'cache_roots') absolute(value);
  list(c.cache_roots, 2, 'cache roots', 2).forEach(absolute);
  demand(/^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/.test(path.basename(c.output)), 'output label');
  const roots = [c.repo, c.output, ...c.cache_roots];
  for (let i = 0; i < roots.length; i++) for (let j = i + 1; j < roots.length; j++)
    demand(!inside(roots[i], roots[j]) && !inside(roots[j], roots[i]), 'repo/output/cache roots overlap');
  demand(c.output !== os.homedir() && path.dirname(c.output) !== c.output, 'broad output directory');
  demand(path.dirname(c.rustc_driver) === path.join(path.dirname(path.dirname(c.rustc)), 'lib')
    && /^librustc_driver-[0-9a-f]+\.so$/.test(path.basename(c.rustc_driver)), 'driver in selected rustc library tree');
  demand(path.basename(c.rustc) === 'rustc' && path.basename(c.cargo) === 'cargo', 'direct rustc/cargo binaries');
  return c;
}
function stream(item, cap = LIMITS.command_output_bytes) {
  exact(item, ['bytes', 'sha256', 'utf8'], 'command stream'); text(item.utf8, cap, 'command stream');
  uint(item.bytes, cap, 'command stream'); digest(item.sha256, 'command stream');
  demand(Buffer.byteLength(item.utf8) === item.bytes && sha256(Buffer.from(item.utf8)) === item.sha256, 'command stream bytes');
}
function measurement(row, specification) {
  exact(row, ['role', 'requested', 'resolved', 'bytes', 'sha256'], 'measurement');
  demand(row.role === specification[0] && row.requested === specification[1], 'measurement roster/order');
  absolute(row.requested); absolute(row.resolved); uint(row.bytes, 512 * MiB, 'measurement', 1); digest(row.sha256, 'measurement');
}
export function validateNavigationCapture(bytes) {
  const capture = parseNavigationCapture(bytes), data = validateNavigationData(capture);
  const c = validateConfiguration(capture.configuration), a = capture.artifacts;
  const roster = measurementRoster(c);
  list(capture.measurements_before, roster.length, 'before measurements', roster.length)
    .forEach((row, i) => measurement(row, roster[i]));
  same(capture.measurements_after, capture.measurements_before, 'measured inputs changed during capture');
  for (const role of ['source', 'manifest', 'lock']) {
    const record = capture.measurements_before.find(row => row.role === role);
    demand(record.bytes === a[role].bytes && record.sha256 === a[role].sha256, 'source input artifact measurement');
  }
  const expected = [
    ['git-head', '/usr/bin/git', ['rev-parse', 'HEAD'], null],
    ['git-status', '/usr/bin/git', ['status', '--porcelain=v1', '--untracked-files=normal'], null],
    ['rustc-version', c.rustc, ['-vV'], null],
    ['export', path.join(c.bin_dir, 'fe2o3-export-sim'), exportArguments(c), null],
    ['inspect', path.join(c.bin_dir, 'fe2o3-author'), ['inspect'], a.snapshot],
  ];
  let start = 0;
  for (const page of a.pages) {
    expected.push(['operations-' + start, path.join(c.bin_dir, 'fe2o3-author'),
      ['operations', '--bundle-identity', data.summary.bundle_identity, '--start', String(start),
        '--limit', String(LIMITS.page_items)], page]);
    start += JSON.parse(page.utf8).operations.length;
  }
  expected.push(['select', path.join(c.bin_dir, 'fe2o3-author'),
    ['select', '--selector', JSON.stringify(data.selector)], a.region]);
  list(capture.stages, expected.length, 'stages', expected.length).forEach((stage, i) => {
    exact(stage, ['name', 'executable', 'args', 'cwd', 'env_delta', 'input', 'code', 'signal', 'reason',
      'elapsed_ms', 'free_bytes_before', 'ram_bytes_before', 'ram_bytes_after', 'stdout', 'stderr'], 'stage');
    const [name, executable, args, output] = expected[i];
    demand(stage.name === name && stage.executable === executable && stage.cwd === c.repo
      && stage.code === 0 && stage.signal === null && stage.reason === null, 'successful exact command');
    same(stage.args, args, 'command argument substitution');
    same(stage.env_delta, name === 'export' ? {
      FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1: path.join(c.output, 'census.json'),
      FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1: capture.run_id } : {}, 'command census environment');
    same(stage.input, i >= 4 ? a.bundle : null, 'command exact bundle stdin');
    uint(stage.elapsed_ms, LIMITS.command_ms + 3000, 'command duration');
    decimal(stage.free_bytes_before, 10n ** 20n - 1n, 'disk reserve', BigInt(LIMITS.minimum_free_bytes));
    for (const key of ['ram_bytes_before', 'ram_bytes_after'])
      decimal(stage[key], 10n ** 20n - 1n, 'RAM reserve', BigInt(LIMITS.minimum_ram_bytes));
    stream(stage.stdout); stream(stage.stderr);
    if (output) {
      same(stage.stdout, output, 'retained query output substitution');
      demand(stage.stderr.bytes === 0, 'read-only query stderr');
    }
  });
  const version = capture.stages[2];
  demand(version.stderr.bytes === 0
    && version.stdout.utf8.split('\n').filter(line => line === 'commit-hash: ' + RUSTC_COMMIT).length === 1
    && version.stdout.utf8.split('\n').filter(line => line === 'release: 1.96.0-nightly').length === 1, 'pinned rustc version');
  demand(/^[0-9a-f]{40}\n$/.test(capture.stages[0].stdout.utf8) && capture.stages[0].stderr.bytes === 0
    && capture.stages[1].stderr.bytes === 0, 'retained checkout observations');
  demand(!capture.stages[3].stderr.utf8.includes('fe2o3 diagnostic source census unavailable:'),
    'export warning means census unavailable');
  list(capture.resource_guards, 2, 'resource guards', 2).forEach((guard, index) => {
    exact(guard, ['name', 'executable', 'args', 'code', 'signal', 'reason', 'elapsed_ms',
      'stdout', 'stderr', 'free_bytes', 'ram_bytes'], 'resource guard');
    demand(guard.name === ['before-export', 'after-export'][index] && guard.executable === '/usr/bin/du'
      && guard.code === 0 && guard.signal === null && guard.reason === null, 'successful resource measurement');
    same(guard.args, ['-sb', '--', ...c.cache_roots, c.output], 'resource measurement roots');
    stream(guard.stdout, 16384); stream(guard.stderr, 16384); demand(guard.stderr.bytes === 0, 'resource stderr');
    uint(guard.elapsed_ms, 33000, 'resource duration');
    const lines = guard.stdout.utf8.split('\n');
    demand(lines.length === 4 && lines[3] === '', 'resource output lines');
    let sum = 0n;
    [...c.cache_roots, c.output].forEach((root, i) => {
      const fields = lines[i].split('\t'); demand(fields.length === 2 && fields[1] === root, 'resource root');
      sum += decimal(fields[0], BigInt(LIMITS.maximum_combined_cache_bytes), 'cache bytes');
    });
    demand(sum <= BigInt(LIMITS.maximum_combined_cache_bytes), 'combined cache/output ceiling');
    decimal(guard.free_bytes, 10n ** 20n - 1n, 'guard disk', BigInt(LIMITS.minimum_free_bytes));
    decimal(guard.ram_bytes, 10n ** 20n - 1n, 'guard RAM', BigInt(LIMITS.minimum_ram_bytes));
  });
  const retained = [...capture.stages.flatMap(row => [row.stdout, row.stderr]),
    ...capture.resource_guards.flatMap(row => [row.stdout, row.stderr])].reduce((sum, row) => sum + row.bytes, 0);
  demand(retained <= LIMITS.retained_bytes, 'cumulative command observation bytes');
  const freeze = value => {
    if (value && typeof value === 'object' && !Object.isFrozen(value)) {
      Object.values(value).forEach(freeze); Object.freeze(value);
    }
    return value;
  };
  return { capture, navigation: freeze(data) };
}
function readBounded(file, cap) {
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd);
    demand(before.isFile() && before.size > 0 && before.size <= cap, 'bounded regular input');
    const buffer = Buffer.alloc(before.size);
    let offset = 0;
    while (offset < buffer.length) {
      const count = fs.readSync(fd, buffer, offset, buffer.length - offset, offset);
      demand(count > 0, 'input shrank'); offset += count;
    }
    const after = fs.fstatSync(fd);
    for (const key of ['dev', 'ino', 'size', 'mtimeMs', 'ctimeMs'])
      demand(after[key] === before[key], 'input changed during read');
    return buffer;
  } finally { fs.closeSync(fd); }
}
function ram() {
  // procfs reports size zero: this fixed kernel file has its own read/text cap.
  const bytes = fs.readFileSync('/proc/meminfo');
  demand(bytes.length > 0 && bytes.length <= 65536, 'kernel meminfo cap');
  const matches = [...bytes.toString().matchAll(/^MemAvailable:\s+(\d+) kB$/gm)];
  demand(matches.length === 1, 'one available RAM observation');
  const result = BigInt(matches[0][1]) * 1024n;
  demand(result >= BigInt(LIMITS.minimum_ram_bytes), 'available RAM below64GiB'); return result.toString();
}
function jsonBytes(value) { return Buffer.from(JSON.stringify(value, null, 2) + '\n'); }
function writeNew(file, bytes) { fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 }); }
export function parseArguments(argv) {
  const names = ['output', 'bin-dir', 'rustc', 'cargo', 'rustc-driver', 'cargo-home', 'cache-root', 'secondary-cache-root'];
  const options = {};
  demand(argv.length === names.length * 2, 'eight explicit absolute options required');
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i].slice(2);
    demand(argv[i].startsWith('--') && names.includes(key) && !Object.hasOwn(options, key), 'unknown/duplicate option');
    options[key] = absolute(argv[i + 1]);
  }
  return validateConfiguration({ repo: path.resolve(HERE, '..'), output: options.output,
    bin_dir: options['bin-dir'], rustc: options.rustc, cargo: options.cargo, rustc_driver: options['rustc-driver'],
    cargo_home: options['cargo-home'], node: process.execPath, cache_roots: [options['cache-root'], options['secondary-cache-root']] });
}
export async function runCapture(configuration) {
  const c = validateConfiguration(configuration);
  demand(process.platform === 'linux' && process.arch === 'x64' && Number(process.versions.node.split('.')[0]) >= 22,
    'Linux x86_64 Node22+ runner required');
  for (const directory of [c.repo, c.bin_dir, c.cargo_home, ...c.cache_roots, path.dirname(c.output)])
    demand(fs.realpathSync(directory) === directory && fs.statSync(directory).isDirectory(), 'resolved existing directory required');
  demand(fs.statSync(c.repo).dev === fs.statSync(path.dirname(c.output)).dev, 'output must use persistent checkout filesystem');
  requireDiskReserve(path.dirname(c.output)); ram();
  demand(!fs.existsSync(sourceTarget(c)), 'source target must be fresh');
  fs.mkdirSync(c.output, { mode: 0o700 });
  const capture = { schema: 'task-authoring-navigation-capture-v1', status: 'running', scope: SCOPE,
    authority: AUTHORITY, run_id: crypto.randomBytes(32).toString('hex'), configuration: c, limits: LIMITS,
    measurements_before: [], measurements_after: [], stages: [], resource_guards: [], artifacts: {}, availability: AVAILABILITY };
  let active = 'initialize';
  try {
    for (const name of ['logs', 'tmp']) fs.mkdirSync(path.join(c.output, name), { mode: 0o700 });
    fs.mkdirSync(sourceTarget(c), { mode: 0o700 });
    capture.measurements_before = measurementRoster(c).map(([role, file]) => ({ role, ...measureNativeBuildInput(file) }));
    const env = { PATH: path.dirname(c.rustc) + ':' + path.dirname(c.cargo) + ':/usr/bin:/bin',
      HOME: os.homedir(), LANG: 'C', LC_ALL: 'C', TMPDIR: path.join(c.output, 'tmp'),
      RUSTC: c.rustc, CARGO: c.cargo, CARGO_HOME: c.cargo_home, CARGO_TARGET_DIR: sourceTarget(c),
      RUSTUP_TOOLCHAIN: 'nightly-2026-04-03', CARGO_BUILD_JOBS: '2', CARGO_INCREMENTAL: '0',
      CARGO_PROFILE_DEV_DEBUG: '0', CARGO_TERM_COLOR: 'never', CARGO_NET_OFFLINE: 'true' };
    let commandBytes = 0;
    const guard = () => { requireDiskReserve(c.output); ram(); };
    const run = async (name, executable, args, input, delta = {}) => {
      active = name;
      const free_bytes_before = requireDiskReserve(c.output).toString(), ram_bytes_before = ram();
      const result = await runNavigationCommand({ executable, args, cwd: c.repo, env: { ...env, ...delta },
        input, timeoutMs: name === 'export' ? LIMITS.command_ms : 30000,
        outputCap: LIMITS.command_output_bytes, guard });
      for (const stream of ['stdout', 'stderr']) writeNew(path.join(c.output, 'logs', name + '.' + stream), result[stream]);
      const record = { name, executable, args, cwd: c.repo, env_delta: delta,
        input: input?.length ? { bytes: input.length, sha256: sha256(input) } : null,
        code: result.code, signal: result.signal, reason: result.reason, elapsed_ms: result.elapsed_ms,
        free_bytes_before, ram_bytes_before, ram_bytes_after: ram(), stdout: artifact(result.stdout), stderr: artifact(result.stderr) };
      commandBytes += result.stdout.length + result.stderr.length;
      demand(commandBytes <= LIMITS.retained_bytes, 'cumulative command observation bytes');
      capture.stages.push(record);
      demand(result.code === 0 && result.signal === null && result.reason === null, name + ': command failure, not semantic evidence');
      return result.stdout;
    };
    const cacheGuard = async name => {
      active = name;
      const args = ['-sb', '--', ...c.cache_roots, c.output];
      const result = await runNavigationCommand({ executable: '/usr/bin/du', args, cwd: c.repo, env,
        timeoutMs: 30000, outputCap: 16384, guard });
      const record = { name, executable: '/usr/bin/du', args, code: result.code, signal: result.signal,
        reason: result.reason, elapsed_ms: result.elapsed_ms, stdout: artifact(result.stdout), stderr: artifact(result.stderr),
        free_bytes: requireDiskReserve(c.output).toString(), ram_bytes: ram() };
      capture.resource_guards.push(record);
      demand(result.code === 0 && result.reason === null && result.signal === null && result.stderr.length === 0, 'cache measurement failed');
      const rows = result.stdout.toString().trimEnd().split('\n');
      demand(rows.length === 3, 'cache measurement rows');
      let sum = 0n;
      [...c.cache_roots, c.output].forEach((root, i) => {
        const fields = rows[i].split('\t'); demand(fields.length === 2 && fields[1] === root, 'cache measurement root');
        sum += decimal(fields[0], BigInt(LIMITS.maximum_combined_cache_bytes), 'cache measurement');
      });
      demand(sum <= BigInt(LIMITS.maximum_combined_cache_bytes), 'combined caches/output exceed20GiB');
    };
    for (const [key, file, cap] of [['source', 'src/lib.rs', LIMITS.source_bytes],
      ['manifest', 'Cargo.toml', LIMITS.artifact_bytes], ['lock', 'Cargo.lock', LIMITS.artifact_bytes]]) {
      capture.artifacts[key] = artifact(readBounded(path.join(c.repo, FIXTURE, file), cap));
      writeNew(path.join(c.output, key + (key === 'source' ? '.rs' : '.txt')), Buffer.from(capture.artifacts[key].utf8));
    }
    await run('git-head', '/usr/bin/git', ['rev-parse', 'HEAD']);
    await run('git-status', '/usr/bin/git', ['status', '--porcelain=v1', '--untracked-files=normal']);
    const version = await run('rustc-version', c.rustc, ['-vV']);
    demand(version.toString().split('\n').filter(line => line === 'commit-hash: ' + RUSTC_COMMIT).length === 1
      && version.toString().split('\n').filter(line => line === 'release: 1.96.0-nightly').length === 1
      && capture.stages.at(-1).stderr.bytes === 0, 'pinned rustc version required before export');
    await cacheGuard('before-export');
    await run('export', path.join(c.bin_dir, 'fe2o3-export-sim'), exportArguments(c), undefined, {
      FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1: path.join(c.output, 'census.json'),
      FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1: capture.run_id });
    await cacheGuard('after-export');
    const bundle = readBounded(path.join(c.output, 'ordinary.fe2sim'), LIMITS.bundle_bytes);
    capture.artifacts.bundle = { bytes: bundle.length, sha256: sha256(bundle) };
    capture.artifacts.census = artifact(readBounded(path.join(c.output, 'census.json'), LIMITS.census_bytes));
    const query = async (key, args) => {
      const bytes = await run(key, path.join(c.bin_dir, 'fe2o3-author'), args, bundle);
      demand(bytes.length <= LIMITS.artifact_bytes, 'query artifact bound');
      writeNew(path.join(c.output, key + '.json'), bytes);
      return artifact(bytes);
    };
    capture.artifacts.snapshot = await query('inspect', ['inspect']);
    const summary = parseNavigationCapture(Buffer.from(capture.artifacts.snapshot.utf8));
    uint(summary.operation_count, LIMITS.operations, 'operation count', 1);
    capture.artifacts.pages = [];
    const operations = [];
    for (let start = 0; start < summary.operation_count;) {
      const page = await query('operations-' + start, ['operations', '--bundle-identity', summary.bundle_identity,
        '--start', String(start), '--limit', String(LIMITS.page_items)]);
      const parsed = parseNavigationCapture(Buffer.from(page.utf8));
      const count = Math.min(LIMITS.page_items, summary.operation_count - start);
      list(parsed.operations, count, 'page progress', count);
      demand(parsed.start === start, 'page start drift');
      demand(capture.artifacts.pages.length < Math.ceil(LIMITS.operations / LIMITS.page_items), 'page count bound');
      capture.artifacts.pages.push(page); operations.push(...parsed.operations); start += parsed.operations.length;
    }
    const matches = operations.filter(row => row.kind === 'binary' && row.semantic_detail === 'BitOr');
    demand(matches.length === 1, 'fixture does not have one exact OR occurrence');
    const selector = { bundle_identity: summary.bundle_identity, canonical_kir_digest: summary.canonical_kir_digest,
      target: summary.target, operations: [matches[0].coordinate] };
    capture.artifacts.selector = artifact(jsonBytes(selector));
    writeNew(path.join(c.output, 'selector.json'), Buffer.from(capture.artifacts.selector.utf8));
    capture.artifacts.region = await query('select', ['select', '--selector', JSON.stringify(selector)]);
    capture.measurements_after = measurementRoster(c).map(([role, file]) => ({ role, ...measureNativeBuildInput(file) }));
    capture.status = 'passed';
    const bytes = jsonBytes(capture);
    validateNavigationCapture(bytes); guard();
    writeNew(path.join(c.output, 'capture.json'), bytes);
    return { output: c.output, capture_sha256: sha256(bytes), status: 'passed',
      scope: SCOPE, source_authentication: false, source_edit_boundary: 'unavailable' };
  } catch (error) {
    capture.status = 'failed';
    writeNew(path.join(c.output, 'failure.json'), jsonBytes({ ...capture, failure: {
      stage: active, message: String(error.message ?? error).slice(0, 4096),
      semantic_refusal_observed: false, synthetic_fallback: false } }));
    throw error;
  }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length === 3 && process.argv[2] === '--help') {
    console.log('Usage: node scripts/authoring-navigation-v1-smoke.mjs --output NEW_ABS --bin-dir ABS --rustc ABS --cargo ABS --rustc-driver ABS --cargo-home ABS --cache-root ABS --secondary-cache-root ABS\nRead-only ordinary-source navigation capture; no source editing, compiler resume or launch. Fresh target, offline jobs2, pinned rustc commit, 40GiB disk/64GiB RAM reserve, 20GiB combined cache/output ceiling.');
  } else {
    runCapture(parseArguments(process.argv.slice(2))).then(result => console.log(JSON.stringify(result)))
      .catch(error => { console.error(String(error.message ?? error)); process.exitCode = 1; });
  }
}
