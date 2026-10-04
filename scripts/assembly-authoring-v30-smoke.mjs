#!/usr/bin/env node
// Actual Rust-source export only. Compiler errors stop this workflow; no fixture
// IR or manufactured provenance substitutes for a failed frontend handoff.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.argv.length !== 3) {
  throw new Error("usage: node scripts/assembly-authoring-v30-smoke.mjs NEW_OUTPUT_DIRECTORY");
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = resolve(process.argv[2]);
mkdirSync(output); // Never overwrite an earlier run's evidence.
const bin = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug");
const fixture = "crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30";
const manifest = join(root, fixture, "Cargo.toml");
const sourcePath = `${fixture}/src/lib.rs`;
const sourceBytes = readFileSync(join(root, sourcePath));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
const saveJson = (name, value) => save(name, `${JSON.stringify(value, null, 2)}\n`);
const hexIdentity = /^[0-9a-f]{64}$/u;

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

function inspectOperations(label, bundle, summary) {
  assert.ok(Number.isInteger(summary.operation_count));
  assert.ok(summary.operation_count > 0 && summary.operation_count <= 65536);
  const operations = [];
  let start = 0;
  while (start < summary.operation_count) {
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
    const next = start + page.operations.length;
    assert.equal(page.next_start, next < summary.operation_count ? next : null);
    start = next;
  }
  assert.equal(operations.length, summary.operation_count);
  saveJson(`${label}-operations.json`, operations);
  return operations;
}

// Independent bit-vector arithmetic: deliberately cross the u32 addition wrap.
const a = 0xfffffff0n;
const b = 0x25n;
const mask = 0xffffffffn;
const wrappedSum = (a + b) & mask;
const restored = (wrappedSum - b) & mask;
assert.equal(wrappedSum, 0x15n);
assert.equal(restored, a);
const low = (restored ^ b) & 255n;
const canary = "deadbeefcafebabe";
const outputWords = 4;
function wordHex(word) {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(Number(word));
  return bytes.toString("hex");
}

function sourceReferences(assembly) {
  assert.equal(assembly.length, 7, "all seven source marker occurrences must remain inspectable");
  const expected = ["v_mov_b32", "v_mov_b32", "v_add_u32", "v_sub_u32", "v_xor_b32", "v_and_b32", "v_or_b32"];
  assert.deepEqual(assembly.map((op) => op.mnemonic).sort(), expected.sort());
  const refs = assembly.map((op) => {
    assert.ok(op.source_spans.length > 0, "real compiler source mapping is required");
    const source = op.inline_assembly_source;
    assert.ok(source, "operations JSON must expose retained inline source references");
    assert.equal(source.authority, "inert_references_not_source_authentication");
    for (const field of ["frontend_unit", "function", "contract", "statement"]) {
      assert.match(source[field], hexIdentity);
      assert.notEqual(source[field], "0".repeat(64));
    }
    return source;
  });
  assert.equal(new Set(refs.map((ref) => ref.statement)).size, 7);
  for (const field of ["frontend_unit", "function", "contract"]) {
    assert.equal(new Set(refs.map((ref) => ref[field])).size, 1, `one exact ${field} per kernel`);
  }
  const moves = assembly.filter((op) => op.mnemonic === "v_mov_b32");
  assert.notDeepEqual(moves[0].coordinate, moves[1].coordinate);
  assert.notEqual(moves[0].inline_assembly_source.statement, moves[1].inline_assembly_source.statement);
  return refs;
}

