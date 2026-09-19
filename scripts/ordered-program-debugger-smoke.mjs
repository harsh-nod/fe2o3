#!/usr/bin/env node
// One closed CPU diagnostic profile. No source export, custody, hardware,
// physical-register observation, proof, artifact, launch or resume authority.
import assert from 'node:assert/strict';
import { constants, statfsSync } from 'node:fs';
import { open, mkdir, realpath } from 'node:fs/promises';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { BOUNDS, BoundedByteCapture, SHARED_UTILITY_URL, encodeExactJson, parseExactJson, runDebuggerQualification }
  from './ordered-program-debugger-client.mjs';

export const SMOKE_BOUNDS = Object.freeze({ kirBytes: 65536, requestBytes: 65536,
  executableBytes: 512 * 1024 * 1024, scriptBytes: 1024 * 1024,
  inspectionMilliseconds: 10000, drainMilliseconds: 1000,
  inspectionStdoutBytes: 8192, inspectionStderrBytes: 8192, executionRecordBytes: 32768,
  outputReserveBytes: 10 * 1024 * 1024 });
const DISK_RESERVE = 40n * 1024n ** 3n;
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const digest = value => assert(typeof value === 'string' && /^[0-9a-f]{64}$/.test(value), 'exact SHA-256 shape');
function integer(value, maximum) {
  assert(Number.isSafeInteger(value) && value >= 0 && value <= maximum, 'bounded unsigned integer required');
}
function absolute(value) {
  assert(typeof value === 'string' && Buffer.byteLength(value) <= 4096 && isAbsolute(value)
    && resolve(value) === value && !/[\x00-\x1f\x7f]/.test(value), 'canonical absolute path required');
}
function keys(value, expected) {
  assert(value && typeof value === 'object' && !Array.isArray(value), 'record required');
  assert.deepEqual(Object.keys(value).sort(), [...expected].sort(), 'unsupported field roster');
}

/** Independent descriptor grammar. No compiler decoder/evaluator is imported. */
export function validateProgramDescriptors(active) {
  assert(Array.isArray(active) && active.length >= 1 && active.length <= 16, 'one to sixteen active descriptors required');
  const defined = [true, true, true, false, false];
  const copy = [];
  for (const word of active) {
    integer(word, 65535);
    assert(Math.floor(word / 1024) === 0, 'reserved descriptor bits');
    const opcode = word % 8, destination = 3 + Math.floor(word / 8) % 2;
    const left = Math.floor(word / 16) % 8, right = Math.floor(word / 128) % 8;
    assert(opcode <= 5 && left <= 4 && right <= 4, 'closed opcode/role grammar');
    assert(opcode !== 0 || right === 0, 'move unused source must be zero');
    assert(defined[left] && (opcode === 0 || defined[right]), 'read before role initialization');
    defined[destination] = true; copy.push(word);
  }
  assert(defined[4], 'program must define output');
  return copy;
}

/** Independent wrapping-U32 oracle; internal values never become observations. */
export function evaluateProgram(active, inputs) {
  const descriptors = validateProgramDescriptors(active);
  assert(Array.isArray(inputs) && inputs.length === 3, 'three U32 inputs required');
  for (const value of inputs) integer(value, 0xffffffff);
  const values = [...inputs.map(BigInt), undefined, undefined];
  for (const word of descriptors) {
    const opcode = word % 8, destination = 3 + Math.floor(word / 8) % 2;
    const left = values[Math.floor(word / 16) % 8], right = values[Math.floor(word / 128) % 8];
    const result = opcode === 0 ? left : opcode === 1 ? left + right : opcode === 2 ? left - right
      : opcode === 3 ? left & right : opcode === 4 ? left | right : left ^ right;
    values[destination] = BigInt.asUintN(32, result);
  }
  return Number(values[4]);
}

