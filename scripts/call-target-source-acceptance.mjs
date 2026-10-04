#!/usr/bin/env node
// Additive normal-CLI re-observation of an unchanged const-u32 source capture.
// Importing exposes synchronous pure validators only. No exporter or simulator is run.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { runNavigationCommand } from './authoring-navigation-v1-process.mjs';
import { requireDiskReserve } from './assembly-region-worker-prototype.mjs';
import { LABELS, INPUTS, LENGTHS, LIMITS as SOURCE_LIMITS, parseJson, sha256, oracle,
  validateSummary, validateOperation, validatePage, validateRoster, validateJoins,
  inspectInstances, directConstant, selectBaseline, validateMaterialization, sourceVariant, request, validateSimulation,
} from './const-u32-helper-source-smoke.mjs';

const KiB = 1024, MiB = KiB * KiB;
export const LIMITS = Object.freeze({ stages: 64, command_ms: 30000, wall_ms: 300000,
  stream_bytes: MiB, output_bytes: 64 * MiB, output_files: 256, selected_pins: 768,
  selected_bytes: 768 * MiB, cumulative_read_bytes: 2 * 1024 * MiB, file_bytes: 512 * MiB,
  capture_owned_bytes: 64 * MiB, receipt_bytes: MiB, bundle_bytes: 4 * MiB, operations: 512,
  failure_reserve_bytes: 64 * KiB });
export const AUTHORITY = Object.freeze({ observation_only: true, authenticates_compiler_execution: false,
  source_authenticated: false, grants_proof_authority: false, grants_production_resume: false,
  grants_load_or_launch: false });
const FIXTURE = 'crates/rustc-codegen-fe2o3/tests/fixtures/ordinary-bitwise-promotion-v1';
const UNKNOWN_KERNEL = 'unavailable_in_current_public_observation';
const PRIOR_EDGE = 'unavailable_in_current_public_operation_projection';
const PRIOR_KEYS = ['schema','status','origin','authority','fixture','original_source_sha256','selector',
  'materialized_helper_sha256','actual_exports','whole_kernel_simulations','exact_cli_refusals','variants',
  'stages','retained_file_pins','retained_pin_bytes','distinct_scalar_function_instances',
  'exact_kernel_to_helper_call_edges','source_authentication','compiler_closure_attestation','protected_proof',
  'production_resume','hardware_observed','native_qualified','physical_register_or_helper_abi_qualified',
  'milestone_completion','limits','task_resource_accounting'];
const PRIOR_FALSE = ['source_authentication','compiler_closure_attestation','protected_proof','production_resume',
  'hardware_observed','native_qualified','physical_register_or_helper_abi_qualified','milestone_completion'];
const PIN_KEYS = ['path','bytes','sha256','device','inode','mode','nlink','mtime_ns','ctime_ns'];
const STAGE_KEYS = ['label','executable','args','stdin_bytes','stdin_sha256','code','signal','reason',
  'elapsed_ms','stdout_bytes','stdout_sha256','stderr_bytes','stderr_sha256'];
