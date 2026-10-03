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

The 2026-09-30 native-custody audit reconciles seventeen omitted or stale
inventory entries. All seventeen source files are unchanged from the published
`edd71e6762252d1990d10d84d10cc53b666d09b7` checkpoint; this is a review record,
not new runtime behavior. The primary reviewed issuer fixed-slot ownership and
the coordinator publication fixture; two independent workers reviewed broker
publication/channel custody and retained-child cleanup respectively. No blocking
safety defect was identified by these static reviews.

- The six broker files contain sixteen blocks and three trait implementations.
  Test credential changes occur only in explicit disposable subprocesses;
  closed-descriptor probes never construct a borrowed owner. Production payload
  acquisition installs each actual owner before later fallible accounting.
  Retirement retains the independently owned barrier across destruction,
  without holding a coordinator mutex or forging `Send`. OFD-lock probes use a
  fully initialized native `flock` and a live, separately opened file.
- The seven spawn files contain fifty-one constructs. Late operations require
  the original account, creator thread, reserved slot and holder identity.
  Prepared tokens retain mutex custody and trace-bound lifetimes. Deferral keeps
  both payload and funding; the temporary `Arc` is dropped before terminal
  publication. Test implementations hold inert counters or empty payloads;
  fake descriptor-free children cannot signal scalar PIDs. The real traced-child
  fixture is an observation test, not protected compiler admission.
- The issuer V3 reader's three blocks inspect inherited flags and duplicate
  descriptors above the entire inherited table before transferring unique
  ownership to `File`. Its two test files contain seven and four blocks;
  fixed-slot mutations and `dup2` run only in isolated children with retained
  high-numbered sources. Raw borrowed slots remain live for each use.
- The coordinator publication fixture's three blocks use a genuine stopped
  trace and its original cleanup lifetime, plus initialized nonblocking OFD-lock
  probes. They neither resume a compiler nor establish production admission.

The inventory gate must still be rerun in an isolated worktree build. These
reviews do not establish native execution, protected proofs or GPU qualification.

The paired native V3 coordinator and provisioner mains each add one reviewed
unsafe call block. These dedicated binaries call their existing unsafe entrypoint
exactly once, before creating threads, handlers, environment borrows or Rust
owners for activation FDs. The coordinator transfers unique FDs 3..16 and retains
the external whole-cgroup termination contract; the provisioner accepts no
activation. Both own argv/environment mutation until exit. Return or unwind
terminates the process, with no retry or alternate entrypoint. These are ordinary
root Rust mains, not protected-child secure-entry shims. The inventory entries
record this source contract only: effective installed unit/LSM/cgroup custody and
direct-root intake startup still require qualification. No other inventory
allowance is changed by these two entry calls.

The dedicated proof-helper entry owns raw FD 3 before the first fallible check,
validates it before opening other descriptors, duplicates it with CLOEXEC and
closes the original exactly once. Its six mechanical unsafe blocks inspect raw
flags, borrow that live descriptor, close it, close unrelated descriptor ranges,
and poll one initialized parent pidfd. The two unsafe mechanical functions and
the public consuming entry require a sole dedicated secure-start process; these
are not shared-application APIs. The binary has one call block and the entry has
two. Four test blocks operate only in disposable subprocesses. Numeric parent
root and startup acknowledgments do not supply host-root provenance or proof
authority; the actual creator and outside whole-domain custodian remain required.
The private root-side helper adapter has one unsafe consuming launch function
and three blocks calling existing sealed staging, retained namespace spawning,
and exec confirmation. The complete backing enters the existing cleanup slot
before clone. Independent exec-status EOF precedes credentialed readiness.
Its staging test has one call block and an unsafe-function signature assertion;
neither constructs a positive runtime or deployment. The shared send test's two
blocks reset/unblock SIGPIPE only in a disposable subprocess, checking that the
real send uses NOSIGNAL; no application or host signal policy is changed.

