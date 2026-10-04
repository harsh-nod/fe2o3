#!/usr/bin/env node
// Ordinary Rust -> actual canonical bitwise operation -> generated typed ISA
// helper -> explicitly integrated named source variant -> fresh source export.
// This is one known-fixture replacement, not a generic source-lifting service.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.argv.length !== 3) {
  throw new Error("usage: node scripts/ordinary-bitwise-promotion-smoke.mjs NEW_OUTPUT_DIRECTORY");
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = resolve(process.argv[2]);
mkdirSync(output); // Require a fresh run; never overwrite previous evidence.
const bin = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug");
const fixturePath = "crates/rustc-codegen-fe2o3/tests/fixtures/ordinary-bitwise-promotion-v1";
const fixture = join(root, fixturePath);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
const saveJson = (name, value) => save(name, `${JSON.stringify(value, null, 2)}\n`);
const identityFields = ["bundle_identity", "canonical_kir_digest", "semantic_mir_identity", "rustc_preflight_plan_receipt_sha256"];

function run(stage, name, args, input) {
  const result = spawnSync(join(bin, name), args, {
    cwd: root, input, encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 300000,
  });
  save(`${stage}.stdout`, result.stdout ?? "");
  save(`${stage}.stderr`, result.stderr ?? "");
  if (result.error || result.status !== 0) {
    throw new Error(`${stage}: ${name} failed (status ${result.status}): ${result.error ?? result.stderr}`);
  }
  return result.stdout;
}

function replaceOnce(source, before, after) {
  assert.equal(source.split(before).length, 2, "explicit fixture replacement must match exactly once");
  return source.replace(before, after);
}

function inspectOperations(label, bundle, summary) {
  assert.ok(Number.isInteger(summary.operation_count));
  assert.ok(summary.operation_count > 0 && summary.operation_count <= 65536);
  const operations = [];
  for (let start = 0; start < summary.operation_count;) {
    const page = JSON.parse(run(`${label}-operations-${start}`, "fe2o3-author", [
      "operations", "--bundle-identity", summary.bundle_identity,
      "--start", String(start), "--limit", "64",
    ], bundle));
    assert.equal(page.start, start);
    assert.equal(page.total_operations, summary.operation_count);
    assert.equal(page.bundle_identity, summary.bundle_identity);
    assert.equal(page.canonical_kir_digest, summary.canonical_kir_digest);
    assert.ok(page.operations.length > 0 && page.operations.length <= 64);
    operations.push(...page.operations);
    start += page.operations.length;
    assert.equal(page.next_start, start < summary.operation_count ? start : null);
  }
  assert.equal(operations.length, summary.operation_count);
  saveJson(`${label}-operations.json`, operations);
  return operations;
}

function exportSource(label, manifest) {
  const bundlePath = join(output, `${label}.fe2sim`);
  run(`${label}-export`, "fe2o3-export-sim", [
    "--crate", "fe2o3_ordinary_bitwise_promotion_v1_fixture", "--output", bundlePath,
    "--bundle-version", "6", "--target", "gfx942", "--target-dir", join(output, `${label}-extraction`),
    "--", "--manifest-path", manifest, "--lib",
  ]);
  const bundle = readFileSync(bundlePath);
  const summary = JSON.parse(run(`${label}-inspect`, "fe2o3-author", ["inspect"], bundle));
  assert.equal(summary.canonical_kir_version, 11);
  assert.equal(summary.authority.grants_production_resume, false);
  assert.equal(summary.authority.source_authenticated, false);
  saveJson(`${label}-snapshot.json`, summary);
  return { bundle, bundlePath, summary, operations: inspectOperations(label, bundle, summary) };
}

