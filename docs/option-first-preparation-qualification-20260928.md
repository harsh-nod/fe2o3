# Source-ordered Option preparation checkpoint — 2026-09-28

The private compiler continuation now retains Option producers and dominance
results on the authenticated source and original resource budget. This internal
checkpoint stops at `BeforeEnumV1`; it does not enable ordinary production
compilation or expose a new kernel-authoring API.

## Implemented order and ownership

Within the intrinsic-analysis suffix, preparation follows the original order:
retired intrinsic prefix, index/leader/predicate/edge preparation, Option
producers, then Option dominance. The one-shot pending owner retains partial
model allocations outside the checked callback and through postflight.
Allocations and owning errors are destroyed before the original budget is
refunded. Sticky denials and invalid custody are not repaired into success.

The callback receives an immutable lexical view. Its output is constrained to
`Copy + 'static`, preventing the view from escaping as an owned proof.
The existing rich/dense checkpoints and ordinary compiler route are unchanged.
This is not the complete outer root chronology: induction, launch, constants
and entry-prefix work belong to earlier stages. It does not claim physical
stack/RSS accounting or universal panic-payload retention by outer wrappers.

## Qualification

All 12 new component controls passed. Full CPU regression passed 356 model and
3,196 backend tests (197 ignored), plus backend/extractor builds. Five fresh
actual Rust-source sessions passed, including 36 positive numerical helper runs
and one completed run in each callback-error/panic control.

Each of the four eligible source sessions executed Compare, CallbackError and
CallbackPanic observations. All three compared complete Option producer and
dominance data against the unchanged original APIs under the same original
budget. Every observation contained one producer and one authenticated
Some-region/block pair. Dominance work was 373 for identity and 375 for the
swapped-return source. The genuine invalid-caller control refused before
entering the callback; wrong-launch never reached this checkpoint.

Complete qualification receipt:
`3acb9662acf50ca4a960c2e5492503c2c507b276296358956ddd1a911be3ff8f`.

Complete source-ladder report:
`07419034ac2c045d5dce3209ee8bbf2c814dbf6bae049301192a669a8880774f`.

Root joined every accepted case to its observed data, actual invocation and
complete stderr. Corrected readback (counts non-null completed runs rather than
the fixed-size report array):
`6c40f430bade00b84659d62a7252992f68106a8e26a1e357e5dd0abc4c2b0bf3`.

## Remaining work

Retained enum/scalar preparation, capability and allocation ownership, earlier
root constants, the joined argument-writer boundary, genuine nonempty Fixed
proofs, joint bounds verification and production routing remain open.
No GPU execution, native debugger capture, public capture activation or global
compiler-pin change is established here. Broad accepted exits remain
M1/V1/V2/U1/U2/U3 (6/18).
