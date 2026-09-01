# cargo-fe2o3

`cargo-fe2o3` coordinates fe2o3 build, binding-only host
checks/tests, inspection, and debugging workflows.
The adjacent `fe2o3-rustc-wrapper` is fail closed for compile invocations while
its trusted execution boundary is built incrementally.

## Protected authority release

Authority-bearing builds must enter through
`cargo fe2o3 authority release <build|run> [args]`. Direct `build` or `run`
requests that select an authority profile still fail before Cargo unless a
debug-only non-production validation escape is explicitly enabled. The
production release command rejects that escape.

The outer release process requires descriptors 0, 1, and 2 to be the only
inherited descriptors; exactly one additional descriptor may be the live
`/proc/<pid>/fd` enumeration directory opened by that check. It pins its
running executable object and bytes, copies the same bytes into a fully sealed
memfd, pins the exact cwd object, measures
the declared Cargo, static binding trampoline, running binding wrapper, rustc,
rustc runtime tree, and backend pins, constructs the canonical
`CompilerClosureV2`, and snapshots the complete raw argument vector and
environment. The closure also binds transition protocol version 1 and derives
its aggregate identity from that protocol and the six ordered pins. It
then admits the sole root-owned canonical compiler-execution client profile at
`/etc/fe2o3/compiler-execution/client-profile-v1` and executes the sealed image
with fixed contract, control, launcher-image, cwd, and client-profile
descriptors. The V3 release contract identity covers the profile descriptor
object and complete profile identity. The child independently seals and
revalidates that profile, checks its own image, its live
parent's PID/start time/uid/image, the retained backing objects, exact argv,
environment, cwd, descriptor manifest, and compiler closure before completing
a fresh two-way one-shot grant. Before exec, the child arms `PR_SET_PDEATHSIG`
with `SIGKILL` and immediately verifies the expected parent PID to close the
setup race; after exec it verifies both that setting and the launcher's exact
PID/start identity. The protected child applies the same race-free boundary to
its pinned Cargo subprocess. Therefore the admitted child and that Cargo
process cannot continue when their respective admitted parent dies.

Release starts from a cleared environment. The complete allowlist is `CARGO`;
the backend, Cargo, binding-trampoline, rustc-path, rustc, and rustc-runtime
`FE2O3_AUTHORITY_*_V1` inputs; `FE2O3_BACKEND`, `FE2O3_TARGET`, optional
`FE2O3_PRODUCTION_BUILD_CONFIG_V1`, `LANG=C`, `LC_ALL=C`, and `TZ=UTC`.
Aliases, extra variables or descriptors, loader variables, rustup/tool
selectors, noncanonical paths, changed backing objects, replayed attempts, and
closure/runtime-tree drift fail closed. Tool digests are operator-provisioned
inputs and are remeasured; no machine-specific digest is compiled in.

Production has no pipeline selector. `FE2O3_CODEGEN_PIPELINE` and
`FE2O3_QUALIFICATION_ORACLE_V1` are rejected in every `cargo-fe2o3` build.
The Cargo package has no qualification feature, simulation command, Worker V2
compiler dependency, or feature-selected compiler behavior. Backend-only
differential oracles must remain outside this executable transaction.

Production accepts exactly one versioned configuration namespace. The existing
`FE2O3_PRODUCTION_BUILD_CONFIG_V1` names a canonical
`fe2o3-production-build-config-v1` recipe and retains its original behavior.
`FE2O3_PRODUCTION_BUILD_CONFIG_V2` names the same workload-neutral link recipe
with the required field `"observation":{"kind":"source-isa-summary-v1"}`.
The matching expected-identity variable is
`FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V1` or
`FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V2`; versions and namespaces cannot be
mixed. Both recipes pin selected Rust compilation units, the upstream-LLVM
worker image, typed link providers, link options, output bound, and execution
limits. Production rejects the retired Worker V2 namespace. Envelope,
exact-workload, and restart-oracle fields are not part of either schema.

