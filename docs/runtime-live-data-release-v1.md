# Live Detached-Data Release

Status: R122 locally qualified above published R121
`f6d3fb50e4fd1eb6949d4a29f55c79432a5989e1`, with
[402 retained artifacts](evidence/local-r122-live-data-release-2026-09-15/README.md)
and two passing independent archive reviews. This accepts the live detached-data
and runtime outer-ownership CPU/test boundary, building on the accepted
[R121 lower-cleanup checkpoint](runtime-ordinary-data-cleanup-v1.md).

## Ownership Contract

`queue_live/data_release.rs` keeps the original typed data in a private cleanup
root before admission. Every returned error or panic retains that root,
including healthy preflight rejection. Healthy rejection does not introduce
terminal-parent transport. Once admission succeeds, identity rejection and
model-opening failure are terminal even when native cleanup has not started.

The existing unbound-dispatch checks retain their precedence. Release requires
exactly one matching five-way storage identity and checks count subtraction
before effects. The borrowed lower cleanup executes inside the actual model
loan/reclaim wrapper. Retake errors precede ordinary lower errors; the original
lower panic precedes a secondary retake error or panic.

Only normal lower success, normal retake success and a complete cleanup receipt
permit identity removal, count reduction and installation of the replacement
hole. A completed native disposal followed by failed retake retains its receipt
without removing the ledger entry. The lower adapter does not mutate the
detached ledger. Under that invariant, forward `Vec::remove` and the subsequent
scalar assignments require no allocation or callback. Search/removal is O(n).
Repeated whole-roster release can therefore remain O(n^2); only the outer
runtime traversal below is linear. This inherited ledger cost remains an
optimization target, not a performance improvement claimed by R122.

The direct API transports a terminal parent after settlement. The lane API ORs
the transport requirement before propagating the result, so swallowing an error
or making a later rejected call cannot clear it. Existing lane custody restores
the selected auxiliary owner before transferring the whole parent.

## Runtime Caller

The runtime's private resident-release custody retains the descriptor vector,
active item and untouched suffix using the original vector allocations. Lane
selection occurs before taking an installed resident roster. Queue availability,
lane admission, consuming release and error formatting remain inside the outer
unwind boundary.

Before handoff, runtime custody owns the active item. The consuming KFD boundary
then roots that item before fallible work; runtime custody continues to own its
descriptors and remaining items. Failure retains these separate owners and does
not reconstruct a partially released roster for reuse. Traversal is forward,
linear, and is not truncated by descriptor/data length mismatch. Its success
path introduces no new allocation; native cleanup has its own costs.

Terminal retention uses existing hard-forget behavior. The existing parent
transport allocates a `Box`; this candidate makes no allocation-free failure,
OOM-recovery, bounded lifetime-retention or total-memory claim.

## Development Evidence

- The initial eight outer-runtime tests passed. After two review additions, the
  complete runtime library suite passed 743 tests with all features enabled.
- Constructed live-release tests reuse the actual lower cleanup engine and
  model loan/reclaim fixtures across primary and auxiliary selections. A later
  slot contains a relocated genuine auxiliary owner, not a second constructed
  auxiliary. Initialized-after-dispatch typing is explicitly synthetic.
- The first live-release build failed on a test-only projection-fault type
  namespace. The second run passed two tests and failed eight at the model
  oracle: an allocation-only checkpoint had incorrectly been expected during
  release. The source trace confirms that neither live loans nor release perform
  that checkpoint. The corrected third run passed all 13 matrix tests.
- Four concrete direct/lane forwarding tests cover preflight precedence,
  missing-engine transport, sticky transport after a suppressed error, and
  restoration of a genuinely prepared bound dispatch. The first run passed
  three and failed one because the test expected `BatchStillRetained` for a
  barrier-probe owner that is rejected earlier as `Completion(Poisoned)`.
  Corrected runs passed all four. Final review additions explicitly compare
  poison flags on healthy rejection and inspect bound ownership after the
  auxiliary slot has been restored.
- Two independently compiled forwarding mutations failed at their predeclared
  behavioral assertions: missing direct-parent retention and clearing sticky
  lane transport. Both exited normally with code 101, not compiler errors or
  signals. The temporary production mutations have been removed; restored
  controls passed all four tests. The focused audit and independent review
  checked exact mutation identities, assertion provenance, source restoration
  and owned-process closure. This is two of the planned twelve mutations,
  not the complete campaign.
- Snapshot compression checks every mapped byte. Zero-filled mappings retain
  their exact length without copying the contents; nonzero mappings remain
  dense. Mapping address, pointer, capacity and byte offset remain observed.
