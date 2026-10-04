# Debug a complete-body Rust kernel on the CPU

The exact V19 diagnostic route connects the [bounded Rust authoring
example](complete-body-source-v19.md) to the existing CPU simulator and
logical debugger. It does not require a GPU or a private rustc callback.

The current source profile is deliberately narrow: gfx942:xnack-, Wave64,
required **and** maximum workgroup 64×1×1, explicit finite max_grid,
one authored block or a four-block selector diamond, and the supported
u32 moves/ALU operations. These examples declare max_grid=[2,1,1].
This is not general assembler coverage or completion of all authoring and
visualization milestones.

## Run the public source command

Use Node 22 or newer and the checkout's installed pinned nightly-2026-04-03
with rustc-dev, rust-src and cached dependencies. Set RUSTC to its absolute rustc path;
the script never installs a toolchain. Build the matching ordinary binaries
under the project's normal serialized build/resource controls:

~~~sh
cargo build --offline --locked -p rustc-codegen-fe2o3 \
  --lib --bin fe2o3-rustc-extract
cargo build --offline --locked -p fe2o3-kir-sim-cli -p fe2o3-debug-cli

node scripts/complete-body-debug-source-v19.mjs one /absolute/new-one-debug
node scripts/complete-body-debug-source-v19.mjs diamond /absolute/new-diamond-debug
~~~

Every output directory must be new. CARGO_TARGET_DIR selects the matching
debug binaries; FE2O3_COMPLETE_BODY_BIN_DIR_V19 may instead name their absolute
directory. The actual source is the normal Cargo fixture's
[src/complete_body_v19.rs](../crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/complete_body_v19.rs),
selected by complete-body-one-v19 or complete-body-diamond-v19.

The command performs this bounded workflow:

1. Ordinary Cargo invokes fe2o3-rustc-extract. The new, mutually exclusive
   FE2O3_EXTRACT_DIAGNOSTIC_KIR_PATH_V19 selector enters the normal authenticated
   Rust/MIR36 continuation, actual ranked/formal checks and canonical emission
   relation. It borrows the same checked owner's exact canonical executable
   bytes and publishes diagnostic-v19.kir using create-new output.
2. The simulator independently decodes and validates those exact V19 bytes.
   Requests use actual exported kernel identity, inputs 19/23/42,
   selectors 0/1/u32::MAX, grid128 and workgroup64. One-block output is 19.
   Diamond output is 19 for zero and 23 otherwise.
3. The result checker verifies all 128 output values, initialization and
   two trailing canary words. The allocation is 520 bytes, satisfying the
   conservative 512-byte LaunchEnvelope requirement for this source bound.
4. The ordinary debugger opens the same bytes and requests. Its JSONL protocol
   discovers capabilities and checks entry → first logical event → entry.
   The command records exact input/output streams and verifies unchanged
   source, executable and diagnostic-byte hashes.

Successful runs retain receipt.json, export-observation.json,
diagnostic-v19.kir, selector-*-request.json, selector-*-cpu.json and
selector-*-debug-requests.jsonl, plus bounded process records and logs.
The debug responses are in selector-*-debug.stdout. A failure preserves
failure.json and available process logs, never a successful receipt.
Each child has a 300-second and 16-MiB combined-log stop bound; the fresh
output tree has an observed 1-GiB/100,000-entry stop bound. These are not disk
reservations or whole-process/compiler RSS guarantees.

## Recorded qualification

Both public source-debug commands passed on 2026-09-24 UTC: six CPU requests
checked all values, initialization and canaries, and six logical debugger
sessions checked entry → first event → entry with preserved diagnostic bytes.
Physical register values, source-variable maps and hardware wave state remain
unavailable. The [compiler evidence record](evidence/complete-body-source-v19-20260924.md)
retains exact public receipt/output pins and distinguishes this path from the
separate 384-case source ladder and native compilation-only checks.

## Use the simulator and debugger directly

After export, existing author tools can consume the diagnostic bytes:

~~~sh
fe2o3-kir-sim \
  --diagnostic-kir-v19 /absolute/new-diamond-debug/diagnostic-v19.kir \
  --request /absolute/new-diamond-debug/selector-1-request.json \
  --output /absolute/new-cpu-result.json

fe2o3-debug sim \
  --diagnostic-kir-v19 /absolute/new-diamond-debug/diagnostic-v19.kir \
  --request /absolute/new-diamond-debug/selector-1-request.json \
  --protocol jsonl --wave-width 64 \
  < /absolute/new-diamond-debug/selector-1-debug-requests.jsonl
~~~

The explicit --diagnostic-kir-v19 flag is required. There is no automatic
fallback from older KIR or bundle versions. Input selection is mutually
exclusive with other diagnostic inputs and bundle selectors.

The existing debugger protocol supplies logical event stepping/reverse replay,
KIR-site breakpoints, SSA value inspection, memory inspection, lane/workgroup
views and logical resource queries. Authored SSA steps use the existing
before/after checkpoint machinery; branches and merges use the existing
control-flow engine. The declaration record is resultless, not a second
interpreter. The script's short replay establishes a usable command path;
the focused Rust tests additionally inspect authored-step before/after SSA
values and both branch memory results.

A front end can use discover_capabilities before presenting each view.
Logical wave/lane views are simulation visualizations, not hardware captures.
Source-variable inspection requires an authenticated source map, which this
raw export does not carry.

## Evidence and authority boundaries

| Stage | What is retained | What is not conferred |
| --- | --- | --- |
| Live compiler continuation | Actual source owner, checked V19 and canonical emission relation | Native worker completion or GPU launch |
| Exported .kir file | Exact typed canonical V19 representation and observation metadata | Compiler/source ownership, proof or resume authority |
| CPU/debugger session | Validated diagnostic program, request, logical values and memory observations | Physical VGPR/EXEC contents, wave timing or hardware validation |

The exporter does not reconstruct source custody from bytes or emit an
authenticated source map. SHA-256 pins and stderr metadata are observations,
not portable source authentication. Serializing the checked representation
does not serialize the checked compiler owner.

The new route explicitly refuses source maps, Wave32, persisted schedules,
schedule exploration/reduction and runtime-observation attachment.
Diagnosis V2 remains unavailable: its existing canonical_kir_v7 field must
not be used to relabel V19. Existing wire and bundle versions retain their
original meanings. Standard logical debugger/resource queries remain
available without that diagnosis envelope.

This command does not invoke LLVM machine-code generation, a native worker,
protected artifact finalization, load/launch or a GPU. Use the separate
[normal LLVM/handoff command and actual-source qualification
ladder](complete-body-source-v19.md) for compiler descriptor/handoff evidence.
Neither command closes the remaining general-body, multi-kernel, source-map,
physical-register, native-code or hardware qualification work.