V2 adds authority-free Source/MIR/KIR-to-sparse-ISA acceptance telemetry to
the same protected Worker V3 transaction. The wrapper releases ordinary V1
broker authority for dependencies and V1 units. For a selected V2 unit it
binds the observer release to the exact configuration, selected-unit identity,
and managed build attempt, and waits for a distinct prepared ACK on the same
authenticated stream before treating the authority as released. The broker
validates policy membership and its non-DIRECT session before that ACK; the
trusted wrapper supplies attempt legitimacy, and the submitted frame must match
the ACKed request exactly. Fresh finalized evidence is reduced once before
publication preparation. A genuine restart recomputes from the exact recovered
finalized evidence. A load-ready restart emits typed unavailable reason `202`
because its durable envelope does not retain finalized correlation evidence.
Observer reduction, framing, submission, and collection failures never grant
or revoke compiler, publication, load, launch, or runtime authority and do not
change Cargo success after the authenticated V2 release succeeds.

After every wrapper has exited, Cargo emits one bounded canonical collection on
stderr with prefix `source-isa-observation-collection-v1`, explicit
`frames`, `missing`, and `failure` counts, lowercase `encoding=hex:...`, and
`authority=observation-only`. The collection is deliberately not written into
the generated artifact directory because every file there participates in the
authoritative generation snapshot. See
[the frozen collection schema](../../docs/source-isa-observation-collection-v1.md).

The rustc wrapper owns all device work directly as a mandatory
`ManagedProductionBuild` transaction. No route enum, optional work slot, or
feature-gated alternate compiler transaction exists in the Cargo package.

Cargo dependency units that are not the selected kernel root are host-only
rustc compilations, not another fe2o3 route. The wrapper removes all managed
compiler arguments, the fe2o3 backend descriptor, qualification selection, and
artifact custody before launching rustc's built-in LLVM backend.

`cargo fe2o3 authority release probe` exercises this exact launcher/handoff
boundary and exits before Cargo, artifact generation, HSA loading, or GPU
dispatch. A successful probe grants no compiler-origin, proof, artifact-safety,
runtime, or GPU authority. A separate integration fixture crosses the admitted
boundary into pinned Cargo and the existing transactional test backend, but
its publication evidence carries only that fixture's existing downstream test
authority. The release contract makes no stronger claims.

### Compiler provenance wiring

For protected builds, the capability broker sends a sealed raw
`CompilerClosureV2` capability and the exact sealed compiler-execution client
profile to the binding wrapper. The wrapper revalidates both, captures the
exact prepared V2 rustc process and child environment, and upgrades that
in-memory observation to
`RustcInvocationDescriptorV3`. The rustc and backend pins duplicated by V2 and
the closure must match.

The wrapper seals the canonical V3 bytes and installs that exact immutable
image at fd 199 for the prepared rustc child. The raw brokered closure stops at
the wrapper and is only an input to V3 construction. The backend revalidates
the inherited V3 descriptor against the live process before collection and
moves the exact invocation custody into Worker V3 publication. This closes the
Cargo-to-backend transport boundary; proof promotion and runtime authorization
remain separate downstream gates.

For the selected kernel root only, the wrapper also derives and seals the exact
issuer policy at rustc fd 202 and creates the compiler-service endpoint at
rustc fd 195 after fork. The parent binds that endpoint to the spawned rustc's
credentials and live pidfd, transfers both to the sole fixed
`/run/fe2o3/compiler-execution-supervisor.sock` endpoint after authenticating
the profile's distinct supervisor UID/GID, and requires one canonical
issuer-readiness record plus EOF. The backend publishes the exact V3 handoff,
derives its complete compiler-execution subject, acquires and independently
decodes the signed carriage, and durably stores its exact bytes in the same
attempt slot. Cargo admits that sidecar against retained readiness and the
sealed profile under the handoff currentness lock, then carries it through
fresh execution, finalized-HSACO recovery, V2 readiness persistence,
application descriptor transfer, and `fe2o3-host` admission. Top-level V1 load
envelopes are rejected on this production route. Any post-spawn handoff or
readiness failure kills and reaps rustc before invalidating the build attempt.
Host dependency units receive none of these descriptors. The carriage remains
authority-free until protected verifier policy and rollback admission.

## External Cargo projects