export function declaredSteps(active, registerPlan) {
  const descriptors = validateProgramDescriptors(active);
  assert(Array.isArray(registerPlan) && registerPlan.length === 5);
  for (const value of registerPlan) integer(value, 63);
  assert.equal(new Set(registerPlan).size, 5);
  const [scratch, output, ...inputs] = registerPlan;
  const physical = [...inputs, scratch, output];
  const names = ['v_mov_b32_e32', 'v_add_u32_e32', 'v_sub_u32_e32', 'v_and_b32_e32', 'v_or_b32_e32', 'v_xor_b32_e32'];
  return descriptors.map(word => {
    const opcode = word % 8, destination = 3 + Math.floor(word / 8) % 2;
    const left = physical[Math.floor(word / 16) % 8], right = physical[Math.floor(word / 128) % 8];
    return { instruction: names[opcode], output: physical[destination], inputs: opcode === 0 ? [left] : [left, right] };
  });
}

function diskReserve(directory) {
  const stat = statfsSync(directory, { bigint: true });
  assert(stat.bavail * stat.bsize >= DISK_RESERVE + BigInt(SMOKE_BOUNDS.outputReserveBytes),
    '40 GiB disk reserve plus bounded output headroom required');
}

export function validateSmokeOptions(options) {
  const o = options;
  keys(o, ['debuggerPath', 'inspectorPath', 'kirPath', 'requestPath', 'outputDirectory',
    'resultMode', 'operandOrder', 'registerPlan', 'descriptors', 'sourceIds']);
  for (const name of ['debuggerPath', 'inspectorPath', 'kirPath', 'requestPath', 'outputDirectory']) absolute(o[name]);
  assert(['used', 'unused'].includes(o.resultMode), 'result mode must be explicit used or unused');
  assert(Array.isArray(o.operandOrder) && o.operandOrder.length === 3, 'three operand positions required');
  assert.deepEqual([...o.operandOrder].sort(), [0, 1, 2], 'operand order must be an exact permutation');
  assert(Array.isArray(o.registerPlan) && o.registerPlan.length === 5, 'five explicit register roles required');
  for (const value of o.registerPlan) integer(value, 63);
  assert.equal(new Set(o.registerPlan).size, 5, 'register roles must be distinct');
  const descriptors = validateProgramDescriptors(o.descriptors);
  assert(Array.isArray(o.sourceIds) && o.sourceIds.length === 4, 'four explicit declared source IDs required');
  for (const value of o.sourceIds) { digest(value); assert.notEqual(value, '0'.repeat(64), 'declared source IDs must be nonzero'); }
  return { ...o, operandOrder: [...o.operandOrder], registerPlan: [...o.registerPlan],
    descriptors, sourceIds: [...o.sourceIds] };
}

export function parseSmokeArguments(args) {
  assert(Array.isArray(args) && args.length === 20, 'ten explicit option/value pairs required');
  const names = new Map([
    ['--debugger', 'debuggerPath'], ['--inspector', 'inspectorPath'], ['--kir', 'kirPath'],
    ['--request', 'requestPath'], ['--output', 'outputDirectory'], ['--result-mode', 'resultMode'],
    ['--operand-order', 'operandOrder'], ['--register-plan', 'registerPlan'],
    ['--descriptors', 'descriptors'], ['--source-ids', 'sourceIds'],
  ]);
  const options = {};
  for (let index = 0; index < args.length; index += 2) {
    const key = names.get(args[index]), value = args[index + 1];
    assert(key && !Object.hasOwn(options, key) && typeof value === 'string'
      && Buffer.byteLength(value) <= 4096, 'unknown, duplicate or malformed option');
    if (key === 'operandOrder' || key === 'registerPlan' || key === 'descriptors') {
      assert(/^(?:0|[1-9][0-9]*)(?:,(?:0|[1-9][0-9]*))*$/.test(value), 'canonical comma-separated integers required');
      options[key] = value.split(',').map(Number);
    } else if (key === 'sourceIds') options[key] = value.split(',');
    else options[key] = value;
  }
  return validateSmokeOptions(options);
}

