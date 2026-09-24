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

The gated namespace collector in `child_namespace_report.rs` uses six unsafe
blocks and one private syscall function because post-clone observation cannot
use allocating or TLS-dependent wrappers. Its x86-64 stat buffer layout and
syscall clobbers are explicit; static NUL-terminated paths, exclusive writable
outputs and single-close descriptor ownership bound the raw accesses. Every
syscall failure is tested. The report is inert without private-pipe provenance,
EOF, retained pidfd custody and the exec gate.

The synthetic native consuming fixture has seven process-bootstrap blocks and
sixteen static-issuer I/O blocks. Bootstrap drops credentials/capabilities only
in the disposable child. I/O adopts fixed inherited descriptors once, checks
initialization before reading syscall outputs, uses finite nonblocking operations
over live buffers, and never retries close. The profile integration fixture's
fifteen blocks cover the same isolated credential setup plus a non-leader UTS
namespace and raw clone3 child, with exclusive pipe/pidfd custody and bounded
reaping. These are test-only privileges and operations, not production authority.
The four consuming cases passed under the exact locked profile on MI350; they
do not establish signing, durable recovery, protected proofs or GPU execution.

The inherited broker test that closed a still-owned descriptor before borrowing
it again has been removed: `forget` after validation cannot repair that I/O
ownership violation. Existing descriptor-substitution and flag-mutation tests
keep every owner valid while checking continuity refusal. The audit inventory
also accounts for moved socket/pidfd inspection sites without treating relocation
as removal of their safety obligations.

The native profile and compiler-execution subject test allocators forward all
pointer/layout operations unchanged to `System`. Their thread-local counters
use saturating increments so exhaustion cannot unwind through `GlobalAlloc`;
deterministic maximum-count tests check saturation and restoration. Ancillary
custody retains separate ownership for rustix's SCM_RIGHTS descriptors and the
guard's SCM_PIDFD descriptors. Its initialized aligned backing, checked record
bounds, one-shot adoption and unwind cleanup remain required despite extraction
into shared modules. KFD debug-metadata publication has a separate retained
no-queue transaction and native ownership contract; its registration does not
grant queue or dispatch authority. See
[no-queue metadata](gfx950-debug-metadata-noqueue-v1.md).

The no-queue review covers one example call block, one consuming unsafe function,
three retained metadata-store blocks, and two native ioctl blocks. Its transport
uses initialized exact-layout inputs and retains all native custody after any
possible exposure. Fresh-process and no-foreign-runtime obligations remain with
the caller; observational isolation checks do not prove them. The transaction
test's single remaining block reads the freshly supplied root synchronously,
before mutation. Later assertions consume copied transition snapshots, never a
saved shared-derived pointer across mutable access. This reconciliation grants
no GPU qualification or production launch authority.

The separate debugger-observation sibling adds one call to that same consuming
no-queue unsafe function, under the unchanged reviewed-process lifetime and
no-foreign-runtime contract. Its three unique host-only rendezvous exports use
`unsafe(no_mangle)` solely to retain fixed breakpoint names; they expose no
pointer or GPU capability and perform only host atomic operations. The first
marker precedes the sibling's KFD/VM work. Marker presence and entry snapshots
do not prove startup or lifetime isolation. The controller must retain owned
process custody and an independent outer family-cleanup contract. These source
inventory additions do not qualify debugger acceptance or GPU execution.

The source-safety fixture `owned_unsafe_closure.rs` deliberately contains one
empty unsafe block inside an owned `FnOnce` closure. It exercises rooted source
rejection even when MIR optimization removes the empty block. There is no unsafe
operation or runtime permission to replace with a safe API: removing the syntax
would remove the negative case. The inventory records this test-only block;
`production_collector_rejects_reachable_unsafe_rust_with_rooted_diagnostics`
must continue rejecting it before artifact export. This reconciles the fixture
introduced in `669713204edf8f6685b79f604bd5f05708cedcf2`, without changing its
source or the inventory gate.

The `workgroup_unsafe_callback.rs` and `workgroup_external_unsafe_callback.rs`
source-safety fixtures likewise each contain one deliberate empty unsafe block.
They ensure that traversing an authenticated `with_workgroup` provider still
checks user callbacks, including an external callback factory under optimized
MIR. These are negative-test syntax, not unsafe runtime operations. Each block
is counted separately, and the rooted source-safety test must reject both
before artifact export; provider authentication grants no callback exemption.

The Wave64 source-capture fixture introduced in ec9a526c55f82825e17c7a507507d6cb1debf7ce
contains one deliberately unsafe call in a macro template and three unsafe
lookalike functions. The call is a negative case: the collector must reject the
user helper's unsafe block before semantic capture. The three unreachable
lookalikes have the provider's scalar signatures but different local definition
identities; the callback mutation checks must reject them as trusted terminals.
Their bodies only return an input. Replacing these declarations with safe
signatures, or removing the unsafe call syntax, would remove the intended
authentication/source-safety negatives. The inventory counts the template
once, not once per scalar expansion. This reconciliation changes no fixture,
source-admission rule, trusted-provider definition or runtime behavior.
See production_rustc_driver_wave64_capture_source_v1_tests.rs and
collector/production_wave64_shuffle_terminal_v1_tests.rs for the actual
callback assertions; an inventory pass alone does not execute those callbacks.

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