`cargo fe2o3 authority release build` and
`cargo fe2o3 authority release run` operate from standalone Cargo projects and
workspace members. Production constructs one fixed two-phase
plan: it first runs Cargo `build` for `amdgcn-amd-amdhsa` through the fe2o3
device compiler, commits the exact generated-artifact generation, and then
runs the requested `build` or `run` for the pinned rustc host target with
ordinary rustc. The host phase has no fe2o3 backend, wrapper, capability
broker, device target, build manifest, qualification, or simulation controls.
Package, feature, profile, manifest, and target-directory arguments apply to
both phases. Arguments after `--` on `run` apply only to the host application;
`build` does not accept an application argument suffix.

The orchestrator owns both targets. Callers must not pass `--target`; target
configuration cannot create another compiler route because each phase receives
its exact target on the Cargo command line. Cargo receives all other original
platform arguments without shell construction or UTF-8 conversion. A separate
bounded `cargo metadata --no-deps` probe resolves the selected
`--manifest-path`, workspace root, and configured target directory. An
explicit `--target-dir` has the same invocation-directory-relative
interpretation as Cargo and takes precedence over metadata;
`CARGO_TARGET_DIR`, repeated `--config`/`-Z` arguments, and
locked/offline/frozen routing are reflected by metadata.

The invocation directory, workspace root, target directory, and
`<target>/fe2o3` output directory are opened without following path-component
symlinks and retained through the operation. On Linux, generated output is
passed through a validated fixed `/proc/self/fd` directory reference. The
backend is copied into a sealed read-only memfd and installed at a separate
fixed child descriptor, so the measured bytes are the bytes selected for
rustc. Path substitution is checked before and after Cargo runs.

Generated output carries an atomic deletion guard and a bounded generation
marker that binds the sealed backend digest, transitive build inputs,
target, effective Cargo build/target/profile configuration, inherited codegen
environment, rustflags, and a snapshot of the generated artifact tree. Cargo's
own environment and configured rustflags remain intact. fe2o3 passes its
backend selector and generation cfg separately to the trusted rustc wrapper,
which rejects response files and any preexisting codegen-backend selector.
Configured or inherited outer rustc wrappers are rejected rather than composed.
A private target-scoped lock serializes preparation, Cargo execution, and
marker publication. Stale or failed owned generations allocate a new Cargo
fingerprint and remove only the opened
`<target>/fe2o3` directory; a missing or malformed deletion guard fails
closed without deletion. Successful incremental builds republish their exact
snapshot. Unrelated host outputs remain available for normal Cargo reuse.

### Offline CPU simulation

`cargo-fe2o3` has no `simulate` command in any feature configuration and does
not depend on `fe2o3-kir-sim-cli`. Hardware commands never fall back to CPU
execution. Simulation is an offline operation over an existing canonical KIR
V7 module and a bounded typed request:

```console
cargo run -p fe2o3-kir-sim-cli --bin fe2o3-kir-sim -- \
  --kir-v7 kernel.kir --request request.json --output result.json
```

`fe2o3-kir-sim-trace` emits deterministic logical thread, wave, and workgroup
observations for the same execution.

This route is independent of source compilation and grants no source-to-KIR,
compiler, refinement-proof, artifact, runtime, performance-prediction, or GPU
authority. It does not initialize HIP, HSA, KFD, DRM, or a GPU. Unsupported KIR
types or operations fail closed.

### Bounded rocprofv3 profiling

`cargo fe2o3 profile` is dry-run only unless collection is separately and
exactly authorized. A plan measures the exact `rocprofv3` script, native
Python interpreter, installed ROCProfiler SDK tool
and core libraries where they use the reviewed ROCm layout, the native target
ELF, the fixed semantic collector configuration, the cleared and bounded
environment, and stable device records read directly from KFD sysfs topology.
The semantic configuration identity excludes output routing and target launch
authority so independently routed captures remain comparable; the collection
authorization binds both. Planning creates no output directory and executes
neither collector nor target. Collection requires `--collect` together with
the exact lowercase digest printed as `collection-authorization`:

```console
cargo fe2o3 profile --kind dispatch-json \
  --output-dir /absolute/new/profile-output -- /absolute/target argument
cargo fe2o3 profile --kind dispatch-json \
  --output-dir /absolute/new/profile-output \
  --collect --authorize-collection <plan-sha256> -- /absolute/target argument
```

