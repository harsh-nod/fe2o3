#!/usr/bin/env node
// Read-only consistency parser. It cannot authenticate execution or grant a budget.
import fs from 'node:fs';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
const SCHEMA = 'fe2o3-recipe-outcome-v1';
const OUTCOME = 'FE2O3_RECIPE_OUTCOME_V1 ';
const SERIES = 'FE2O3_RECIPE_OUTCOME_SERIES_V1 ';
const OUTCOME_CAP = 1056768;
const STREAM_CAP = 4 * 1024 * 1024;
const REFUSAL = 'local-order recipe source revision changed';
const HEX = /^[0-9a-f]{64}$/;
function need(value, message) { if (!value) throw new Error(message); }
function plain(value) { return value !== null && typeof value === 'object' && !Array.isArray(value); }
function keys(value, expected, label) {
  need(plain(value), label + ': object');
  need(JSON.stringify(Object.keys(value).sort()) === JSON.stringify([...expected].sort()), label + ': closed fields');
}
export function canonical(value) {
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  if (plain(value)) return '{' + Object.keys(value).sort().map(k => JSON.stringify(k) + ':' + canonical(value[k])).join(',') + '}';
  return JSON.stringify(value);
}
function decode(text) {
  const value = JSON.parse(text);
  need(canonical(value) === text, 'noncanonical or duplicate-field record');
  return value;
}
function sha(bytes) { return crypto.createHash('sha256').update(bytes).digest('hex'); }
function uint(value, max = Number.MAX_SAFE_INTEGER) { return Number.isSafeInteger(value) && value >= 0 && value <= max; }
function eq(a, b, why) { need(canonical(a) === canonical(b), why); }
function byteArray(a, n) { return Array.isArray(a) && a.length === n && a.every(v => uint(v, 255)); }
function hex(a) { need(byteArray(a,32), 'exact digest array'); return Buffer.from(a).toString('hex'); }
function binding(value) {
  keys(value, ['workload','source','current_source_sha256','origin_source_sha256','origin_normal_sha256','origin_source_initializer','recipe_sha256','instance_axes'], 'binding');
  need(['checked_rebind','exact_revision_refusal'].includes(value.workload), 'closed workload');
  need(typeof value.source === 'string' && value.source.length > 0 && Buffer.byteLength(value.source) <= 4096, 'source path');
  for (const key of ['current_source_sha256','origin_source_sha256','origin_normal_sha256','recipe_sha256']) need(HEX.test(value[key]), 'binding digest');
  need(value.current_source_sha256 !== value.origin_source_sha256, 'source was not edited');
  need(Array.isArray(value.origin_source_initializer) && value.origin_source_initializer.length === 4 && value.origin_source_initializer.every(v => uint(v, 0xffffffff)), 'origin span');
  need(Array.isArray(value.instance_axes) && value.instance_axes.length === 5 && value.instance_axes.every(v => byteArray(v,32)), 'five instance axes');
}
function output(value) {
  keys(value, ['schema','binding','callback_count','compiler_callback_count','status','normal_utf8','failure','grants_authority'], 'outcome');
  need(value.schema === SCHEMA && value.callback_count === 1 && value.compiler_callback_count === 1 && value.grants_authority === false, 'outcome identity/callback/authority');
  binding(value.binding);
  if (value.binding.workload === 'exact_revision_refusal') {
    need(value.status === 'refused' && value.normal_utf8 === null, 'refusal cannot be success');
    keys(value.failure, ['phase','diagnostic','compiler_fatal'], 'failure');
    eq(value.failure, {phase:'RecipeBinding',diagnostic:REFUSAL,compiler_fatal:false}, 'wrong designated typed refusal');
  } else {
    need(value.status === 'accepted' && value.failure === null && typeof value.normal_utf8 === 'string', 'checked rebind requires normal output');
    need(value.normal_utf8.endsWith('\n') && Buffer.byteLength(value.normal_utf8) <= 524288, 'full normal byte cap');
    const normal = decode(value.normal_utf8.slice(0,-1));
    keys(normal, ['llvm','created_recipe','evidence','grants_artifact_or_launch_authority'], 'original normal');
    need(typeof normal.llvm === 'string' && normal.created_recipe === null && normal.grants_artifact_or_launch_authority === false, 'replay original output');
    const e = normal.evidence;
    need(plain(e), 'original full evidence');
    const required = ['source_sha256','source_initializer','semantic_sha256','instance_axes','original','input','output','requested_order','requested_relation','strength','source_binding_mode','actual_relation','constraint_outcome','region','output_result_order','prefix_execution_bytes','transition_sha256','transition_bytes','transition_rows','fresh_formal_counts','llvm_sha256','descriptor_sha256','recipe_sha256','canonical_work','canonical_peak_storage','created','descriptor_producer','composition','grants_authority'];
    keys(e, required, 'original evidence');
    need(hex(e.source_sha256) === value.binding.current_source_sha256 && hex(e.recipe_sha256) === value.binding.recipe_sha256, 'current source/recipe differ');
    eq(e.instance_axes, value.binding.instance_axes, 'current item/instance differs');
    need(canonical(e.source_initializer) !== canonical(value.binding.origin_source_initializer), 'current span unchanged');
    need(e.created === false && e.grants_authority === false && e.source_binding_mode === 'rebind_current'
      && e.requested_order === 'reverse_ready' && e.requested_relation === 'or_before_xor'
      && e.actual_relation === 'or_before_xor' && e.strength === 'exact', 'current checked-rebind contract');
    eq(e.constraint_outcome, {status:'honored',relation:'or_before_xor'}, 'current checked relation');
  }
}
function records(stdout, stderr) {
  need(typeof stdout === 'string' && typeof stderr === 'string', 'raw streams required');
  need(Buffer.byteLength(stdout) <= STREAM_CAP && Buffer.byteLength(stderr) <= STREAM_CAP, 'stream cap');
  need(stdout.endsWith('\n') && stderr.endsWith('\n'), 'complete LF-terminated streams required');
  const terminals = stdout.split('\n').filter(line => line.startsWith('test result:'));
  need(terminals.length === 1 && !stderr.split('\n').some(line => line.startsWith('test result:')), 'exact terminal census');
  need(/^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9]+(?:\.[0-9]+)?s$/.test(terminals[0]), 'exact successful child test terminal');
  const outcomes = [], rows = [];
  for (const [label,text] of [['stdout',stdout],['stderr',stderr]]) {
    for (const line of text.split('\n')) {
      if (line.startsWith(OUTCOME)) {
        need(label === 'stdout', 'outcome on wrong stream');
        const raw = line.slice(OUTCOME.length) + '\n';
        need(Buffer.byteLength(raw) <= OUTCOME_CAP, 'outcome byte cap');
        const value = decode(raw.slice(0,-1)); output(value); outcomes.push({raw,value});
      } else if (line.startsWith(SERIES)) {
        need(label === 'stderr' && Buffer.byteLength(line.slice(SERIES.length)) + 1 <= 4096, 'row stream/cap');
        rows.push(decode(line.slice(SERIES.length)));
      } else {
        need(!line.includes('FE2O3_RECIPE_'), 'foreign/old recipe record');
      }
    }
  }
  need(outcomes.length === 1 && rows.length <= 37, 'exact one outcome and bounded row census');

  need(!rows.some(r => ['incomplete','child_refused'].includes(r.kind)), 'incomplete series is never qualified');
  return {outcome:outcomes[0], rows};
}
export function parseOrdinary(stdout, stderr) {
  const got = records(stdout, stderr);
  need(got.rows.length === 1, 'ordinary exact row census');
  const row = got.rows[0];
  keys(row, ['kind','schema','binding','outcome_bytes','outcome_sha256','elapsed_ns','statistics','retained_logical_bytes','grants_authority'], 'ordinary complete');
  need(row.kind === 'ordinary_complete' && row.schema === SCHEMA && row.elapsed_ns === null
    && row.statistics === null && row.retained_logical_bytes === null && row.grants_authority === false, 'ordinary contract');
  eq(row.binding, got.outcome.value.binding, 'ordinary binding mismatch');
  need(row.outcome_bytes === Buffer.byteLength(got.outcome.raw) && row.outcome_sha256 === sha(got.outcome.raw), 'ordinary full byte pin');
  return got;
}
export function parsePair(ordinaryStdout, ordinaryStderr, seriesStdout, seriesStderr) {
  const ordinary = parseOrdinary(ordinaryStdout, ordinaryStderr);
  const got = records(seriesStdout, seriesStderr);
  need(got.outcome.raw === ordinary.outcome.raw, 'full ordinary outcome byte oracle differs');
  need(got.rows.length === 37, 'plan +35 samples +complete required');
  const plan = got.rows[0], complete = got.rows[36];
  keys(plan, ['kind','schema','binding','calls','calibration','measured','upfront_selected_input_reservations','additional_origin_read_bytes','additional_ordinary_oracle_read_bytes','callback_stopping_seconds','external_deadline_required','ordinary_oracle_sha256','ordinary_oracle_bytes','retained_logical_bytes','storage_walks_in_timer','grants_authority'], 'plan');
  need(plan.kind === 'plan' && plan.schema === SCHEMA && plan.calls === 35 && plan.calibration === 5 && plan.measured === 30, 'plan counts');
  eq(plan.upfront_selected_input_reservations, {calls:35,maximum_selected_retained_source_read_bytes:110100585,maximum_selected_recipe_read_bytes:860265,maximum_additional_capture_path_read_bytes:2293795}, 'unchanged original reservations');
  need(plan.additional_origin_read_bytes === 1572867 && plan.additional_ordinary_oracle_read_bytes === 3170307
    && plan.callback_stopping_seconds === 60 && plan.external_deadline_required === true
    && plan.retained_logical_bytes === null && plan.storage_walks_in_timer === false && plan.grants_authority === false, 'plan bounds/limitations');
  eq(plan.binding, ordinary.outcome.value.binding, 'plan wrong workload/source');
  const digest = sha(ordinary.outcome.raw), bytes = Buffer.byteLength(ordinary.outcome.raw);
  need(plan.ordinary_oracle_sha256 === digest && plan.ordinary_oracle_bytes === bytes, 'plan external oracle pin');
  const times = [];
  for (let i = 0; i < 35; i++) {
    const event = got.rows[i+1]; keys(event,['kind','sample'],'sample event');
    need(event.kind === 'sample', 'ordered event kind');
    const sample = event.sample;
    keys(sample,['ordinal','calibration','elapsed_ns','outcome_bytes','outcome_sha256','exact_ordinary_oracle_equal'],'sample');
    need(sample.ordinal === i+1 && sample.calibration === (i<5) && uint(sample.elapsed_ns)
      && sample.outcome_bytes === bytes && sample.outcome_sha256 === digest
      && sample.exact_ordinary_oracle_equal === true, 'sample order/calibration/oracle');
    if (i >= 5) times.push(sample.elapsed_ns);
  }
  times.sort((a,b)=>a-b);
  const stats = {p50_ns:times[14],p95_ns:times[28],max_ns:times[29]};
  keys(complete,['kind','schema','binding','callback_count','raw_samples','calibration','measured','statistics','percentile_method','ordinary_oracle_sha256','ordinary_oracle_bytes','all35_exact_ordinary_oracle_equal','frontend_reused','admitted_owner_reused','scope','companion_serialization_hash_equality_in_timer','api_internal_checks_in_timer','retained_logical_bytes','peak_heap_measured','rss_measured','budget_accepted','grants_authority'],'series complete');
  need(complete.kind === 'series_complete' && complete.schema === SCHEMA && complete.callback_count === 1
    && complete.raw_samples === 35 && complete.calibration === 5 && complete.measured === 30
    && complete.percentile_method === 'nearest_rank_30_rank15_rank29_rank30'
    && complete.ordinary_oracle_sha256 === digest && complete.ordinary_oracle_bytes === bytes
    && complete.all35_exact_ordinary_oracle_equal === true && complete.frontend_reused === true
    && complete.admitted_owner_reused === false && complete.scope === 'fresh_transaction_creation_through_original_consuming_recipe_return'
    && complete.companion_serialization_hash_equality_in_timer === false && complete.api_internal_checks_in_timer === true
    && complete.retained_logical_bytes === null && complete.peak_heap_measured === false && complete.rss_measured === false
    && complete.budget_accepted === false && complete.grants_authority === false, 'complete counts/limitations');
  eq(complete.binding, plan.binding, 'complete workload/source changed');
  eq(complete.statistics, stats, 'recomputed exact nearest ranks');
  return {
    schema:'fe2o3-recipe-outcome-series-consistency-v1', binding:plan.binding,
    ordinary_outcome_bytes:bytes, ordinary_outcome_sha256:digest,
    raw_samples:35,calibration:5,measured:30,statistics:stats,
    execution_authenticated:false,budget_accepted:false,retained_logical_bytes:null,grants_authority:false
  };
}
function read(path) {
  need(typeof path === 'string' && path.startsWith('/') && Buffer.byteLength(path) <= 4096, 'absolute input path');
  const fd = fs.openSync(path, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd);
    need(before.isFile() && before.nlink === 1 && before.size > 0 && before.size <= STREAM_CAP, 'bounded regular stream');
    const bytes = Buffer.alloc(before.size + 1); let offset=0;
    while (offset < bytes.length) {
      const count = fs.readSync(fd, bytes, offset, bytes.length-offset, null);
      if (!count) break; offset += count;
    }
    const after=fs.fstatSync(fd), pathNow=fs.lstatSync(path);
    for (const key of ['dev','ino','mode','nlink','size','mtimeMs','ctimeMs']) {
      need(before[key] === after[key] && before[key] === pathNow[key], 'stream currentness');
    }
    need(offset === before.size, 'stream size changed');
    const exact = bytes.subarray(0,offset), value = exact.toString('utf8');
    need(Buffer.from(value).equals(exact), 'strict UTF8');
    return value;
  } finally { fs.closeSync(fd); }
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    need(process.argv.length === 6, 'usage: parser ordinary.stdout ordinary.stderr series.stdout series.stderr (absolute paths)');
    process.stdout.write(JSON.stringify(parsePair(...process.argv.slice(2).map(read)))+'\n');
  } catch (error) { process.stderr.write(String(error.message)+'\n'); process.exitCode=1; }
}
