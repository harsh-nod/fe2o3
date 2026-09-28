# Source-owned nominal helper query — 2026-09-26

The compiler now has a bounded, nonescaping query for one checked BF16 helper
call. It joins the actual retained Rust source Call, the same canonical
executable owner, the caller/helper correspondence, the canonical Call and
Matrix, twelve logical inputs and four returned F32 components. Identity and
Swap01 remain distinct return transports.

The lowerer entrypoint is
`ProductionPreRankedKirOwnerV1::with_checked_bf16_nominal_call_v1`.
This is compiler infrastructure for a later nominal ranked projection, not an
author-facing switch to bypass the frontend or resume an uploaded IR recording.
Generic helper and normal-compilation refusals remain unchanged.

## Association and accounting

The query requires the inventory to belong to the exact executable owner and
the source call to be the actual retained call object. A structurally equal
foreign inventory, cloned call, wrong caller/root/block or detached altered
callee/continuation cannot obtain the borrowed view.

It checks the closed gfx942 BF16/F32 m16n16k16 Wave64 operation contract, source
and canonical coordinates, ordered operand/result transport and the supported
return permutation. The sealed retained source owner supplies the already
established full-wave and source-coverage premises. This query does not invent
a generic Rust ABI or physical-register calling convention.

The same supplied cumulative work ledger pays traversal and query scratch.
An exact inventory-storage accessor uses the same checked payload formula as
inventory allocation, including all thirteen retained vectors/indexes. Query
entry requires simultaneous accounting for the source owner, preexisting
occurrence capture and actual inventory.

Success, callback error and caught panic release only query-owned scratch,
after dropping temporary state. Callback reservations and cumulative work/peak
remain. A replaced ledger, undercut protected floor or ignored sticky denial
cannot be silently repaired or reported as success. These are logical resource
contracts, not a process RSS limit or protection from arbitrary unsafe code.

## Validation boundaries

Focused qualification passed 205 test executions with zero failures, including
14 inventory tests, eight query tests, 179 lowerer doc tests and four existing
BF16 source-observation controls (counts overlap configurations). Two live
entrypoints remained intentionally ignored in that focused invocation.
The subsequent broad gate passed 8,783 test executions across 127 result groups,
with zero failures and 298 intentionally ignored entries (counts include repeated
configurations). It covered seven crates' all-target tests, lowerer doc tests,
the real frontend helper ladder, historical helper transport, root-only
core/normal/CPU ladders, and the unsafe-source inventory policy.

Its receipt is 53,274 bytes /
`5cdf513f9ef787b3ba02447f97888d504e812ffa652f3693bdef5464f29b2c6c`;
the source census is the same as the focused gate below. All request/input/
stream pins and before/after source/tool observations were checked.

The fresh five-session helper report is 281,023 bytes /
`3abcbbcc14b7ba0f50545b39569487348d3a4a6eed46e4f0d50ea120662d4cb0`.
Identity and Swap01 each passed 18 numerical cases and 16 request refusals;
wrong launch refused, and error/panic sessions each completed one numerical run.
Lossless comparison with the prior source report found all numerical bit
patterns, masks, refusals, storage and phase flags unchanged. Only cumulative
work increased: 50,483,119 units for Identity/error/panic and 50,483,159 for
Swap01. The source/reverification peak remained 1,632,943,151 bytes under the
unchanged 2 GiB logical limit. Parent, raw child, accepted child and invocation
records were joined separately; raw child records remain non-accepting.

Root comparison audit: 7,167 bytes /
`7d34ed59458b9b461138dd5c954c3b56314295ce86503ad8bcb48aa40d93baac`.

The genuine-source hook invokes the real public query on Identity and Swap01.
It covers equal-but-foreign owner/source objects, unchanged generic refusal,
callback success/error/panic and preserved reservations. Six further accounting
controls test missing inventory reservation, one-byte-short combined retained
floor, exact floor, exact measured whole-query work/storage, one-short work and
one-short storage. The short cases must refuse before callback entry.

Those six controls use isolated negative-test ledgers, never a replacement
production ledger. The real source phase remains live and prepays all bounded
probe work and simultaneous scratch. Exact measured costs apply to this
fixture and unit-result callback, not arbitrary callbacks or all future code.

The focused gate used compiler HEAD
`73a6c84ded9e863e08573dd834e2098f1f86084c` plus the reviewed seven-file
query change. Its exact source census was 8,280 files / 118,604,944 bytes /
`d10a72fcbc6943aa49227058634c12097e830182a554d61fe17a3f6057526b45`.
Receipt: 27,084 bytes /
`51b0a4ee80a731e48c1203376f2342c9da80e4a9f162370a13a56e06c90351f1`.
Source review and a separate whole-public-query boundary review preceded the
genuine qualification. No prior CPU receipt is relabeled as query evidence.

## Pipeline boundary

The query sits after source analysis and retained canonical emission, before
successful nominal ranked projection. A matrix helper's empty physical-memory
effect summary is not scalar semantics and cannot discharge tensor-layout,
full-wave, result-transport or ABI obligations.

Normal ranked/formal/target continuation, helper LLVM emission, edited-tile
promotion and hardware execution remain separate work. This adds no generic
call admission, publication token or runtime permission. Accepted broad exits
remain M1/V1/V2/U1/U2/U3 (6/18).