The private root-to-V3-issuer composition has two unsafe functions and four call
blocks: delegation with the original confirmed compiler trace, sealed staging,
retained spawning, and exec confirmation. These calls retain the actual prepared
owners and compiler backing in the funded cleanup slot before clone. Final
descriptor checks, profile validation, independent exec EOF, exact readiness plus
EOF, and liveness checks do not remove the caller's creator-thread, exclusive-wait,
bounded-Drop and inherited-lock-alias obligations. Its one test-only unsafe block
stages inert files to check the fixed descriptor map; it never spawns or admits
them. The opt-in isolated root startup matrix adds four reviewed call blocks:
compiler staging, retained clone, held-exec confirmation, and actual V3 issuer
launch. It constructs genuine prepared inputs, closes all inherited lock aliases,
keeps the compiler at its first exec stop, and drains the independently funded
cleanup pool before its creator exits, including during unwinding. These tests
do not resume a compiler or admit a deployment. The scoped root observation view
uses safe pidfd/procfs wrappers; its original thread/account and unreaped custody
checks grant neither compiler admission nor tree supervision.
The shared pipe framer and socket-mode normalization use safe descriptor
APIs. This inventory does not establish installed production integration, an
authorized compiler occurrence, protected proof execution or GPU qualification.

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

The shared x86-64 secure entrypoint restores confinement before inspecting the
kernel's initial argument table. It requires one nonempty, bounded argv0 and an
empty initial environment, then writes a private atomic once before libc startup.
No public setter exists. Rust admission requires that observation and rechecks
bounded command bytes; it never restores dumpability to read root-owned procfs
environment data. This extends the existing reviewed assembly boundary without
adding an unsafe Rust site or granting service identity or proof authority.

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

### Nullary Typed Kernel Rejection Fixture

The test-only `fe2o3-macros/src/zero_argument_typed_v1_tests.rs` contains one
`unsafe fn` inside `parse_quote!`. This is parsed syntax supplied to
`validate_typed_kernel_profile_v1`, which must reject it even when the kernel
has no arguments. The declaration is never compiled as a function or called;
there is no unsafe memory, OS, FFI or GPU operation. Replacing its signature
with safe Rust would remove the source-safety negative being tested.

The inventory counts macro-template syntax once, so this file receives exactly
one `function` entry. The tokenizer, validator, fixture and every other
inventory entry are unchanged. Run the nullary macro tests and the complete
source-inventory gate. This reconciliation grants no runtime or launch authority.

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

### Fixed One-Stop Debug Target

The separately reviewed one-stop sibling adds two consuming unsafe functions:
`Gfx950DebugExecutionPreparationV1::prepare_fixed_one_stop` and
`Gfx950DebugOneStopPreparedV1::publish_at_owned_checkpoint_once`.
The first retains the original isolated disposable-process, no-foreign-runtime,
no-injection and exclusive native-actor obligations for the entire lifetime,
including failure and process teardown. The second additionally requires the
actual SAME native debugger client to join this exact owned process, source
checkpoint, executable, object, queue, packet and metadata before the first
publication effect. Actual attach-before-runtime, LoadedSuccess, a sole
successful event_processed acknowledgment, reviewed trap/CWSR/TTMP setup,
sampling exclusion and an exact one-shot pre-resume gate are caller obligations.
The client and supervisor must retain control through the unique debug stop,
separately qualified completion-only resume and bounded local retirement.
A JSON record, source digest, runtime-enable return or breakpoint hit cannot
prove those conditions. The source checkpoint itself does not enforce them.

One new unsafe attribute, `#[unsafe(no_mangle)]`, gives the host-only
`fe2o3_gfx950_one_stop_prepublication_checkpoint_v1` rendezvous its fixed,
unique symbol. The safe marker merely passes a borrowed record pointer to
`black_box`; it never dereferences a caller pointer, reads an acceptance
flag or grants native authority. The record stays in the original private Box;
all object addresses, packet bytes, signal BASE and separate VALUE+8 come from
actual retained owners. ABI/symbol collision and native debugger interaction
remain reviewed external boundaries, not properties established by the marker.

There are no new raw-memory or ioctl blocks in this donor. It reuses the existing
bounded memory/AQL/native primitives under retained custody, with INVALID body
before the single release header and one doorbell store. Completion comes from
the actual initialized signal and queue frontiers, not a report. Every failure
or unwind retains uncertain resources; all nine allocations retire before the
distinct same-owner terminal witness, and owned descriptors close last.
The original deadline is rechecked after descriptor Drop; late refusal preserves
the fact that those descriptors closed without attempting to close them again.

