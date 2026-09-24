# Early Async Operation Event CPU Qualification

Status: CPU-tested public async integration. This packet adds no native,
formal-refinement, aggregate-memory or matched-performance acceptance.

## Implementation

The [contract](../../runtime-async-operation-events-v1.md) adds independent early
event receipts to frozen typed launches, ordinary async copies and directed
peer copies. Submission, exact event recording and completion observation use
separate advances of the existing rooted operation driver. Two reply credits
precede command admission; one registry entry retains the operation. Directed
copies acquire no implicit flush or event observer. Event recording can fail
after submission without authorizing cancellation or retry.

Seventeen new regressions cover pending shared-source diamonds with the optional
journal on/off, frozen launches and ordinary copies, both non-Send owner modes,
reply/command/registry bounds, disconnected and closed admission, cancellation,
Stop at different advances, dropped observers, invalid event handles, submit
and record errors, record panic, and successful/exhausted drain. An explicit
early-release rejection distinguishes command enqueue from Context admission.
These use scripted backend leaves, not native execution. Source reviewers found
no release blocker in the final candidate.

Residual coverage limits include no direct new test of an accepted operation
crossing drain before its recording advance, or actual Context event-capacity
and Unsupported recording preflight errors. Existing admission/registry and
recording-error paths apply; these observations are not new test claims.

## Final Qualification

All nine serialized commands finish successfully with their owned child process
groups absent:

- GNU and musl all-feature runtime libraries each pass 1,358 tests, with twenty
  existing hardware-only ignores and no failures or filtered tests.
- All seventeen new early-event regression names pass on both targets.
- Runtime doctests pass 4 plus 42, totaling 46, without ignores or filters.
- Runtime formatting and strict all-feature/all-target Clippy pass.
- Rust/Cargo before/after output bytes match. All 3,906 selected source, build
  and harness inputs remain unchanged across the run.

`inputs-before.json` and `inputs-after.json` bind the runner and input roster.
Each command directory retains exact argv, status, timestamps, process group,
absence result and raw stdout/stderr. The included runner records environment
and timeout policy. Raw blank lines are preserved. These are source-bound
development checks, not hermetic toolchain attestations or production proofs.

Runner SHA-256:
`bf706612eb428ed7e6cbca5c52e24d0a575ec35a8739def630bb066bf0e9ee21`.

## Development History

Private logs remain under
`/home/harsh/.codex-tmp/fe2o3-early-event-20260924-AYxTYZID`.
The initial direct focused compilation failed because the test helper accepted
only command futures, not existing tracked operation futures. The helper now
accepts any future with the same result type. That unbracketed compiler output
is development history, not qualification evidence.

The bounded `focused1` run then passed fourteen tests and failed one: its
ordinary-copy fixture used the same allocation for source and destination.
Context correctly rejects that case, even with nonoverlapping regions. The
fixture now uses a distinct destination; the production guard is unchanged.
The final frozen run above includes both corrections and two later regressions.
Earlier failures are not reclassified or substituted for the final cohort.

The exclusively owned 661-MiB target cache is removed after qualification; raw
logs remain. No shared cache, remote directory or GPU resource was used. Native
R125, Admission R118B, Resources R116/V3 and A1/A2 acceptance remain unchanged.