The output directory must be new. The collector receives the target as an
exact argument vector without a shell, under a fixed timeout and stdout,
stderr, file-count, depth, and total-storage policy. The orchestrator creates
the directory with mode `0700`, retains an ownership guard, rejects symbolic
links and non-regular artifacts, records deterministic content identities,
and removes its owned directory after spawn, timeout, output, collector, or
artifact-validation failure. Successful collection retains
`fe2o3-profile-manifest-v1.txt` plus the bounded collector artifacts.

`--kir-sha256`, `--kir-len`, and `--wave-width` make the dry run print the
exact `fe2o3-profiler-import` Bundle V4 argument vector. With rocprof's
`--agent-index absolute` configuration, each device argument binds the emitted
agent ID to the stable direct-KFD identity for that same KFD node; import joins
by that ID and does not depend on device-vector or first-dispatch order. The
collection authorization covers both the node number and stable identity, and
the complete mapping is re-observed immediately before and after collection.
Any remap observed by either check fails closed and the owned output is
cleaned. ATT import
is deferred until the output directory identifies the selected absolute agent
and every manifest-relative artifact has been content-bound. The source
manifest or dispatch file must also fit the importer's 8 MiB source limit;
larger collected artifacts are retained but labeled non-importable. The
profile target is not treated as proof of an executed kernel code object, so
cross-run duration deltas remain unavailable until a separate kernel artifact
identity is supplied. The resulting Bundle V4 is queried with
`fe2o3-profiler-query`.

The orchestrator itself has no HIP or HSA runtime dependency. `rocprofv3`
injects ROCProfiler SDK into the target, however, and its installed option
surface does not prove that it can observe dispatches submitted directly via
KFD. The plan identifies the four direct collector entry objects above but
labels their transitive dynamic-library closure unavailable; it does not call
that record an authenticated complete installation closure. Collector success
and JSON/CSV/ATT-looking filenames remain explicitly unvalidated. Only
successful Bundle V4 import establishes the corresponding profiler record
shape; it does not grant compiler, runtime, or performance authority.

Generic CI runs the parser, planning, authorization, path-substitution,
bounded-output, cleanup, and fake-collector tests without discovering or
executing a host `rocprofv3`. Real GPU collection is intentionally excluded
from pull-request CI. It requires an operator-selected target, a new private
output path, and the plan-bound collection authorization on a protected GPU
runner.

Deletion guards are structural accident and substitution defenses, not
authentication. Their random tokens correlate an interrupted creation with
the directory completed by that operation, but every record is stored inside
same-UID-writable filesystem state. A malicious process running as the same
UID can forge or replace them and is outside this cleanup threat model.

### Trust boundary

The external Cargo path defends against pathname substitution, accidental
descriptor inheritance by Cargo, malformed compiler invocations, and
cross-generation publication. It does not sandbox the package being built.
The selected Cargo and rustc executables, Cargo configuration and environment
(including `CARGO`), the codegen backend and its dynamic dependencies, project
and dependency build scripts, procedural macros, native helpers, linkers, and
compiler-launched tools are trusted inputs. Same-UID processes are also trusted
where host isolation does not prevent their interference.

Cargo children inherit the broker endpoint, build session, wrapper path, and
managed compiler arguments. A hostile build script can deliberately execute
the same wrapper with a forged compiler command and request the brokered
backend and artifact descriptors. Procedural macros run inside a
descriptor-bearing rustc process after those descriptors are installed.
Process ancestry cannot
distinguish either case from an intended compiler invocation without trusted
Cargo or rustc cooperation. Same-UID process inspection or injection may also
cross the boundary where host policy permits it. The broker compares a request
with the expected build session, profile, and build-configuration identity.
The client and broker also verify Linux peer credentials and exact
`cargo-fe2o3` executable object and bytes, then authenticate the exchange with a
fresh challenge bound to a separate 256-bit broker secret. These checks reject
an arbitrary substitute broker and route downgrade, but they do not authenticate
Cargo's intent: a hostile build script can replay the same trusted wrapper, and
same-UID environment inspection, rewriting, or process injection remains outside
this boundary. Altered inherited routing values fail closed unless they still
name and authenticate the exact prepared broker exchange.

The artifact descriptor is opened with `O_RDONLY`, which prevents file I/O on
the directory descriptor itself but still grants namespace authority through
descriptor-relative operations such as `openat`. Treat it as writable
publication authority. The broker is dropped immediately after Cargo exits,
before generation validation and commit, to stop serving escaped descendants
during post-build processing.

