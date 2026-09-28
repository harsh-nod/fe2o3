# Source-ordered Option and enum checkpoint — 2026-09-28

The private compiler continuation now reaches `BeforeScalarV1` after the original
Option preparation and actual retained enum analysis. Both owners remain outside
the checked-call postflight. This is an internal continuation, not ordinary
production admission or a public kernel-authoring API.

## Implemented

A separate authenticated entry constructs the combined pending owner under the
original source owner, inventory and resource budget. It does not nest an enum
owner inside the older BeforeEnum entry's earlier refund boundary.
Preparation preserves Option-first order, then invokes the retained enum
analyzer and requires completed data before exposing an immutable lexical view.

Partial and completed allocations, together with saved backend errors, survive
postflight. The wrapper checks original custody and sticky denials, destroys the
pending owners, and only then refunds their accepted credits. Invalid custody is
not repaired into success. The existing BeforeEnum entry and ordinary routes
remain unchanged.

Nineteen explicit typed rows are added to the unchanged parent policy; the
existing retained enum policy remains separately charged. This is logical
accounting, not a measurement of stack, allocator capacity or RSS. The unchanged
checked wrapper still converts panic payloads at its existing boundary; universal
panic-payload retention is not claimed.

## Qualification

All 13 new controls passed, including exact independent original data, partial
failure custody, exact/one-short limits, wrong ledger/source, occupied retry,
Option-first failure, and an enum failure preceding a separately demonstrated
allocation failure on the same synthetic source.

Full qualification passed 356 model and 3,209 backend tests (197 ignored),
backend/extractor builds, and five actual Rust-source sessions. The two positive
sources completed 36 numerical helper runs; the outer callback-error and
callback-panic controls each completed one numerical run.

Every eligible session executed Compare, CallbackError and CallbackPanic through
the new genuine entry. All three compared complete producer, Option-dominance
and enum-dominance data against the unchanged original APIs using the same
source and budget. Actual enum invocation/completion and positive work were
required: enum work was 408 for identity and 410 for swapped returns.
The invalid-caller control refused before callback entry; wrong launch never
entered the hook. Existing Option markers were identical; historical Fixed
markers agreed after excluding process-specific addresses, whose same-block
identity was checked within each session.

Complete qualification receipt:
`48468d97aac89ebcf2030f693789c2ad25a8913155febf11028f47c1bb1570a6`.

Complete source-ladder report:
`7af9650d94252673fdc90017f1b68b55d7e86d697beee9060c4df78a8e436de5`.

Root complete case/invocation/output readback:
`069a3e2ed5efef45bb90bfa03096e425444a4ce346c255a588d9094b1e766e0d`.

## Remaining boundary

Genuine nonempty enum availability is not established by analyzer invocation
alone. Synthetic nonempty coverage and authentic execution are separate evidence.

Retained scalar/provenance/allocation/capability preparation, earlier root
constants, argument writers, genuine nonempty Fixed coverage, joint bounds and
production routing remain open. No GPU execution, native debugger capture,
public activation or global compiler-pin change is established. Accepted broad
exits remain M1/V1/V2/U1/U2/U3 (6/18).
