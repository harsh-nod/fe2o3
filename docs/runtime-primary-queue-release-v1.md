# R126 Primary Queue Release

## Development Status

R125 remains the last qualified Native checkpoint. R126 is in development:
the borrowed foundation-restoration fix is wired into the existing production
queue adapter, and lower control cleanup now supports separate unmap and
finish-release operations. The retained primary teardown route is not yet
implemented. A1/A2, issue #182 and full HIP/HSA parity remain incomplete.

The development parent is R125 commit
`cc3721a4f027fba19e86a63d564370285e719971`. Its archived evidence describes that
source, not these subsequent edits.

## Implemented Prerequisites

`QueueModelOwnershipV1::restore_foundation` borrows the real queue foundation
until complete validation and certificate revocation succeed. Only then does
it swap the two foundations and commit session ownership. Failure preserves
both owners and their ownership phase. `ComputeAqlQueueSessionV1` no longer
replaces its authentic foundation with an empty placeholder before validation;
its `foundation_in_engine` flag changes only after success.

`ControlCleanupCustodyV1` supports two one-shot borrowed phases. Successful
unmap reaches `CleanupStageV1::Unmapped` only after the model commit; disposal
requires this exact state and rejects failures and repeated attempts. Each
phase retains its original tokens, native outcomes and terminal disposal
receipts. `release_v1` still invokes both phases consecutively, preserving the
existing dispatch-control and coherent-data ordering and quarantine policy.

The eight new tests cover foundation ownership and loan-generation retention,
invalid restoration, exact split-control ordering, invalid reentry, projection
errors/panics, intervening revision exhaustion, a panicking poison callback,
and release-currentness failures. The split fixture uses existing dispatch
controls, not the four actual primary-queue resources.

## Development Validation

- Full GNU KFD/runtime library tests: 1,252/744 passed, zero failed/ignored/filtered.
- Musl KFD shared-memory tests: 263 passed, zero failed/ignored, 989 filtered.
- Strict Clippy for both crates, all features and all targets: passed.
- Two bounded read-only source reviews found no correctness blockers.

Commands:

```sh
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib
cargo test --locked --offline -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --lib shared_memory::tests
cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
```

These are local development checks, not a full R126 qualification campaign.
No native GPU, formal implementation, total-memory or HIP/HSA performance
claim follows. No SSH work was needed.

## Remaining Integration

1. Extend typed cleanup custody to `AqlQueueGttV1` and
   `UserptrAqlControlGttV1`. Preserve USERPTR's free-before-CPU-unmap order and
   the absence of a separate native VA-release call.
2. Add borrowed event, payload, runtime and doorbell teardown with retained
   per-call outcomes. A protected but still mapped payload must never be
   re-zeroed. Runtime-disable failure must not re-lock an already held gate.
3. Install a boxed primary teardown owner in the runtime backend before native
   effects. Retain the parent, all lower cleanup owners and the process gate
   through errors and unwinds; preserve the original panic payload.
4. Preserve queue teardown ordering: queue/event destruction, payload/runtime,
   doorbell, resource publication/model restoration, all four GPU unmaps, all
   four releases, shadows, dispatch, signals, then confirmed gate completion.
5. Reject unsupported profiles before effects. Include custody in terminal,
   vacancy and backing-accounting checks; retire the queue only on full success.
6. Test the constructed primary owner, including queue-side restoration flags,
   actual resource identities, every native/model failure prefix, untouched
   owners, backing charges and blocked slot/gate reuse.

The initial ordinary-primary profile does not remove other queue profiles from
the final objective. Generated DATA-ADOPT, ISSUE, completion/readback/typed
replies, Stop/drain/graphs, resource proofs and production Context integration
remain subsequent requirements.