- Host-data disposal is recorded separately from queue-control disposal, with
  unique, disjoint identities and exact terminal native/accounting checks.
- On the corrected source, `r122-development-release-regression-v1` passed all
  35 selected KFD tests with all features: 13 live-release matrix tests, four
  concrete forwarding tests and 18 existing/shared cleanup tests, including
  the new snapshot calibration. This covers the tightened exact partial-unmap
  oracle. The run closed normally with unchanged endpoint source maps and no
  live members in its owned process group.
- `r122-development-runtime-all-v2` passed all 743 runtime library tests with
  all features on that same source map, with normal process closure and no
  remaining live process-group members.

The pre-Clippy development source map had 5,700 identities and SHA-256
`a732399cc06c8608ec86f2064f64e3fc85c3e099e570471e0425db5117625f40`.
The focused regression, runtime suite and forwarding audit above bind that
source map and four restored forwarding controls, not the newer source below.

Strict Clippy then rejected intentional `mem::forget` of the currently no-drop
custody token and a redundant `usize` conversion in a test. The production
retention policy now has a narrow, documented lint allowance; the redundant
conversion was removed. These changes preserve behavior but change the source
identity. Strict Clippy passed on the new 5,700-identity source map:
`178fa1fb2c9d3267440dbf36c50e4d79916e66fb2765446d7e1fa8acb8fc90c4`.
Full GNU testing on this source passed 2,846 tests with five ignored, across
48 libtest executables and one CSV benchmark target. This includes 1,190 KFD
and 743 runtime library tests. The recorded run closed normally after
1,351.129 seconds, with unchanged endpoint source maps and no live members in
its owned process group. The baseline checker and GNU gate checker confirmed
the exact executable, passing and ignored rosters and all summary fields.
Full musl testing on the same source also passed 2,846 tests with five ignored,
with the same exact executable and test rosters confirmed by the musl gate
checker. It closed normally after 1,511.564 seconds, with unchanged endpoint
source maps and no live members in its owned process group.

Independent static review tightened the development mutation checker to bind
the exact test executable, authenticate scoped patches by reverse/reapply, and
recheck the complete mutant source map before reporting success. The corrected
checker passed 51 parser calibration cases; the compatibility-gate parser
passed another 30. These in-memory calibrations include historical transcript
fixtures and are not compiled runtime mutations or native execution evidence.
The two earlier forwarding mutations were repeated in the final twelve-case
compiled campaign on the corrected source.

Endpoint equality is not a continuous source-immutability proof.

## Accepted Final-Source Evidence

All seventeen source gates and ten auxiliary checks pass, including the full
GNU/musl runs and 25 further compatibility commands. Twelve compiled production
mutations produce twelve distinct source maps and reach their exact declared
behavioral assertions with normal Cargo 101 exits, not compilation failures,
signals or destructor double panics. Each accepted restoration matches all
5,700 source identities. Final restored controls pass 35 KFD and ten runtime
tests, with no ignored tests and normal process closure.

The first compiled mutation exposed an evidence-checker defect: Rust retains
`construction_primary/../construction_auxiliary` in its diagnostic source path.
Checker v2 rejected that spelling despite the intended assertion firing. The
failed checker and immediate source restoration remain unqualified history.
Checker v3 maps only that exact declared file to its compiler spelling while
preserving the original assertion line, column and diagnostic. Eight additional
parser calibrations pass, giving 89 cases across the retained checker versions:
51 + 30 + 8, with 28 accepts and 61 rejects. These are helper tests, not extra
compiled runtime mutations.

The closed collector and complete archive replay pass. All 402 raw artifacts
match their recorded bytes and hashes, including the exact 15-file source
bundle with four additions. Independent contract and evidence reviews pass on
prepared summary SHA-256
`cd7f21b796bda5f8eaa324a0e7628e4492735cf8ef87b9576340533caf2bf6a4`.
All 93 recorded owned process groups, including the collector, are absent.
Earlier development cohorts and failures remain separately identified; they do
not substitute for accepted final-source results.

## Remaining Work

Other N4-L live detach/control-release routes and N4-Q queue destruction remain
outside this data-release checkpoint. The next live route is retained
persistent-control release, keeping its cleanup root outside the model loan
through retake and restoring the exact original dispatch on pre-effect opening
rejection. Generated DATA-ADOPT and ISSUE/COMPLETE still require production
integration. Whole-roster ledger complexity also remains an optimization target.

CPU fixtures do not establish Linux/KFD execution, authenticated Verus
correspondence, aggregate memory bounds, multi-device behavior or matched
HIP/HSA performance. A1/A2 and issue #182 remain incomplete.