const a = 0xfffffff0n;
const b = 0x25n;
const low = (a ^ b) & 255n;
const canary = "deadbeefcafebabe";
const request = {
  schema: "fe2o3-simulation-request-v1", kernel: "bitwise_chain",
  grid: [4, 1, 1], workgroup: [64, 1, 1],
  arguments: [
    { kind: "buffer", element: "u32", access: "read_write", alignment: 4, bytes: `0x${"a5a5a5a5".repeat(4)}${canary}` },
    { kind: "scalar", type: "u32", bits: "0xfffffff0" },
    { kind: "scalar", type: "u32", bits: "0x00000025" },
  ],
};
function simulate(label, bundlePath, edited) {
  saveJson(`${label}-request.json`, request);
  const reportText = run(`${label}-simulation`, "fe2o3-kir-sim", [
    "--bundle-v6", bundlePath, "--request", join(output, `${label}-request.json`),
  ]);
  const report = JSON.parse(reportText);
  const expectedWord = edited ? low & 256n : low | 256n;
  assert.equal(expectedWord, edited ? 0n : 469n);
  const word = Buffer.alloc(4);
  word.writeUInt32LE(Number(expectedWord));
  assert.equal(report.status, "ok");
  assert.equal(report.arguments[0].value.bytes, `0x${word.toString("hex").repeat(4)}${canary}`);
  assert.equal(report.arguments[0].value.initialized, "0xffffff");
  assert.equal(report.arguments[1].bits, request.arguments[1].bits);
  assert.equal(report.arguments[2].bits, request.arguments[2].bits);
  assert.equal(report.counts.invocations_executed, 4);
  assert.equal(report.hardware_observed, false);
  saveJson(`${label}-simulation.json`, report);
  return { expected_word: Number(expectedWord), simulation_sha256: hash(reportText) };
}