Do not use this path to build untrusted packages. Supporting that threat model
requires an OS sandbox for build scripts and procedural macros or a redesigned
publisher that never grants compiler-side code a writable final-artifact
directory. The broker endpoint and session bind routing and build identity;
they are not bearer-secret authentication.

When `FE2O3_BACKEND` is unset, a source-tree build of `cargo-fe2o3` builds the
backend into `<selected-target>/.fe2o3-backend-build-v1`, passed to the child
through its own pinned descriptor. It never shares the fe2o3 source tree's
ordinary `target`. Packaged deployments without that source tree must provide
a built backend through `FE2O3_BACKEND`.

`cargo fe2o3 run` places a narrow application boundary in front of Cargo's
exact-target application. Production always requires the authorized locked
compiler closure and canonical Worker V3 load envelope; it has no no-envelope
mode and rejects an intermediate Cargo runner. Every application runner starts
with an empty environment; no `PATH`, `TMPDIR`, build control, or arbitrary
inherited variable is retained. Application paths and arguments remain
byte-preserving. Recursive cargo-fe2o3 runners, including aliases and hardlinks
to the same executable inode, fail closed.

## Cleanup

`cargo fe2o3 clean` removes only `<selected-target>/fe2o3`. It honors
`--manifest-path`, `--target-dir`, `CARGO_TARGET_DIR`, and Cargo target
configuration. `--dry-run` reports the exact opened directory without
removing it.

`cargo fe2o3 clean` never removes the selected target directory or any sibling
output. `--all-target` is rejected because fe2o3 has no authority to identify
those outputs as disposable; broader cleanup remains the responsibility of an
explicit standard `cargo clean` invocation. Package, workspace, exclude, and
other partial-clean selectors are also rejected because they do not map to an
unambiguous fe2o3 directory capability.
Destructive cleanup is supported only where descriptor-relative recursive
removal is available; symlinked or substituted selected paths fail closed.

## Production Worker V3 handoff flow

`cargo-fe2o3` contains one protected compiler handoff, worker transaction, and
application protocol. `FE2O3_PRODUCTION_BUILD_CONFIG_V1` selects exact Rust
compilation units and pins the upstream LLVM worker, typed providers, link
options, output bound, and execution limits. The wrapper authenticates the
manifest identity, protected rustc invocation, compiler closure, and managed
build attempt before consuming one semantic compiler handoff.

`ManagedProductionBuild` has only `Fresh`, `Recovered`, and `Ready` states.
Fresh work performs strict Worker V3 preflight and one-shot handoff consumption;
recovery reconstructs the same retained inputs without respawning rustc; ready
work reuses only the exact durable load envelope. V1/V2 state machines, restart
modules, workload parsers, fixture binaries, and `fe2o3-worker-v2-bundle`
dependency have been deleted from this package.

The worker uses pinned upstream LLVM target-machine APIs and in-process LLD.
It does not use COMGR or command-line `clang`, `llc`, or `ld.lld`. Independent
inspection must agree with the exact target, exports, descriptors, AMDHSA
metadata, compiler closure, and publication plan before finalization authority
can be consumed.

`cargo fe2o3 run` accepts only the canonical Worker V3 load envelope. Stale
Worker V2 filenames and environment variables remain recognizable solely so
they can be rejected before child spawn. The application integration feature
depends only on Worker V3/runtime fixtures; its fault-injection feature changes
timeouts and readiness coordination, never compiler selection.

The typed publication plan is derived only from retained inspection evidence.
Publication is attempt-scoped,
durable, digest-bound, and followed by managed attempt completion; exact
in-process retries recover the same publication without rebinding its inputs.

After finalized HSACO publication commits, the wrapper constructs the canonical
Worker V3 load envelope directly from the retained protected publication owner.
It persists exact replay custody, verifies load readiness, retires the matching
publication intent, and only then finishes the build attempt. Recovery repeats
those joins from durable V3 records without rereading an operator path or
respawning rustc. Attempt, compiler closure, receipt, payload, publication, and
envelope substitution fail closed; truncated or conflicting canonical files
are never replaced implicitly.

