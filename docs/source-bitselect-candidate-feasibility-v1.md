# Typed ordinary-source candidate feasibility

This is a **test-only U2 feasibility path**, not a public editing command,
production continuation, generic decompiler, protected artifact or hardware
qualification. It exercises real rustc callbacks and compiler-owned source/KIR
correspondence rather than treating a source-map span or serialized report as
permission to replace source.

## Exact supported experiment

The ordinary Rust initializer is `b ^ ((a ^ b) & mask)`, with three distinct
immutable `u32` formal parameters in a single direct kernel root. The selected
three operations must have exact typed HIR/semantic/KIR correspondence and
source attribution, no escaping intermediate, no local memory effect, and one
live result. The target is authenticated gfx942:xnack-/wave64 with required
**and** maximum workgroup size64x1x1. Aliases, expanded/normalized source,
ambiguous candidates, other targets and launch contracts are unsupported.

The actual baseline follows ordinary import, middle end, SSA, materialization,
ranked checks and target-neutral attachment. A borrowed live V8 owner supplies
the checked operation/parameter correspondence; it is not a diagnostic V11
bundle recreated from bytes. A retained regular-file descriptor and exact
parsed compiler source/hash/normalization checks bind the selected initializer.
These checks do not claim rustc read through that descriptor or authenticate
every dependency by hashing this one file.

The shared inert formatter emits one structured
`fe2o3_device::amdgpu_ordered_program!` expression with three instructions:

```text
xor(scratch, input0, input1);
and(scratch, scratch, input2);
xor(out, input1, scratch);
```

Only the checked initializer range is replaced in a new candidate file.
Original bytes are rechecked; staged bytes are read back and published with
no replacement of an existing destination. The original is never overwritten.
The formatter independently validates bounded identifiers/registers but by
itself supplies no source, compiler-owner or insertion authority.

A **new actual frontend callback** parses the candidate and uses the existing
pre-ranked MIR32/KIR17 ordered-program route. It checks fresh sealed Instance
identities, parameter ordinals/types, actual SSA operands, descriptors and
register roles. The admitted program is evaluated for128 deterministic edge/
random vectors against the independent Boolean expression
`(a & mask) | (b & !mask)`. This is a program-level finite oracle, not execution
of the entire candidate kernel or a universal equivalence proof. It does not
reuse the baseline's ranked checks, proof, source map or native receipt.

## Reproduction

Use the repository's pinned nightly and normal installed provider/toolchain
closure. Run from the repository root, with the usual bounded Cargo target and
build-job settings. Each output path must be new, absolute and task-owned:

```sh
FE2O3_TEST_SOURCE_BITSELECT_OUTPUT=/absolute/new/bitselect-source-run \
  cargo test --offline --locked -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::actual_source_bitselect_feasibility_ladder \
  -- --exact --ignored --nocapture

FE2O3_TEST_SOURCE_BITSELECT_ROUNDTRIP_OUTPUT=/absolute/new/bitselect-roundtrip-run \
  cargo test --offline --locked -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::actual_source_bitselect_candidate_roundtrip_ladder \
  -- --exact --ignored --nocapture
```

The second harness places fresh sources under the repository's ignored
`target/source-bitselect-candidate-roundtrip/<output-basename>` directory.
Provider-relative source authentication still uses the repository working
directory. It refuses symlinked containers and existing per-run roots. This
path is separate from `CARGO_TARGET_DIR`; its storage is charged separately.
The basename is a bounded safe component, not an arbitrary source path.

## September 19 working-tree qualification on mi350-2

The retained `phase11-bitselect-roundtrip-r3` observation has SHA-256
`1d625bcd1890c9fa7c7a30b023b212b2c5a1194716868df971103e0ae1626fe1`.
Eight cases make ten actual compiler callbacks:

| Case | Required result |
| --- | --- |
| Ordinary baseline → new candidate → fresh frontend | Exact MIR32/KIR17 program/parameter binding and128 oracle vectors |
| Authenticated alternate target | Candidate target guard refuses |
| Required/max128x1x1 | Admitted ordinary launch reaches candidate64x1x1 guard and refuses |
| Changed original after checked join | Publication source recheck refuses |
| Same-byte original inode replacement | Retained identity recheck refuses |
| Existing candidate | No-replace publication refuses; destination preserved |
| Original as destination | No-replace publication refuses; original preserved |
| Candidate changed after retention, before new parse | Fresh parsed-source comparison refuses |

The original source-only ladder separately passes one real typed baseline and
three refusals (ambiguous initializer, local alias and BOM normalization).
The frozen candidate batch passes ten selected unit tests, the full backend
library/binary suite (**1,226 passed,40 ignored**), source-observation
library/binary/integration suite (**130 passed**), formatting and workspace
policy. Ignored entries are not passes; the two actual ladders are explicitly
selected in separate runs.

All relevant Rust/Cargo/crate-README inputs remain unchanged within those
candidate gates:3,697 files,72,973,847 bytes, census SHA-256
`626ee8786a297c73e74945df6dc82550a3e3c4af9b4cc8a2599ebd96e19c3e5a`.
This later documentation file is outside that census. Results are from the
isolated working tree, not clean-release or closure attestation.

Earlier failures remain separate: a child working directory hid the required
provider source; a required64/max128 fixture was rejected by the existing
frontend before the intended candidate guard. Repository cwd and an admitted
required128/max128 fixture corrected setup without weakening either guard.
Neither earlier failure is counted as a successful negative.

## Resource and ownership limits

Own source scans charge bounded work/storage before allocation; source input
is capped64KiB and candidate output128KiB. The retained-file envelope charges
worst-case reads/publication separately. Compiler queries/replay and diagnostic
JSON are explicitly outside that small local scan ledger, not silently claimed
as whole-compiler accounting. The task runner additionally enforces process
timeouts, stream caps and combined-cache/disk/RAM guards.

The harness limits its fixed source-copy inventory to40 files/3MiB; this run
retains28 files/15,561 bytes. Fresh compiler/dependency outputs are separately
charged. No successful result admits an edited canonical graph, resumes
compilation from a report, widens source admission or bypasses protected gates.

Remaining U2 work includes a reviewed public ownership/materialization
interface, user edits and resource refusals, complete fresh analyses/admission,
whole-kernel comparison and applicable final machine/resource checks. General
control flow/helpers, physical lifetimes, other target profiles and source
recipe continuation remain separate milestones.