try {
  const original = readFileSync(join(fixture, "src/lib.rs"), "utf8");
  const originalManifest = readFileSync(join(fixture, "Cargo.toml"), "utf8");
  const lock = readFileSync(join(fixture, "Cargo.lock"));
  assert.doesNotMatch(original, /amdgpu_asm/u, "baseline must be ordinary Rust, not authored ISA");
  save("original-source.rs", original);
  saveJson("input.json", {
    source: `${fixturePath}/src/lib.rs`, source_sha256: hash(original),
    manifest_sha256: hash(originalManifest), lock_sha256: hash(lock), hardware_observed: false,
  });
  const baseline = exportSource("ordinary", join(fixture, "Cargo.toml"));
  assert.ok(baseline.operations.every((operation) => operation.mnemonic === null));
  assert.ok(baseline.operations.every((operation) => operation.inline_assembly_source === null));
  const selectedOperations = baseline.operations.filter((operation) =>
    operation.kind === "binary" && operation.semantic_detail === "BitOr");
  assert.equal(selectedOperations.length, 1, "select only the known fixture's unique final OR");
  const selected = selectedOperations[0];
  assert.equal(selected.mnemonic, null, "ordinary binary must not be displayed as an existing ISA operation");
  assert.ok(selected.source_spans.length > 0);
  assert.equal(selected.inputs.length, 2);
  assert.equal(selected.results.length, 1);
  assert.ok([...selected.inputs, ...selected.results].every((value) => value.ty === "Scalar(U32)"));
  const selector = {
    bundle_identity: baseline.summary.bundle_identity,
    canonical_kir_digest: baseline.summary.canonical_kir_digest,
    target: baseline.summary.target,
    operations: [selected.coordinate],
  };
  saveJson("ordinary-selector.json", selector);
  const region = JSON.parse(run("ordinary-select", "fe2o3-author", [
    "select", "--selector", JSON.stringify(selector),
  ], baseline.bundle));
  assert.equal(region.operations.length, 1);
  assert.equal(region.materialization, "diagnostic_rust_draft_available");
  const generated = JSON.parse(run("ordinary-materialize", "fe2o3-author", [
    "materialize", "--selector", JSON.stringify(selector), "--helper", "bitwise_promoted_region",
  ], baseline.bundle));
  assert.equal(generated.status, "diagnostic_source_draft_only");
  assert.equal(generated.semantic_equivalence, "unproved");
  assert.equal(generated.authority.grants_production_resume, false);
  assert.deepEqual(generated.live_in, selected.inputs, "bind helper arguments in the actual selected operand order");
  assert.deepEqual(generated.live_out, selected.results);
  assert.match(generated.source, /pub fn bitwise_promoted_region\(/u);
  assert.match(generated.source, /amdgpu_asm!\(v_or_b32\(/u);
  save("generated-helper.rs", generated.source);
  saveJson("generated-helper.json", generated);
  const ordinaryResult = simulate("ordinary", baseline.bundlePath, false);

  const replaced = "    let result = low | 256;";
  const replacement = "    let result = bitwise_promoted_region(low, 256).0;";
  const caller = replaceOnce(original, replaced, replacement);
  let manifest = originalManifest;
  for (const dependency of ["fe2o3-device", "fe2o3-host"]) {
    manifest = replaceOnce(manifest, `"../../../../${dependency}"`, JSON.stringify(join(root, "crates", dependency)));
  }
  const cases = [];
  for (const edited of [false, true]) {
    const label = edited ? "edited-instruction" : "no-edit";
    const directory = join(output, `${label}-source`);
    mkdirSync(join(directory, "src"), { recursive: true });
    const helper = edited ? replaceOnce(generated.source,
      "amdgpu_asm!(v_or_b32(", "amdgpu_asm!(v_and_b32(") : generated.source;
    const source = `${caller}\n${helper}`;
    writeFileSync(join(directory, "src/lib.rs"), source, { flag: "wx" });
    writeFileSync(join(directory, "Cargo.toml"), manifest, { flag: "wx" });
    writeFileSync(join(directory, "Cargo.lock"), lock, { flag: "wx" });
    saveJson(`${label}-source-change.json`, {
      original_source_sha256: hash(original), source_sha256: hash(source),
      selector, replaced_text: replaced, replacement_text: replacement,
      appended_helper_sha256: hash(helper), instruction_edit: edited ? "v_or_b32 -> v_and_b32" : null,
      application: "explicit_known_fixture_replacement_named_concrete_variant_original_untouched",
      source_dependency_paths: "explicit_current_compiler_device_and_host_path_dependencies",
    });
    const current = exportSource(label, join(directory, "Cargo.toml"));
    for (const field of identityFields) assert.notEqual(current.summary[field], baseline.summary[field]);
    const assembly = current.operations.filter((operation) => operation.mnemonic !== null);
    assert.equal(assembly.length, 1, "only the selected ordinary OR becomes a typed ISA occurrence");
    assert.equal(assembly[0].mnemonic, edited ? "v_and_b32" : "v_or_b32");
    assert.ok(assembly[0].source_spans.length > 0);
    const reference = assembly[0].inline_assembly_source;
    assert.equal(reference.authority, "inert_references_not_source_authentication");
    for (const field of ["frontend_unit", "function", "contract", "statement"]) {
      assert.match(reference[field], /^[0-9a-f]{64}$/u);
      assert.notEqual(reference[field], "0".repeat(64));
    }
    const calls = current.operations.filter((operation) => operation.kind === "call");
    assert.ok(calls.some((operation) => operation.coordinate.function !== assembly[0].coordinate.function),
      "generated helper must actually be called from the kernel");
    assert.ok(!current.operations.some((operation) => operation.kind === "binary" && operation.semantic_detail === "BitOr"),
      "original final OR must not remain as a hidden executable fallback");
    for (const detail of ["BitXor", "BitAnd"]) {
      assert.ok(current.operations.some((operation) => operation.kind === "binary" && operation.semantic_detail === detail),
        `outer ordinary ${detail} remains compiler-managed`);
    }
    cases.push({
      label, source_directory: directory, source_sha256: hash(source), helper_sha256: hash(helper),
      manifest_sha256: hash(manifest), lock_sha256: hash(lock), bundle_sha256: hash(current.bundle),
      summary: current.summary, inline_source_reference: reference,
      ...simulate(label, current.bundlePath, edited),
    });
  }
  for (const field of identityFields) assert.notEqual(cases[0].summary[field], cases[1].summary[field]);
  for (const field of ["frontend_unit", "statement"]) {
    assert.notEqual(cases[0].inline_source_reference[field], cases[1].inline_source_reference[field]);
  }
  assert.equal(cases[0].inline_source_reference.contract, cases[1].inline_source_reference.contract);
  assert.equal(hash(readFileSync(join(fixture, "src/lib.rs"))), hash(original));
  saveJson("receipt.json", {
    schema: "fe2o3-ordinary-bitwise-promotion-smoke-v1",
    source: `${fixturePath}/src/lib.rs`, original_source_sha256: hash(original),
    ordinary_bundle_sha256: hash(baseline.bundle), ordinary_summary: baseline.summary,
    ordinary_result: ordinaryResult, selector, cases,
    checks: ["ordinary_rust_baseline_without_isa", "exact_binary_bitor_selection", "actual_materializer_output",
      "explicit_selected_source_replacement", "unedited_generated_helper_fresh_compilation",
      "edited_generated_instruction_fresh_compilation", "independent_integer_oracles", "four_outputs_and_two_canaries",
      "remaining_compute_stays_ordinary_rust", "original_source_preserved", "fresh_executable_identities"],
    equivalence: "one_independent_differential_case_not_universal_proof",
    source_change: "named_concrete_variants_not_generic_reconstruction",
    final_machine_inspection: "not_exercised", hardware_observed: false, production_qualification: false,
  });
  console.log(`Ordinary Rust to generated typed ISA source promotion passed; evidence: ${output}`);
} catch (error) {
  saveJson("failure.json", { status: "failed", error: String(error), synthetic_fallback: false, hardware_observed: false });
  throw error;
}