The exact inventory delta is two functions (one preexisting plus one new) in
`engineering_gfx950_debug_execution_v1.rs`, one function in
`engineering_gfx950_debug_one_stop_owner_v1.rs`, and one attribute in
`engineering_gfx950_debug_one_stop_checkpoint_v1.rs`.
No other inventory allowance changes. Comments and compile-fail doc examples
are excluded by the actual tokenizer; the checkpoint attribute is included.

This policy review does not qualify one-stop queue publication, a debug stop,
same-client acknowledgment or physical sampling. Earlier packet-incapable
EmptyQueue qualification and static fixture/startup results do not transfer
to this new owner or any future debugger build. Run the source-inventory,
engineering library, doctest and strict Clippy gates before any separately
reviewed native qualification. No native invocation was performed for this
inventory reconciliation.

### Opt-In Engineering Wait and Token-Program Entries

The entries added in `807f0bef7` preserve the expert disposable-process,
selected-device consent, trusted machine-code and terminal-error obligations.
The legacy entry now forwards through one private unsafe policy selector; two
separate public unsafe entries select bounded active polling or token programs.
These are the three additional unsafe functions and three forwarding blocks
in `engineering_gfx950.rs`, taking its inventory from eight/five to eleven/eight.
Safe argument validation still cannot establish the supplied machine code's
actual memory accesses, ABI compliance or termination, so the unsafe boundary
must remain explicit. No pointer, mapping, ioctl or kernel-trust obligation is
removed or discharged by these wrappers.

Root reviewed the three exact forwarding blocks, the shared worker lifecycle,
the retained token-program owner and the ordered wait/publication delegation.
Policy is selected before Ready and cannot change during the worker lifetime.
The default remains 50us sleep with token programs disabled. Active polling
keeps the original completion/currentness checks and aggregate deadline, uses
a bounded 10ms window, and rejects timestamp profiling. Token registration
retains descriptions rather than queue packets; each execution validates all
arguments before any group publishes and shares one aggregate deadline.
Resource mutation requires releasing the registered program. Partial execution
is terminal, poisons the ordered path and retains uncertain native owners until
disposable-process teardown; it is never a retry or safe-launch guarantee.

This reconciliation changes only this one inventory entry and review records.
Run the source-inventory gate, engineering host tests and compile-fail doctests;
CPU evidence does not qualify native GPU execution or protected authority.

### Conditional Invocation Binding Comparison Fixture

Peer commit 5e1f9c3441066f13f5d501a95349a8a99fe886be adds one test-only unsafe
implementation in conditional_authority_tests.rs. The private
BindingComparisonFixture is compiled only as a child of authorized_execution
under cfg(test). Its constructors and all uses remain in three comparison
tests; none supplies it to an execution entrypoint or constructs a checked
device, prepared dispatch, telemetry token, or native request.

An unsafe impl is necessary to exercise the actual generic
validate_authority_bindings_v1 helper without duplicating its behavior or
weakening its unsafe trait bound. The fixture returns inert identity values
and updates a local Cell counter; it performs no raw memory, OS, FFI or device
operation. Tests check that family/payload mismatch precedes currentness,
that matching values still require currentness, and that contract/device
mismatches still refuse. This retains the existing private preflight-test
pattern, not a new execution-authority constructor.

These fixture values do not authenticate a Worker V3 publication or satisfy
the trait contract for native execution. Safety depends on reviewed private
test-only confinement and the complete call-site census, not the hashes,
fixture name, a test-configuration exemption, or the inventory itself.
Any future escape or execution-entry use requires a new safety review even
if its count remains one. Real authority still requires the unchanged
verifier, invocation, device and retained-currentness obligations.

Only this file receives impl: 1 in the inventory. No unsafe syntax, production
implementation, tokenizer, gate or other allowance changes. After integration,
run the conditional-authority and existing identity-comparison tests, runtime
compile-fail doctests, and the full source-inventory gate. This reconciliation
grants no runtime, native, GPU or protected authority.

### Fallible CPU Codec Node Allocation