The published HSACO and load envelope remain inert. The envelope contains no
process-local currentness lease. Cargo binds the protected compiler closure and
publication lineage, but no production verifier yet authenticates the carried
Verus and machine-effect evidence for HSA use. Downstream admission must
revalidate the durable claim and acquire fresh process-local authority.

For required-envelope `cargo fe2o3 run`, Cargo retains the owner-controlled
artifact-directory descriptor, opens the exact canonical envelope with
`openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS|RESOLVE_NO_XDEV)`, copies the exact
initial application bytes into a bounded anonymous image, validates the copy,
and seals it against writes and size changes before deriving the handoff
identity. The child executes a read-only descriptor for that sealed snapshot;
later source-path or same-inode mutation cannot change the admitted image.
Cargo also retains a fresh current-publication lease. The child receives
read-only envelope and directory descriptors plus the bounded handoff values;
no pathname or external-HSACO fallback exists. The public ACK is protocol
completion only. Cargo retains and revalidates its private lease through the
handoff. Parent discovery and child revalidation both reject an artifact
directory with more than 4,096 visible entries before filtering publication
names; unrelated entries consume the same deterministic scan budget.

The initial image profile is ELF64 x86-64 static executable/static PIE. It
checks page-rounded virtual PT_LOAD mappings and enforces W^X over declared
segment bytes. This is not alias-level W^X over rounded file offsets: normal
static Rust binaries may privately map raw-disjoint bytes from one boundary
file page through RX and RW segments. The profile also validates static-PIE
dynamic metadata and relocations. At most one PT_TLS is admitted when it is
well formed and wholly owned by a writable, non-executable PT_LOAD; malformed,
executable-load-backed, or outside-load TLS is rejected. `no_new_privs` plus
seccomp admits only the controlled initial `execve`, denies later
`execve`/`execveat`, and denies fork/clone, namespace/session creation, and
io_uring. Missing kernel support for the required seccomp listener or
`close_range(CLOSE_RANGE_CLOEXEC)` fails launch without leaving a blocked
supervisor.

Application launches run in a dedicated supervisor process, not in the
Cargo-facing runner process. The frontend first locks one of 32 fixed per-UID
admission slots, starts the supervisor, and authenticates a bounded inherited
stream with a fresh challenge. Before Rust acquires either hidden-CLI
descriptor, the supervisor uses raw libc operations to prove that both numbers
are open, distinct, correctly typed, peer-bound, and members of the fixed slot
pool. It sets and verifies `FD_CLOEXEC`, duplicates each with
`F_DUPFD_CLOEXEC`, and closes the attacker-selected numbers before sending
READY. The inherited slot's open-file-description lock survives that adoption.

The supervisor becomes the application's actual parent, starts the seccomp
worker for required-envelope launches, and owns every application authority and
ACK descriptor. Immediately before the required-envelope application exec,
`close_range(CLOSE_RANGE_CLOEXEC)` protects all non-stdio descriptors.
Required-envelope launch then clears `FD_CLOEXEC` only for its exact envelope,
artifact-directory, ACK, and test-only readiness ABI.
The protocol channel, admission slot, seccomp-parent socket, evidence and build
directories, and unrelated Cargo descriptors cannot survive exec. Consequently
an application or orphan descendant cannot release or retain a slot. Saturated
admission still rejects a 33rd live supervisor before any application is
spawned.

The random challenge remains visible in the hidden supervisor argv and is not a
bearer secret or the sole protocol authority. A result is accepted only over
the inherited stream bound to the expected peer, and that stream is never part
of the application ABI. Reading the parent's command line therefore does not
let an application forge READY or a pending completion result.

Application teardown never performs a blocking child wait after an ACK,
validation, or containment failure. The dedicated process retains the `Child`
identity and uses `try_wait` plus process-group `waitpid(WNOHANG)`. Its one
cleanup worker is published only after thread creation succeeds, its retained
`JoinHandle` is monitored, panics are reported, and later admission fails
closed after worker death. The process itself finishes already-owned jobs if
that worker dies. If cleanup exceeds the frontend deadline, the supervisor
reports a precise pending result and remains alive after the frontend exits;
the kernel's configured child adopter then owns the supervisor status. The
supervisor releases its global slot and exits only after process and sandbox
cleanup become terminal. No additional helper process is created per retry.

