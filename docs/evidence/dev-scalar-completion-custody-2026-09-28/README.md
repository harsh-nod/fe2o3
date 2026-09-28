# Indexed Scalar Completion Custody

CPU developer evidence, not GPU execution, executable formal refinement,
complete #182/A1/A2 closure, HIP/HSA parity, or matched performance evidence.

Baseline: `6fc79a423f64cd7172926511b4eadad2082f9835`.
Signed implementation: `3c5c713990bd84a06417f4eaca60cb63ac685299`.
Source tree: `5645d9a0a92f3b9d1a1fa86b3d62891b72c18696`.

## Change

The previous scalar completion path removed Active before consuming native
poll/recycle, detach and frontier-retirement receipts. Explicit failures kept
the lower owner but lost the outer logical descriptor. Non-storage restoration
also allocated a new Box after typed native input had returned. The strengthened
existing Poll fault test reproduced the missing Active before this change.

Scalar preparation now reserves a boxed completion root before input extraction
or binding. Successful binding installs Active, then donates the original empty
bind-recovery box to the completion root. Authenticated Write/ReadWrite origins
also reserve the replay box needed for the changed output type. Read and replay
origins need no replacement input box; InitializedStorage retains its existing
record-owned restoration shells. Clean bind rejection keeps its original path.

The root is carried through prepared publication. Poll and wait both enter an
indexed completion transition without removing Active. Published, recycled,
detached and retired receipts are reindexed before timing, validation, hooks or
diagnostics. Consuming lower calls leave a NativeOwned marker; returned terminal
custody is retained before error formatting. Unwind poisons the backend and
preserves the original panic payload without dropping the indexed outer owner.

Retired input remains indexed while effect, exact slot and shell are checked.
Restoration fills a previously allocated box and then marks Restored. Commit
checks exact allocation/module/stream/custody accounting and result capacity
before settlement. Active stays indexed through completion profiling and is
removed last. Failures before commit do not manufacture successful completion.

Prepared publication and cancellation authenticate the completion root's exact
admission, Reserved phase and source-compatible shell as well. Scalar Write and
ReadWrite both admit authenticated input's native replay result. Three-binding
completion itself is unchanged and still requires a separate custody audit.

## Coverage

The fixtures use real public scripted H2D transport, typed storage conversion
and prior completed scripted compute for replay. They do not fabricate native
dispatch receipts or execute kernel arithmetic.

- Twenty-one successful scalar completions cover Read, Write and ReadWrite
  from ready/storage origins, read replay, and three repeated launches per
  combination. Counted public scripted completion allocates zero and preserves
  exact device owner, bytes and Box identity. Original bind shells are also
  checked at the completion root. Write effects change metadata, not simulated
  GPU bytes; these controls do not establish kernel-result correctness.
- Three zero-deadline wait controls preserve the exact published owner, root,
  accounting and zero-allocation timeout, then complete successfully.
- Nine reservation controls count one root allocation, plus one missing replay
  shell only for authenticated writers. Three public synthetic reservation
  refusals verify unchanged original storage before extraction and unpublished
  failure settlement. This is not allocator-exhaustion injection at every site.
- Forty-two subprocess cases cover scripted Poll/Recycle/Detach failure,
  post-retirement unwind, and slot/shell/effect mismatch for three origins and
  both poll/wait entry points. They preserve exact typed custody, Active,
  queued successor recipes/FIFO, accounting, bytes and owner identities.
- Twenty-one subprocess cases cover malformed published admission/phase/roster,
  missing reservation/custody and a restored-but-uncommitted roster failure.
- Eighteen subprocess cases reject malformed completion roots before prepared
  publication or cancellation, retaining the original armed input and ledger.

All subprocess controls require an inspection marker followed by actual SIGABRT
on unrepaired backend Drop, with core dumps disabled. These are private adapter
contract-violation tests, not arbitrary-memory-corruption or whole-ledger proofs.

## Qualification

Final checks ran sequentially against the signed source above with
`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`. The packet contains commands, output,
exit statuses, timestamps, source identity and signature verification. Final
source continuity passed after all build, test and formatting processes ended.

- New completion selection: 7 tests passed, including all 81 fault children.
- Existing scripted persistent selection: 11 tests passed.
- Lower KFD persistent selection: 166 tests passed.
- Runtime unit tests: 1,747 passed, 3 failed, 28 ignored.
- Runtime model unit tests: 1,080 passed, 19 ignored.
- Integration tests: 11 passed, 3 hardware tests ignored.
- Doctests: 52 runtime and 29 runtime-model tests passed.
- Strict all-feature/all-target Clippy, runtime no-default-features check and
  workspace formatting check passed.

The full test command exited 101. The same three authorization/telemetry tests
failed at `authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. These failures are not
waived; the complete suite is not green.

Development receipts retain the expected pre-fix missing-Active failure and
nonfinal focused checks. The review notes identify the earlier formatter/check
overlap; only the frozen signed-source campaign establishes final qualification.
Read-only reviewers found no blocking legal-path defect after the entry, commit
and Prepared-root preflight corrections.

## Open Boundaries

Counted zero-allocation completion is a scripted observation of the shared
restore/commit adapter. The native poll/recycle/detach/frontier-retirement calls,
native returned variants and native lower-call unwind are not executed by these
controls. Lower KFD contract tests are separate evidence, not a coupled hardware
campaign. Native profiling faults and post-commit observer failure require
additional qualification, as do three-binding completion, Context composition,
generated execution, production refinement and matched HIP/HSA performance.

The root adds a pre-bind allocation. Reusing original input boxes removes
redundant replacement shells, but this packet claims no end-to-end launch speedup
or zero-allocation launch. Accepted checkpoints, A1/A2 and parity remain unchanged.

MI300X hostname resolution failed again; no remote files or jobs were created.
