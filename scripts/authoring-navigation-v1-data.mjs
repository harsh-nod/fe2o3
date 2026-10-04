// Private, bounded example-capture validation. No new compiler protocol or owner.
import crypto from 'node:crypto';
import path from 'node:path';
import { parseBoundedJson } from './ordered-program-source-native.mjs';

export const MiB = 1024 * 1024;
export const FIXTURE = 'crates/rustc-codegen-fe2o3/tests/fixtures/ordinary-bitwise-promotion-v1';
export const TARGET = 'gfx942:xnack-';
export const RUSTC_COMMIT = '55e86c996809902e8bbad512cfb4d2c18be446d9';
export const LIMITS = Object.freeze({ source_bytes: 16384, artifact_bytes: 65536,
  census_bytes: 262144, retained_bytes: 512 * 1024, bundle_bytes: 4 * MiB,
  operations: 64, page_items: 16, spans_per_operation: 16, values: 16,
  command_ms: 300000, command_output_bytes: MiB, jobs: 2,
  minimum_free_bytes: String(40n * 1024n ** 3n), minimum_ram_bytes: String(64n * 1024n ** 3n),
  maximum_combined_cache_bytes: String(20n * 1024n ** 3n) });
export const AUTHORITY = Object.freeze({ observation_only: true, authenticates_compiler_execution: false,
  source_authenticated: false, grants_proof_authority: false, grants_production_resume: false,
  grants_load_or_launch: false });
export const AVAILABILITY = Object.freeze({
  rust_text: 'census_joined_retained_bytes_diagnostic_only',
  source_to_kir: 'many_to_many_attribution_not_ssa_binding_ownership',
  canonical_simt_v11: 'available_exact_observation_and_selected_structural_boundary',
  typed_rust_hir: 'unavailable_no_typed_snapshot', semantic_mir: 'identity_only_no_body',
  scheduled_tile_rust: 'unavailable', separate_neutral_target_lineage: 'unavailable',
  compiler_handoff_llvm: 'unavailable_not_retained', final_artifact_isa: 'unavailable_export_precedes_artifact',
  physical_resources: 'unavailable_logical_canonical_stage', source_edit_boundary: 'unavailable',
  cpu_selected_values: 'unavailable_not_observed', traps: 'not_analyzed', convergence: 'not_analyzed',
  compiler_policy: 'unavailable_in_v6', runtime_closure: 'unavailable_selected_file_measurements_only' });
const absent = level => ({ level, read: 'unavailable', select: 'unavailable', materialize: 'unavailable',
  edit: 'unavailable', readmit: 'unavailable', simulate: 'unavailable', inspect: 'unavailable' });