Once the retained leader has been reaped and group wait reports `ECHILD`, the
process-group identity is permanently terminal. Sandbox-thread polling is a
separate state and never signals or waits on that numeric PGID again. Pending
and active sandbox drops only request shutdown and may join a worker that is
already finished; no drop or admission-timeout path performs an unbounded
join. A stalled sandbox worker retains the dedicated process and its slot but
does not block process containment or cause stale-PGID signalling.

This does not make Linux `SIGKILL` synchronous. A task stuck in uninterruptible
kernel sleep can remain unreapable indefinitely; its dedicated supervisor and
one fixed admission slot also remain indefinitely. The frontend still reports
cleanup pending within its deadline, but neither fe2o3 nor the kernel can
complete reaping until that kernel operation returns. If the frontend exits
while cleanup is pending, eventual supervisor-status reaping depends on the
host's kernel reparent/adopter configuration. The private `/tmp` admission
directory excludes other UIDs; hostile processes with the same UID remain
outside this boundary.

This no-fork/no-re-exec startup boundary does not constrain arbitrary
same-process behavior. The syscall profile still permits operations including
`openat`, `mmap`, `mprotect`, and `pwrite64`; code already running in the
process can perform in-process loading or self-modification where ordinary OS
permissions allow.
Dynamic HIP applications and their interpreter/runtime-library closure remain
out of scope and require a separate identity-bound broker design.

## Inspection and tool plans

`cargo fe2o3 inspect` performs bounded, read-only decoding of fe2o3 v1
manifests, artifact containers, bundle indexes, AMDGPU HSACO metadata, and
Source/ISA observation collections. Its output is descriptive only: inspection
neither loads code nor grants compiler, proof, artifact, launch, runtime, or
hardware-observation authority. Auto-detection uses validated wire magic, and
`--format` can require one exact decoder.

`--format source-isa-observation --output agent-json-v1 <path>` emits a typed
first-page response, including `invalid_collection` for malformed bounded
input. Without a path, `--output agent-json-v1` serves a bounded, stateful
JSONL stream with nonzero unique request IDs, monotonic response revisions,
explicit pagination cursors, and a flush after every response. Agents consume
this protocol rather than parsing the human presentation.

Without `--execute`, `cargo fe2o3 sanitize -- <program>` and
`cargo fe2o3 debug -- <program>` print normalized ROCgdb invocation plans.
With `--execute`, both commands run an exact descriptor-pinned native ROCgdb
image under bounded timeout, output, environment, working-directory, and
process supervision. Debug execution additionally requires an explicit
`--batch` or `--interactive` mode.
Discovery checks `ROCM_PATH`, `HIP_PATH`, supported `/opt/rocm` roots, and
absolute `PATH` entries in a fixed order; `--tool` accepts one explicit absolute
ROCgdb path. The sanitize foundation enables ROCgdb precise-memory mode, which
improves GPU memory-fault location but is not a race, uninitialized-memory, or
synchronization sanitizer. The debug foundation does not itself establish that
source maps or Rust aggregate layouts are complete.

## Pinned rustc executable

The managed binding wrapper uses a pinned-executable primitive for native
`rustc` execution. On Linux it:

- opens the selected path read-only with `O_NOFOLLOW`, `O_NONBLOCK`, and
  `O_CLOEXEC` so a FIFO or device cannot stall validation;
- requires a non-empty regular file with execute permission and a size no
  larger than 512 MiB;
- hashes exactly the opened object's reported length with SHA-256, rejects
  short reads, growth, and metadata changes during hashing, and rewinds it;
- retains the opened descriptor; and
- constructs commands through a validated `/proc/self/fd/<fd>` reference whose
  lifetime is tied to the retained descriptor.

The production path pins the compilation cwd as a directory descriptor and
performs `fchdir` immediately before exec. Its bounded V3 consistency record
covers the exact executable object and bytes, raw argv including argv0, cwd
object identity, protected source-tree identity, and the complete cleared child
environment. The backend reconstructs the same record from the actual process
and consumes the sealed parent expectation. The aggregate canonical encoding
is limited to 8 MiB.