/** Independent arithmetic and full backing/init oracle; no debugger input. */
export function deriveRequestExpectation(request, { resultMode, operandOrder, descriptors }) {
  assert(['used', 'unused'].includes(resultMode));
  assert.deepEqual([...operandOrder].sort(), [0, 1, 2]);
  keys(request, ['schema', 'kernel', 'grid', 'workgroup', 'arguments', 'shared_buffers']);
  assert.equal(request.schema, 'fe2o3-simulation-request-v1');
  assert(typeof request.kernel === 'string' && /^[A-Za-z_][A-Za-z0-9_:]{0,255}$/.test(request.kernel), 'bounded kernel name');
  assert.deepEqual(request.grid, [64, 1, 1]);
  assert.deepEqual(request.workgroup, [64, 1, 1]);
  assert(Array.isArray(request.arguments) && request.arguments.length === 4, 'output slice then three U32 scalars required');
  assert.deepEqual(request.arguments[0], { kind: 'buffer_view', backing: 1, element: 'u32',
    access: 'read_write', alignment: 4, byte_offset: 4, elements: 64 });
  const inputs = request.arguments.slice(1).map(argument => {
    keys(argument, ['kind', 'type', 'bits']);
    assert.equal(argument.kind, 'scalar'); assert.equal(argument.type, 'u32');
    assert(typeof argument.bits === 'string' && /^0x[0-9a-f]{8}$/.test(argument.bits), 'exact U32 bits required');
    return Number(BigInt(argument.bits));
  });
  assert.deepEqual(request.shared_buffers, [{ id: 1, element: 'u32', access: 'read_write', alignment: 4,
    bytes: `0x${'a5'.repeat(264)}`, initialized: `0x${'00'.repeat(33)}` }], 'closed 264-byte canary/init profile required');
  const beforeInputs = operandOrder.map(index => inputs[index]);
  const afterResult = evaluateProgram(descriptors, beforeInputs);
  const outputValue = resultMode === 'used' ? afterResult : inputs[0];
  const backing = Buffer.alloc(264, 0xa5);
  for (let lane = 0; lane < 64; lane++) backing.writeUInt32LE(outputValue, 4 + 4 * lane);
  return { kernel: request.kernel, inputs, beforeInputs, afterResult, outputValue,
    lane: 0, workgroup: [0, 0, 0], wave: 0, expectedAccessCount: 64, expectedAccessByteOffset: 4,
    outputMemory: { allocation: { ordinal: 1, generation: 0 }, byteOffset: 0,
      bytes: `0x${backing.toString('hex')}`, initialized: `0xf0${'ff'.repeat(31)}0f` } };
}