export const CAPABILITIES = Object.freeze([
  { ...absent('structured_rust'), read: 'source_locations_only' }, absent('scheduled_tile_rust'),
  { level: 'canonical_simt_v11', read: 'available', select: 'bounded_contiguous_block',
    materialize: 'bounded_u32_bitwise_or_validated_inline_isa_draft', edit: 'external_source_edit_only',
    readmit: 'requires_fresh_source_compilation_supported_subset',
    simulate: 'separate_existing_simulator_with_coverage_checks', inspect: 'available' },
  absent('physical_register_exact_region'),
]);
export const demand = (value, reason) => { if (!value) throw new Error(reason); };
export const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export function exact(value, keys, label) {
  demand(value && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key)),
  label + ': exact object fields');
}
export function same(actual, expected, label) {
  demand(JSON.stringify(actual) === JSON.stringify(expected), label);
}
export function uint(value, max, label, min = 0) {
  demand(Number.isSafeInteger(value) && value >= min && value <= max, label + ': integer bound');
  return value;
}
export function list(value, max, label, min = 0) {
  demand(Array.isArray(value) && value.length >= min && value.length <= max
    && Array.from({ length: value.length }, (_, i) => Object.hasOwn(value, i)).every(Boolean), label + ': dense array');
  return value;
}
export function text(value, max, label) {
  demand(typeof value === 'string' && Buffer.byteLength(value) <= max, label + ': text bound');
  for (const c of value) demand(c.codePointAt(0) < 0xd800 || c.codePointAt(0) > 0xdfff, label + ': lone surrogate');
  return value;
}
export function digest(value, label) {
  demand(typeof value === 'string' && /^[0-9a-f]{64}$/.test(value) && value !== '0'.repeat(64), label + ': digest');
  return value;
}
export function decimal(value, maximum, label, minimum = 0n) {
  demand(typeof value === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(value), label + ': decimal');
  const result = BigInt(value);
  demand(result >= minimum && result <= maximum, label + ': decimal bound');
  return result;
}
export function absolute(value) {
  text(value, 4096, 'path');
  demand(path.isAbsolute(value) && path.resolve(value) === value && !/[\x00-\x1f\x7f]/.test(value), 'canonical absolute path');
  return value;
}
export function artifact(bytes) {
  demand(Buffer.isBuffer(bytes), 'artifact bytes');
  return { bytes: bytes.length, sha256: sha256(bytes), utf8: new TextDecoder('utf-8', { fatal: true }).decode(bytes) };
}
export function coordinate(value) {
  exact(value, ['function', 'block', 'operation'], 'coordinate');
  Object.values(value).forEach(v => uint(v, 0xffff_ffff, 'coordinate'));
  return [value.function, value.block, value.operation].join(':');
}
function authority(value) { same(value, AUTHORITY, 'observation-only authority'); }
function values(value) {
  return list(value, LIMITS.values, 'values').map(row => {
    exact(row, ['value', 'ty'], 'value'); uint(row.value, 0xffff_ffff, 'SSA label');
    text(row.ty, 512, 'type'); return row;
  });
}
function sourceSpan(span, files, source) {
  exact(span, ['file_identity', 'display_path', 'byte_start', 'byte_end', 'line', 'column'], 'source span');
  digest(span.file_identity, 'source file'); text(span.display_path, 4096, 'source label');
  const file = files.get(span.file_identity);
  demand(file, 'source span file identity lacks exact census join');
  const start = Number(decimal(span.byte_start, BigInt(source.length), 'source start'));
  const end = Number(decimal(span.byte_end, BigInt(source.length), 'source end'));
  demand(start <= end, 'reversed source range');
  for (const n of [start, end]) demand(n === source.length || (source[n] & 0xc0) !== 0x80, 'mid-UTF-8 source endpoint');
  uint(span.line, 0xffff_ffff, 'line', 1); uint(span.column, 0xffff_ffff, 'column', 1);
  return { file_identity: span.file_identity, byte_start: start, byte_end: end };
}
function operation(row, files, source) {
  exact(row, ['coordinate', 'function_name', 'kind', 'semantic_detail', 'mnemonic', 'inline_assembly_source',
    'inputs', 'results', 'local_memory_effects', 'complete_local_effect_summary', 'convergence', 'traps',
    'physical_resources', 'source_binding', 'source_spans', 'materialization'], 'operation');
  coordinate(row.coordinate);
  demand(row.coordinate.function === 0 && row.function_name === 'bitwise_chain', 'closed ordinary fixture function roster');
  demand(row.mnemonic === null && row.inline_assembly_source === null && row.kind !== 'inline_assembly',
    'ordinary source must not be relabelled as authored ISA');
  demand(/^[a-z_]{1,64}$/.test(row.kind), 'operation kind');
  if (row.semantic_detail !== null) text(row.semantic_detail, 512, 'operation detail');
  values(row.inputs); values(row.results);
  list(row.local_memory_effects, 16, 'effects').forEach(effect => text(effect, 4096, 'effect'));
  demand(typeof row.complete_local_effect_summary === 'boolean', 'effect completeness');
  demand(row.convergence === 'not_analyzed' && row.traps === 'not_analyzed'
    && row.physical_resources === 'unavailable_logical_canonical_stage', 'operation availability');
  const spans = list(row.source_spans, LIMITS.spans_per_operation, 'source spans');
  demand(row.source_binding === (spans.length ? 'bundle_content_bound_not_authenticated' : 'unavailable_no_source_span'),
    'source attribution availability');
  demand(['diagnostic_rust_draft_available', 'unavailable_unsupported_operation_or_contract'].includes(row.materialization),
    'materialization availability');
  return spans.map(span => sourceSpan(span, files, source));
}
function censusFiles(census, capture, source) {
  exact(census, ['schema', 'diagnosticOnly', 'qualified', 'authenticatesCompilerExecution', 'extractionSucceeded',
    'arguments', 'workingDirectory', 'extractionMode', 'runId', 'selection'], 'census');
  demand(census.schema === 'fe2o3-diagnostic-source-census-v1' && census.diagnosticOnly === true
    && census.qualified === false && census.authenticatesCompilerExecution === false
    && census.extractionSucceeded === true && census.runId === capture.run_id, 'successful same-run diagnostic census');
  same(census.extractionMode, { kind: 'simulation-bundle', version: 6 }, 'same-run census extraction mode');
  absolute(census.workingDirectory);
  list(census.arguments, 128, 'census arguments', 1).forEach(arg => text(arg, 4096, 'census argument'));
  const crateFlags = census.arguments.flatMap((arg, i) => arg === '--crate-name' ? [i] : []);
  demand(crateFlags.length === 1 && census.arguments[crateFlags[0] + 1] === 'fe2o3_ordinary_bitwise_promotion_v1_fixture',
    'census invocation crate selection');
  const sourceArguments = census.arguments.filter(arg => !arg.startsWith('-') && arg.endsWith('.rs'));
  demand(sourceArguments.length === 1 && path.resolve(census.workingDirectory, sourceArguments[0])
    === path.join(capture.configuration.repo, FIXTURE, 'src/lib.rs'), 'census invocation source selection');
  exact(census.selection, ['status', 'value'], 'census selection');
  demand(census.selection.status === 'available', 'census selection unavailable');
  const selection = census.selection.value;
  exact(selection, ['target', 'functions', 'files'], 'census selection value');
  demand(selection.target === TARGET, 'census target');
  list(selection.functions, 1, 'one collected ordinary kernel', 1).forEach(fn => {
    exact(fn, ['functionIdentity', 'definitionIdentity', 'monomorphizationIdentity', 'role',
      'exportName', 'logicalName', 'definition', 'identifier'], 'census function');
    for (const key of ['functionIdentity', 'definitionIdentity', 'monomorphizationIdentity']) digest(fn[key], key);
    demand(fn.role === 'kernel-entry' && fn.exportName === 'bitwise_chain', 'ordinary census entry');
    demand(fn.logicalName === null || typeof fn.logicalName === 'string', 'logical name');
    // These are diagnostic function observations, NOT an operation/SSA ownership join.
    for (const key of ['definition', 'identifier']) {
      exact(fn[key], ['status', 'value'], 'census function source availability');
      demand(['available', 'unavailable'].includes(fn[key].status), 'census source availability');
    }
  });
  const files = new Map();
  for (const file of list(selection.files, 16, 'census files', 1)) {
    exact(file, ['identity', 'displayPath', 'compiledSourceHash', 'originalSha256', 'originalBytes', 'normalizedBytes'], 'census file');
    digest(file.identity, 'census file identity'); text(file.displayPath, 4096, 'census display path');
    text(file.compiledSourceHash, 128, 'compiled source hash');
    demand(file.compiledSourceHash.length > 0 && !files.has(file.identity), 'duplicate or empty census file metadata');
    digest(file.originalSha256, 'census original hash');
    uint(file.originalBytes, 4 * MiB, 'census original bytes', 1);
    uint(file.normalizedBytes, 4 * MiB, 'census normalized bytes', 1);
    // The initial fixture profile has only one source file. Never fall back to paths.
    demand(file.originalSha256 === sha256(source) && file.originalBytes === source.length
      && file.normalizedBytes === source.length, 'census file-to-bytes or normalization mismatch');
    files.set(file.identity, file);
  }
  demand(files.size === 1, 'ambiguous census file-to-source association');
  return files;
}
export function validateNavigationData(capture) {
  exact(capture, ['schema', 'status', 'scope', 'authority', 'run_id', 'configuration', 'limits',
    'measurements_before', 'measurements_after', 'stages', 'resource_guards', 'artifacts', 'availability'], 'capture');
  demand(capture.schema === 'task-authoring-navigation-capture-v1' && capture.status === 'passed'
    && capture.scope === 'retained actual ordinary-source read-only navigation; not compiler or source authority', 'capture scope');
  authority(capture.authority); digest(capture.run_id, 'run ID'); same(capture.limits, LIMITS, 'capture limits');
  same(capture.availability, AVAILABILITY, 'capture availability');
  exact(capture.artifacts, ['source', 'manifest', 'lock', 'bundle', 'census', 'snapshot', 'pages', 'selector', 'region'], 'artifacts');
  let total = 0;
  const read = (item, cap, label, json = true) => {
    exact(item, ['bytes', 'sha256', 'utf8'], label); text(item.utf8, cap, label);
    const bytes = Buffer.from(item.utf8);
    uint(item.bytes, cap, label, 1); digest(item.sha256, label);
    demand(bytes.length === item.bytes && sha256(bytes) === item.sha256, label + ': exact artifact bytes');
    total += bytes.length; demand(total <= LIMITS.retained_bytes, 'cumulative retained artifact bytes');
    return json ? parseBoundedJson(bytes, cap) : bytes;
  };
  const a = capture.artifacts, source = read(a.source, LIMITS.source_bytes, 'source', false);
  demand(!source.includes(13) && !source.subarray(0, 3).equals(Buffer.from([239, 187, 191])),
    'normalized source offsets unavailable for CR/BOM profile');
  const sourceText = new TextDecoder('utf-8', { fatal: true }).decode(source);
  demand(!sourceText.includes('amdgpu_asm') && sourceText.includes('pub fn bitwise_chain('), 'ordinary fixture source');
  read(a.manifest, LIMITS.artifact_bytes, 'manifest', false); read(a.lock, LIMITS.artifact_bytes, 'lock', false);
  exact(a.bundle, ['bytes', 'sha256'], 'bundle'); uint(a.bundle.bytes, LIMITS.bundle_bytes, 'bundle', 1); digest(a.bundle.sha256, 'bundle');
  const census = read(a.census, LIMITS.census_bytes, 'census'), files = censusFiles(census, capture, source);
  const summary = read(a.snapshot, LIMITS.artifact_bytes, 'snapshot');
  exact(summary, ['schema', 'authority', 'bundle_identity', 'bundle_subject_identity', 'canonical_kir_version',
    'canonical_kir_digest', 'canonical_kir_bytes', 'target', 'source_map_identity', 'semantic_mir_identity',
    'rustc_identity_inventory_receipt_sha256', 'rustc_identity_inventory_receipt_bytes',
    'rustc_preflight_plan_receipt_sha256', 'rustc_preflight_plan_receipt_bytes', 'compiler_policy_identity',
    'final_artifact_identity', 'operation_count', 'eliminated_source_span_count', 'capabilities'], 'snapshot');
  authority(summary.authority);
  demand(summary.schema === 'fe2o3-multilevel-authoring-observation-v1' && summary.canonical_kir_version === 11
    && summary.target === TARGET && summary.compiler_policy_identity === 'unavailable_in_v6'
    && summary.final_artifact_identity === 'unavailable_extraction_precedes_final_artifact', 'snapshot version/availability');
  for (const key of ['bundle_identity', 'bundle_subject_identity', 'canonical_kir_digest', 'source_map_identity',
    'semantic_mir_identity', 'rustc_identity_inventory_receipt_sha256', 'rustc_preflight_plan_receipt_sha256']) digest(summary[key], key);
  for (const key of ['canonical_kir_bytes', 'rustc_identity_inventory_receipt_bytes', 'rustc_preflight_plan_receipt_bytes'])
    decimal(summary[key], BigInt(LIMITS.bundle_bytes), key, 1n);
  uint(summary.operation_count, LIMITS.operations, 'operation count', 1);
  uint(summary.eliminated_source_span_count, 65536, 'eliminated span count');
  list(summary.capabilities, 4, 'capabilities', 4).forEach(row => {
    exact(row, ['level', 'read', 'select', 'materialize', 'edit', 'readmit', 'simulate', 'inspect'], 'capability');
    Object.values(row).forEach(value => text(value, 128, 'capability value'));
  });
  // Key order is not a capability: compare values after checking exact keys.
  summary.capabilities.forEach((row, index) => {
    for (const key of Object.keys(CAPABILITIES[index]))
      demand(row[key] === CAPABILITIES[index][key], 'snapshot capability changed');
  });
  const operations = [], attributions = [], seen = new Set();
  const pages = list(a.pages, Math.ceil(LIMITS.operations / LIMITS.page_items), 'operation pages', 1);
  for (const item of pages) {
    const page = read(item, LIMITS.artifact_bytes, 'operation page');
    exact(page, ['authority', 'bundle_identity', 'canonical_kir_digest', 'target', 'start', 'total_operations', 'operations', 'next_start'], 'page');
    authority(page.authority);
    demand(page.bundle_identity === summary.bundle_identity && page.canonical_kir_digest === summary.canonical_kir_digest
      && page.target === TARGET && page.start === operations.length && page.total_operations === summary.operation_count,
    'exact contiguous page identity');
    const expectedCount = Math.min(LIMITS.page_items, summary.operation_count - operations.length);
    list(page.operations, expectedCount, 'page operations', expectedCount).forEach(row => {
      const key = coordinate(row.coordinate);
      demand(!seen.has(key), 'duplicate operation occurrence'); seen.add(key);
      const prior = operations.at(-1)?.coordinate;
      demand(!prior || row.coordinate.block > prior.block
        || (row.coordinate.block === prior.block && row.coordinate.operation === prior.operation + 1), 'canonical operation roster order');
      demand(prior?.block === row.coordinate.block || row.coordinate.operation === 0, 'canonical block operation origin');
      const spans = operation(row, files, source);
      attributions.push(...spans.map(span => ({ coordinate: row.coordinate, ...span })));
      operations.push(row);
    });
    demand(page.next_start === (operations.length < summary.operation_count ? operations.length : null), 'page continuation');
  }
  demand(operations.length === summary.operation_count, 'incomplete operation pages');
  const selector = read(a.selector, 16384, 'selector');
  exact(selector, ['bundle_identity', 'canonical_kir_digest', 'target', 'operations'], 'selector');
  demand(selector.bundle_identity === summary.bundle_identity && selector.canonical_kir_digest === summary.canonical_kir_digest
    && selector.target === TARGET, 'stale selector identity/target');
  list(selector.operations, 1, 'selected operation', 1);
  const selected = operations.find(row => coordinate(row.coordinate) === coordinate(selector.operations[0]));
  demand(selected && selected.kind === 'binary' && selected.semantic_detail === 'BitOr', 'selected ordinary OR occurrence');
  demand(operations.filter(row => row.kind === 'binary' && row.semantic_detail === 'BitOr').length === 1, 'ambiguous fixture OR selection');
  demand(selected.inputs.length === 2 && selected.results.length === 1
    && [...selected.inputs, ...selected.results].every(v => v.ty === 'Scalar(U32)'), 'selected scalar boundary profile');
  const region = read(a.region, LIMITS.artifact_bytes, 'region');
  exact(region, ['authority', 'selector', 'structural_boundary', 'source_insertion_boundary',
    'live_in', 'live_out', 'operations', 'materialization'], 'region');
  authority(region.authority); same(region.selector, selector, 'region exact selector');
  same(region.operations, [selected], 'region selected operation');
  same(values(region.live_in), selected.inputs, 'selected exact live-in boundary');
  same(values(region.live_out), selected.results, 'selected exact live-out boundary');
  demand(region.structural_boundary === 'contiguous_operations_in_one_block_no_terminator_selected'
    && region.source_insertion_boundary === 'unavailable_source_application_not_admitted'
    && region.materialization === 'diagnostic_rust_draft_available', 'region structural-only boundary');
  demand(selected.complete_local_effect_summary && selected.local_memory_effects.length === 0, 'selected complete local effect contract');
  const selectedSpans = attributions.filter(row => coordinate(row.coordinate) === coordinate(selected.coordinate));
  demand(selectedSpans.length > 0, 'selected source attribution unavailable');
  const alternatives = attributions.filter(row => selectedSpans.some(span => span.file_identity === row.file_identity
    && span.byte_start === row.byte_start && span.byte_end === row.byte_end));
  demand(alternatives.some(row => coordinate(row.coordinate) !== coordinate(selected.coordinate)), 'expected actual many-to-one attribution');
  const source_file_identity = [...files.keys()][0];
  return { source: { utf8: sourceText, bytes: source.length, sha256: sha256(source), file_identity: source_file_identity },
    summary, source_file_identity, selector, region, operations, attributions,
    selected_range_occurrences: alternatives, source_binding: 'census_joined_attribution_not_ssa_ownership',
    authority: AUTHORITY, availability: AVAILABILITY };
}
export function parseNavigationCapture(bytes) {
  return parseBoundedJson(bytes, 2 * MiB);
}
