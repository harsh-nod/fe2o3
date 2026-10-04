# Retained nominal capability driver — 2026-09-28

The private driver is implemented and compiles with the existing backend.
Its helper/source controls pass; a genuine full-driver execution is not yet
qualified, and no ordinary compilation route is enabled by this checkpoint.

Within the closed acyclic, single-nominal-call, no-pipeline profile, the driver
performs actual owner discovery, initial FIFO propagation, source-order
payload scanning, repeated FIFO propagation, final source-index replay and
read-effect binding. Both FIFO owners, scratch, effect arrays and partial
bound rows remain attached. Original semantic work is separate from added
resource/prepayment work; duplicate FIFO visits are not collapsed.

The nominal block calls the unchanged real consumer/query/authentication
path. It does not use the generic Defined-call branch or cached DATA as
authority. Operand consumption and destination removal occur only after
the checked query and its postflight succeed. Initial and repeated query
counts may exceed one; final replay requires one authenticated nominal call.
This describes implemented source behavior, not an observed genuine run.

The driver adds 32 wrapper-work units and 60 typed accounting rows, including
a separately identified inherited shared-transfer envelope. Two review
corrections explicitly account all constructor counters and mutable CFG-vector
aliases. No algorithm, original API, early refund or legacy mode was changed.
Logical accounting does not measure native stack, allocator capacity or RSS.

All 11 synthetic helper/source controls passed, with 356 model tests,
3,337 backend tests (197 ignored), 227 serial authority/capability tests,
nine approval-policy integration tests and backend/extractor builds.
The controls independently compare original owner, payload, initial-state
and read-binding DATA; test source association, failure/partial ownership,
separate work counters and exact/short storage; and check driver stage order.
Their synthetic consumer cannot establish successful full-driver execution.

Full regression receipt:
`3b064ab0103eb33ca04e13e230c9c39d1885a002e7c075e170ee4b9b4dfef715`.
Independent corrected-source review:
`443f160d3c0e2628bee869afcbe030845ccd1cb367d8cf2b031c7cf7a40e1cf0`.

Next, an actual Rust-source factory must retain constants in their original
early position, actual entry preparation, preceding Option/enum/scalar/
provenance/allocation owners and this driver through checked postflight on
one physical budget. Complete original DATA/work comparison and separate
actual query/authentication observations remain required. Earlier whole-root
analyses, argument writers, joint bounds, genuine nonempty Fixed coverage and
production admission remain open. No GPU or debugger execution is established.
Broad accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).