/** Current immutable-owner metadata only; this does not authenticate source. */
export function validateInspection(view, { kirBytes, kernel, registerPlan, descriptors, sourceIds }) {
  integer(kirBytes, SMOKE_BOUNDS.kirBytes); assert(kirBytes > 0);
  const active = validateProgramDescriptors(descriptors);
  keys(view, ['kind', 'authority', 'canonical', 'kernel', 'function', 'coordinate', 'raw_block_id',
    'input_value_ids', 'result_value_id', 'declared_target', 'declared_wave_width', 'profile',
    'register_plan', 'declared_program', 'declared_instruction_steps', 'declared_source_ids',
    'memory_effect', 'ordered_region_effect', 'pure_or_movable', 'logical_observation_granularity',
    'source_authentication', 'source_map_available', 'physical_register_values_available',
    'instruction_microsteps_available', 'register_lifetime_or_final_allocation_proof', 'proof_authority',
    'artifact_authority', 'production_resume_authority', 'hardware_execution', 'cpu_preflight_passed',
    'inspection_counts', 'inspection_max_canonical_bytes_after_admission',
    'cpu_preflight_resident_limit_bytes', 'output_buffer_bytes', 'accounting_scope']);
  assert.equal(view.kind, 'diagnostic_ordered_program_inspection_example');
  assert.equal(view.authority, 'observation_only');
  keys(view.canonical, ['wire_version', 'sha256', 'bytes']);
  assert.equal(view.canonical.wire_version, 17); assert.equal(view.canonical.bytes, kirBytes);
  digest(view.canonical.sha256);
  assert.equal(view.kernel, kernel);
  assert(typeof view.function === 'string' && view.function.length > 0
    && Buffer.byteLength(view.function) <= 1024, 'bounded current function name');
  assert.equal(view.declared_target, 'gfx942:xnack-'); assert.equal(view.declared_wave_width, 64);
  assert.equal(view.profile, 'closed_u32_program_e32_v1'); assert.equal(view.memory_effect, 'NoMemory');
  assert.equal(view.ordered_region_effect, true); assert.equal(view.cpu_preflight_passed, true);
  assert.equal(view.logical_observation_granularity, 'whole_program_before_after');
  for (const flag of ['source_authentication', 'source_map_available', 'physical_register_values_available',
    'instruction_microsteps_available', 'register_lifetime_or_final_allocation_proof', 'proof_authority',
    'artifact_authority', 'production_resume_authority', 'hardware_execution', 'pure_or_movable']) assert.equal(view[flag], false);
  const [scratch, output, a, b, c] = registerPlan;
  assert.deepEqual(view.register_plan, { scratch, output, inputs: [a, b, c], vgpr_high_water: Math.max(...registerPlan) + 1 });
  const padded = [...active, ...Array(16 - active.length).fill(0)];
  assert.deepEqual(view.declared_program, { count: active.length, descriptors: padded });
  assert.deepEqual(view.declared_instruction_steps, declaredSteps(active, registerPlan));
  keys(view.coordinate, ['function_ordinal', 'block_ordinal', 'operation_ordinal']);
  assert.equal(view.coordinate.function_ordinal, 0);
  integer(view.coordinate.block_ordinal, 127); integer(view.coordinate.operation_ordinal, 4095);
  integer(view.raw_block_id, 0xffffffff); integer(view.result_value_id, 0xffffffff);
  assert(Array.isArray(view.input_value_ids) && view.input_value_ids.length === 3);
  for (const value of view.input_value_ids) integer(value, 0xffffffff);
  assert.equal(new Set([...view.input_value_ids, view.result_value_id]).size, 4, 'distinct current SSA roles required');
  keys(view.declared_source_ids, ['frontend_unit', 'function', 'contract', 'statement']);
  Object.values(view.declared_source_ids).forEach(digest);
  assert(Array.isArray(sourceIds) && sourceIds.length === 4);
  for (const value of sourceIds) { digest(value); assert.notEqual(value, '0'.repeat(64)); }
  assert.deepEqual(view.declared_source_ids, { frontend_unit: sourceIds[0], function: sourceIds[1],
    contract: sourceIds[2], statement: sourceIds[3] }, 'inspector declaration differs from explicit source-ID expectation');
  keys(view.inspection_counts, ['blocks', 'operations', 'ssa_definitions', 'capability_entries', 'name_bytes']);
  for (const [key, maximum] of [['blocks', 128], ['operations', 4096], ['ssa_definitions', 8192],
    ['capability_entries', 256], ['name_bytes', 16384]]) integer(view.inspection_counts[key], maximum);
  assert(view.inspection_counts.blocks > view.coordinate.block_ordinal);
  assert(view.inspection_counts.operations > view.coordinate.operation_ordinal);
  assert(view.inspection_counts.ssa_definitions >= 4);
  assert.equal(view.inspection_max_canonical_bytes_after_admission, 65536);
  assert.equal(view.cpu_preflight_resident_limit_bytes, 67108864);
  assert.equal(view.output_buffer_bytes, 8192);
  assert(typeof view.accounting_scope === 'string' && view.accounting_scope.length <= 1024);
  return { region: { functionOrdinal: view.coordinate.function_ordinal, blockOrdinal: view.coordinate.block_ordinal,
    operationOrdinal: view.coordinate.operation_ordinal }, rawBlockId: view.raw_block_id,
    canonicalIdentity: view.canonical.sha256, inputValueIds: [...view.input_value_ids], resultValueId: view.result_value_id };
}

