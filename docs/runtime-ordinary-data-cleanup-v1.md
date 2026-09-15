# Ordinary Dispatch and Typed Data Cleanup

## Status

R121 is locally qualified above published R119
`be50052c74a7956ba0a50df0e2efe8d9568de78a`, with
[665 retained raw artifacts](evidence/local-r121-ordinary-data-cleanup-2026-09-15/README.md)
and two passing independent archive reviews. Review strengthened the cold Ordinary control-only
rejection assertion and independently checked fixed-input identity,
initialization and content expectations. The corrected source now passes the
fresh integrated GNU suite and completed musl retry: each has 2,818 passed and
five ignored across 48 libtest harnesses and one separately checked harnessless
CSV benchmark. This includes
1,172 KFD tests and 733 runtime tests, with exactly 23 KFD additions to R119
(21 behavioral tests and two supplemental source-routing guards).

All 26 declared production mutation executions on this corrected source reach
their intended behavioral failures. They cover 25 distinct production source
variants; two executions test the same generation-policy change against
different behaviors. Each has a closed normal Cargo 101 result, a passing
read-only checker and complete restoration of all 5,696 source identities.
No old-cohort negative is reused as a corrected-source result.

Fresh restored dispatch-control and pristine cleanup suites also pass 59 and
17 tests, respectively, on the corrected source. The first integrated musl
attempt reached its 30-minute deadline before completing the final model
harness. Its completed KFD and runtime library results do not qualify the full
suite. The separately declared musl retry completed successfully in 1,195.58
seconds under the unchanged 30-minute deadline. All seventeen source gates,
ten auxiliary checks and thirty parser calibration cases pass. The closed
collector replays exactly and matches the independently reviewed archive.
These local tests do not establish live
Linux/KFD execution, formal refinement, aggregate memory bounds, multi-device
behavior or HIP/HSA performance parity. The
initialized-after-dispatch fixture establishes type handling only; it is not a
hardware completion observation.

## Qualification Records

The corrected source map is
`5fe28e8a3accfc845c4c20182b702c1e96961e542c02405b3841a91b4be62f04`.
The fresh full result is `r121-development-gnu-all-v2`. The reviewed v2
gate checker separately verifies its exact target roster, all summary fields,
corrected prerequisites, input-byte stability and final source/HEAD identity.
Its closed `r121-development-gate-check-gnu-v2` execution also passed an
independent evidence audit. It accepts only this GNU gate, not the full packet.

`r121-development-musl-all-v1` is retained as rejected timeout history. The
runner recorded exit 124, a closed SIGKILL child, an unchanged corrected source
map and no remaining live members of its owned process group. The transcript
contains completed 1,172-test KFD and 733-test runtime libraries and no failed
test rows, but the final model harness has no completed summary. A separately
declared `r121-development-musl-retry1` completed with the identical source,
command, environment and deadline: normal exit zero, unchanged source and no
remaining live members of its owned process group. The closed
`r121-development-gate-check-musl-v3` execution checks both complete GNU/musl
rosters. The v2 gate plan and v3 gate checker name that retry explicitly and
retain the timeout as a chronological predecessor, not a passing prerequisite.
No original record is overwritten.

The earlier `r121-development-gate-parser-calibration-v3` execution passes
20 synthetic transcript cases. The standalone-lockfile command subsequently
passed all 32 files, but its strict v3 checker rejected an informational Cargo
package-cache wait line. A separately recorded checker reproduction preserves
that rejection. No source gate was repeated or its transcript rewritten.

The v4 checker ignores only that exact full informational line for the
standalone gate and still requires the complete ordered roster and completion
count. All thirty cases in `r121-development-gate-parser-calibration-v4` pass,
including missing/duplicate lockfiles, wrong counts, warnings and errors.
These are parser calibrations, not runtime mutations or timeout-history-branch
calibrations. The remaining ten gates ran after revalidating the first fifteen.
The closed `r121-development-gate-check-all-v4` accepts all 27 declared gates;
`r121-development-qualification-collect-v6` binds their exact results, all
corrected negatives, calibrations and restored suites. Both independent reviews
verify the complete archive and distinguish superseded helper drafts and
historical attempts from current qualification.

Corrected negatives use `r121-development-corrected-negative-<case>-v1`, with
the pinned v2 mutation plan and v3 negative checker. Cases 09-26 ran first,
followed by fresh executions of 01-08. The original interrupted
`r121-development-gnu-all-v1` remains excluded from passing evidence; its raw
record is retained as the chronological predecessor of corrected formatting.

### Historical Development Cohort

The locally retained `r121-development-kfd-all-v1` record reports a normal
1,172-test pass, an unchanged source map and a closed child with no remaining
live process-group members. The map contains 5,696 non-document source identities
and has SHA-256
`19b8774019ea6c5f70aa90cfdaa5458282c389f31201a584d098156b9e2e7332`.
On that original cohort, the runtime library also passed 733 tests, and the
restored dispatch-control and pristine cleanup suites passed 59 and 17 tests.
Strict KFD/runtime Clippy, formatting and whitespace checks passed there.
These historical results do not qualify the corrected source.