The private `Scope::boxed` helper in
`fe2o3-verifier/src/portable_reference_v1/codec.rs` adds one unsafe block.
The codec must prepay storage and report allocation failure on the original
budget. It uses the stable fallible Vec allocator rather than adding an unstable
allocator API requirement or making an infallible Box allocation.

The helper rejects zero-sized types, reserves exactly one element, verifies the
returned capacity, and initializes exactly one value. The resulting boxed slice
and that value have identical allocation size, alignment, and global allocator.
The raw cast removes only slice-length metadata; ownership transfers once into
`Box<T>`, with no surviving alias or duplicate destructor. Failure drops the
original value and allocation before the enclosing scratch reservation is
released. The code comment records these obligations at the conversion.

Review covers the complete private helper and its recursive-expression callers.
Tests cover zero-sized refusal, over-alignment, a single destructor on normal,
error and unwind paths, and budget cleanup. Run the CPU codec tests and the
source-inventory gate after integration. This allocation primitive authenticates
no CPU source, proof, compiler artifact, or GPU launch.

### Shared Compiler-Execution Input Slots

The selected compiler admission and the native V3 preparation share one private
guard for inherited policy slot 202 and service slot 195. Both must be live and
non-CLOEXEC before policy duplication: otherwise the duplicate could occupy a
missing service slot and acquire a second closer. Admission exclusively consumes
these protocol slots. The two production unsafe blocks inspect scalar F_GETFD
flags and close an owned slot once; neither fabricates a BorrowedFd or OwnedFd
for a potentially absent input. Ownership is cleared before closing or passing
the service slot to its consuming client, and close is never retried. The native
path still charges its original account before inspecting either descriptor.

Three test-only unsafe blocks cover isolated-child pre-exec cleanup, F_GETFD
absence checks, and adoption of one installed live descriptor after disarming
the guard. Pre-exec performs only scalar close/error handling on child copies.
Each child has a finite deadline and is reaped on timeout or polling failure.
All descriptor duplication uses the existing rustix safe API without replacing
live owners. The native test now shares those helpers and has no unsafe blocks.
Tests cover missing/CLOEXEC inputs, policy and client refusal, the actual
descriptor-allocation collision, unwind cleanup and transferred ownership.

The inventory adds only the three reviewed test blocks. The selected production
file remains at two; the native production and test files now contain none.
Unrelated inventory mismatches are not approved by this entry. These refusal
tests grant no protected execution, native activation or GPU qualification.

### Native Process Boundaries

The native compiler-execution and external-anchor startup families need raw
descriptor operations at their isolated-process entrypoints. Their callers must
transfer exclusive ownership of the fixed input slots; inspecting a descriptor
or authenticating its bytes does not establish Rust I/O ownership. Validate the
whole input table before opening or duplicating files. Duplication leaves the
source owned until the guard is disarmed immediately before its consuming close.
Close errors never permit a retry, including during unwinding. Resource refusal
must obey the same ownership contract as successful admission.

Rustc's dynamic loader does not provide that transfer contract: it can construct
multiple backends after initializing threads. The safe backend factory therefore
only duplicates slots into new close-on-exec owners, without adopting or closing
the originals. Scalar `fcntl` errors do not fabricate borrowed descriptors. The
original slots remain process/caller-owned until compiler exit; successful
capture marks both close-on-exec. Failed capture can leave originals inheritable
and does not assume their cleanup obligations. Successful child completion is
required before finalization. An owned, mutex-protected input is consumed once
per backend instance by the later safe callback, not once per process.
Capture errors retain no vacancy reservation, and repeated loading never closes
another backend's descriptors. Consuming client/host intake APIs remain unsafe
and require a real exclusive transfer. This distinction adds no execution or
signer-policy authority.

Sealed-image inheritance and the Cargo binding-wrapper hooks retain their source
owners through spawn. Child hooks use descriptor syscalls and scalar error
handling without allocation or locking. Coordinator argv/environment intake
requires stable readable backing; NSS calls use matching buffer lengths, check
both status and returned-pointer identity, and copy only scalar account IDs.
Signal-mask storage is initialized and thread-affine, with restoration following
successful cleanup.

