# Unsafe Code Policy

The target is safe public APIs with a small, reviewed hardware/OS implementation
boundary, not zero `unsafe` keywords. KFD MMIO, mappings, custom ioctls, raw device
memory, ABI exports, and some process setup require explicit contracts. Moving
these operations into generated strings, C, dependencies, or larger unsafe blocks
does not discharge those contracts.

## Source Inventory Gate

The normal `cargo-fe2o3` test suite, already run by `scripts/ci-local.sh`, checks
`scripts/unsafe-source-baseline.json`. The check tokenizes every tracked and
non-ignored untracked Rust source and compares per-file, per-kind counts against
the reviewed baseline. Comments and literals are excluded; test code,
feature-disabled code, and macro templates are included. Templates are counted
once where written, not once per expansion. Unreadable or untokenizable files
fail the check.

The inventory requires a Git checkout; it is not a source-archive test. Run the gate:

```sh
cargo test --locked -p cargo-fe2o3 --test unsafe_source_policy
```

After reviewing a deliberate addition or removal, refresh and review the diff:

```sh
cargo test --locked -p cargo-fe2o3 --test unsafe_source_policy \
  refresh_reviewed_unsafe_inventory -- --ignored --exact
git diff -- scripts/unsafe-source-baseline.json
cargo test --locked -p cargo-fe2o3 --test unsafe_source_policy
```

Do not increase a baseline merely to make CI pass. A new unsafe operation needs
an explanation of why safe APIs cannot express it and a reviewed invariant.
Reductions also update the baseline, preventing an old allowance from remaining
available. Counts are a review aid, not a proof: replacing an operation without
changing its kind/count still needs normal code review. Generated Rust strings,
doctests, dependency implementations, and native C/C++/assembly are outside this
source inventory and remain part of the system's trust boundary.

## Implementation Rules

- Keep pure compiler, simulator, debugger-engine, and profiler-analysis modules
  unsafe-free, using `forbid(unsafe_code)` where the crate boundary supports it.
- Prefer existing safe typed OS APIs. Keep descriptor ownership in `OwnedFd`,
  `BorrowedFd`, `File`, or another owner; do not scatter `borrow_raw` conversions.
- Preserve exact descriptor identity, symlink policy, CLOEXEC, publication,
  locking, error, and teardown behavior during replacements.
- Keep hardware and process boundaries narrow and document why each unsafe
  operation's lifetime, aliasing, initialization, ABI, and concurrency
  requirements hold. Post-fork callbacks need async-signal-safety review even
  when the functions they call are individually safe Rust.
- Do not remove unsafe traits or raw-launch preconditions without replacing
  their guarantees. Extend checked/generated APIs to reduce caller obligations.
- Retire HIP/HSA qualification code only after its users and test coverage have
  replacements. Default production execution remains direct KFD.
- Test ownership and failure behavior, including compile-fail tests where
  applicable. Host tests supplement but do not replace GPU/MMIO validation.

## Engineering Dispatch Review

The engineering dispatch preparation and sequence changes (`7abce5c16`), peer
ownership (`2804bf6b6`), and peer sequences (`902fef6e1`) retain explicit unsafe
entry points because argument validation cannot prove that unauthenticated
machine code honors its ABI, buffer access, bounds, and termination contracts.
The operator must supply those guarantees and use the documented disposable,
exclusive process. Every error is terminal; uncertain native owners are retained
until process teardown rather than freed speculatively.

Dispatch preparation and consumption occur under the owning context or group's
mutable borrow, without an intervening free, map, or load. Complete sequences
are prevalidated before the first packet is published. Serial completion and
idle checks prevent argument storage from being reused while a prior dispatch
is outstanding. Peer tokens bind the exact group incarnation and allocation;
participant checks bracket dispatches, and errors quarantine the group. Peer
read-only access is an argument-binding policy, not hardware-enforced memory
protection. The caller still must trust every kernel's actual accesses.

The reviewed inventory counts for `engineering_gfx950.rs`,
`engineering_gfx950_peer.rs`, and `engineering_gfx950_peer_performance.rs` are
respectively four blocks/four unsafe functions, one/two, and one/one. Host tests
cover pointer bindings, bounded sequence ordering, injected failures, partial
mapping cleanup, quarantine, and ownership. Run them and the inventory gate with:

```sh
cargo test --locked -p fe2o3-kfd --features engineering-gfx950 --lib
cargo test --locked -p fe2o3-kfd --features engineering-gfx950 --doc
cargo test --locked -p cargo-fe2o3 --test unsafe_source_policy
```

These checks do not qualify native queue/MMIO execution, authenticate kernel
semantics, or grant protected runtime authority. No runtime behavior changes
as part of this inventory reconciliation.

### Independent-Rank Engineering Rounds

The additive round API separates private publication and completion observation
under the same exclusive owner. Its extra private unsafe publication boundary
does not manufacture a safe launch API: unauthenticated machine code still needs
the trusted-kernel obligations documented in
`gfx950-engineering-peer-round-v1.md`. Cross-command write conflicts reject,
all commands prepare before publication, and each distinct queue retains its
own kernarg and signal. Full group entry/exit checks remain, with live checks
during overlap. Errors retain every uncertain native owner until process exit.

The revised inventory records five blocks/five unsafe functions in
`engineering_gfx950.rs`, one block/one unsafe function in the new round module,
and one test-only block exercising empty-round rejection without native owners.
The peer owner and serial-sequence inventory entries are unchanged. The new
test block contains no GPU operation. Host fault tests and inventory checks are
required but do not substitute for independently scheduled native qualification.

### Ordered Engineering Batches

The ordered-batch additions reviewed against `21682228486f` retain the same
exclusive disposable-process and trusted-machine-code obligations. The worker
call in `engineering_gfx950.rs` enters the private unsafe
`Context::dispatch_ordered_batch` boundary; neither site establishes safe
arbitrary-kernel execution or production gfx950 admission.

All dispatches are checked before exposure. The context retains kernels,
buffers, and separate kernarg and signal slots through ordered publication and
completion. Observing the final signal is insufficient: every retained signal,
queue identity and completion frontier is checked before reuse. Any error
poisons the ordered path; the worker retains uncertain native ownership until
the caller terminates the process. Existing host regressions cover incomplete
publication, deadlines, signal failures, identity checks, and queue rollover.
They do not discharge the trusted-code or native queue/MMIO obligations.

This review changes only the inventory to six blocks/five unsafe functions in
`engineering_gfx950.rs` and one unsafe function in
`engineering_gfx950_ordered_batch.rs`. The library, doctest and inventory
commands above remain required; no runtime behavior is changed.

## Initial Reduction

The initial audit of `d9f6bbcd0` found 1,924 source sites in 288 Rust files:
1,026 in non-test crate source across all configurations, 874 in tests/fixtures,
and 24 in examples/benchmarks. This includes legacy qualification code, not just
the default production build. The checked-in baseline records the current
post-reduction counts without requiring a GPU to reproduce them.
