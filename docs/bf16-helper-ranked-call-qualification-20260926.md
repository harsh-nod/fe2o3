# Borrowed BF16 helper-call projection checkpoint — 2026-09-26

This is the N2a observational slice for #280/#282. It does not complete
nominal helper ranked/formal/LLVM continuation or a broad milestone.
Accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Implemented boundary

The crate-private `with_bf16_nominal_call_projection_v1` enters the
[source-owned N1 query](bf16-nominal-helper-query-qualification-20260926.md)
and independently derives the canonical call-effect report from the same
inventory and original resource ledger. It joins the actual source MFMA
intrinsic, ordered BF16/F32 operands, exact canonical tensor contract and
Identity/Swap01 return layout. Only a closed single-Matrix helper with the
expected call and effect ordering is eligible.

The candidate is borrowed, non-Clone and available only inside a higher-ranked
callback whose result is Copy + 'static. The original ledger identity, source
floor, report reservation and separate construction/callback unwind accounting
remain checked. A complete empty physical-memory effect report is explicitly
not proof of scalar semantics, convergence, caller capabilities, tensor-layout
transport or complete call-access admission.

The candidate states the still-required caller-capability, tensor-layout,
full-wave and exact-result obligations. Existing ordinary helper and generic
materialized-source refusals remain in place. No public kernel launch,
artifact publication or editable-tile promotion is added by this checkpoint.

Six new synthetic projection controls and a genuine Identity/Swap01 source
hook exercise this seam. Exact/one-short qualification of the complete future
N2b routing path remains separate; these isolated controls do not establish it.

## Merged qualification

The final gate ran on mi350 over HEAD
`fc4629ca5ae8f1b3df889798622be4f53c46d257` plus the five recorded changed/new
source leaves. The measured source census was 8,325 files / 119,104,132 bytes,
SHA-256 `ffa5790ddc3230b46e30b7ec3ab74d89c074f87e3bf2d127dd6c516576c9af96`.
It preserved the concurrent conditional-report V2 change in the parent module.

Gate `compiler-bf16-n2a-merged-r1` passed 10,393 test executions across 210
result groups, zero failures and 331 ignored executions. Counts include repeated
focused/broad configurations; they are not unique-test counts. Coverage includes:

- All-target checks of 13 crates: kernel analysis, descriptors, verifier,
  kernel IR, pliron, lowerer, backend, simulator, simulator CLI/trace, debug CLI,
  compiler FFI and HSACO finalizer.
- 179 lowerer doctests, including 175 compile-fail controls.
- Fresh backend/extractor builds, five genuine helper source sessions and four
  root-only source sessions.
- Unsafe-source inventory and whitespace checks.

Receipt: 45,283 bytes,
SHA-256 `95471c6b1042e3cebc62b567e74979bc95de4ece791d4f9cafa3eefc61251aa2`.
Elapsed command time: 920,787 ms. Source, explicitly pinned inputs and tools
were unchanged before/after; exit status was zero and output drains completed.

An earlier peer-merged gate exposed a test-only mistake: an untouched work meter
with a usize::MAX limit must accept an exact usize::MAX charge. The overflow
control now charges a one-unit prefix before attempting usize::MAX. Production
accounting was not weakened. The failed receipt is retained and the corrected
control passed both focused and broad checks.

## Subsequent main integration

Concurrent main changes were preserved in merge
`a72036d3e931151a4e76cbc5ba393d6fde7f10f3`.
A separate post-merge library regression of the AMDGCN model, lowerer and backend
passed 4,421 test executions, zero failures and 192 ignored executions, followed
by a clean whitespace check. It did not rerun the fresh source-session ladders.
The 8,336-file / 119,220,939-byte source census was
`c8b1723dec1a2a981c56eba803aa6f931969673f51b67c6eb1b928931b7f6be0`.
Receipt: 24,315 bytes,
`f47b3bd198f86c7614eeec873dc42444168be83edff4da84579d8a5a5f2a8626`;
elapsed 140,013 ms, with unchanged source/input/tool pins and complete drains.

## Fresh source observations

The R5 helper parent report (281,023 bytes,
`2f73edffb11701aef9310abedc65f766779e2263b1188bd6cf8d123f0f3ab2db`)
contains actual Rust source sessions for Identity, Swap01, wrong launch, callback
error and callback panic. Identity and Swap01 each retain 18 numerical cases
and 16 request refusals. Raw/accepted/invocation records were joined back to the
successful parent, with exactly one rustc callback per session and six unchanged
source pins.

A lossless numeric comparison to R3 found only the expected +2,700 work units:
41 work fields in each positive session, seven in each callback-control session,
and no changes in wrong-launch observations. Numerical bit patterns, masks,
refusals, storage accounting and authority flags were unchanged. The audit is
6,837 bytes, SHA-256
`6b3b51f0f7bfff6ac4a22b290ec000ef692d9b96fbda9422a5b47f79ba624af2`.

The fresh root-only CPU parent also passed four source sessions, 18 positive
cases, 16 request refusals, wrong-launch refusal and error/unwind controls.
Its report is 100,654 bytes,
`53f57f36144e1471f656b473304443fd6369ce7ac1412c294fb11600afa987a6`.

All copied reports remain historical observations. Normal helper continuation,
native execution and artifact/launch authority remain false.

## Remaining implementation

N2b must route the actual borrowed candidate through canonical facts and both
guarded/projected forwarders, with genuine-source and whole-route resource
controls. N2c must join caller A/B/accumulator capabilities, full-wave context,
helper-qualified layout occurrences and exact returned components across all
dataflow passes. N3 must then connect the retained ranked owner, source replay,
formal and target checks to ordinary LLVM continuation without bypassing
existing refusal or resource boundaries.

Debugger work proceeds independently; see the
[final startup checkpoint](final-debugger-startup-qualification-20260926.md).
Neither its startup nor this CPU checkpoint establishes physical capture.