Native supervisor startup destructively closes unrelated descriptors only under
its dedicated-process contract, preserving exactly slots `3..12` and `220`.
Readiness transport uses initialized, aligned control storage and bounded payload
lengths. Received `SCM_RIGHTS` and rejected `SCM_PIDFD` descriptors acquire distinct
owners immediately and are dropped on refusal. GNU/musl ancillary fixtures zero
initialize the pinned integer-field header, including musl padding, before
bounded writes. Synthetic ancillary tests do not establish actual `SCM_PIDFD`
reception on a deployed kernel.

The protected-service spawn and supervisor cleanup bridges reserve custody
before clone and adopt the returned PID, pidfd and spawn lease before fallible
parent work. Post-clone callbacks are finite and allocation-free; credential
changes re-arm and check parent-death signaling. Exec confirmation releases only
the spawn lease. Pending or quarantined children retain dependencies; only a
consuming terminal wait permits direct-child retirement. Fresh-domain custody
additionally requires observed aggregate emptiness and successful removal of
the exact retained cgroup. `WNOWAIT` and `ECHILD` are not terminal
evidence. See [cleanup custody](compiler-execution-cleanup-custody.md).

The compiler-only pre-exec restriction module retains one unsafe function/block.
It consumes the actual child's pinned personality observation before three direct
`prctl` calls install/check the immutable 57-instruction filter and its live
native ABI header. The kernel copies the header and filter synchronously; no
allocation, lock, callback or borrowed pointer escapes the child call.

The private `native_compiler_personality.rs` module has nine blocks and six
unsafe functions. After required namespace mapping, before credential and
dumpability changes, it opens verified procfs directories and the calling child's
personality inode using bounded, no-symlink paths. The proc thread-self coordinates
must match actual getpid/gettid. The retained descriptor survives the profile
transition; a bounded read must contain exactly eight hexadecimal digits and a
newline, followed by EOF, with READ_IMPLIES_EXEC absent. Identity, filesystem,
read, format and close failures refuse; there is no personality-syscall query
fallback, supplied descriptor or parent snapshot. Each private descriptor is
consumed once, including abort paths, without retrying close. The caller excludes
foreign FD and mount mutation and retains the original child identity.

The original parent prepays `native_compiler_personality::{WORK, SCRATCH}`
through `native_work::COMPILER_RESTRICTION_WORK`, the compiler restriction
scratch constant and `StagedProtectedServiceExecV2::SPAWN_SCRATCH` before clone.
These are logical work/storage charges, not process RSS or syscall deadlines.
Acquisition refusal is stage 15, post-profile observation refusal is stage 16,
and compiler-filter installation refusal is stage 13; each precedes READY and
exec and retains the original cleanup owner. Ordinary service stages do not
install the compiler filter. This observes pre-exec state only, not personality
established by ELF exec, complete W^X, immutable backing, descendant confinement,
source/output enforcement or runtime admission.

Twelve diagnostic blocks in `native_compiler_restrictions_exec_tests.rs`
stage/clone retained inert inputs, query/set/restore isolated creator state,
inspect initialized scalar buffers and install outer test-only denial filters.
Restorable controls arm their guard before mutation; dirty locked controls
terminate their disposable process without clearing the state. Exact-test
subprocesses isolate irreversible filters and dumpability changes. The controls
distinguish acquisition, profile, record/EOF and installation failures; neither
an earlier refusal nor a filter supplied by the host qualifies the intended
boundary. Each native selector requires its stated isolated deployment and
static `-pthread` fixture. Execution evidence remains separate from this review.

The unsafe sites in eleven reconciled inventory entries are inherited from
public `8ecfb2a5e3af052fbd36034c73b51ab0fca7804f`. The review also repaired the
root-request subprocess test to prepay `CreatorScope::CONTROL_WORK` before
scope entry, as the unsafe API requires. The production entrypoint already
prepays this control allowance before constructing its creator. In the
coordinator, the reviewed entries are
`native_compiler_attempt.rs`, `native_compiler_attempt_tests.rs`,
`native_entrypoint.rs`, `native_root_request.rs`,
`native_root_request_preexec_process_tests.rs` and
`native_root_request_process_tests.rs`. Original request ownership is installed
before fallible preparation; helper/child backing enters the original funded
cleanup slot before clone. Original work lifetimes, received objects, exclusive
consuming waits and outside whole-domain custody remain required. The compiler
exec gate stays closed. The attempt test's unsafe-function site is a signature
assertion, not an unsafe call.

