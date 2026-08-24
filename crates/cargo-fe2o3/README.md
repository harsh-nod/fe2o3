# cargo-fe2o3

`cargo-fe2o3` coordinates the current fe2o3 build and smoke-test workflows.
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
then executes the sealed image with fixed contract, control, launcher-image,
and cwd descriptors. The child independently checks its own image, its live
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
`FE2O3_AUTHORITY_*_V1` inputs; `FE2O3_BACKEND`,
`FE2O3_CODEGEN_PIPELINE`, `FE2O3_TARGET`, optional
`FE2O3_WORKER_V2_CONFIG_V2`, `LANG=C`, `LC_ALL=C`, and `TZ=UTC`.
Aliases, extra variables or descriptors, loader variables, rustup/tool
selectors, noncanonical paths, changed backing objects, replayed attempts, and
closure/runtime-tree drift fail closed. Tool digests are operator-provisioned
inputs and are remeasured; no machine-specific digest is compiled in.

`cargo fe2o3 authority release probe` exercises this exact launcher/handoff
boundary and exits before Cargo, artifact generation, HSA loading, or GPU
dispatch. A successful probe grants no compiler-origin, proof, artifact-safety,
runtime, or GPU authority. A separate integration fixture crosses the admitted
boundary into pinned Cargo and the existing transactional test backend, but
its publication evidence carries only that fixture's existing downstream test
authority. The release contract makes no stronger claims.

The fixed row-softmax production action currently stops after this admitted
launcher/handoff at `stage=binding-wrapper`. Its direct Rust workspace wrapper
is dynamically linked to `librustc_driver`, but production clears loader
variables and must reject Cargo's mutable target deps directory when Cargo
prepares the wrapper environment. The non-integrated C trampoline is not an
authority path, and no debug normalization is used. An exact static binding
wrapper must be integrated and admitted before row-softmax can enter Cargo and
the backend. Consequently the staged 25-pin finalizer/runtime path has no
production compiler, artifact, launch, or GPU authority.

### Compiler provenance wiring

For protected builds, the capability broker sends a sealed raw
`CompilerClosureV2` capability to the binding wrapper. The wrapper revalidates
the complete closure, captures the exact prepared V2 rustc process and child
environment, and upgrades that in-memory observation to
`RustcInvocationDescriptorV3`. The rustc and backend pins duplicated by V2 and
the closure must match.

The wrapper seals the canonical V3 bytes and installs that exact immutable
image at fd 199 for the prepared rustc child. The raw brokered closure stops at
the wrapper and is only an input to V3 construction. Backend admission of the
inherited V3 descriptor is the next boundary and is not wired yet, so this
transport remains coordination evidence rather than an execution receipt or
compiler-authorship proof. Unprotected compatibility captures remain V2 and
receive no fd 199 invocation capability.

## External Cargo projects

`cargo fe2o3 build` and `cargo fe2o3 run` operate from standalone Cargo
projects and workspace members. Cargo receives the original platform argument
vector without shell construction or UTF-8 conversion. A separate bounded
`cargo metadata --no-deps` probe resolves the selected `--manifest-path`,
workspace root, and configured target directory. An explicit `--target-dir`
has the same invocation-directory-relative interpretation as Cargo and takes
precedence over metadata; `CARGO_TARGET_DIR`, repeated `--config`/`-Z`
arguments, and locked/offline/frozen routing are reflected by metadata.

The invocation directory, workspace root, target directory, and
`<target>/fe2o3` output directory are opened without following path-component
symlinks and retained through the operation. On Linux, generated output is
passed through a validated fixed `/proc/self/fd` directory reference. The
backend is copied into a sealed read-only memfd and installed at a separate
fixed child descriptor, so the measured bytes are the bytes selected for
rustc. Path substitution is checked before and after Cargo runs.

Generated output carries an atomic deletion guard and a bounded generation
marker that binds the sealed backend digest, transitive Worker V2 inputs,
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

### CPU simulation from source

`cargo fe2o3 simulate` runs the ordinary Cargo/source frontend through the
generic verified MIR-to-KIR lowering and then executes the exact canonical KIR
V6 bytes with the bounded CPU simulator:

```console
cargo fe2o3 simulate \
  --request request.json \
  --output result.json \
  -- --package my-kernel
```

The request uses `fe2o3-simulation-request-v1` and names one kernel, launch
grid, workgroup, and typed scalar or buffer arguments. The result uses
`fe2o3-simulation-result-v1`; its additive evidence fields explicitly report
`simulated=true`, no hardware observation or validation, no performance
prediction, the scalar target profile, deterministic scheduler, and exact KIR
identity. Omitting `--output` writes exactly one JSON document to stdout.
Output-file publication is private, durable, atomic, and no-replace.