const strictText = bytes => new TextDecoder('utf-8', { fatal: true }).decode(bytes);
function exact(value, keys) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), 'closed fields');
}
function nat(value, max = Number.MAX_SAFE_INTEGER, min = 0) {
  assert.ok(Number.isSafeInteger(value) && value >= min && value <= max, 'bounded integer'); return value;
}
function digest(value) { assert.equal(typeof value, 'string'); assert.match(value, /^[0-9a-f]{64}$/u); assert.notEqual(value, '0'.repeat(64)); return value; }
function text(value, cap = 4096) {
  assert.ok(typeof value === 'string' && value.length > 0 && Buffer.byteLength(value) <= cap &&
    !/[\u0000-\u001f\u007f]/u.test(value), 'bounded text'); return value;
}
function absolute(value) { text(value); assert.ok(path.isAbsolute(value) && path.resolve(value) === value); return value; }
function inside(parent, child) { return child === parent || child.startsWith(parent + '/'); }
function scalar(value) { exact(value, ['value','ty']); nat(value.value, 65535); text(value.ty); }
function pinShape(value) {
  exact(value, PIN_KEYS); absolute(value.path); nat(value.bytes, LIMITS.file_bytes); digest(value.sha256);
  for (const field of PIN_KEYS.slice(3)) { assert.equal(typeof value[field], 'string'); assert.match(value[field], /^(0|[1-9][0-9]*)$/u); }
}
export function callSelector(summary, operation) {
  validateSummary(summary); validateOperation(operation);
  return { bundle_identity: summary.bundle_identity, canonical_kir_digest: summary.canonical_kir_digest,
    target: summary.target, operations: [structuredClone(operation.coordinate)] };
}
/** Strict static same-owner report join. Duplicate caller operands are deliberately retained. */
export function validateCallTarget(report, summary, selected) {
  validateSummary(summary); validateOperation(selected); assert.equal(selected.kind, 'call');
  exact(report, ['schema','authority','selector','call','caller','callee','arguments','results','correspondence',
    'transitive_helper_closure','dynamic_invocation','physical_abi']);
  assert.equal(report.schema, 'fe2o3-authoring-call-target-v1'); assert.deepEqual(report.authority, AUTHORITY);
  assert.deepEqual(report.selector, callSelector(summary, selected)); assert.deepEqual(report.call, selected);
  exact(report.caller,['function','function_id','role','kernel_registrations']);
  nat(report.caller.function,65535);text(report.caller.function_id);
  assert.equal(report.caller.function,selected.coordinate.function,'exact retained caller ordinal');
  assert.equal(report.caller.function_id,selected.function_name,'exact retained caller FunctionId');
  assert.ok(['kernel_entry','internal_helper'].includes(report.caller.role));
  assert.ok(Array.isArray(report.caller.kernel_registrations)&&report.caller.kernel_registrations.length<=64);
  const kernelIds=new Set();let priorKernel=-1;
  for(const registration of report.caller.kernel_registrations) {
    exact(registration,['kernel','kernel_id','entry_function_id']);
    nat(registration.kernel,65535);text(registration.kernel_id);text(registration.entry_function_id);
    assert.ok(registration.kernel>priorKernel,'complete registrations retain strict canonical ordinal order');
    priorKernel=registration.kernel;assert.ok(!kernelIds.has(registration.kernel_id),'unique retained KernelId');
    kernelIds.add(registration.kernel_id);
    assert.equal(registration.entry_function_id,report.caller.function_id,'same-owner kernel entry relation');
  }
  if(report.caller.role==='kernel_entry') assert.ok(report.caller.kernel_registrations.length>0,'registered kernel entry');
  else assert.equal(report.caller.kernel_registrations.length,0,'non-kernel caller cannot carry registrations');
  exact(report.callee, ['function','function_id','role']);
  nat(report.callee.function, 65535); text(report.callee.function_id); assert.equal(report.callee.role, 'internal_helper');
  assert.equal(report.correspondence, 'exact_retained_call_operand_to_formal_position');
  assert.equal(report.transitive_helper_closure, 'not_traversed');
  assert.equal(report.dynamic_invocation, 'unavailable_static_call_site_only');
  assert.equal(report.physical_abi, 'unavailable_logical_canonical_call');
  for (const field of ['arguments','results']) assert.ok(Array.isArray(report[field]) && report[field].length <= 64);
  assert.equal(report.arguments.length, selected.inputs.length); assert.equal(report.results.length, selected.results.length);
  const formals = new Set();
  report.arguments.forEach((row, index) => {
    exact(row, ['position','operand','formal']); assert.equal(row.position, index);
    scalar(row.operand); scalar(row.formal); assert.deepEqual(row.operand, selected.inputs[index]);
    assert.equal(row.formal.ty, row.operand.ty, 'actual positional argument/formal type');
    assert.ok(!formals.has(row.formal.value), 'distinct formal definitions'); formals.add(row.formal.value);
  });
  report.results.forEach((row, index) => {
    exact(row, ['position','result','signature_type']); assert.equal(row.position, index);
    scalar(row.result); text(row.signature_type); assert.deepEqual(row.result, selected.results[index]);
    assert.equal(row.signature_type, row.result.ty, 'actual result signature type');
  });
  return report;
}
function one(items, message) { assert.equal(items.length, 1, message); return items[0]; }
function typeU32(values) { values.forEach(value => assert.equal(value.ty, 'Scalar(U32)')); }
/** Joins complete operation-bearing bodies to the exact R2 same-caller registration projection. */
export function observeCallEdges(label, summary, operations, reports, expectedKernel) {
  validateSummary(summary); validateRoster(operations, summary);
  const instances = inspectInstances(label, summary, operations), names = new Map();
  for (const op of operations) {
    const previous = names.get(op.coordinate.function);
    if (previous !== undefined) assert.equal(op.function_name, previous, 'one exact opaque FunctionId per ordinal');
    names.set(op.coordinate.function, op.function_name);
  }
  const calls = operations.filter(op => op.kind === 'call');
  assert.ok(Array.isArray(reports)); assert.equal(reports.length, calls.length, 'every retained Call queried exactly once');
  if (label === 'baseline') {
    assert.equal(calls.length, 0, 'ordinary baseline has no helper calls');
    assert.equal(names.size, 1, 'one operation-bearing baseline function');
    return { caller_function: [...names.keys()][0], caller_role: UNKNOWN_KERNEL,
      registered_kernel_relation: UNKNOWN_KERNEL, edges: [], helper_call_operations: 0,
      body_return_mapping: 'unavailable_not_projected', transitive_helper_closure: 'not_traversed' };
  }
  const helperFunctions = new Set(instances.map(item => item.coordinate.function));
  assert.ok(!calls.some(op => helperFunctions.has(op.coordinate.function)), 'no nested Call in selected helper operation rosters');
  assert.equal(calls.length, label === 'two' ? 2 : 1, 'exact finite static Call count');
  const caller = calls[0].coordinate.function;
  assert.ok(calls.every(op => op.coordinate.function === caller), 'one operation-bearing caller');
  assert.ok(!helperFunctions.has(caller));
  assert.equal(names.size, helperFunctions.size + 1, 'no unrelated operation-bearing function');
  assert.ok([...names.keys()].every(fn => fn === caller || helperFunctions.has(fn)));
  text(expectedKernel);
  const used = new Set(), edges = calls.map((call, index) => {
    const report = validateCallTarget(reports[index], summary, call);
    assert.equal(report.caller.role,'kernel_entry','closed source profile calls must have an actual kernel-entry caller');
    if(index>0) assert.deepEqual(report.caller,reports[0].caller,'all actual static sites share one exact caller registration roster');
    one(report.caller.kernel_registrations.filter(item=>item.kernel_id===expectedKernel),
      'exact retained simulator-request KernelId must register this caller');
    assert.equal(call.source_binding,'bundle_content_bound_not_authenticated');
    assert.equal(call.source_spans.length,1,'closed source fixture needs one actual call-site span');
    const instance = one(instances.filter(item => item.coordinate.function === report.callee.function),
      'callee ordinal must be an actual observed typed-constant helper instance');
    assert.ok(!used.has(report.callee.function), 'each intended specialization has one observed static site'); used.add(report.callee.function);
    const body = operations.filter(op => op.coordinate.function === report.callee.function);
    assert.equal(body.length, 2, 'closed helper has exactly constant plus scalar instruction operations');
    assert.ok(body.every(op => op.function_name === report.callee.function_id), 'exact callee FunctionId join, no prefix inference');
    const assembly = one(body.filter(op => op.mnemonic === 'v_or_b32'), 'one exact helper scalar instruction');
    assert.ok(assembly.source_spans.every(span=>span.file_identity===call.source_spans[0].file_identity),
      'actual caller/helper spans must name the same retained source file identity');
    const constant = directConstant(operations, assembly);
    assert.deepEqual(constant, instance.typed_constant);
    assert.equal(report.arguments.length, 1); assert.equal(report.results.length, 1);
    const runtime = one(assembly.inputs.filter(input => input.value !== constant.value.value), 'one helper runtime input');
    assert.deepEqual(report.arguments[0].formal, runtime, 'formal is the actual scalar instruction runtime input');
    assert.ok(!body.some(op => op.results.some(result => result.value === runtime.value)), 'runtime formal is not an operation result');
    typeU32([...call.inputs, ...call.results, runtime]);
    assert.equal(report.results[0].signature_type, 'Scalar(U32)');
    return { call_coordinate: call.coordinate, callee: report.callee, typed_constant: constant,
      source_reference: instance.source_reference, arguments: report.arguments, results: report.results,
      call_source_spans: call.source_spans, body_operation_coordinates: body.map(op => op.coordinate),
      helper_call_operations: 0, body_return_mapping: 'unavailable_not_projected' };
  });
  assert.equal(used.size, helperFunctions.size);
  // Bind each finite fixture call to an actual observed caller dataflow choice.
  // This is not a map from Rust parameter names to SSA IDs.
  const callerOps = operations.filter(op => op.coordinate.function === caller);
  const low = one(callerOps.filter(op => op.kind === 'binary' && op.semantic_detail === 'BitAnd'), 'one observed caller mask');
  const mask = directConstant(operations, low); assert.equal(mask.original_value, 255);
  const xorInput = one(low.inputs.filter(input => input.value !== mask.value.value), 'one mask runtime input');
  const xor = one(callerOps.filter(op => op.kind === 'binary' && op.semantic_detail === 'BitXor' &&
    op.results.length === 1 && op.results[0].value === xorInput.value), 'actual preceding XOR definition');
  assert.deepEqual(xor.results[0], xorInput); assert.equal(xor.inputs.length, 2); typeU32(xor.inputs);
  const lowEdge = one(edges.filter(edge => edge.typed_constant.original_value === (label === 'two' || label === 'default256' ? 256 : 512)),
    'one mask-input specialization');
  assert.deepEqual(lowEdge.arguments[0].operand, low.results[0], 'actual caller mask result reaches intended specialization');
  let outputValue = lowEdge.results[0].result;
  if (label === 'two') {
    const second = one(edges.filter(edge => edge.typed_constant.original_value === 512), 'one second specialization');
    assert.deepEqual(second.arguments[0].operand, xor.inputs[0], 'second source choice joins the observed baseline XOR lhs, not a guessed source name');
    const combined = one(callerOps.filter(op => op.kind === 'binary' && op.semantic_detail === 'BitXor' &&
      op.inputs.length === 2 && op.inputs[0].value === lowEdge.results[0].result.value &&
      op.inputs[1].value === second.results[0].result.value), 'actual two-call result combination');
    assert.equal(combined.results.length, 1); typeU32([...combined.inputs, ...combined.results]); outputValue = combined.results[0];
  }
  const store = one(callerOps.filter(op => op.kind === 'store'), 'one observed output store');
  assert.equal(store.inputs.length, 2); assert.deepEqual(store.inputs[1], outputValue, 'actual call result reaches output store');
  return { caller_function: caller, caller_role:'kernel_entry', caller:reports[0].caller,
    requested_kernel_id:expectedKernel, registered_kernel_relation:'exact_same_owner_kernel_registration_to_caller',
    edges, helper_call_operations: 0, body_return_mapping: 'unavailable_not_projected',
    source_parameter_name_to_ssa_mapping: 'not_supplied', transitive_helper_closure: 'not_traversed' };
}
export function validateCallSourceSlices(label, source, observation) {
  assert.ok(Buffer.isBuffer(source)&&source.length>0&&source.length<=SOURCE_LIMITS.source_bytes);
  strictText(source);
  for(const edge of observation.edges) {
    assert.equal(edge.call_source_spans.length,1);
    const span=edge.call_source_spans[0];assert.equal(span.display_path,'src/lib.rs');
    const start=nat(Number(span.byte_start),source.length),end=nat(Number(span.byte_end),source.length,start);
    assert.equal(String(start),span.byte_start);assert.equal(String(end),span.byte_end);
    const operand=label==='two'&&edge.typed_constant.original_value===512?'a':'low';
    const expected='specialized_or::<'+edge.typed_constant.original_value+'u32>('+operand+')';
    assert.equal(strictText(source.subarray(start,end)),expected,'actual retained source call span bytes');
    const prefix=strictText(source.subarray(0,start)),lines=prefix.split('\n');
    assert.equal(span.line,lines.length);assert.equal(span.column,lines.at(-1).length+1);
  }
  return {call_spans_checked:observation.edges.length,
    source_file_identity_domain:'retained_owner_identity_not_reinterpreted_as_source_sha256',
    source_authentication:false};
}
export function validatePriorReceipt(receipt) {
  exact(receipt, PRIOR_KEYS);
  assert.equal(receipt.schema, 'task-const-u32-source-acceptance-v1'); assert.equal(receipt.status, 'passed');
  assert.equal(receipt.origin, 'current_ordinary_source_normal_bundle_v6_and_author_cli');
  assert.equal(receipt.authority, 'observation_only'); assert.equal(receipt.fixture, FIXTURE);
  assert.equal(receipt.actual_exports, 5); assert.equal(receipt.whole_kernel_simulations, 150); assert.equal(receipt.exact_cli_refusals, 3);
  for (const key of ['original_source_sha256','materialized_helper_sha256']) digest(receipt[key]);
  assert.equal(receipt.distinct_scalar_function_instances, 'observed_typed_u32_constants_and_inert_function_references');
  assert.equal(receipt.exact_kernel_to_helper_call_edges, PRIOR_EDGE);
  for (const key of PRIOR_FALSE) assert.equal(receipt[key], false);
  assert.deepEqual(receipt.limits, SOURCE_LIMITS);
  assert.equal(receipt.task_resource_accounting, 'external_current_scope_supervisor_required');
  assert.ok(Array.isArray(receipt.variants)); assert.deepEqual(receipt.variants.map(v => v.label), LABELS);
  for (const variant of receipt.variants) {
    exact(variant, ['label','source_sha256','bundle_file_sha256','bundle_bytes','summary','instances','simulations','exact_kernel_to_helper_call_edges']);
    digest(variant.source_sha256); digest(variant.bundle_file_sha256); nat(variant.bundle_bytes, LIMITS.bundle_bytes, 1);
    validateSummary(variant.summary); assert.equal(variant.exact_kernel_to_helper_call_edges, PRIOR_EDGE);
    assert.ok(Array.isArray(variant.instances) && variant.instances.length <= 2);
    assert.ok(Array.isArray(variant.simulations)); assert.equal(variant.simulations.length, 30);
    let index = 0;
    for (let input = 0; input < INPUTS.length; input++) for (const elements of LENGTHS) for (let replay = 0; replay < 2; replay++) {
      assert.deepEqual(variant.simulations[index++], { input, elements, replay,
        expected_word: oracle(variant.label, INPUTS[input]), output_words: elements,
        backing_bytes: 8 + 4 * elements, guard_bytes: 8 }, 'unchanged prior finite-oracle metadata');
    }
  }
  validateJoins(receipt.variants);
  assert.ok(Array.isArray(receipt.retained_file_pins) && receipt.retained_file_pins.length <= SOURCE_LIMITS.pins);
  const pins = new Map(); let bytes = 0;
  for (const pin of receipt.retained_file_pins) {
    pinShape(pin); assert.ok(!pins.has(pin.path), 'unique prior pinned path'); pins.set(pin.path, pin); bytes += pin.bytes;
  }
  assert.equal(bytes, receipt.retained_pin_bytes); nat(bytes, SOURCE_LIMITS.pin_bytes);
  assert.ok(Array.isArray(receipt.stages) && receipt.stages.length <= SOURCE_LIMITS.stages);
  const stages = new Map();
  for (const stage of receipt.stages) {
    exact(stage, STAGE_KEYS); text(stage.label, 128); absolute(stage.executable);
    assert.ok(!stages.has(stage.label), 'unique prior stage label'); stages.set(stage.label, stage);
    assert.ok(Array.isArray(stage.args) && stage.args.length <= 32); stage.args.forEach(arg => text(arg, 16384));
    for (const field of ['stdin_sha256','stdout_sha256','stderr_sha256']) digest(stage[field]);
    nat(stage.stdin_bytes, LIMITS.bundle_bytes); nat(stage.stdout_bytes, MiB); nat(stage.stderr_bytes, MiB);
    nat(stage.elapsed_ms, 1200000); assert.equal(stage.signal, null); assert.equal(stage.reason, null);
    assert.equal(stage.code, stage.label.startsWith('refuse-') ? 1 : 0);
    assert.ok(pins.has(stage.executable), 'prior command executable has a retained pin');
  }
  assert.equal([...stages.keys()].filter(name => name.startsWith('refuse-')).length, 3);
  assert.equal([...stages.keys()].filter(name => name.endsWith('-export')).length, 5);
  assert.equal([...stages.keys()].filter(name => /-case-[0-4]-length-(0|1|65)-replay-[01]$/u.test(name)).length, 150);
  return { pins, stages };
}
/** Recheck retained requests/results only; no simulator is invoked and no receipt is rewritten. */
export function validatePriorSimulationRequests(label, summary, records, capture) {
  assert.ok(LABELS.includes(label));validateSummary(summary);absolute(capture);
  assert.ok(Array.isArray(records));assert.equal(records.length,INPUTS.length*LENGTHS.length);
  const simulations=[];let index=0,kernel;
  for(let input=0;input<INPUTS.length;input++)for(const elements of LENGTHS) {
    const record=records[index++];exact(record,['input','elements','request_bytes','replays']);
    assert.equal(record.input,input);assert.equal(record.elements,elements);
    assert.ok(Buffer.isBuffer(record.request_bytes)&&record.request_bytes.length<=MiB);
    const query=parseJson(record.request_bytes);assert.deepEqual(query,request(INPUTS[input],elements));
    text(query.kernel);if(kernel===undefined)kernel=query.kernel;else assert.equal(query.kernel,kernel);
    assert.ok(Array.isArray(record.replays));assert.equal(record.replays.length,2);
    const name=label+'-case-'+input+'-length-'+elements;
    for(let replay=0;replay<2;replay++) {
      const observation=record.replays[replay];exact(observation,['stage','result_bytes']);
      const stage=observation.stage;exact(stage,STAGE_KEYS);
      assert.equal(stage.label,name+'-replay-'+replay);
      assert.deepEqual(stage.args,['--bundle-v6',path.join(capture,label+'.fe2sim'),
        '--request',path.join(capture,name+'.request.json')],'exact preserved simulator invocation paths');
      assert.equal(stage.code,0);assert.equal(stage.signal,null);assert.equal(stage.reason,null);
      assert.equal(stage.stdin_bytes,0);assert.equal(stage.stdin_sha256,sha256(Buffer.alloc(0)));
      assert.equal(stage.stderr_bytes,0);assert.equal(stage.stderr_sha256,sha256(Buffer.alloc(0)));
      assert.ok(Buffer.isBuffer(observation.result_bytes)&&observation.result_bytes.length<=MiB);
      assert.equal(stage.stdout_bytes,observation.result_bytes.length);assert.equal(stage.stdout_sha256,sha256(observation.result_bytes));
      const result=parseJson(observation.result_bytes);
      simulations.push({input,elements,replay,...validateSimulation(result,query,summary,label,INPUTS[input],elements)});
    }
  }
  return {kernel_id:kernel,request_records:records.length,simulations_revalidated:simulations.length,
    simulations_executed:0,simulations};
}
export function validateRefusal(result, kind, help) {
  assert.equal(result.reason, null, 'transport failure is not a refusal');
  assert.equal(result.signal, null); assert.equal(result.code, 1); assert.equal(result.stdout.length, 0);
  const stderr = strictText(result.stderr);
  const exactErrors = {
    stale_bundle: 'stale or malformed exact V6 bundle identity',
    stale_canonical: 'stale or malformed exact canonical V11 identity',
    target: 'selector target differs from the exact bundle target',
    not_call: 'selected canonical operation is not a direct Call',
  };
  if (Object.hasOwn(exactErrors, kind)) assert.equal(stderr, 'fe2o3-author: ' + exactErrors[kind] + '\n', 'exact first refusal');
  else if (kind === 'selector_callee') assert.match(stderr,
    /^fe2o3-author: invalid selector: unknown field \x60callee\x60, expected one of \x60bundle_identity\x60, \x60canonical_kir_digest\x60, \x60target\x60, \x60operations\x60 at line 1 column [1-9][0-9]*\n$/u);
  else if (kind === 'argv_callee') {
    assert.ok(Buffer.isBuffer(help));
    const usage = strictText(help);
    assert.ok(usage.startsWith('usage: fe2o3-author inspect\n') && usage.endsWith('\n') &&
      usage.includes('\n       fe2o3-author call-target --selector JSON\n') &&
      !usage.includes('fe2o3-author:'), 'current exact normal help required');
    assert.equal(stderr, 'fe2o3-author: ' + usage, 'exact complete current usage refusal');
  }
  else assert.fail('unsupported refusal expectation');
}
export function parseOptions(argv) {
  const names = ['repo','author','library-dir','capture','receipt-bytes','receipt-sha256','mode','output'];
  assert.equal(argv.length, names.length * 2); const opt = {};
  for (let i = 0; i < argv.length; i += 2) {
    assert.ok(argv[i].startsWith('--')); const key = argv[i].slice(2);
    assert.ok(names.includes(key) && !Object.hasOwn(opt, key), 'closed unique options'); opt[key] = argv[i + 1];
  }
  for (const key of ['repo','author','library-dir','capture','output']) absolute(opt[key]);
  assert.match(opt['receipt-bytes'], /^[1-9][0-9]*$/u); opt['receipt-bytes'] = nat(Number(opt['receipt-bytes']), MiB, 1);
  digest(opt['receipt-sha256']);
  assert.ok(['current_tool_capture','historical_reobservation'].includes(opt.mode));
  for (const source of [opt.repo,opt.capture,path.dirname(opt.author),opt['library-dir']])
    assert.ok(!inside(source,opt.output) && !inside(opt.output,source), 'output/input directory separation');
  return Object.freeze(opt);
}
function fileIdentity(st) {
  return { device:String(st.dev), inode:String(st.ino), mode:String(st.mode), nlink:String(st.nlink),
    mtime_ns:String(st.mtimeNs), ctime_ns:String(st.ctimeNs) };
}
async function run(opt) {
  const started = Date.now(), pins = new Map(), stages = [], observations = [];
  let selectedBytes = 0, readBytes = 0, outputBytes = 0, outputFiles = 0;
  const check = () => { assert.ok(Date.now() - started < LIMITS.wall_ms, 'cooperative wall bound'); requireDiskReserve(path.dirname(opt.output)); };
  for (const dir of [opt.repo,opt.capture,opt['library-dir'],path.dirname(opt.author),path.dirname(opt.output)]) {
    assert.equal(fs.realpathSync(dir), dir); assert.ok(fs.lstatSync(dir).isDirectory());
  }
  assert.ok(!fs.existsSync(opt.output), 'new-only output'); fs.mkdirSync(opt.output, { mode:0o700 });
  function read(file, cap, retain = false) {
    check(); absolute(file); assert.equal(fs.realpathSync(file), file, 'canonical input path');
    const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
    try {
      const before = fs.fstatSync(fd, { bigint:true }); assert.ok(before.isFile() && before.size >= 0n && before.size <= BigInt(cap));
      const hash = createHash('sha256'), chunks = [], block = Buffer.alloc(64 * KiB); let count = 0;
      for (;;) {
        check(); const size = fs.readSync(fd, block, 0, Math.min(block.length, Number(before.size) - count + 1), null);
        if (!size) break; count += size; readBytes += size;
        assert.ok(count <= Number(before.size) && readBytes <= LIMITS.cumulative_read_bytes);
        hash.update(block.subarray(0,size)); if (retain) chunks.push(Buffer.from(block.subarray(0,size)));
      }
      assert.equal(BigInt(count), before.size);
      for (const st of [fs.fstatSync(fd,{bigint:true}),fs.lstatSync(file,{bigint:true})]) {
        assert.deepEqual(fileIdentity(st),fileIdentity(before)); assert.equal(st.size,before.size);
      }
      assert.equal(fs.realpathSync(file),file);
      return { pin:{path:file,bytes:count,sha256:hash.digest('hex'),...fileIdentity(before)}, bytes:retain ? Buffer.concat(chunks) : undefined };
    } finally { fs.closeSync(fd); }
  }
  function keep(file, cap, retain = false, expected) {
    const value = read(file,cap,retain), old = pins.get(file);
    if (expected) assert.deepEqual(value.pin,expected,'exact retained artifact identity changed');
    if (old) assert.deepEqual(value.pin,old.pin,'selected input changed');
    else { assert.ok(pins.size < LIMITS.selected_pins); selectedBytes += value.pin.bytes;
      assert.ok(selectedBytes <= LIMITS.selected_bytes); pins.set(file,{pin:value.pin,cap}); }
    return value.bytes;
  }
  function save(name, bytes) {
    assert.match(name,/^[a-z0-9][a-z0-9.-]*$/u); const length = Buffer.byteLength(bytes);
    const ceiling = name === 'failure.json' ? LIMITS.output_bytes : LIMITS.output_bytes - LIMITS.failure_reserve_bytes;
    assert.ok(++outputFiles <= LIMITS.output_files && outputBytes + length <= ceiling); outputBytes += length;
    const file = path.join(opt.output,name); fs.writeFileSync(file,bytes,{flag:'wx',mode:0o600});
    return {path:file,bytes:length,sha256:sha256(bytes)};
  }
  const saveJson = (name,value) => { const bytes=Buffer.from(JSON.stringify(value,null,2)+'\n'); assert.ok(bytes.length<=MiB); return save(name,bytes); };
  function unchanged() { for (const {pin,cap} of pins.values()) assert.deepEqual(read(pin.path,cap).pin,pin,'selected source/tool/receipt input changed'); }
  const env = Object.freeze({PATH:'/usr/bin:/bin',LANG:'C',LC_ALL:'C',LD_LIBRARY_PATH:opt['library-dir']+':'+path.dirname(opt.author)});
  let help;
  async function command(label,args,input,refusal) {
    check(); assert.ok(stages.length < LIMITS.stages);
    assert.ok(args.length <= 8 && args.every(arg => typeof arg === 'string' && Buffer.byteLength(arg)<=16384));
    keep(opt.author,LIMITS.file_bytes); assert.ok(Buffer.isBuffer(input) && input.length<=LIMITS.bundle_bytes);
    const result = await runNavigationCommand({executable:opt.author,args,cwd:opt.repo,env,input,
      timeoutMs:Math.min(LIMITS.command_ms,Math.max(1,LIMITS.wall_ms-(Date.now()-started))),
      outputCap:LIMITS.stream_bytes,guard:check});
    const stdout=save(label+'.stdout',result.stdout),stderr=save(label+'.stderr',result.stderr);
    stages.push({label,executable:opt.author,args,cwd:opt.repo,stdin_bytes:input.length,stdin_sha256:sha256(input),
      code:result.code,signal:result.signal,reason:result.reason,elapsed_ms:result.elapsed_ms,stdout,stderr});
    if (refusal) validateRefusal(result,refusal,help);
    else { assert.equal(result.code,0,label);assert.equal(result.signal,null);assert.equal(result.reason,null);
      assert.equal(result.stderr.length,0,'normal author query must not emit diagnostics'); }
    keep(opt.author,LIMITS.file_bytes); return result.stdout;
  }
  try {
    const here=path.dirname(fileURLToPath(import.meta.url));
    assert.equal(here,path.join(opt.repo,'scripts'),'stage this reviewed additive script with its exact existing imports');
    for(const name of [path.basename(fileURLToPath(import.meta.url)),'const-u32-helper-source-smoke.mjs',
      'authoring-navigation-v1-process.mjs','ordered-program-source-native.mjs',
      'ordered-program-worker-prototype.mjs','assembly-region-worker-prototype.mjs']) keep(path.join(here,name),MiB);
    for(const relative of ['Cargo.toml','Cargo.lock','rust-toolchain.toml',
      'crates/fe2o3-source-isa-observation/Cargo.toml',
      'crates/fe2o3-source-isa-observation/src/multilevel_authoring_v1.rs',
      'crates/fe2o3-source-isa-observation/src/multilevel_authoring_call_target_v1.rs',
      'crates/fe2o3-source-isa-observation/src/bin/fe2o3-author.rs']) keep(path.join(opt.repo,relative),MiB);
    keep(opt.author,LIMITS.file_bytes); keep(process.execPath,LIMITS.file_bytes);
    const receiptPath=path.join(opt.capture,'receipt.json'),raw=keep(receiptPath,MiB,true);
    assert.equal(raw.length,opt['receipt-bytes']);assert.equal(sha256(raw),opt['receipt-sha256']);
    const prior=parseJson(raw), validated=validatePriorReceipt(prior), owned=new Map(); let ownedBytes=0;
    if(opt.mode==='current_tool_capture') {
      for(const relative of [FIXTURE+'/src/lib.rs',FIXTURE+'/Cargo.toml',FIXTURE+'/Cargo.lock',
        'scripts/const-u32-helper-source-smoke.mjs']) {
        const file=path.join(opt.repo,relative);assert.ok(validated.pins.has(file),'current source owner path must be retained');
        keep(file,MiB,false,validated.pins.get(file));
      }
    }
    for(const [file,pin] of validated.pins) if(inside(opt.capture,file)) {
      assert.notEqual(file,receiptPath,'historical receipt cannot pin itself');
      ownedBytes+=pin.bytes;assert.ok(ownedBytes<=LIMITS.capture_owned_bytes);
      owned.set(file,keep(file,LIMITS.bundle_bytes,true,pin));
    }
    function artifact(relative) {
      const file=path.join(opt.capture,relative);assert.ok(inside(opt.capture,file)&&owned.has(file),'exact owned artifact is absent');
      return owned.get(file);
    }
    // All historical stage streams are the exact retained files, never replacement JSON.
    for(const stage of prior.stages) for(const stream of ['stdout','stderr']) {
      const bytes=artifact(stage.label+'.'+stream);
      assert.equal(bytes.length,stage[stream+'_bytes']);assert.equal(sha256(bytes),stage[stream+'_sha256']);
    }
    const original=artifact('original-source.rs'),helper=artifact('generated-helper.rs');
    assert.equal(sha256(original),prior.original_source_sha256);assert.equal(sha256(helper),prior.materialized_helper_sha256);
    help=await command('current-help',['--help'],Buffer.alloc(0));
    assert.ok(strictText(help).startsWith('usage: fe2o3-author inspect\n') &&
      strictText(help).includes('\n       fe2o3-author call-target --selector JSON\n'),'normal CLI call-target prerequisite');
    const bundles=new Map(), current=new Map();
    for(const previous of prior.variants) {
      const label=previous.label,bundle=artifact(label+'.fe2sim');assert.equal(bundle.length,previous.bundle_bytes);
      assert.equal(sha256(bundle),previous.bundle_file_sha256);bundles.set(label,bundle);
      const source=label==='baseline'?original:artifact((label==='repeat'?'edited512':label)+'-source/src/lib.rs');
      assert.equal(sha256(source),previous.source_sha256);
      if(label!=='baseline') assert.deepEqual(source,sourceVariant(original,strictText(helper),label==='repeat'?'edited512':label));
      const oldInspect=validated.stages.get(label+'-inspect');assert.ok(oldInspect);
      assert.deepEqual(oldInspect.args,['inspect']);assert.equal(oldInspect.stdin_bytes,bundle.length);
      assert.equal(oldInspect.stdin_sha256,sha256(bundle));
      if(opt.mode==='current_tool_capture') {
        assert.equal(oldInspect.executable,opt.author,'source capture must use this exact normal author tool');
        assert.deepEqual(pins.get(opt.author).pin,validated.pins.get(opt.author));
      }
      assert.deepEqual(parseJson(artifact(label+'-inspect.stdout')),previous.summary);
      const summary=parseJson(await command(label+'-inspect',['inspect'],bundle));validateSummary(summary);
      assert.deepEqual(summary,previous.summary,'fresh normal inspection must preserve exact prior bundle/semantic/source identities');
      const operations=[];
      for(let at=0;at<summary.operation_count;) {
        const page=parseJson(await command(label+'-operations-'+at,
          ['operations','--bundle-identity',summary.bundle_identity,'--start',String(at),'--limit','64'],bundle));
        at=validatePage(page,summary,at);operations.push(...page.operations);
      }
      validateRoster(operations,summary);assert.deepEqual(operations,parseJson(artifact(label+'-operations.json')));
      assert.deepEqual(inspectInstances(label,summary,operations),previous.instances);
      const requestRecords=INPUTS.flatMap((_,input)=>LENGTHS.map(elements=>{
        const name=label+'-case-'+input+'-length-'+elements;
        return {input,elements,request_bytes:artifact(name+'.request.json'),replays:[0,1].map(replay=>{
          const stage=validated.stages.get(name+'-replay-'+replay);assert.ok(stage);
          return {stage,result_bytes:artifact(stage.label+'.stdout')};
        })};
      }));
      const retainedSimulation=validatePriorSimulationRequests(label,summary,requestRecords,opt.capture);
      assert.deepEqual(retainedSimulation.simulations,previous.simulations,'exact prior result/oracle metadata');
      const calls=operations.filter(op=>op.kind==='call'),reports=[];
      assert.ok(calls.length <= 2,'closed source fixture Call budget before subprocesses');
      for(let index=0;index<calls.length;index++) reports.push(parseJson(await command(label+'-call-'+index,
        ['call-target','--selector',JSON.stringify(callSelector(summary,calls[index]))],bundle)));
      const observed=observeCallEdges(label,summary,operations,reports,retainedSimulation.kernel_id);
      const sourceSpans=validateCallSourceSlices(label,source,observed);
      saveJson(label+'-call-edges.json',observed);
      current.set(label,{summary,operations,reports,observed});
      observations.push({label,source_sha256:previous.source_sha256,bundle_bytes:bundle.length,bundle_sha256:sha256(bundle),
        summary,instances:previous.instances,source_spans:sourceSpans,
        retained_simulation:{kernel_id:retainedSimulation.kernel_id,request_records:retainedSimulation.request_records,
          simulations_revalidated:retainedSimulation.simulations_revalidated,simulations_executed:0},...observed});
    }
    assert.equal(new Set(observations.map(item=>item.retained_simulation.kernel_id)).size,1,
      'all original requests retain the same explicit kernel selection');
    const baseline=current.get('baseline'),generated=parseJson(artifact('materialized.json'));
    const selection=selectBaseline(baseline.summary,baseline.operations);
    assert.deepEqual(selection.selector,prior.selector);assert.deepEqual(selection.selector,parseJson(artifact('selector.json')));
    validateMaterialization(generated,selection);assert.equal(generated.source,strictText(helper));
    assert.deepEqual(current.get('edited512').reports,current.get('repeat').reports,'repeat exact call reports');
    assert.deepEqual(current.get('edited512').observed,current.get('repeat').observed,'repeat exact static edges');
    const first=current.get('default256'),call=one(first.operations.filter(op=>op.kind==='call'),'default call'),
      selector=callSelector(first.summary,call),empty=Buffer.alloc(0);
    for(const other of ['edited512','two']) await command('refuse-old-selector-'+other,
      ['call-target','--selector',JSON.stringify(selector)],bundles.get(other),'stale_bundle');
    await command('refuse-canonical',['call-target','--selector',JSON.stringify({...selector,canonical_kir_digest:'0'.repeat(64)})],
      bundles.get('default256'),'stale_canonical');
    await command('refuse-target',['call-target','--selector',JSON.stringify({...selector,target:'gfx950:xnack-'})],
      bundles.get('default256'),'target');
    const notCall=first.operations.find(op=>op.kind!=='call');assert.ok(notCall);
    await command('refuse-non-call',['call-target','--selector',JSON.stringify(callSelector(first.summary,notCall))],
      bundles.get('default256'),'not_call');
    await command('refuse-caller-callee-selector',['call-target','--selector',JSON.stringify({...selector,callee:'caller-cannot-choose'})],
      empty,'selector_callee');
    await command('refuse-caller-callee-argv',['call-target','--selector',JSON.stringify(selector),'--callee','caller-cannot-choose'],
      empty,'argv_callee');
    unchanged();
    const report={schema:'task-call-target-source-observation-v2',status:'passed',authority:'observation_only',mode:opt.mode,
      prior_source_receipt:{path:receiptPath,bytes:raw.length,sha256:sha256(raw)},
      historical_receipt_rewritten:false,source_exports_executed:0,simulations_executed:0,
      prior_simulations:150,prior_simulations_revalidated:150,prior_simulation_status:'retained_raw_results_revalidated_with_original_oracle_not_rerun',
      normal_call_target_queries:observations.reduce((count,item)=>count+item.edges.length,0),exact_cli_refusals:7,
      registered_kernel_relation:'exact_same_owner_kernel_registration_to_nonbaseline_caller',
      baseline_registered_kernel_relation:UNKNOWN_KERNEL,transitive_helper_closure:'not_traversed',
      source_parameter_name_to_ssa_mapping:'not_supplied',body_return_mapping:'unavailable_not_projected',
      source_authentication:false,compiler_closure_attestation:false,protected_proof:false,production_resume:false,
      physical_helper_abi:false,native_qualified:false,hardware_observed:false,milestone_completion:false,
      source_build_freshness:'external_root_prerequisite_not_inferred_from_receipt_status',
      observations,stages,selected_input_pins:[...pins.values()].map(item=>item.pin),selected_inputs_unchanged:true,
      capture_owned_pins:owned.size,capture_owned_bytes:ownedBytes,selected_bytes:selectedBytes,cumulative_read_bytes:readBytes,
      output_files_before_receipt:outputFiles,output_bytes_before_receipt:outputBytes,limits:LIMITS,elapsed_ms:Date.now()-started,
      runtime_environment:env,task_resource_accounting:'external_current_scope_supervisor_required'};
    const pin=saveJson('receipt.json',report);process.stdout.write(JSON.stringify({status:'passed',receipt:pin})+'\n');
  } catch(error) {
    try { saveJson('failure.json',{schema:'task-call-target-source-observation-v2',status:'failed',
      error:String(error).slice(0,4096),stages,completed_variants:observations.map(item=>item.label),
      manufactured_fallback:false,selected_inputs_unchanged:false,source_exports_executed:0,simulations_executed:0,
      cleanup_or_rollback_claimed:false,hardware_observed:false}); } catch {}
    throw error;
  }
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url))
  run(parseOptions(process.argv.slice(2))).catch(error=>{process.stderr.write(String(error).slice(0,4096)+'\n');process.exitCode=1;});