The other five entries, in protected-service spawn, are
`native_compiler_personality.rs`, `native_compiler_restrictions_exec_tests.rs`,
`native_compiler_spawn_tests.rs`, `native_namespace_restrictions.rs` and
`native_cgroup_mount_tests.rs`. The namespace filter uses one unsafe function
and block with a synchronous immutable native ABI program; compiler and actually
mapped children install it before READY, with stage-14 failure, while unmapped
service creators retain their existing behavior. Inert staging tests launch no
child. Mount controls use live C strings and change only a disposable process's
unshared, private-propagation mount namespace. Counts add 32 blocks and ten
function syntax sites; no production behavior or test gate changes. This static
review does not execute or qualify the ignored native controls, establish
deployment provenance, or grant compiler, proof or GPU authority.

The optional native fresh-domain spawn extends that same clone path with
`CLONE_INTO_CGROUP`. It reserves rollback custody before mkdir and transfers the
actual domain, atomic pidfd, spawn lease and original cleanup slot before any
fallible parent check. Its unsafe caller must exclude privileged cgroup/mount
writers and must not export cgroup controls or allow child migration/delegation.
No caller path or descriptor constructs a domain owner. Root numeric credentials
and cgroup placement alone establish neither trusted deployment origin nor
proof-process isolation. The child remains nondumpable; production proof gates
are unchanged. One additional unsafe mechanical entry point and two ignored
root-diagnostic call sites are reviewed here, not qualified by their counts.

Fixed-slot, credential and process-transition fixtures run in isolated children.
Their source descriptors stay above the destination slots; raw adoption happens
once after successful creation. Polling failures and timeouts must kill and reap
the test child. Descriptor-free cleanup fixtures exercise only the documented
inert exception. The shared issuer launch-input tests retain the same obligations
after their V2-specific filename is removed. The compiler image-budget fixture
uses safe `rustix` memfd creation and needs no raw-descriptor adoption.

Inventory reconciliation records moved implementation sites, shrinking the old
sealed-image, client, provisioner and service allowances alongside their new
modules. Macro/include sites are counted once where written. This review covers
OS ownership and unsafe-call contracts, not protected execution, proof-policy
admission, native production activation, or GPU qualification.

## Retained Proof Controller

The gated proof launcher performs raw fork only after preparing every descriptor,
C string and argument pointer. Its child performs syscall-only setup and exec;
it must not allocate, unwind, acquire inherited locks or run Rust destructors.
Successful pipe creation transfers each descriptor into exactly one owner.
Current-thread observations use argument-free `gettid`; retained task identifiers
are not authority to act after their terminal wait has been consumed.

Stable inspection initializes register storage before ptrace writes it and
checks status before interpreting the result. Task custody is published before
fallible parent work; unresolved creation, cleanup or unwind remains retained
in the original process. This does not establish external-writer isolation.
The artifact spawn lease must separately survive the entire pre-exec interval,
including failure quarantine: a safe pointer/descriptor inventory cannot replace
that lifecycle obligation.

Socket, mapping, stable-controller and quarantine fixtures run their raw process
operations in owned diagnostic domains. Ancillary buffers are initialized and
bounded. The memory-policy fixture owns one private, writable, non-executable
mapping, mutates one valid byte, and unmaps it once; it never executes that data.
Fixture kill/reap and raw syscall operations provide test observations, not
production launch authority. The reviewed eight-file inventory contains 53
unsafe blocks and eight extern blocks, including 29 test-only blocks. Its old
controller allowance shrinks from 17 blocks to 15 as operations move into the
private spawn and inspection modules. Counts do not qualify protected execution.

## Native Compiler Exec and Root Trace

The compiler mode in `fe2o3-protected-service-spawn` uses the existing raw
clone/profile/gate/exec path. One unsafe staging function requires authenticated
input custody and full overlapping charges. Owned CString buffers and frozen
pointer tables survive source drops and stage moves. The existing child syscall
block adds `fchdir` and exact argv/environment selection; no second launcher or
unsafe Send implementation is introduced. Standard destinations preserve shared
open-file status and offsets; `Some` clears descriptor CLOEXEC. The caller must
map captured absent or original-CLOEXEC streams to `None` for post-exec absence.