The command securely reads and strictly admits the complete request before
starting Cargo. It binds the byte length and SHA-256 and verifies both again
immediately before execution, so request replacement during a long build fails
without producing output when it changes any admitted byte. Byte-identical
pathname or inode replacement is content-equivalent. Compiler completions
publish inert, attempt-scoped canonical KIR. After Cargo exits, the parent
requires exactly one kernel-bearing module. Zero or multiple modules are
rejected deterministically; a module may contain multiple kernels, and the
typed request selects exactly one of them. Use normal Cargo package/target
selection after `--` to select the intended producer.

Each invocation supplies a fresh, nonzero tracked simulation-attempt identity
to every supported `#[kernel]` expansion. The identity changes Cargo's
dependency observation without changing generated kernel tokens, so a reused
Cargo target must rerun each kernel-bearing source crate and publish a new
one-shot KIR handoff. The `<target>/fe2o3` simulation generation is never
committed or cached: it is explicitly deleted after success, Cargo failure,
request rejection, simulator rejection, or output-publication failure.
Unrelated Cargo host outputs remain reusable. Manually forging reserved kernel
registration symbols is not a supported source root; if a root does not pass
through `#[kernel]` and therefore does not observe the fresh attempt, reuse
produces no handoff and fails closed.

Simulation sets the existing `FE2O3_HIP_SYS_DISABLE` build boundary and the
default `cargo-fe2o3` dependency and ELF closures exclude `fe2o3-core`,
`fe2o3-host`, `fe2o3-hsa-runtime`, HIP, HSA, KFD, DRM, and ROCm libraries.
It does not enumerate a GPU or initialize a GPU runtime. Hardware commands
remain explicit and never fall back to simulation. The existing legacy direct
row-softmax HSA runtime and related hardware test fixtures are compiled only
with the explicit `legacy-hsa-runtime` feature; a default command image fails
closed before that compatibility boundary. New runtime work remains KFD-only.
Simulation has no HSA, HIP, KFD, or ROCm runtime.

This is a qualification route, not the production compiler transaction. It
provides exact KIR V6 custody from the current generic frontend and runs every
kernel/type/operation admitted by both that lowering and the simulator. An
unsupported source construct or simulator operation fails closed. It grants no
compiler, refinement-proof, artifact, runtime, performance-prediction, or GPU
authority.

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
with the expected build session, profile, and Worker V2 configuration identity.
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
effective exact-target runner. Every application or configured runner starts
with an empty environment; no `PATH`, `TMPDIR`, build control, or arbitrary
inherited variable is retained. The configured runner command, application
path, and arguments remain byte-preserving. String and array runners from
target configuration and target-runner environment variables are supported,
including non-UTF-8 Unix values while Cargo resolves the runner.
Recursive cargo-fe2o3 runners, including aliases and hardlinks to the same
executable inode, and runner selections that cannot be resolved unambiguously
fail closed. In particular, a `cfg(...)` runner must currently be made explicit
as `target.<triple>.runner` for `cargo fe2o3 run`.

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

## Narrow Worker V2 handoff flow

`FE2O3_CODEGEN_PIPELINE=kernel-ir-worker-v2` requires
`FE2O3_WORKER_V2_CONFIG_V2` to name an absolute path to a strict V2 JSON
manifest. The manifest is an explicit operator policy input, not compiler
attestation. It must be compact canonical JSON with sorted object keys and
contains exact compilation-unit selectors, a measured worker, measured typed
providers, all four supported link options, and explicit output and process
limits. Unknown fields, defaults, relative paths, identity mismatches, and
noncanonical collections are rejected before rustc is spawned.

V2 has no operator-supplied `final_symbols` field. The verified compiler module
emits a canonical role manifest covering kernel entries, `<kernel>.kd`
descriptor symbols, device FFI exports, internal helpers, and contracted
external imports. Worker requests derive the complete final dynamic-symbol
closure from that manifest; unknown JSON fields are rejected, so an operator
cannot inject or omit final symbols.

Each `units` selector binds `crate_name`, rustc's exact `source` path spelling,
and the absolute `working_directory`. An unselected compilation receives no
managed attempt, allowing inherited host and dependency compilations with no
device kernels to proceed. If such a compilation unexpectedly contains a
device kernel, the backend rejects it because the required managed attempt is
absent. A selected compilation must publish exactly one attempt-scoped handoff;
a missing handoff is an error and invalidates the attempt.

