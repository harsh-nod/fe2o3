#!/usr/bin/env node
// Fresh Rust source admission of an actually materialized instruction helper.
// This fixture-specific source replacement is explicit, not a general lifting
// or source-edit authority. No edited snapshot is accepted for execution.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.argv.length !== 4) {
  throw new Error("usage: node scripts/assembly-source-roundtrip-smoke.mjs SOURCE_SMOKE_DIRECTORY NEW_OUTPUT_DIRECTORY");
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const baseline = resolve(process.argv[2]);
const output = resolve(process.argv[3]);
mkdirSync(output);
const bin = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug");
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
const saveJson = (name, value) => save(name, `${JSON.stringify(value, null, 2)}\n`);

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
  assert.equal(source.split(before).length, 2, "fixture replacement must match exactly once");
  return source.replace(before, after);
}

try {
  const baselineReceipt = JSON.parse(readFileSync(join(baseline, "receipt.json"), "utf8"));
  assert.equal(baselineReceipt.schema, "fe2o3-assembly-authoring-source-smoke-v30");
  assert.equal(baselineReceipt.hardware_observed, false);
  const fixture = join(root, "crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30");
  const original = readFileSync(join(fixture, "src/lib.rs"), "utf8");
  assert.equal(hash(original), baselineReceipt.source_sha256);
  const baselineBundle = readFileSync(join(baseline, "base-v6.fe2sim"));
  assert.equal(hash(baselineBundle), baselineReceipt.base.bundle_sha256);
  const selector = baselineReceipt.base.selector;
  const generated = JSON.parse(run("materialize", "fe2o3-author", [
    "materialize", "--selector", JSON.stringify(selector), "--helper", "assembly_promoted_region",
  ], baselineBundle));
  assert.equal(generated.status, "diagnostic_source_draft_only");
  assert.equal(generated.live_in.length, 2);
  assert.ok(generated.live_in.every((value) => value.ty === "Scalar(U32)"));
  assert.equal(generated.live_out.length, 1);
  assert.match(generated.source, /pub fn assembly_promoted_region\(/u);
  assert.match(generated.source, /amdgpu_asm!\(v_or_b32\(/u);
  save("generated-helper.rs", generated.source);
  saveJson("generated-helper.json", generated);

  const oldRegion = '    #[cfg(not(feature = "edited"))]\n' +
    '    let result = amdgpu_asm!(v_or_b32(low, 256));\n' +
    '    #[cfg(feature = "edited")]\n' +
    '    let result = amdgpu_asm!(v_or_b32(low, 512));';
  const caller = replaceOnce(original, oldRegion,
    "    let result = assembly_promoted_region(low, 256).0;");
  let manifest = readFileSync(join(fixture, "Cargo.toml"), "utf8");
  for (const name of ["fe2o3-device", "fe2o3-host"]) {
    manifest = replaceOnce(manifest, `"../../../../${name}"`, JSON.stringify(join(root, "crates", name)));
  }
  const lock = readFileSync(join(fixture, "Cargo.lock"));
  const request = JSON.parse(readFileSync(join(baseline, "base-request.json"), "utf8"));
  const a = 0xfffffff0n, b = 0x25n, mask = 0xffffffffn;
  const restored = (((a + b) & mask) - b) & mask;
  const low = (restored ^ b) & 255n;
  const canary = "deadbeefcafebabe";
  const cases = [];
  for (const edited of [false, true]) {
    const label = edited ? "edited-instruction" : "no-edit";
    const sourceDirectory = join(output, `${label}-source`);
    mkdirSync(join(sourceDirectory, "src"), { recursive: true });
    const helper = edited ? replaceOnce(generated.source,
      "amdgpu_asm!(v_or_b32(", "amdgpu_asm!(v_and_b32(") : generated.source;
    const source = `${caller}\n${helper}`;
    writeFileSync(join(sourceDirectory, "src/lib.rs"), source, { flag: "wx" });
    writeFileSync(join(sourceDirectory, "Cargo.toml"), manifest, { flag: "wx" });
    writeFileSync(join(sourceDirectory, "Cargo.lock"), lock, { flag: "wx" });
    saveJson(`${label}-source-change.json`, {
      original_source_sha256: hash(original), source_sha256: hash(source),
      replaced_text: oldRegion, replacement_text: "    let result = assembly_promoted_region(low, 256).0;",
      appended_helper_sha256: hash(helper), explicit_instruction_edit: edited ? "v_or_b32 -> v_and_b32" : null,
      source_application: "fixture_specific_named_candidate_original_untouched",
    });
    const bundlePath = join(output, `${label}.fe2sim`);
    run(`${label}-export`, "fe2o3-export-sim", [
      "--crate", "fe2o3_assembly_authoring_v30_fixture", "--output", bundlePath,
      "--bundle-version", "6", "--target", "gfx942", "--target-dir", join(output, `${label}-extraction`),
      "--", "--manifest-path", join(sourceDirectory, "Cargo.toml"), "--lib",
    ]);
    const bundle = readFileSync(bundlePath);
    const summary = JSON.parse(run(`${label}-inspect`, "fe2o3-author", ["inspect"], bundle));
    assert.equal(summary.authority.grants_production_resume, false);
    for (const field of ["bundle_identity", "canonical_kir_digest", "semantic_mir_identity", "rustc_preflight_plan_receipt_sha256"]) {
      assert.notEqual(summary[field], baselineReceipt.base.summary[field]);
    }
    const operations = [];
    for (let start = 0; start < summary.operation_count;) {
      const page = JSON.parse(run(`${label}-operations-${start}`, "fe2o3-author", [
        "operations", "--bundle-identity", summary.bundle_identity, "--start", String(start), "--limit", "64",
      ], bundle));
      assert.equal(page.start, start);
      assert.ok(page.operations.length > 0 && page.operations.length <= 64);
      operations.push(...page.operations);
      start += page.operations.length;
    }
    const assembly = operations.filter((operation) => operation.mnemonic !== null);
    assert.equal(assembly.length, 7);
    const expectedMnemonics = ["v_mov_b32", "v_mov_b32", "v_add_u32", "v_sub_u32", "v_xor_b32", "v_and_b32", edited ? "v_and_b32" : "v_or_b32"];
    assert.deepEqual(assembly.map((operation) => operation.mnemonic).sort(), expectedMnemonics.sort());
    for (const operation of assembly) {
      const reference = operation.inline_assembly_source;
      assert.equal(reference.authority, "inert_references_not_source_authentication");
      for (const field of ["frontend_unit", "function", "contract", "statement"]) {
        assert.match(reference[field], /^[0-9a-f]{64}$/u);
        assert.notEqual(reference[field], "0".repeat(64));
      }
    }
    assert.equal(new Set(assembly.map((operation) => operation.inline_assembly_source.statement)).size, 7);
    assert.equal(new Set(assembly.map((operation) => operation.inline_assembly_source.frontend_unit)).size, 1);
    assert.equal(new Set(assembly.map((operation) => operation.inline_assembly_source.contract)).size, 1);
    assert.equal(new Set(assembly.map((operation) => operation.inline_assembly_source.function)).size, 2,
      "this fixture must retain the root and generated helper as distinct functions");
    assert.ok(operations.some((operation) => operation.kind === "call"), "generated helper must be called");
    assert.ok(assembly.every((operation) => operation.source_spans.length > 0));
    saveJson(`${label}-operations.json`, operations);
    saveJson(`${label}-request.json`, request);
    const simulation = JSON.parse(run(`${label}-simulation`, "fe2o3-kir-sim", [
      "--bundle-v6", bundlePath, "--request", join(output, `${label}-request.json`),
    ]));
    const expectedWord = edited ? low & 256n : low | 256n;
    assert.equal(expectedWord, edited ? 0n : 469n);
    const word = Buffer.alloc(4);
    word.writeUInt32LE(Number(expectedWord));
    assert.equal(simulation.status, "ok");
    assert.equal(simulation.arguments[0].value.bytes, `0x${word.toString("hex").repeat(4)}${canary}`);
    assert.equal(simulation.arguments[0].value.initialized, "0xffffff");
    assert.equal(simulation.counts.invocations_executed, 4);
    assert.equal(simulation.hardware_observed, false);
    saveJson(`${label}-simulation.json`, simulation);
    cases.push({ label, source_directory: sourceDirectory, source_sha256: hash(source),
      helper_sha256: hash(helper), manifest_sha256: hash(manifest), lock_sha256: hash(lock),
      bundle_sha256: hash(bundle), summary, expected_word: Number(expectedWord) });
  }
  for (const field of ["bundle_identity", "canonical_kir_digest", "semantic_mir_identity", "rustc_preflight_plan_receipt_sha256"]) {
    assert.notEqual(cases[0].summary[field], cases[1].summary[field]);
  }
  assert.equal(hash(readFileSync(join(fixture, "src/lib.rs"))), hash(original));
  saveJson("receipt.json", {
    schema: "fe2o3-assembly-source-roundtrip-smoke-v1", baseline_bundle_sha256: hash(baselineBundle),
    cases, checks: ["actual_materializer_output_compiled", "explicit_selected_occurrence_replacement",
      "unedited_generated_helper_readmission", "edited_instruction_readmission", "independent_cpu_oracles",
      "canaries_unchanged", "fresh_source_and_executable_identities"],
    source_change: "named_concrete_candidates_not_generic_reconstruction",
    semantic_equivalence: "differential_case_checked_not_universal_proof",
    final_machine_inspection: "not_exercised", hardware_observed: false, production_qualification: false,
  });
  console.log(`Generated Rust helper no-edit and edited-instruction roundtrip passed; evidence: ${output}`);
} catch (error) {
  saveJson("failure.json", { status: "failed", error: String(error), hardware_observed: false });
  throw error;
}