Root tracing adds one scalar ptrace block and one unsafe exec-confirmation
function/block. The retained wrapper adds one forwarding function/block. The
controller consumes the existing child, pidfd, cleanup slot, artifact lease and
backing; it binds operations to the original thread, Budget address and ledger.
Its fixed TRACEEXEC/EXITKILL options introduce no descendant or TRACEEXIT stops.
Only the same owner's consuming terminal wait notifies cleanup. Exec observation
alone does not release the lease: confirmation additionally requires the caller
to authenticate native exec and closure of every inherited artifact-lock alias.
It does not establish process-tree isolation or compiler authority.

`fe2o3-process-identity/native_capture.rs` uses three blocks for scalar
standard-slot probes, CLOEXEC duplication above FD2, and adoption of newly owned
descriptors. Capture is safe, inert and non-atomic: concurrent slot replacement
can mix observations, while shared flags, offsets and contents remain mutable.
It neither creates a borrowed lifetime for a raw standard slot nor grants
invocation provenance. The authenticated native stage's obligations are separate.
Cwd access borrows the existing pin without reopening
its pathname. Inode/flag comparison is not evidence of OFD provenance.

Reviewed test-only additions are 12 capture blocks, two compiler staging blocks,
two native-exec blocks, and nine trace-confirmation blocks. Isolated subprocess
tests cover absent/CLOEXEC streams, offsets, partial capture failure, actual
trace waits, short funding and retained cleanup. The explicit root diagnostic
uses a static C fixture through the production clone path, with no compiler,
proof or GPU authority. The C fixture checks inputs and descriptors and exits;
its libc boundary remains part of the diagnostic, outside the Rust inventory.
No unrelated inventory allowances change.

## Wrapper Input Custody

The cwd hook now owns its CLOEXEC duplicate through Command's lifetime, including
reuse and failure, instead of capturing a borrowed raw descriptor. The existing
unsafe block still performs only pre-exec fstat/fchdir; registration can refuse
before changing Command. Six test-only blocks cover descriptor probes and an
isolated soft-limit guard.

The binding wrapper captures actual runtime-sanitized entry stdio before opening
files, and retains it with protected parent invocation custody. Two installation
blocks perform raw standard-slot probes and register a closure owning high-FD
duplicates. Only bounded descriptor operations run before exec. Absent or
original-CLOEXEC slots close; other streams preserve shared offsets/status flags.
The wrapper keeps current parent slots open so pinning and Command's error pipe
cannot occupy a slot the final hook replaces. Capture and sampled revalidation
do not prove immutable aliases, pre-runtime inheritance or compiler authority.

The child-channel transfer now requires kernel message credentials matching both
the declared child PID and the service socket's creation credentials. No new
production unsafe block is needed. Its relay test registers one existing scalar
send helper in a child-only pre-exec hook. The original same-UID/GID relationship,
pidfd liveness and protected-runtime refusals remain unchanged.

## Compiler Trace And Key Transfer

The private compiler-channel trace adds one unsafe confirmation function and one
forwarding block. It preserves the existing original-thread/account trace and
requires the caller to establish exact native exec-status EOF and closure of all
inherited artifact-lock aliases, including any in untraced descendants. Its
diagnostic adds one call block while the non-forking native stage's first exec
is held. Confirmation releases only the spawn lease, not compiler authority.

The fresh service-owned V3 key transfer adds no production unsafe site. Its
isolated receiver test adds two scalar libc blocks to drop real/effective/saved
GID then UID before exercising the unchanged receiver admission. The test requires
an exact single-test invocation in a disposable process with CHOWN/SETGID/SETUID;
it is not a shared-process credential API. The root template remains unchanged,
and a separate read-only sealed image is assigned the deployment's credentials.

## Initial Reduction

The initial audit of `d9f6bbcd0` found 1,924 source sites in 288 Rust files:
1,026 in non-test crate source across all configurations, 874 in tests/fixtures,
and 24 in examples/benchmarks. This includes legacy qualification code, not just
the default production build. The checked-in baseline records the current
post-reduction counts without requiring a GPU to reproduce them.