async function measure(filename, maximum, retain = false) {
  absolute(filename); assert.equal(await realpath(filename), filename, 'input path redirects');
  const file = await open(filename, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const before = await file.stat({ bigint: true });
    assert(before.isFile() && before.size <= BigInt(maximum), 'regular-file byte bound');
    const digest = createHash('sha256'), buffer = Buffer.alloc(65536), parts = [];
    let bytes = 0;
    for (;;) {
      const read = await file.read(buffer, 0, Math.min(buffer.length, maximum + 1 - bytes), null);
      if (!read.bytesRead) break;
      bytes += read.bytesRead; assert(bytes <= maximum, 'input grew past bound');
      digest.update(buffer.subarray(0, read.bytesRead));
      if (retain) parts.push(Buffer.from(buffer.subarray(0, read.bytesRead)));
    }
    const after = await file.stat({ bigint: true });
    for (const key of ['dev', 'ino', 'size', 'mtimeNs', 'ctimeNs']) assert.equal(after[key], before[key], 'input changed while reading');
    assert.equal(BigInt(bytes), before.size);
    return { pin: { path: filename, bytes, sha256: digest.digest('hex'), device: String(before.dev),
      inode: String(before.ino), mtime_ns: String(before.mtimeNs), ctime_ns: String(before.ctimeNs) },
    ...(retain ? { content: Buffer.concat(parts, bytes) } : {}) };
  } finally { await file.close(); }
}

