# Persisted intent, generated refusals and runtime cursor qualification

Actual root-run observations on mi350-2, 2026-09-19. These extend the private
qualification paths for #280/#281/#282. They are not public recipe admission,
production continuation, or U2/U3/V2 closure.

## Exact qualified inputs

Base HEAD was `fc88ee8b1b37cc721007e8d8b9b7adb76576fb78` plus the additive
recipe, generated-negative and observed-session leaves in this commit.
Rust/Cargo/crate-README census: 3,784 files, 73,773,378 bytes, SHA256
`0d3614d8c89a3ff45ffd3783b992dc29e34a9e06b5862e383458f8b4c65b06cf`.
Full census for the fresh runtime export/cursor: 5,950 files, 91,424,413 bytes,
SHA256 `bdaf359489e063bb6f22bb6d420734d8ce6db82f66d0ab6c80b84b91f8ebe414`.
These identify the actual pre-documentation worktree; later documentation and
commit changes are not retroactively assigned to those receipts.

Pinned nightly-2026-04-03; offline locked Cargo; jobs=2, incremental=0 and one
test thread. Existing combined cache/output cap 20 GiB, disk floor 40 GiB and RAM
floor 64 GiB remained unchanged. Failed attempts and successful runs are retained
under separate task labels. No GPU dispatch occurred.

## Persisted local-order intent

Nine private unit/filesystem controls pass. The separate real-source recipe
ladder passes fifteen actual frontend callbacks: two generations, seven replays
and six exact refusals. Seven replays run 210 full-kernel simulator scenarios
with independent reference outputs, input immutability, initialization and
canaries. It exercises original/reversed ready order, repeatability, parameter
rename plus UTF-8 span movement, and explicit regeneration for a new kernel item.

The inert recipe stores intent, an exact target/profile and all five measured
Instance identity axes. Each use derives current operation coordinates from
the live captured owner and uses the existing checked scheduling and independent
transition replay. Historical origin bytes never reconstruct an owner or bypass
fixed production policy. Unsupported source shape, changed item/instance,
wrong target, stale source and changed retained recipe are refused at their
named boundaries.

Descriptor and final named-path snapshots are checked after payload decoding.
Filesystem tests exercise real same-byte inode replacement and second-file
create-new collision. Two additional tests deliberately inject one-byte valid
JSON whitespace corruption and require real subsequent readback rejection;
these are not organically observed disk failures.

Publication is sequential and nontransactional. Failure can retain preceding
files and a partial/changed current file; no success report or compiler owner
is emitted. No fsync, pair atomicity, crash durability or atomic CAS is claimed.
Later replay still requires a fresh live compiler owner.

Actual recipe report r1: 132,533 bytes, SHA256
`1bd1560f7bdabb89bac098cf9e3ec1dc8d724ae70963679b198c4d0f5e15fb83`.
Public placement under the existing #134/#271/#275 scheduling model remains
unresolved; the private JSON is not a new allocated public schema.

## Generated-candidate refusals

A new ordinary-source baseline generated the unmodified assembly candidate.
That candidate passed thirty whole-kernel simulator runs. Four fresh edited
candidates reached actual frontend callbacks and their precise existing
refusals, for six callbacks total:

| Edit | Exact obligation exercised |
| --- | --- |
| scratch v4 to v64 | Closed physical role profile permits only v0 through v63 |
| scratch v4 to input v0 | Scratch/output/input roles must remain distinct |
| input0 binding a to b | Fresh source/live-in role join differs |
| final XOR input1 to input0 | Fresh checked bitselect program binding differs |

The physical checks are bounded role/profile and input-alias refusals, not
general occupancy or native clobber-lifetime proof. The last two enforce this
specific replacement contract, not a ban on valid different assembly programs.

Negative-only source-machine r5 report: 38,988 bytes, SHA256
`95a7f8492af9517cec70f27f5d08bbeec6b2191fa3e88df515a7a5809c961940`.
It is deliberately not the seven-callback source/native adapter input.
Old generated-map/capture rejection remains pending its genuine producer/join;
this report does not manufacture that result from stale file checks.

The first unit build failed on a wrongly nested prepare helper path in the new
negative harness. Root corrected the module path, preserved the failed r1 log,
and reran recipe units as r2. No diagnostic expectations or compiler checks
were relaxed.

## Actual-source occurrence-aware navigation

Thirty-five retention/session/topology tests pass. ObservedSession owns the
actual DebugSession transcript and its same-capture origin sidecar; siblings
cannot call a raw arbitrary transcript/sidecar joining helper. It retains full
invocation, helper activation and attempted operation identity.

Forward/reverse into and over use exact dynamic occurrence pairs. Existing
break/watch matchers and reversible prefix counts are reused. Reaching a terminal
fault stops once, then reaches End; reversing and revisiting can expose the
fault again. Bounds and cumulative work are charged before navigation.

A newly built primary observer and exporter produced runtime-source r10.
The reviewed cursor runner then consumed that same retained Bundle V6 and
same source census, without another export or relabeling older r9 evidence.
Actual results:

- Six contextual and six opt-out comparisons, with identical legacy execution.
- Thirty-two helper activations and 1,208 retained records.
- Sixteen focus breakpoint hits and six write watchpoint hits.
- Independent full output/initialization/canary checks for the Rust loop/helper.
- Bundle model identity
  `79beaa12b0089fa39a9981b515f2ed0c1930601e72aa752b1dfb04ecc9607b8b`,
  distinct from the raw bundle SHA and canonical KIR digest.

Runtime-source r10 capture: 212,190 bytes, SHA256
`00119ecadf9796b9926e72873d89584006159d261ce62fe0118210d12c7b26b5`.
Cursor r1 receipt: 64,341 bytes, SHA256
`dbd600c677c170f906e91b03237ba6c4c1881e8212c930c0e56081d225ce81e3`.
Seventeen runner controls pass. Input/census payloads are precharged before
allocation; exact final receipt/pointer payloads are reserved before publication.
Resource records include production viewer measurements. These are advisory
accounting checks, not OS allocation or exclusive disk-space reservations.

The inherited process helper requests group termination at its deadline but
does not guarantee close/drain/reap completion; no stronger timeout guarantee
is asserted by this qualification.

## Remaining boundaries

The private owned-predicate refusal cleanup caveat is not yet repaired in this
qualified session revision. Do not expose it as a hostile-input/public API.
This work does not implement helper step-out/caller-frame reconstruction,
public CLI/DAP occurrence queries, allocation release/reuse generations,
physical registers, hardware values, or a live visualization adapter.
Those and #216 owner acceptance remain required for V2 and later milestones.

Generated source/native observations, editable source candidates and persisted
intent remain separate from protected compiler/finalizer authority. Public
source promotion, complete-body ABI support, broader memory/synchronization,
gfx950/matrix coverage and the full multi-level/tiled curriculum remain open.
Original milestones remain M1/V1/U1 qualified, fifteen open.

Compiler main publication is separately blocked by inherited unsigned commit
`7dfbe5cc453f12c1b8b32eeb49f08778876bf6ce`; unchanged DCO checks for both
repositories fail exactly there. The maintainer disposition is tracked in
[#271](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5738874740).
No exception, forged sign-off, history rewrite or range narrowing was applied.