The inert prepared invocation capture binds the canonical cwd pathname supplied
to rustc. Production captures use `RustcInvocationDescriptorV3`, which contains
the exact process/environment and the canonical compiler closure. The
process-consistency record above
separately binds the pinned cwd object. No object-identity join between that
object and the descriptor pathname is claimed. V3 is sealed and inherited by
rustc at fd 199. Protected production routes consume it before collection and
compare its argv, working directory, complete compile environment, target,
rustc image, backend image, and full compiler closure with the live process.

## Pinned codegen-backend object

The wrapper also contains a Linux primitive for the codegen-backend
dynamic-library object. It applies the same final-component `O_NOFOLLOW` and
nonblocking source-open policy, requires a non-empty regular file no larger
than 512 MiB, and copies exactly the source bytes into an anonymous memfd while
hashing them. It rehashes the image, applies and verifies immutable
write/grow/shrink/seal seals, drops the writable descriptor, and retains only a
read-only `O_CLOEXEC` descriptor.

A prepared child command rejects pre-existing joined, split, underscore, and
response-file backend selectors. It appends one descriptor-backed selector and
uses a child-only `pre_exec` step to revalidate the image and seals before
clearing `FD_CLOEXEC`. The prepared command borrows the pin and exposes no
argument mutation, so the selector cannot be replaced after preparation. The
external Cargo path actively uses the same primitive with a reserved stable
descriptor; the separate `fe2o3-rustc-wrapper` compile path remains disabled.

## Production compiler regression ownership

Production validation no longer has an unprotected `production-v1` success
test. Protected production requires the authority-release boundary, the sealed
V3 rustc-invocation capability, and the full compiler closure. Dependency
crates receive the pinned backend but not the selected kernel unit's pipeline,
artifact, Worker, or authority signals.

The regression path is split along real ownership boundaries:

- `production_build_config` structurally proves that Cargo has one fixed
  device/host plan, one production manifest, one managed transaction, and no
  Worker V2 feature or compiler implementation surface;
- `worker_v3_load_envelope_vertical` exercises durable V3 publication,
  verification admission, generated argument packing, application handoff,
  and hostile runtime substitutions;
- `production_extraction_driver_v1` rejects reachable unsafe Rust and verifies
  attributed-kernel collection in a real AMD dependency graph;
- `production_ranked_bounds_driver_v1` projects ordinary Rust into ranked
  PLIRON and runs the fixed bounds, atomic-legality, race, barrier-convergence,
  workgroup-memory, and semantic-refinement pass sequence before lowering;
- `reproducible_first_build_worker_v3` and `worker_v3_hsaco_admission` bind the
  exact V3 invocation, compiler closure, transaction, link plan, Worker
  measurement, response, raw HSACO, and finalized output.

The first three suites run with repository fixtures. The ignored native V3
worker case additionally requires a measured upstream LLVM/LLD worker and a
rustc-produced gfx942 handoff. Historical S09 evidence under `tests/evidence`
documents the older observation-only route; it is not current execution
guidance and grants no production authority.

## Platform and trust limits

Linux with a trustworthy, mounted procfs is the only supported execution
strategy. The process-consistency crate has an explicit Linux build gate;
other platforms must not fall back to reopening the selected pathname. The
current strategy is intended for native `rustc` binaries. Interpreter scripts
can fail when the descriptor is close-on-exec and are outside this boundary.

The rustc executable descriptor prevents pathname replacement from redirecting
execution, but does not make its writable inode immutable. The backend memfd
does provide an immutable snapshot after successful capture, so later source
pathname replacement or inode mutation cannot change child-visible bytes.
Parent-directory symlinks are resolved during each initial executable open, and
a race before that open can choose which object is measured. Protected-source
components are instead traversed relative to the pinned cwd with `O_NOFOLLOW`.
All resulting digests remain inert observations in this implementation.

ELF interpreters, transitive shared libraries of either rustc or the codegen
backend, dynamic-loader search and loading behavior, procfs mount/identity
semantics, and the kernel remain outside these primitives' boundary. The
backend starts too late to authenticate any pre-backend interpreter or loader
history. A protected supervisor is required to establish that authority. The
broker installs the backend descriptor only after a compile-shaped managed
wrapper invocation; the executable selected by that invocation is not
authenticated as rustc. Trusted Cargo descendants can deliberately replay the
wrapper as described above. The `run` application boundary closes the
descriptor before application exec.
Pinning the backend object does not pin its shared dependencies.