function inspectProcess(program, args, outputDirectory) {
  return new Promise(resolveResult => {
    const started = performance.now();
    const stdout = new BoundedByteCapture(SMOKE_BOUNDS.inspectionStdoutBytes);
    const stderr = new BoundedByteCapture(SMOKE_BOUNDS.inspectionStderrBytes);
    let reason = null, truncated = false, finished = false, drainTimer;
    const child = spawn(program, args, { detached: true, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, LC_ALL: 'C' } });
    const finish = (code, signal, forcedDrain = false) => {
      if (finished) return; finished = true;
      clearTimeout(timer); clearTimeout(drainTimer); clearInterval(monitor);
      resolveResult({ code, signal, reason, truncated, forced_drain: forcedDrain,
        elapsed_ms: Math.ceil(performance.now() - started),
        stdout: stdout.snapshot(SMOKE_BOUNDS.inspectionStdoutBytes), stderr: stderr.snapshot(SMOKE_BOUNDS.inspectionStderrBytes) });
    };
    const stop = why => {
      if (finished) return; reason ??= why;
      if (Number.isInteger(child.pid) && child.pid > 0) {
        try { process.kill(-child.pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') child.kill('SIGKILL'); }
      } else child.kill('SIGKILL');
      drainTimer ??= setTimeout(() => {
        truncated = true; child.stdout.destroy(); child.stderr.destroy(); finish(null, 'SIGKILL', true);
      }, SMOKE_BOUNDS.drainMilliseconds);
    };
    const timer = setTimeout(() => stop('timeout'), SMOKE_BOUNDS.inspectionMilliseconds);
    const monitor = setInterval(() => { try { diskReserve(outputDirectory); } catch { stop('disk_reserve'); } }, 1000);
    child.once('error', error => stop(`spawn:${error.code ?? 'error'}`));
    child.once('close', (code, signal) => finish(code, signal));
    for (const [name, capture] of [['stdout', stdout], ['stderr', stderr]]) {
      child[name].on('error', error => stop(`${name}:${error.code ?? 'error'}`));
      child[name].on('data', chunk => {
        if (capture.append(chunk)) { truncated = true; stop(`${name}_cap`); }
      });
    }
  });
}

export async function runSmoke(options) {
  const o = validateSmokeOptions(options);
  assert.equal(process.platform, 'linux', 'Linux process-group policy required');
  assert.equal(encodeExactJson(parseExactJson('{"mask":18446744073709551615}')), '{"mask":18446744073709551615}');
  assert.equal(await realpath(dirname(o.outputDirectory)), dirname(o.outputDirectory), 'output parent redirects');
  diskReserve(dirname(o.outputDirectory));
  await mkdir(o.outputDirectory, { mode: 0o700 });
  const handles = {};
  try {
    for (const name of ['inspection.stdout', 'inspection.stderr', 'inspection-execution.json', 'smoke.json'])
      handles[name] = await open(join(o.outputDirectory, name), 'wx', 0o600);
  } catch (error) {
    await Promise.allSettled(Object.values(handles).map(handle => handle.close())); throw error;
  }
  const pins = new Map();
  let failure, inspection, session, expectation, report;
  const retain = async (filename, maximum, content = false) => {
    const item = await measure(filename, maximum, content);
    assert(!pins.has(filename), 'duplicate measurement role'); pins.set(filename, { pin: item.pin, maximum }); return item;
  };
  const recheck = async () => {
    for (const [filename, { pin, maximum }] of pins) assert.deepEqual((await measure(filename, maximum)).pin, pin, 'retained file changed');
  };
  try {
    const input = {};
    for (const [key, filename, maximum] of [
      ['debugger', o.debuggerPath, SMOKE_BOUNDS.executableBytes], ['inspector', o.inspectorPath, SMOKE_BOUNDS.executableBytes],
      ['kir', o.kirPath, SMOKE_BOUNDS.kirBytes], ['request', o.requestPath, SMOKE_BOUNDS.requestBytes],
      ['driver', fileURLToPath(import.meta.url), SMOKE_BOUNDS.scriptBytes],
      ['client', fileURLToPath(new URL('./ordered-program-debugger-client.mjs', import.meta.url)), SMOKE_BOUNDS.scriptBytes],
      ['shared_utility', fileURLToPath(SHARED_UTILITY_URL), SMOKE_BOUNDS.scriptBytes],
    ]) { input[key] = await retain(filename, maximum, key === 'request'); assert(input[key].pin.bytes > 0, 'nonempty input required'); }
    const request = parseExactJson(new TextDecoder('utf-8', { fatal: true }).decode(input.request.content));
    expectation = deriveRequestExpectation(request, o);
    await recheck(); diskReserve(o.outputDirectory);
    const result = await inspectProcess(o.inspectorPath, [o.kirPath, o.requestPath], o.outputDirectory);
    await handles['inspection.stdout'].writeFile(result.stdout);
    await handles['inspection.stderr'].writeFile(result.stderr);
    const execution = { executable: input.inspector.pin, args: [o.kirPath, o.requestPath],
      inputs: [input.kir.pin, input.request.pin], code: result.code, signal: result.signal, reason: result.reason,
      truncated: result.truncated, forced_drain: result.forced_drain, elapsed_ms: result.elapsed_ms,
      stdout: { bytes: result.stdout.length, sha256: sha(result.stdout) }, stderr: { bytes: result.stderr.length, sha256: sha(result.stderr) } };
    const executionBytes = Buffer.from(encodeExactJson(execution) + '\n');
    assert(executionBytes.length <= SMOKE_BOUNDS.executionRecordBytes, 'inspection execution record bound');
    await handles['inspection-execution.json'].writeFile(executionBytes);
    for (const [name, bytes, maximum] of [
      ['inspection.stdout', result.stdout, SMOKE_BOUNDS.inspectionStdoutBytes],
      ['inspection.stderr', result.stderr, SMOKE_BOUNDS.inspectionStderrBytes],
      ['inspection-execution.json', executionBytes, SMOKE_BOUNDS.executionRecordBytes],
    ]) {
      const saved = await retain(join(o.outputDirectory, name), maximum);
      assert.equal(saved.pin.bytes, bytes.length); assert.equal(saved.pin.sha256, sha(bytes), 'saved inspection bytes changed');
    }
    await recheck();
    assert.equal(result.code, 0); assert.equal(result.signal, null); assert.equal(result.reason, null);
    assert.equal(result.truncated, false); assert.equal(result.forced_drain, false); assert.equal(result.stderr.length, 0);
    const text = new TextDecoder('utf-8', { fatal: true }).decode(result.stdout);
    assert(text.endsWith('\n') && text.indexOf('\n') === text.length - 1, 'one exact inspector JSON line required');
    inspection = parseExactJson(text);
    const owner = validateInspection(inspection, { kirBytes: input.kir.pin.bytes, kernel: expectation.kernel,
      registerPlan: o.registerPlan, descriptors: o.descriptors, sourceIds: o.sourceIds });
    diskReserve(o.outputDirectory);
    session = await runDebuggerQualification({ debuggerPath: o.debuggerPath, kirPath: o.kirPath, requestPath: o.requestPath,
      outputDirectory: join(o.outputDirectory, 'session'), debuggerFileSha256: input.debugger.pin.sha256,
      kirFileSha256: input.kir.pin.sha256, requestFileSha256: input.request.pin.sha256,
      expected: { ...owner, beforeInputs: expectation.beforeInputs, afterResult: expectation.afterResult,
        lane: expectation.lane, workgroup: expectation.workgroup, wave: expectation.wave,
        expectedAccessCount: expectation.expectedAccessCount, expectedAccessByteOffset: expectation.expectedAccessByteOffset,
        outputMemory: expectation.outputMemory } });
    assert.equal(session.status, 'passed');
    for (const [name, maximum] of [['requests.jsonl', BOUNDS.requestBytes], ['responses.jsonl', BOUNDS.stdoutBytes],
      ['stderr.log', BOUNDS.stderrBytes], ['observation.json', BOUNDS.lineBytes]]) {
      const item = await retain(join(o.outputDirectory, 'session', name), maximum);
      if (name !== 'observation.json') assert.deepEqual({ bytes: item.pin.bytes, sha256: item.pin.sha256 }, session.artifacts[name]);
    }
    await recheck(); diskReserve(o.outputDirectory);
    report = { status: 'passed', commands: session.commands, configuration_identity: session.configurationIdentity,
      canonical: inspection.canonical, coordinate: inspection.coordinate, raw_block_id: inspection.raw_block_id,
      input_value_ids: inspection.input_value_ids, result_value_id: inspection.result_value_id,
      declared_register_plan: inspection.register_plan, declared_program: inspection.declared_program,
      declared_source_ids: inspection.declared_source_ids };
  } catch (error) { failure = error; report = { status: 'failed', error: String(error.message).slice(0, 2048) }; }
  finally {
    try {
      report = { kind: 'ordered_program_debugger_smoke_observation_draft_v1', ...report, options: o,
        expectation, retained_file_pins: [...pins.values()].map(item => item.pin), bounds: SMOKE_BOUNDS,
        selected_lane: 0, all_64_lanes_output_memory_and_access_history_checked: report.status === 'passed',
        all_64_lane_logical_values_checked: false, foreign_session_tokens_checked: false,
        source_authentication: false, hardware_observed: false, physical_register_values: 'unavailable',
        instruction_microsteps: 'unavailable', grants_proof_or_resume_authority: false,
        grants_artifact_or_launch_authority: false };
      await handles['smoke.json'].writeFile(encodeExactJson(report) + '\n');
    } finally {
      const closed = await Promise.allSettled(Object.values(handles).map(handle => handle.close()));
      const rejected = closed.find(item => item.status === 'rejected'); if (rejected) throw rejected.reason;
    }
  }
  if (failure) { const error = new Error(`Diagnostic smoke failed; evidence retained at ${o.outputDirectory}: ${report.error}`); error.report = report; throw error; }
  return report;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const report = await runSmoke(parseSmokeArguments(process.argv.slice(2)));
    console.log(encodeExactJson({ status: report.status, output: report.options.outputDirectory, commands: report.commands,
      observation_only: true, hardware_observed: false }));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}

