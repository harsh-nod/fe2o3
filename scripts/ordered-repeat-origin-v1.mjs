// Closed legacy/origin-v1 capture profiles and bounded inert sidecar joins.
// Pure validation only: no filesystem reads, compilation, or source authentication.
import assert from 'node:assert/strict';
import path from 'node:path';
import { parseBoundedJson } from './ordered-program-source-native.mjs';

export const ORIGIN_BYTES_V1 = 16384;
const LABELS = ['one', 'two', 'fifteen', 'repeat'];
const REFUSALS = ['zero', 'sixteen', 'huge', 'expanded-seventeen', 'dynamic', 'bad-init', 'nested', 'physical-alias'];
const FLAG = '--diagnostic-ordered-origin-v1';

export const REPORT_KEYS = [
  'schema','diagnostic_only','authenticates_source','authenticates_compiler_execution',
  'grants_proof_resume_artifact_launch_authority','stage','target','wave_width',
  'canonical_version','canonical_sha256','canonical_bytes','semantic_version','semantic_sha256',
  'source_inventory_sha256','source_preflight_sha256','root_function_sha256',
  'root_monomorphization_sha256','rustc_mir_body_sha256','rustc_mir_block',
  'semantic_block_identity','semantic_function','semantic_block','kir_roster_coordinate',
  'kir_raw_block','declared_source_ids','origin_association','origin_scope','expansion','call_site',
  'expansion_chain_sha256','expansion_depth','macro_expansion_frames','fine_step_origins',
  'declared_instructions','declared_register_roles','compiler_policy_identity','source_map_identity',
  'edit_epoch','schedule_identity','final_artifact','physical_register_values',
  'physical_register_lifetimes','limits',
];
const exact = (value, keys) => {
  assert(value && typeof value === 'object' && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort());
};
const natural = (value, max, min = 0) => assert(Number.isSafeInteger(value) && value >= min && value <= max);
const digest = value => { assert.equal(typeof value, 'string'); assert.match(value,/^[0-9a-f]{64}$/u); assert.notEqual(value, '0'.repeat(64)); };
function span(value) {
  exact(value,['file_identity','byte_start','byte_end','line_start','column_start','line_end','column_end']);
  digest(value.file_identity);
  for (const name of ['byte_start','byte_end']) natural(value[name],Number.MAX_SAFE_INTEGER);
  for (const name of ['line_start','line_end']) natural(value[name],0xffffffff,1);
  for (const name of ['column_start','column_end']) natural(value[name],0xffffffff);
  assert(value.byte_start <= value.byte_end);
  assert(value.line_start < value.line_end ||
    (value.line_start === value.line_end && value.column_start <= value.column_end));
}
function validateOrigin(value, inspection, rawBytes) {
  exact(value,REPORT_KEYS);
  assert.equal(value.schema,'fe2o3-diagnostic-ordered-program-origin-v1');
  assert.equal(value.diagnostic_only,true);
  for (const key of ['authenticates_source','authenticates_compiler_execution',
    'grants_proof_resume_artifact_launch_authority']) assert.equal(value[key],false);
  assert.equal(value.stage,'pre_ranked_diagnostic');
  assert.equal(value.target,inspection.declared_target);
  assert.equal(value.wave_width,inspection.declared_wave_width);
  assert.equal(value.target,'gfx942:xnack-'); assert.equal(value.wave_width,64);
  assert(Buffer.isBuffer(rawBytes) && rawBytes.length>0 && rawBytes.length<=65536);
  assert.equal(value.canonical_version,17); assert.equal(value.semantic_version,32);
  assert.equal(value.canonical_sha256,inspection.canonical.sha256);
  assert.equal(value.canonical_bytes,inspection.canonical.bytes);
  assert.equal(value.canonical_bytes,rawBytes.length);
  // Raw file SHA is intentionally NOT compared with the domain-separated canonical identity.
  for (const key of ['canonical_sha256','semantic_sha256','source_inventory_sha256',
    'source_preflight_sha256','root_function_sha256','root_monomorphization_sha256',
    'rustc_mir_body_sha256','semantic_block_identity','expansion_chain_sha256']) digest(value[key]);
  for (const key of ['rustc_mir_block','semantic_function','semantic_block','kir_raw_block'])
    natural(value[key],0xffffffff);
  assert.deepEqual(value.kir_roster_coordinate,[
    inspection.coordinate.function_ordinal,inspection.coordinate.block_ordinal,
    inspection.coordinate.operation_ordinal]);
  assert.equal(value.kir_raw_block,inspection.raw_block_id);
  assert.deepEqual(value.declared_source_ids,inspection.declared_source_ids);
  assert.equal(value.root_function_sha256,value.declared_source_ids.function);
  assert.equal(value.origin_association,'retained_semantic_correspondence_and_live_rustc_block_identity');
  assert.equal(value.origin_scope,'whole_ordered_region');
  span(value.expansion); span(value.call_site);
  natural(value.expansion_depth,256);
  assert.equal(value.macro_expansion_frames,'unavailable_only_digest_and_depth_retained');
  assert.equal(value.fine_step_origins,'unavailable_flat_descriptor_program');
  const count=inspection.declared_program.count;
  natural(count,16,1);
  assert.deepEqual(value.declared_instructions,Array.from({length:count},(_,ordinal)=>({
    ordinal,descriptor:inspection.declared_program.descriptors[ordinal],
    source_association:'whole_ordered_region_only',
  })));
  assert.deepEqual(value.declared_register_roles,{
    scratch:inspection.register_plan.scratch,output:inspection.register_plan.output,
    inputs:inspection.register_plan.inputs,
  });
  for (const [key,expected] of Object.entries({
    compiler_policy_identity:'unavailable_in_this_diagnostic',
    source_map_identity:'unavailable_no_debug_map_exported',edit_epoch:'unavailable',
    schedule_identity:'unavailable',final_artifact:'unavailable_no_native_compilation',
    physical_register_values:'unavailable',physical_register_lifetimes:'unavailable',
  })) assert.equal(value[key],expected);
  exact(value.limits,['source_reobservation_work','source_reobservation_work_used',
    'maximum_expansion_depth','report_bytes','rustc_internal_allocations_accounted']);
  assert.equal(value.limits.source_reobservation_work,1048576);
  natural(value.limits.source_reobservation_work_used,1048576,1);
  assert.equal(value.limits.maximum_expansion_depth,256);
  assert.equal(value.limits.report_bytes,16384);
  assert.equal(value.limits.rustc_internal_allocations_accounted,false);
  return {macro_depth_observed:value.expansion_depth,
    fine_step_ancestry_qualified:false,compiler_execution_authenticated:false};
}

