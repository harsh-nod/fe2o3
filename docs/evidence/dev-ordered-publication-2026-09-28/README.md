# Indexed Ordered Publication

Development implementation and CPU qualification only. A1/A2, protected Worker
application, native execution and HIP/HSA parity remain incomplete.

## Source

- Signed implementation: `0811e1100ff6bde17d6b5da33bc7c567dec971de`.
- Qualified source tree: `67fff1a8590da3d7306b04fdf62c663a84a68501`.

The final qualification uses unchanged runtime/KFD sources. The unrelated
untracked owner-inspection packet was preserved.

## Implementation

Same-recipe ordinary successors now acquire a distinct indexed Publishing slot
before native submission without replacing their predecessor. Staging advances
the physical slot generation and occupancy, but not the logical epoch or commit
frontier. The attempt records consuming state before the lower call and stores
the returned outcome inside the lane callback, before its outer close.

Only an actually returned retry after successful outer close permits withdrawal.
Withdrawal preserves Pending custody and the next logical epoch while burning
the physical generation. A confirmed publication advances the epoch once and
installs the published owner before profiling. Each attempt still prepares and
authenticates its exact logical invocation. It reuses the live immutable native
recipe without native rebinding, storage overwrite or DATA rematerialization.

Before-attempt, consuming, returned-retry and returned-published terminal states
retain the exact indexed target. Returned error and unwind settle the Pending
FIFO and explicit-success dependency handoff once, without releasing deferred
ordering, allocation/module custody, the completion reservation or lane lease.
Other owners and later Pending recipes remain intact. Publishing is not Prepared
and has no cancellation authority. Checked slot/epoch exhaustion refuses further
publication before the native call without wrapping an identity.

## Qualification

Commands, UTC timestamps, full logs and exit statuses are retained. Cargo runs
are serialized with `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`.

| Final-source check | Result |
| --- | --- |
| Ordered-publication selection, serial | 9 passed |
| Materialized selection, serial | 35 passed |
| Full all-feature runtime library, serial | 1794 passed, 3 failed, 28 ignored |
| KFD backend subset of that full runtime run | 897 passed, 28 ignored |
| Runtime + KFD strict all-feature/all-target Clippy | Passed |
| Runtime no-default-features check | Passed |
| Workspace format check, signature and source continuity | Passed |

Selections overlap and cannot be summed. The backend subset is parsed from the
completed full-runtime log, not a standalone backend run. The three unwaived
failures occur at `authorized_execution.rs:1317` with
`InspectSocket(PermissionDenied)` / `Operation not permitted`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`.
No guard was bypassed and no failing test was skipped. The qualification runner
returns nonzero because that full-suite gate failed.

Full KFD tests, runtime-model tests,
runtime doctests and profiler-protocol tests were not rerun in this packet.
No new Verus or production correspondence campaign was run.

Preliminary receipts retain a strict Clippy test-style failure and two AUX
fixture-script failures. The public poll path observes both occupied lanes
before revisiting the ordered predecessor; the corrected scripts account for
all three observations and verify a fully advanced explicit-dependency cursor.
No production polling behavior was relaxed to pass these tests.

## Scope

Nine registered groups contain four container groups and five runtime workflow
groups. The workflows exercise 40 write-binding fault cases, 40 matching
unrepaired-Drop subprocesses, 13 corrupt-custody refusals, four identity-exhaustion
controls and 12 read/write retry-publication-ordered-completion controls. The
fault matrix covers primary/AUX lanes and frontier/Published-pipeline
predecessors; Completed/PhysicallyRetired predecessors occur in healthy controls.
These are case counts, not separate registered tests or GPU launches.

B/C/D pipeline owners now originate from public Pending admission and flush;
their submit/completion outcomes are still explicit scripts. Snapshots cover
untouched lane/frontier/pipeline identities, recipe backing, descriptors,
writebacks, resource retains, reservations, queued recipes and event prefixes.
Exact original panic payloads and terminal-operation refusal are checked.
Unrepaired Drop abort is tested; scripted disposal is not healthy native cleanup.

The container tests include 128 withdrawals in each of empty/older configurations,
stale-generation rejection, corrupt/quarantined metadata, exhaustion and zero
counted allocations for preallocated stage-confirm or stage-withdraw transitions.
Owner construction, preparation, public flush, profiling and native submission
are outside that allocation claim. Custody and slot preflight scan configured
storage and run before the publication timer. This is not an end-to-end latency,
high-depth scaling or matched HIP/HSA performance result.

## Remaining Gates

The independent R60 model is not a source refinement of the indexed adapters.
Its enqueue allocates an epoch before publication and its abstract retry is an
identity transition; the new physical staging/withdrawal requires a separate
relation. R60 also makes logical commit and profile visibility atomic, unlike the
runtime's committed prefix before optional/fallible reporting. Its capacity is
64, not the 1024-slot development profile.

Next, compile the exact private receipt-storage transition into KFD tests using
genuine typed lower receipts, then refine staged occupancy, outer-close custody
and ordered commit with production-body mutation controls. A recycled observation
contains packet-count metadata, not an independently bound target identity;
identity must follow exact consuming/returning custody and lower checks.
These steps still do not replace hardware, Context composition, protected Worker,
resource-closure or matched HIP/HSA campaigns.

MI300X SSH failed name resolution before connection; no remote files or jobs were
created. GitHub API refresh failed. A separate cached web view of
[#182](https://github.com/harsh-nod/fe2o3/issues/182) displayed Open, but was marked
crawled last week and is not a live API status confirmation. Native R125,
Admission R118B, Resources R116/V3 and A0-A7 acceptance statuses are unchanged.

Source pushes to both `origin` (harsh-nod) and `upstream` (powderluv) failed
resolving `github.com`. These failures were recorded without requesting
permissions or escalation. They do not establish remote publication.

`receipts.tar.xz` contains the frozen campaign, source identity/signature checks,
failed preliminary attempts, review notes and the implementation-ready next
refinement/receipt work. Verify it with `sha256sum -c receipts.tar.xz.sha256`
in this directory.
