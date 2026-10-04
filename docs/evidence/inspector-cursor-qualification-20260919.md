# Inspector packaging and fresh cursor qualification — 2026-09-19

These are qualified implementation slices, not closure of M2–M6, V2–V5 or U2–U4.
All runs used the primary checkout on mi350-2. Compiler publication remains
separately blocked by the inherited unsigned main commit; tests do not waive DCO.

## Public inspector packaging

The normal `fe2o3-program-inspect` binary, the retained
`inspect_diagnostic_ordered_program_v17` example and the shared library entry
point use one implementation. Cargo default-run preserves `fe2o3-kir-sim`.
The prior diagnostic report kind, admission, preflight and authority flags are
unchanged. `--help` and installed-command usage text are additive.
No compiler, simulator execution, native artifact or GPU launch is performed.

Current gates: source-inspector r1 (106 tests), source-inspector-build r1,
source-inspector-install r1, source-backend-full r6 (1,326 passed, 56 ignored),
debug-regression r4 (338 passed, four ignored), source-format r8 and
source-policy r5 passed. The installation was an offline, locked debug build
into a fresh task-local cache root; it did not alter user PATH or a global install.
All these gates measured an unchanged Rust/Cargo/crate-README census:
`0caab044a8a8eeb7c756623f99b973585ff8380d0c5dbe127272d119cd9c5052`
(3,792 files, 73,835,558 bytes), at parent HEAD f798cf93 with the reviewed changes.

The historical-input comparison r1 passed all 18 actual normal/example/installed
processes across six original source exports: one, three and sixteen declared
instructions, each with used and unused results. It joined original receipt/
export/raw-byte/request identities, independently checked actual block/SSA/
ordinal coordinates, descriptors and declared roles, and required byte-for-byte
equality with the retained historical inspector output. Those source callbacks
are historical, not newly executed or authenticated here.
Selected inputs/tools (142,320,278 bytes) were measured before and rechecked after.
This is not complete toolchain/runtime-closure attestation.

Comparison receipt under the retained task cache:
`secondary/phase11-program-inspector-compare-r1/receipt.json`, 54,561 bytes,
SHA-256 `7fbb4f74435e57b73163bde66a991806c98e219e01ad4f504845eac0f11baa45`.
All records stay under charged cache; original historical evidence is unchanged.

## Refused breakpoint ownership and fresh runtime navigation

The move-only rejected-breakpoint path returns the original predicate on
refusal. Validation, budget/id/hit checks and reservation occur before ownership
transfer; bounded diagnostics do not recursively walk the predicate.
This is private observation-session behavior, not a new debugger wire contract.

Fresh runtime-source r11 and origin-cursor r2 passed on one unchanged full
repository census: 5,964 files, 91,521,546 bytes, SHA-256
`9c253511149c1354a0000614dce232ba43c414aa162269240aa1a96d7b825be6`.
The newly produced bundle identity was
`5a576e1cd3d43830cc60666eab9bc631b15540d36b78627725252139d37d6721`.
Six contextual and six opt-out runs retained 1,208 records, 32 helper activations,
16 breakpoint hits and six watchpoint hits. Reverse/repeated cursor selection
was checked against the same retained capture, not synthesized frame values.

Runtime-source r11 receipt: 212,130 bytes, SHA-256
`9c0eb9e86c106d7fb6ecc9099728156d9e2fdd30447bc62b5e801ee37e7622ff`.
Origin-cursor r2 receipt: 66,597 bytes, SHA-256
`9be966202bcde2f0fd182c19f7c7b2916b8ed154a1ee0b93b22d579f7deeeb58`.
The cursor resource roster explicitly charged both whole-body experiment roots.
Combined charged storage remained about 15.02 GB, below the unchanged 20 GiB cap.
There is no public frame-navigation/step-out, allocation-generation/reuse,
hardware timing, physical-register-state or production-resume completion claim.
