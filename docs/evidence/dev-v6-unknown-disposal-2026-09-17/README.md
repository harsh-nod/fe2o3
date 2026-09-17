# V6 Ordinary Unknown Writer Disposal

CPU/test-qualified development above canonical base
`0a58128b74282aaae59194d6edb91aa424b814e5`. Accepted checkpoints remain Native
R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3. This closes the
ordinary Submission Unknown allocation-owner disposal gap, not V6, A1/A2, #182,
protected native composition or HIP/HSA parity.

## Change

Ordinary submission roots now retain each original destination allocation record
and canonical journal member before backend entry. Per-allocation release binds
its exact Unknown writer and complete original roster before the first effect.
Only backend release Ok creates a disposal receipt. The successful handle is
removed immediately, without releasing siblings; every model member and all
writer request credits remain retained until all original destinations are gone.

The final member triggers the existing atomic model `dispose_unknown`, followed
by phase removal, exact credit refund, matching submission-marker clearing and
root removal. No content lineage or successful completion is published. Partial
Rejected/Quiescent releases retry only still-live members. Terminal failure or
panic retains receipts and quarantines the entire writer's remaining credits.
A post-model credit rejection retains an explicitly finalizing root, still
counted in cleanup. Fresh allocations can reuse already-disposed backend handles
without final retirement removing their indexes or credits.

Canonicalization uses transient O(b) IDs and retains O(k) records/members for k
distinct destinations. First disposal validation and finalization are O(k);
intermediate member lookup is O(log k). Storage is acquired before submission,
not during disposal. See the [contract](../../runtime-context-version-journal-disposal-v1.md).

## Qualification

The frozen source ran locally in
`/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917`. GNU and scoped musl each pass:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| fe2o3-runtime | 999 | 17 |
| fe2o3-host | 271 | 4 |
| fe2o3-runtime-model | 779 | 2 |

Each target has 2,049 passes, 23 ignored, and zero failed, measured or filtered
tests. Eight new regression tests pass on both targets. The main disposal-order
matrix has 96 rows spanning six orders, four Unknown origins, configured versus
unconfigured credits, and retained versus released submission metadata. Further
matrices cover boxed error/panic identity, partial cleanup, every release-failure
class, recycled backend handles, missing/substituted custody, pre-model final
bookkeeping failure, injected post-model credit rejection and independent mock
device accounts. Exact original members, disposal flags, remaining native-mock
storage, facade/backend indexes and credit charges are checked.

Strict all-feature/all-target Clippy, scoped formatting, no-default checks,
86 doctests and unsafe-source policy (5 passed, 1 ignored) pass. Musl uses
`FE2O3_HIP_SYS_DISABLE=1`; unrestricted musl/HIP linkage is not qualified.
The suite covers runtime/host/model, not a fresh complete lower-KFD campaign.
An exploratory runtime run found one historical assertion that still expected
recoverable cleanup to remain incomplete. It was updated before source freeze;
the frozen campaign passes in full.

Sixteen serial raw records retain argv, UTC start/finish, output and exit zero.
The hardened parser checks exact package/target identity, unique test rows and
matching summaries; its calibration accepts both immutable baseline targets and
rejects ten malformed-log mutations. GNU/musl complete rosters match across all
2,072 named entries, SHA-256
`aa0b8c553a8e675853655774424357d891bfb668e150ad75aa5468b4a9bf5521`.
All six executed test binaries are hashed and rechecked after the quality gates.

The manifest covers 15 changed source files: eleven Rust and four docs, checked
before and after qualification. Its SHA-256 is
`ad35716e56361fa77636f2c88baaeba42b4c76ad69bb69fccd034ac2cc8d230d`.
The exact base-relative patch has SHA-256
`96fa34b9caf4fe58818cbeafe663880f11027fc5bc2af25d03ca50437a8fcbe5`.
Independent read-only reviews checked implementation, tests, contract and evidence.
The seal rechecks source, patch, binary hashes and record ordering before
manifesting every archive file. Sealed evidence is immutable.

## Limits

No native GPU, formal solver or matched HIP/HSA benchmark was run for this packet;
no remote files or jobs were created. Mock allocation-owner release does not
prove native pages were unmapped or that KFD pool residency reached zero.
Bookkeeping failures are CPU fault injections, including a test-only credit
release rejection after genuine model retirement; they are not native failures.

The backend release premise remains Contracted. Existing model/proof results do
not prove this Rust/native integration. Generated protected writers, input/read
leases, backend aliases, ordered overlapping writers, cross-run version reuse,
content recovery, aggregate residency and its physical-disposal accounting remain
open, as do native depth/overlap/failure qualification and matched benchmarks.