function absolute(value) {
  assert.equal(typeof value, 'string');
  assert.ok(path.isAbsolute(value) && path.resolve(value) === value && Buffer.byteLength(value) <= 4096);
  assert.ok(!/[\u0000-\u001f\u007f]/u.test(value)); return value;
}
function profile(value) { assert.ok(value === 'legacy' || value === 'origin-v1', 'closed repeat exporter profile'); }
function labelName(label, negative) {
  assert.ok(negative ? REFUSALS.some(value => label === 'refuse-' + value) : LABELS.includes(label), 'closed capture label');
}
export function originOutputPathV1(directory, label) {
  absolute(directory); labelName(label, label.startsWith('refuse-'));
  return path.join(directory, label + '.origin.json');
}
export function repeatExportArgumentsV1(selected, directory, label, source, negative = false) {
  profile(selected); absolute(directory); absolute(source); assert.equal(typeof negative, 'boolean'); labelName(label, negative);
  return ['--diagnostic-kir-v17', '--crate', 'fe2o3_assembly_authoring_v30_fixture',
    '--output', path.join(directory, label + '.kir'), '--target', 'gfx942',
    '--target-dir', path.join(directory, label + '-extraction'),
    ...(selected === 'origin-v1' && !negative ? [FLAG, originOutputPathV1(directory, label)] : []),
    '--', '--manifest-path', path.join(path.dirname(path.dirname(source)), 'Cargo.toml'), '--lib', '--offline',
    ...(negative ? ['--message-format=json'] : [])];
}
/** Infers only one of two exact, all-four-or-none command profiles, never arbitrary extra args. */
export function validateRepeatExportProfileV1(capture, directory) {
  absolute(directory);
  assert.ok(Array.isArray(capture.stages) && capture.stages.length === 136);
  const stages = new Map(capture.stages.map(stage => [stage.label, stage]));
  assert.equal(stages.size, 136, 'unique complete stage labels');
  assert.ok(Array.isArray(capture.variants) && capture.variants.length === 4);
  assert.ok(Array.isArray(capture.refusals) && capture.refusals.length === 8);
  const first = stages.get('one-export'); assert.ok(first && Array.isArray(first.args));
  const selected = first.args.includes(FLAG) ? 'origin-v1' : 'legacy';
  for (const [index, variant] of capture.variants.entries()) {
    const label = LABELS[index]; assert.equal(variant.label, label);
    const source = path.join(directory, LABELS[Math.min(index, 2)] + '-source/src/lib.rs');
    assert.equal(variant.source_path, source);
    assert.deepEqual(stages.get(label + '-export')?.args, repeatExportArgumentsV1(selected, directory, label, source));
  }
  for (const [index, refusal] of capture.refusals.entries()) {
    assert.equal(refusal.label, REFUSALS[index]);
    const label = 'refuse-' + refusal.label, source = path.join(directory, label + '-source/src/lib.rs');
    assert.equal(refusal.source_path, source);
    assert.deepEqual(stages.get(label + '-export')?.args, repeatExportArgumentsV1('legacy', directory, label, source, true));
  }
  assert.ok(Array.isArray(capture.retained_file_pins) && capture.retained_file_pins.length <= 512);
  const pins = new Map(capture.retained_file_pins.map(pin => [pin.path, pin]));
  assert.equal(pins.size, capture.retained_file_pins.length, 'unique source pins');
  for (const label of [...LABELS, ...REFUSALS.map(value => 'refuse-' + value)]) {
    const pin = pins.get(originOutputPathV1(directory, label));
    if (selected === 'origin-v1' && LABELS.includes(label)) {
      assert.ok(pin, 'same-invocation origin must be retained');
      natural(pin.bytes, ORIGIN_BYTES_V1, 1); digest(pin.sha256);
    } else assert.equal(pin, undefined, 'unrequested origin sidecar cannot be admitted');
  }
  return selected;
}
/** Joins a whole, strict UTF-8 sidecar to the actual raw KIR inspector and exporter metadata. */
export function validateRepeatOriginV1(bytes, exported, inspection, rawBytes) {
  assert.ok(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= ORIGIN_BYTES_V1);
  assert.ok(!(bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf), 'origin UTF-8 must not have a BOM');
  const value = parseBoundedJson(bytes, ORIGIN_BYTES_V1);
  validateOrigin(value, inspection, rawBytes);
  exact(exported, ['canonical_sha256', 'canonical_bytes', 'retained_source_inventory',
    'retained_source_preflight', 'semantic_identity']);
  for (const [field, observed] of [
    ['canonical_sha256', value.canonical_sha256], ['canonical_bytes', value.canonical_bytes],
    ['semantic_identity', value.semantic_sha256], ['retained_source_inventory', value.source_inventory_sha256],
    ['retained_source_preflight', value.source_preflight_sha256],
  ]) assert.equal(exported[field], observed, 'same live exporter origin identity');
  return value;
}
