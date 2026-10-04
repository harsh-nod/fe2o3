#!/usr/bin/env node
// Observe LLVM text from already captured real-source V6 bundles. This does not
// invoke LLVM tools or create an artifact, production handoff, or proof receipt.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.argv.length !== 5) {
  throw new Error("usage: node scripts/assembly-source-llvm-inspection-smoke.mjs SOURCE_SMOKE_DIRECTORY SOURCE_ROUNDTRIP_DIRECTORY NEW_OUTPUT_DIRECTORY");
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const baseline = resolve(process.argv[2]);
const roundtrip = resolve(process.argv[3]);
const output = resolve(process.argv[4]);
mkdirSync(output);
const executable = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")),
  "debug", "examples", "inspect_bundle_v6_llvm");
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
const saveJson = (name, value) => save(name, `${JSON.stringify(value, null, 2)}\n`);

function run(path) {
  return spawnSync(executable, [path], {
    cwd: root, encoding: "utf8", timeout: 60000, maxBuffer: 65 * 1024 * 1024,
  });
}

try {
  const originalReceipt = JSON.parse(readFileSync(join(baseline, "receipt.json"), "utf8"));
  const sourceReceipt = JSON.parse(readFileSync(join(roundtrip, "receipt.json"), "utf8"));
  assert.equal(originalReceipt.schema, "fe2o3-assembly-authoring-source-smoke-v30");
  assert.equal(sourceReceipt.schema, "fe2o3-assembly-source-roundtrip-smoke-v1");
  assert.equal(originalReceipt.hardware_observed, false);
  assert.equal(sourceReceipt.hardware_observed, false);
  assert.equal(sourceReceipt.baseline_bundle_sha256, originalReceipt.base.bundle_sha256);
  const cases = [
    { ...originalReceipt.base, label: "base", path: join(baseline, "base-v6.fe2sim"),
      operationsPath: join(baseline, "base-operations.json") },
    ...sourceReceipt.cases.map((entry) => ({ ...entry, path: join(roundtrip, `${entry.label}.fe2sim`),
      operationsPath: join(roundtrip, `${entry.label}-operations.json`) })),
  ];
  assert.deepEqual(cases.map(({ label }) => label), ["base", "no-edit", "edited-instruction"]);
  const observations = [];
  for (const entry of cases) {
    const bytes = readFileSync(entry.path);
    assert.equal(hash(bytes), entry.bundle_sha256);
    const result = run(entry.path);
    save(`${entry.label}.ll`, result.stdout ?? "");
    save(`${entry.label}.stderr`, result.stderr ?? "");
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    const separator = "; llvm_text_begin\n";
    const parts = result.stdout.split(separator);
    assert.equal(parts.length, 2);
    const [header, llvm] = parts;
    const fields = Object.fromEntries([...header.matchAll(/^; ([a-z0-9_]+): (.*)$/gmu)]
      .map((match) => [match[1], match[2]]));
    assert.equal(fields.schema, "fe2o3-bundle-v6-llvm-text-observation-v1");
    assert.equal(fields.observation_only, "true");
    for (const field of ["authenticates_compiler_execution", "source_authenticated",
      "grants_production_resume", "grants_proof_authority", "grants_artifact_authority",
      "grants_load_or_launch", "machine_code_generated"]) {
      assert.equal(fields[field], "false", field);
    }
    assert.equal(fields.final_machine_inspection, "not_exercised");
    assert.equal(fields.target, "gfx942:xnack-");
    for (const field of ["bundle_identity", "bundle_subject_identity", "canonical_kir_digest",
      "semantic_mir_identity", "rustc_preflight_plan_receipt_sha256"]) {
      assert.equal(fields[field], entry.summary[field], field);
    }
    assert.equal(fields.canonical_kir_version, "11");
    assert.equal(fields.canonical_kir_bytes, entry.summary.canonical_kir_bytes);
    assert.equal(fields.target_binding, "existing_target_transform_only_no_refinement");
    assert.equal(fields.target_bound_kir_version, "11");
    assert.match(fields.target_bound_kir_digest, /^[0-9a-f]{64}$/u);
    assert.notEqual(fields.target_bound_kir_digest, fields.canonical_kir_digest);
    assert.ok(Number(fields.target_bound_kir_bytes) > Number(fields.canonical_kir_bytes));
    assert.equal(fields.llvm_text_sha256, hash(llvm));
    assert.match(llvm, /target triple = "amdgcn-amd-amdhsa"/u);
    assert.match(llvm, /"target-cpu"="gfx942"/u);
    assert.match(llvm, /"target-features"="[^"]*-xnack/u);
    const emitted = [...llvm.matchAll(/call i32 asm sideeffect "([a-z0-9_]+) ([^"]+)", "([^"]+)"/gu)];
    const expected = ["v_mov_b32", "v_mov_b32", "v_add_u32", "v_sub_u32", "v_xor_b32", "v_and_b32",
      entry.label === "edited-instruction" ? "v_and_b32" : "v_or_b32"];
    assert.deepEqual(emitted.map((match) => match[1]).sort(), expected.sort());
    for (const match of emitted) {
      const unary = match[1] === "v_mov_b32";
      assert.equal(match[2], unary ? "$0, $1" : "$0, $1, $2");
      assert.equal(match[3], unary ? "=v,v" : "=v,v,v");
    }
    assert.equal(fields.inline_assembly_count, "7");
    const sourceRefs = [...header.matchAll(/^; inline_assembly .* source_function=([0-9a-f]{64}) .* statement=([0-9a-f]{64})$/gmu)];
    assert.equal(sourceRefs.length, 7);
    assert.equal(new Set(sourceRefs.map((match) => match[2])).size, 7);
    assert.equal(new Set(sourceRefs.map((match) => match[1])).size, entry.label === "base" ? 1 : 2);
    const operationBytes = readFileSync(entry.operationsPath);
    const operations = JSON.parse(operationBytes);
    const expectedRows = operations.filter((operation) => operation.kind === "inline_assembly")
      .map(({ coordinate, mnemonic, inline_assembly_source: source }) => ({
        coordinate, mnemonic, frontend_unit: source.frontend_unit,
        source_function: source.function, contract: source.contract, statement: source.statement,
      }));
    const actualRows = [...header.matchAll(/^; inline_assembly (.*)$/gmu)].map((match) => {
      const row = Object.fromEntries(match[1].split(" ").map((field) => field.split("=")));
      return { coordinate: { function: Number(row.function_ordinal), block: Number(row.block_ordinal),
        operation: Number(row.operation_ordinal) }, mnemonic: row.mnemonic, frontend_unit: row.frontend_unit,
      source_function: row.source_function, contract: row.contract, statement: row.statement };
    });
    assert.deepEqual(actualRows, expectedRows, "every coordinate and source identity must match its retained operation");
    const helpers = [...llvm.matchAll(/^define (?:internal )?i32 (@"(?:[^"\\]|\\.)*"|@[A-Za-z0-9_.$-]+)\(/gmu)];
    if (entry.label !== "base") {
      assert.equal(helpers.length, 1, "generated scalar helper definition must remain");
      assert.ok(llvm.includes(`call i32 ${helpers[0][1]}(`), "generated helper call must remain");
      assert.equal(fields.call_operation_count, "1");
    }
    observations.push({ label: entry.label, bundle_sha256: entry.bundle_sha256,
      bundle_identity: fields.bundle_identity, llvm_text_sha256: fields.llvm_text_sha256,
      original_kir_digest: fields.canonical_kir_digest, target_bound_kir_digest: fields.target_bound_kir_digest,
      observation_sha256: hash(result.stdout), inline_assembly_count: emitted.length,
      retained_operations_sha256: hash(operationBytes),
      retained_helper_calls: Number(fields.call_operation_count) });
  }
  assert.equal(new Set(observations.map((entry) => entry.llvm_text_sha256)).size, 3);
  // Independently corrupt committed bytes: the example must refuse before
  // emitting even an observation header. These are not accepted edited KIRs.
  const original = readFileSync(cases[0].path);
  for (const [label, bytes] of [
    ["truncated", original.subarray(0, 8)],
    ["corrupt-tail", Buffer.from(original)],
    ["legacy-magic", Buffer.from(original)],
  ]) {
    if (label === "corrupt-tail") bytes[bytes.length - 1] ^= 1;
    if (label === "legacy-magic") bytes[7] = "5".charCodeAt(0);
    save(`${label}.fe2sim`, bytes);
    const result = run(join(output, `${label}.fe2sim`));
    save(`${label}.stderr`, result.stderr ?? "");
    assert.ifError(result.error);
    assert.notEqual(result.status, 0);
    assert.equal(result.stdout, "");
    assert.match(result.stderr, /V6 admission:/u);
  }
  saveJson("receipt.json", {
    schema: "fe2o3-assembly-source-llvm-text-inspection-smoke-v1", observations,
    checks: ["exact_bundle_and_source_identity_references", "all_six_typed_instruction_templates",
      "exact_vgpr_constraints", "generated_helper_definition_and_call_retained",
      "edited_instruction_changes_llvm_text", "malformed_bundle_rejected_without_output"],
    observation_only: true, authenticates_compiler_execution: false,
    target_binding: "existing_target_transform_only_no_refinement",
    machine_code_generated: false, final_machine_inspection: "not_exercised",
    grants_proof_authority: false, grants_artifact_authority: false,
    grants_production_resume: false, grants_load_or_launch: false,
  });
  console.log(`Real-source V6 LLVM-text inspection passed; observations: ${output}`);
} catch (error) {
  saveJson("failure.json", { status: "failed", error: String(error), observation_only: true,
    machine_code_generated: false, grants_production_resume: false });
  throw error;
}
