# Actual KIR Prefix Assembly Qualification

Development checkpoint against base `794bed143948841db0b6fd1774745a96f846dd64`.
The archived candidate patch identifies the exact changed source. This is a
bounded semantic-admission prerequisite, not a multi-GPU launch or parity claim.

## Implementation

The actual borrowed KIR operation slice now uses shared executable constant,
terminal and assembly bodies. Sparse u32 value IDs map directly to argument or
constant origins. This removes the constant-step vector, dense constant-state
suffix and second fold. Work remains O(n log n), with at most 384 map entries;
no allocation is sized by the largest raw ID.

The profile checks positional argument rows, unique definitions, sole U32
constant results, exact Checked(Add), fresh distinct value/overflow outputs in
`[U32, Bool]` order, capture-derived IDs/literal and RHS identity with the
immediately preceding constant. Fewer than two or more than 257 operations and
more than 128 arguments reject before indexing. Missing LHS retains the existing
`ValueMismatch` classification; invalid KIR shapes return `Kernel`.

## Evidence

- Two complete Verus positives: **86 obligations, zero errors**.
- **68 logical negative mutants**, including 20 new KIR mutations; **14 runner
  controls**; **73 owned stages** with exact source continuity and process-group
  absence. Reject-all is a negative: the contract proves acceptance completeness.
- **141 verifier tests passed**, four existing subprocess/runtime-closure helpers
  ignored, plus **30 doctests**. Five new KIR tests cover sparse/MAX IDs, exact
  capacities, duplicate/shadowed definitions and malformed terminal profiles.
- Six genuine pinned-Rust compiler cases: two accepted distinct source profiles
  and four expected rejections. Conditional boundary values are independently
  replayed; compiler scratch is absent after completion.
- Strict production-library Clippy, no-default compilation, formatting, local
  CI dispatch, shell syntax and diff checks pass. Cargo JSON records the exact
  verifier, wrapper and extraction-test executable identities.

`qualification.tar.xz` contains command/output receipts, exact original and staged
proof inputs, mutants, authenticated Verus closure checks, independent replay,
source patch and per-file manifest. `SHA256SUMS` covers this README and archive.
An exploratory checked-operator mutation initially produced two diagnostic
locations and was correctly rejected by the strict classifier. The final mutant
widens Add to Add-or-Subtract, producing the required single logical failure.
Exploratory records are retained but are not qualification stages.
The first CPU cohort also caught macro formatting; the complete accepted
`*-03` CPU/solver cohort reruns against the formatted final source bytes.

## Limits

The proof relates exact KIR acceptance to independent typed constant and
checked-add evaluation, then composes it with the source fold. Every consulted
KIR discriminant/field and container is source-checked, with explicit erasure of
rejected payloads. Capture getters and original-slice forwarding are checked.
This bridge is not a Rust parser, name-resolution, layout or compiler theorem.
The pinned Verus standard-library BTreeMap specifications remain trusted.

Terminal source-AST validation, ABI discovery, LLVM/HSACO correspondence,
physical entry, stores, continuation, completion and protected production Worker
providers remain open. Generated invocation additionally needs exact multi-device
routing and an authenticated peer handoff for its distinct generated storage.
No launch authority, full-adapter proof, native execution or performance gain
is established here. MI300X was not used; no shared-host resources were created.