The protected producer, consumer, completion, cleanup, and restart route now
uses the closure-bound compiler-handoff V2 and publication-intent V2 protocols
end to end. It rejects V1 and obsolete protected state instead of probing or
downgrading to it. Required-envelope restart joins the exact compiler closure,
attempt, admission, intent, output, receipt, envelope inputs, and durable load
envelope before resuming. Cleanup uses a durable escrow and exact successor
lease; a crash after publishing a newer `Ready` marker resumes predecessor
retirement idempotently. Directory scans are bounded, and authoritative path
and argument bytes are never compared through lossy UTF-8 conversion.

The ordinary non-protected route retains its separate V1 compatibility state
machine. Schema separation prevents ordinary V1 records from entering or
clearing protected V2 state. This closes the protected transport and restart
provenance chain, but it does not authenticate compiler or proof claims and
does not grant HSA load or launch authority.

For a selected unit, the wrapper pins and validates all configured inputs,
binds a domain-separated identity of the exact manifest, sealed worker image,
and provider bytes into both the Cargo generation and `BuildInvocation`,
rereads that identity in the wrapper before use, runs rustc, consumes the
handoff once, and invokes the reproducible GenericLink
V1 plus compiler-aware Worker V2 workflow. It requires byte-identical output
from two executions, independently inspects the raw HSACO target, exports,
descriptors, and AMDHSA launch metadata, then selects descriptor-free COV5 raw
compatibility or canonical finalization of a descriptor-bearing COV6 output.
Required-envelope mode accepts only the finalized COV6 route. The production
worker uses pinned upstream LLVM target-machine APIs and the in-process LLD
library API, with no COMGR or command-line `clang`, `llc`, or `ld.lld` path.
The typed publication plan is derived only from retained inspection evidence.
Publication is attempt-scoped,
durable, digest-bound, and followed by managed attempt completion; exact
in-process retries recover the same publication without rebinding its inputs.

Required-envelope mode additionally measures and canonical-decodes its bounded
input capsule before Cargo starts, then revalidates and durably retains the
exact capsule before a fresh selected attempt can become recoverable. After the
finalized HSACO publication is committed, the wrapper reconstructs the
container, descriptor lineage, bundle/direct-link evidence, proofs, exact raw
and finalized identities, and durable publication claim from those retained
inputs. It writes the canonical load envelope with create-new semantics, syncs
the file and containing directory, verifies the exact bytes, and only then
advances the restart marker to completed and clears the publication intent and
attempt state. Recovery at every committed boundary repeats those joins from
the durable intent and capsule without rereading the operator path or
respawning rustc. Package, generation, receipt, capsule, proof, payload, and
envelope substitution fail closed; truncated or conflicting canonical files
are never replaced implicitly.

The published HSACO and load envelope remain inert. The envelope contains
only a durable claim and explicitly contains no process-local currentness
lease. This flow does not authenticate compiler origin or Verus proof evidence
and grants no HSA loading or launch authority; downstream admission must
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
ACK descriptor. Immediately before every application or configured-runner exec,
including the no-envelope path, `close_range(CLOSE_RANGE_CLOEXEC)` protects all
non-stdio descriptors. Required-envelope launch then clears `FD_CLOEXEC` only
for its exact envelope, artifact-directory, ACK, and test-only readiness ABI.
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
manifests, artifact containers, bundle indexes, and AMDGPU HSACO metadata. Its
output is descriptive only: inspection neither loads code nor grants launch
authority. Auto-detection uses validated wire magic, and `--format` can require
one exact decoder.

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

The protected S09 path also pins the compilation cwd as a directory descriptor
and performs `fchdir` immediately before exec. Its bounded V3 consistency
record covers the exact executable object and bytes, raw argv including argv0,
cwd object identity, one exact alpha-only source path/length/SHA observation,
and the complete cleared child environment. The backend reconstructs the same
record from the actual process and consumes a sealed parent expectation. The
aggregate canonical encoding is limited to 8 MiB. The separate
`fe2o3-rustc-wrapper` compile path remains disabled.

The inert prepared invocation capture binds the canonical cwd pathname supplied
to rustc. Protected captures use `RustcInvocationDescriptorV3`, which contains
the exact V2 process/environment and the canonical compiler closure;
compatibility captures may remain V2. The process-consistency record above
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

- `external_project_cli::protected_release_build_reaches_authenticated_cargo_publication_fixture`
  exercises the protected launcher, static binding trampoline, V3 capability
  broker, selected rustc child, and authenticated publication boundary;
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