function exportAndCheck(label, edited) {
  const bundlePath = join(output, `${label}-v6.fe2sim`);
  const cargoArgs = ["--manifest-path", manifest, "--lib"];
  if (edited) cargoArgs.push("--features", "edited");
  run(`${label}-export`, "fe2o3-export-sim", [
    "--crate", "fe2o3_assembly_authoring_v30_fixture", "--output", bundlePath,
    "--bundle-version", "6", "--target", "gfx942",
    "--target-dir", join(output, `${label}-extraction`), "--", ...cargoArgs,
  ]);
  const bundle = readFileSync(bundlePath);
  const summary = JSON.parse(run(`${label}-inspect`, "fe2o3-author", ["inspect"], bundle));
  assert.equal(summary.canonical_kir_version, 11);
  assert.equal(summary.authority.grants_production_resume, false);
  assert.equal(summary.authority.source_authenticated, false);
  saveJson(`${label}-snapshot.json`, summary);
  const operations = inspectOperations(label, bundle, summary);
  const assembly = operations.filter((op) => op.mnemonic !== null);
  const references = sourceReferences(assembly);
  const selected = assembly.find((op) => op.mnemonic === "v_or_b32");
  const selector = {
    bundle_identity: summary.bundle_identity,
    canonical_kir_digest: summary.canonical_kir_digest,
    target: summary.target,
    operations: [selected.coordinate],
  };
  saveJson(`${label}-selector.json`, selector);
  const region = JSON.parse(run(`${label}-select`, "fe2o3-author", [
    "select", "--selector", JSON.stringify(selector),
  ], bundle));
  assert.equal(region.operations.length, 1);
  assert.equal(region.materialization, "diagnostic_rust_draft_available");
  const draft = JSON.parse(run(`${label}-materialize`, "fe2o3-author", [
    "materialize", "--selector", JSON.stringify(selector), "--helper", `assembly_${label}_draft`,
  ], bundle));
  assert.equal(draft.status, "diagnostic_source_draft_only");
  assert.equal(draft.semantic_equivalence, "unproved");
  assert.equal(draft.authority.grants_production_resume, false);
  assert.match(draft.source, /amdgpu_asm!\(v_or_b32/u);
  saveJson(`${label}-draft.json`, draft);
  save(`${label}-draft.rs`, draft.source);

  const request = {
    schema: "fe2o3-simulation-request-v1", kernel: "assembly_chain",
    grid: [outputWords, 1, 1], workgroup: [64, 1, 1],
    arguments: [
      { kind: "buffer", element: "u32", access: "read_write", alignment: 4, bytes: `0x${"a5a5a5a5".repeat(outputWords)}${canary}` },
      { kind: "scalar", type: "u32", bits: "0xfffffff0" },
      { kind: "scalar", type: "u32", bits: "0x00000025" },
    ],
  };
  saveJson(`${label}-request.json`, request);
  const simulationText = run(`${label}-simulation`, "fe2o3-kir-sim", [
    "--bundle-v6", bundlePath, "--request", join(output, `${label}-request.json`),
  ]);
  const simulation = JSON.parse(simulationText);
  const expectedWord = low | (edited ? 512n : 256n);
  assert.equal(expectedWord, edited ? 0x2d5n : 0x1d5n);
  assert.equal(simulation.status, "ok");
  assert.equal(simulation.arguments[0].value.bytes, `0x${wordHex(expectedWord).repeat(outputWords)}${canary}`);
  assert.equal(simulation.arguments[0].value.initialized, "0xffffff");
  assert.equal(simulation.arguments[1].bits, request.arguments[1].bits);
  assert.equal(simulation.arguments[2].bits, request.arguments[2].bits);
  assert.equal(simulation.counts.invocations_executed, outputWords);
  assert.equal(simulation.hardware_observed, false);
  saveJson(`${label}-simulation.json`, simulation);
  return {
    features: edited ? ["edited"] : [],
    bundle_sha256: hash(bundle), summary, inline_source_references: references,
    expected_word: Number(expectedWord), simulation_sha256: hash(simulationText),
    selector, draft_source_sha256: hash(draft.source),
  };
}

try {
  saveJson("input.json", { source: sourcePath, source_sha256: hash(sourceBytes), manifest, hardware_observed: false });
  const base = exportAndCheck("base", false);
  const edited = exportAndCheck("edited", true);
  for (const field of ["bundle_identity", "canonical_kir_digest", "semantic_mir_identity", "rustc_preflight_plan_receipt_sha256"]) {
    assert.notEqual(base.summary[field], edited.summary[field], `source feature edit must change ${field}`);
  }
  assert.notEqual(base.inline_source_references[0].frontend_unit, edited.inline_source_references[0].frontend_unit);
  assert.equal(base.inline_source_references[0].contract, edited.inline_source_references[0].contract);
  const originalStatements = new Set(base.inline_source_references.map((ref) => ref.statement));
  assert.ok(edited.inline_source_references.every((ref) => !originalStatements.has(ref.statement)));
  assert.equal(hash(readFileSync(join(root, sourcePath))), hash(sourceBytes), "fixture source must remain unchanged");
  saveJson("receipt.json", {
    schema: "fe2o3-assembly-authoring-source-smoke-v30", source: sourcePath,
    source_sha256: hash(sourceBytes), base, edited,
    checks: ["actual_source_exports", "all_six_typed_instructions", "distinct_same_marker_occurrences", "exact_one_block_selection", "diagnostic_source_draft", "independent_wrapping_u32_oracles", "four_output_words_and_canaries", "changed_compiler_observed_identities"],
    edited_input: "explicit_cargo_feature_changes_final_or_operand_only",
    draft_readmission: "not_exercised", source_application: "not_exercised",
    source_reference_authority: "inert_references_not_source_authentication",
    hardware_observed: false,
  });
  console.log(`Actual-source V30 authoring and CPU simulation checks passed; evidence: ${output}`);
} catch (error) {
  saveJson("failure.json", { status: "failed", message: String(error), synthetic_fallback: false, hardware_observed: false });
  throw error;
}