### Engineering Dispatch Timestamp Diagnostics

The explicit ordered64 profiling command adds one unsafe worker call and one
private unsafe entry. The entry preserves the same trusted-machine-code and
disposable-process obligations as ordinary ordered64; no safe launch authority
is created. Five orchestration blocks delegate existing ordered execution and
the four new raw-memory helpers under the exclusively borrowed queue owner.

The raw-memory module has four unsafe functions and seven unsafe blocks. Safe
byte slices cannot represent concurrently GPU-visible control/signal storage.
These operations access only the reviewed aligned property word and signal
timestamp words: enable/restore use CPU-owned properties only while idle;
clearing requires no outstanding packet; reading follows acquired completion
and forbids signal reuse. Every pointer retains extent/alignment checks. Any
error poisons the owner; only fully successful capture restores properties.
Ten test-only blocks exercise those same helpers on inert aligned owned bytes,
including pending/wrong-kind signals and a rejected misaligned pointer. They
perform no GPU operation. The orchestration/wire test module has no unsafe sites.

The reviewed final inventory is eight blocks/five functions in
`engineering_gfx950.rs`, five/one in its new timestamp module, seven/four in
`memory_linux_dispatch_timestamps.rs`, and ten blocks in its test module.
Of the engineering entry's change from the old baseline six to eight blocks,
one already exists in d10f49bf's ordered64 worker route; only one is new here.
Separately, d10f49bf's ordered-batch module already contains two unsafe blocks
and three unsafe functions, while its older baseline lists only one function.
Those existing ordered64 wrappers retain identical trusted-code obligations;
this patch reconciles their inventory without changing their implementation.
No unrelated inventory entries are refreshed.

Run the existing full source-inventory gate and the engineering library,
doctest and strict Clippy checks remotely. Inventory review and CPU tests do
not qualify the profiling ABI on installed firmware, turn raw GPU clock ticks
into nanoseconds, or establish shader-only execution time.

### Packet-Incapable Debug Empty-Queue Retirement

The opt-in gfx950 empty-queue successor adds one consuming unsafe function,
`Gfx950DebugExecutionPreparationV1::begin_empty_queue_runtime`. The caller
must retain an isolated disposable process, exclude foreign KFD/ROCr runtimes,
queues, code injection and concurrent runtime actors through completion or
process termination (also after failure or Drop), and externally bound/reap
blocking native calls. Neither static preparation nor the in-library gate
proves these lifetime obligations. The returned move-only owners expose no
packet publication, doorbell write, dispatch or sampling operation.

Two new blocks in `runtime_debug_empty_transport_v1.rs` perform the fixed
runtime-disable and trap-clear ioctls. Initialized 16-byte input/output and
24-byte input-only ABI records respectively borrow the actual retained
Context/device fd exclusively; mode-zero output must remain exact. Two blocks
in `queue_linux.rs` destroy the same retained event with an owned 8-byte setter
record and unmap the exact privately owned doorbell VMA. Event poisoning and
doorbell deactivation precede their native attempts, preventing retry after
an error or ambiguous return; no raw pointer or numeric-owner constructor
escapes. These kernel ABI and VMA operations cannot be replaced by ordinary
safe Rust memory access.

The private cursor marks each step Attempting before its action and never
advances on error or unwind. Actual queue destruction precedes event destruction,
metadata withdrawal, runtime-disable return, trap clear and doorbell unmap;
allocation GPU/CPU unmap, handle free, VA release and accounting follow.
Unresolved backing and original descriptors remain in cold custody on failure,
panic or Drop, with no Drop ioctl or speculative cleanup. Only the private
same-owner terminal witness releases retention and closes descriptors last.
Fork checks precede native work and inherited gate-lock access. Existing
ambiguous-VA cleanup retains its abort policy. These are local retirement
invariants, not debugger ACK, sampler exclusion, physical capture or dispatch
qualification.

The exact inventory delta is `queue_linux.rs` 45 to 47 blocks (its one extern
block unchanged), two blocks in `runtime_debug_empty_transport_v1.rs`, and
one function in `engineering_gfx950_debug_execution_v1.rs`. Failure-order,
ABI and compile-fail controls supplement review; the source-inventory gate
and feature tests remain required. This reconciliation changes no runtime
implementation or existing plain/noqueue/gfx942 behavior.

## Initial Reduction

The initial audit of `d9f6bbcd0` found 1,924 source sites in 288 Rust files:
1,026 in non-test crate source across all configurations, 874 in tests/fixtures,
and 24 in examples/benchmarks. This includes legacy qualification code, not just
the default production build. The checked-in baseline records the current
post-reduction counts without requiring a GPU to reproduce them.
