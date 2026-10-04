# Native Compiler Channel Checkpoint

Date: 2026-09-28 (PDT). Tested code: `bdeeb678a411d29ec59eab9397c9993a20def4a9`.

[Issue #272](https://github.com/harsh-nod/fe2o3/issues/272) remains incomplete.
These changes validate process mechanics, not a production compiler transaction,
protected proof execution, GPU execution, or 47/47 tutorial completion.

## Implemented

- The existing native compiler stage can create a service socket after the
  child's credential transition. It transfers the service endpoint before READY,
  keeps the client high and CLOEXEC while gated, and installs FD195 after the
  ordinary descriptor bindings. Collisions refuse before clone; sends do not retry.
- A private coordinator receiver checks packet credentials, unnamed socket shape,
  peer credentials, canonical child/parent claims, liveness and deadline. It
  duplicates the original clone-owned pidfd, never reopening a numeric PID.
  The public same-UID handoff protocol remains unchanged.
- Retained proof-helper custody now permits scoped compiler-backing access and
  one-use shutdown through a shared reference. Reentry refuses without waiting;
  failure or unwind cannot replay Finish. Cancellation keeps the original pool
  and backing. These lifecycle tests do not fabricate an approved runtime.
- Shared wire helpers, logical resource charges and the reviewed unsafe inventory
  cover the additions. The receiver itself remains inert: actual admitted-profile
  and original-runtime-account binding belong to the unfinished owning attempt.

## Validation

The final serialized test and compiler-check guards used one unchanged
9,264-file source snapshot:
`54fa34347f622915fc1c581b731885f3462283857aa23963f3e78f41cc74804f`.
This includes the concurrent integer-worklist change from `10789570b`, preserved
by a normal merge; the V2 worklist is not activated in the default pipeline.

- Coordinator, spawn, client, supervisor and GPU dialect: **894 unit/integration
  tests and 256 doctests passed**; 51 environment-dependent tests ignored locally.
- `cargo-fe2o3`, `rustc-codegen-fe2o3` and coordinator: all-target check passed,
  with existing warnings.
- Unsafe-source policy: 5 passed; its maintenance test remained ignored.
- Scoped formatting, diff checks and eight hygiene-policy self-tests passed.
  Hygiene deltas passed before merge from `6152314066` and after merge from
  `10789570b`. Nested test processes are not extra suite counts.

Two isolated-root diagnostics passed on `mi350-2`:

1. Native channel traffic survived gate release and exec; a full transfer queue
   and an invalid gate token refused through the existing failure/cleanup path.
2. The final coordinator test binary passed receipt, exec-stop and cleanup plus
   expired deadline, disabled credentials, wrong profile, and one-short work/storage
   cases. Refusals checked FD disposal, storage rollback and denial history. All
   cases shared one funded cleanup pool, followed by empty shutdown.

The tests used a read-only, networkless, resource-limited container and a static
fixture, not an approved compiler. The final coordinator binary SHA256 was
`d996edb8b1fea3de06099d6f13df2449733c9c3c03054b663d2a728068ad96cc`;
the native-spawn binary was
`2e2c654e165ea0c54148a879d3c79c44fd945220fb927f1c7733b7ee99093ac7`.
The coordinator diagnostic was rerun after the merge; the native-spawn binary
remained byte-for-byte identical to its successful diagnostic run.
Every attempt's container, scratch files and private SSH connection were removed.
Earlier diagnostic fixture failures were preserved and fixed, not counted as passes.

## Still Required

The installed coordinator still does not accept compiler attempts. Authenticated
wrapper intake, actual cwd/stdio and loader/input mappings, retained helper/compiler
composition, existing issuer-manifest integration, full proof-session association,
descendant enforcement and installed activation remain required before the complete
production transaction and 47-kernel GPU matrix can be validated. Forced low-FD
allocation cases also remain a targeted native diagnostic coverage gap.