Eight separately applied mutations test active-owner retention, remaining-data
retention, interrupted-disposal receipts, post-free currentness order, in-flight
generation rejection, ordinary-wrapper success, typed-wrapper success and
receipt identity. Each compiles, runs its exact selected test and fails at the
declared behavioral assertion. The eight distinct source maps, raw failures,
read-only checker results and complete source restoration records are retained.
No compile failure or source-routing guard substitutes for a behavioral negative.
An independent review of the baseline and eight mutation/check/restoration
sequences found no evidence problem. It checked 26 closed records, exact mutation
identities, named assertions, clocks and source restoration. This bounded review
does not replace complete packet qualification.

The runner preserves raw UTC and same-boot monotonic observations, command
deadlines, endpoint source maps, exit status and owned-process-group cleanup.
The read-only checker validates the successful baseline, exact mutation hashes,
named failure and final assertion, and clock relationships. Its source-restored
check establishes equality only; primary separately observes process closure
before applying each restoration. Historical development records do not by
themselves qualify the corrected source. Neither the archive nor endpoint
equality supplies a continuous-immutability proof or authenticated external
proof-toolchain record.

## Production Boundary

R121 joins ordinary `DispatchResourceOwnerV1::release`,
`SharedGttMemorySessionV1::release_fixed_dispatch_data` and
`release_initialized_gfx942_device_memory` to retained cleanup. It extends the
existing dispatch-control root and shared memory engine, rather than adding
another allocation, mapping or disposal engine.

Ordinary cleanup retains the complete dispatch before generation validation.
Its existing poison and vacant-slot checks remain authoritative. It does not
import persistent generation/cardinality checks or allocate returned-data
storage. Cleanup order remains kernarg, forward code, then forward data.

After controls complete, the original data vector moves once into a forward
iterator retained in the root. Each active data owner is rooted before its
borrowed cleanup callback. An error, panic or incomplete callback retains the
active owner and untouched suffix, alongside original metadata. Completion
requires every lower cleanup to finish. The one-shot guard prevents re-entry;
ordinary custody cannot enter a returning or persistent extraction path.

## Typed Data Disposal

`shared_memory/data_cleanup.rs` retains the original fixed input or dispatch
authority. Its infallible decomposition keeps identity, layout, initialization
and dispatch facts beside the corresponding host token or device lease. All
five existing fixed-data variants use this driver.

Host-visible data reuses the borrowed control-cleanup driver with its actual
coherent-host profile and existing foundation projections. Device data reuses
borrowed device unmap and release operations with native device records and
backing accounting. It does not invent coherent-host model keys or CPU unmaps
for device allocations.

Native attempts and returned outcomes are recorded around each destructive
call. Successful device VA disposal is recorded before closing currentness and
accounting. The driver converts the consumed unmapped lease into an inert
identity/layout receipt even when a subsequent check returns an error or
panics. That receipt has no operation that can reconstruct allocation authority.

Pre-effect rejection preserves a healthy receiving session. Existing
currentness/foundation quarantine is not cleared. Errors after attempted
device unmap retain quarantine; panic retains custody and resumes the original
payload. Consuming wrappers retain incomplete cleanup and report success only
after actual completion. No automatic cleanup retry or duplicate refund is
introduced.

## Behavioral Oracles

The ordinary tests cover successful consuming release, empty data, original
generation policy, nonempty genuine preparation metadata, every control/data
position, currentness boundaries, partial unmap, actual model revision
exhaustion, incomplete callbacks, wrong modes and retry rejection. They compare
completed effects, active identity/phase/receipt, untouched suffix, exact native
arguments/order, accounting and original iterator storage.

The typed tests cover all five inputs, successful consuming wrappers, native
errors and original panic payloads, partial unmap error precedence, closing
currentness failures, actual host model-commit rejection, accounting underflow,
foreign-input rejection and retry prevention. Expected native prefixes derive
from the injected boundary, not the progress fields under test. Terminal
receipts are checked against actual original identities and layouts.

Two source-routing guards additionally check the public entrypoint wiring.
They supplement behavioral shared-adapter tests and do not substitute for
executing a live Linux session. Unchanged neighboring engine records in a
fixture do not establish retention of neighboring capability values that the
fixture no longer owns.

The traversal is linear in controls plus data, excluding existing generation
slot validation and adapter costs. Retaining an interrupted root is a safety
property, not a bound on total retained memory.

## Subsequent Integration

The first live join is `release_detached_fixed_dispatch_data` in
`queue_live/fixed_dispatch.rs`. A settled helper must keep typed cleanup outside
the model loan, then require completion and retake before committing identity
removal, count reduction and the reusable insertion index. Pre-effect rejection
must retain the consumed input without weakening existing error precedence.

The direct-session wrapper can transport a terminal parent after settlement.
The lane facade must instead propagate `terminal_transport` so auxiliary lane
ownership is restored before the whole parent is retained. Calling a consuming
direct-session wrapper from that facade would transport the parent too early.

Live cleanup must retain these roots outside the actual model loan, including
completed disposal through retake and metadata commit. Queue teardown must
retain taken parents, returned data and remaining signal/control ownership
through each later failure. Failed cleanup cannot create a reusable allocation
hole or queue slot.

The runtime's outer `release_resident_data_v1` roster also needs retained active
and remaining data through fallible release and lane selection. Qualifying one
lower data item does not establish that caller's complete-roster retention.

Those compositions feed nonpublishing generated resource adoption and actual
async ISSUE/COMPLETE. Production journal hooks, authenticated proofs, total
memory accounting and hardware/performance qualification remain separate
requirements. Generic raw-token APIs and XGMI teardown are not qualified by
this lower typed-data packet.
