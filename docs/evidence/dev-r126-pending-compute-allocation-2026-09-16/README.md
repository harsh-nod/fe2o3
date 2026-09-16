# R126 Allocation During Pending Compute: Development Receipt

This packet extends `1f7cbd1c5ba937226eb0cb9067327671ea4f1459`.
R125 remains the accepted CPU/test checkpoint. R126, A1/A2, issue #182,
formal implementation correspondence and HIP/HSA parity remain incomplete.

`source-files.sha256` identifies all seven changed Rust files. `source.patch`
contains their complete source/test delta, SHA-256
`37c2cc68932e4529114347d59fc5432082f3b1d1830db3fd13871181bec7fece`.

## Implementation

The runtime reuses established primary/directional SDMA ownership during
pending compute. Readiness requires both the owner and enabled engines.
Unsupported native availability still precedes this shortcut; active compute
still prevents queue creation or engine enablement on a cold route. Existing
request validation and ID-consumption precedence remain unchanged.

No allocator, pool, model loan/retake, promotion, zero-copy or completion
algorithm is replaced. Lower preflight, typed capacity disposition, exact
persistent coexistence admission and terminal retention remain in force. The
shortcut does not poll, drain, select a compute lane or complete a submission.
The phase audit found no additional guard necessary: memory mutation preserves
live records, while dispatch/completion/persistent owners remain separately
retained. This is source review, not a formal implementation proof.

An additive borrowed observer forwards exact lane and published-receipt
authentication to existing lower validators and domain-separates the result for
ordinary dispatch observation. It rejects terminal, swapped-lane and persistent
attachment states. It neither reads a completion signal nor supplies admission,
reuse, release or physical-completion authority.

## CPU Test Scope

Seven added runtime tests run matrices across ordinary primary-only,
auxiliary-only, both active lanes, a third queued stream, and persistent
prepared/published workflows established by real Scripted H2D and compute calls.
They compare exact owner IDs/bytes/box addresses, shadow identity/metadata,
active and pending recipes, resource retains, stream leases, tails and completion
reservations. Ordinary success fixtures finish original submissions and release
their Scripted owners through the normal runtime paths. Device fixture disposal
skips its scrub; explicit terminal disarming is not native cleanup evidence.

Warm typed Capacity rejection and cold-active Busy refund configured Context
credits without settling compute. A successful retry consumes the new credit.
Allocation errors/panics keep original work retained. Partial initialization
failures preserve the exact new Host owner and nonzero suffix; DeviceLocal
staging failures additionally retain the hidden indexed Device and its charge.
Configured Context quarantines exactly eight requested bytes/one record and
rejects subsequent calls as terminal. Context tests adopt an already-active
Scripted backend: this is facade allocation accounting, not an all-facade
compute workflow or a native execution proof.

Ordinary allocation success checks the exact `AllocationCreated` payload.
The existing Scripted persistent H2D setup lacks native profiling facts and
closes the retained profile prefix. Those cases verify its unchanged prefix
plus exactly one additional dropped observation, not that observation's payload.
Raw Scripted profile assertions are not full capture validation.

Preliminary operator logs retain the initial expected warm-guard failures and
three fixture corrections: third-stream flush while both lanes were occupied,
explicitly flushing queued work during later cleanup, and the persistent
profile-prefix boundary. The strengthened preliminary run passes seven tests;
these logs do not independently authenticate their executable identities.

## Final CPU Results

| Target | All-Feature Runtime Library Suite | Failed | Ignored | Libtest Duration |
| --- | ---: | ---: | ---: | ---: |
| GNU | 831 passed | 0 | 8 | 24.27 s |
| musl | 831 passed | 0 | 8 | 26.76 s |

Each target also passes eight focused KFD tests: exact epoch/completion
substitution, published-occurrence observation, lane-generation reuse, and five
coexistence tests. These are reused component tests, not a full KFD regression.
All eight ignored runtime tests require isolated native hardware, including the
two new pending-allocation probes. Test durations are not benchmarks.

Strict all-feature/all-target KFD/runtime Clippy, formatting, runtime
no-default-feature compilation and before/after source checks return zero.
Unsafe-source policy passes five tests with its explicit maintenance test ignored.
The final runner completed with status zero; no previous packet's full-suite
results are substituted for this source. Independent read-only source and oracle
reviews found no remaining blocker; those reviews are not proofs or test runs.

## Native And Qualification Boundaries

Two new ignored native probes extend the existing public vecadd workflow for
primary-only and primary-plus-auxiliary ordinary compute. They require native
Host/Device owners, exact live Device backing accounting, unchanged real
published-receipt commitments and wrong-lane rejection, full original output,
zeroed new allocations, profile ordering and eventual backing refunds.

The probes are unexecuted. A brief utilization dip on MI300X did not establish
isolation: the subsequent process/VRAM check confirmed other jobs on all eight
GPUs. No GPU job, upload or remote scratch directory was created. Previous
hardware results do not qualify this patch. Even a successful probe would show
logically pending native custody, not physical GPU overlap or performance.

Scripted persistent execution does not instantiate the native lower attachment
predicate. Ordinary prepared/completed, three-binding and pipeline phase
qualification remains open; there is no real Scripted pipeline publication
route. Focused lower tests exercise reused authentication, lane admission and
coexistence components, not positive native routing through the new observer.
Cold no-handle credit recovery after queue creation, broader native faults,
formal correspondence, aggregate memory and matched HIP/HSA benchmarks remain
open. This packet does not advance milestone acceptance.

`verify-pending-allocation.sh CHECKOUT OUTPUT_DIRECTORY` runs the CPU development
campaign with locked offline dependencies, `jq`, GNU `prlimit` and installed
GNU/musl Rust targets. The output directory must already exist. It checks source
hashes before/after, records commands/timestamps/statuses, identifies test
executables from Cargo JSON and records their hashes. This is not the closed
full-workspace qualification runner, a full KFD suite or a benchmark. No test
binary is included in the archive. `SHA256SUMS` covers the archive's other files.
Raw log endings and diff context are preserved rather than rewritten for
whitespace checks. `archive-checks.json` separates source/documentation checks
from the unfiltered raw archive check.
