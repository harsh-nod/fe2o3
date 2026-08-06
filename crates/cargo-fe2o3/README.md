# cargo-fe2o3

`cargo-fe2o3` coordinates the current fe2o3 build and smoke-test workflows.
The adjacent `fe2o3-rustc-wrapper` is fail closed for compile invocations while
its trusted execution boundary is built incrementally.

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

For a selected unit, the wrapper pins and validates all configured inputs,
binds the SHA-256 identity of the exact manifest bytes into `BuildInvocation`,
runs rustc, consumes the handoff once, and invokes the reproducible GenericLink
V1 plus compiler-aware Worker V2 workflow. It requires byte-identical output
from two executions, independently inspects the raw HSACO target, exports,
descriptors, and AMDHSA launch metadata, and derives a typed publication plan
only from the retained inspection evidence. Publication is attempt-scoped,
durable, digest-bound, and followed by managed attempt completion; exact
in-process retries recover the same publication without rebinding its inputs.

The published raw HSACO remains inert. This flow does not authenticate compiler
origin or Verus proof evidence, does not run canonical `.fe2o3.kd.v1`
descriptor-table finalization, and grants no HSA loading or launch authority.
Process-restart recovery after handoff consumption still requires a persisted
publication intent and retained handoff design.

## Inspection and tool plans

`cargo fe2o3 inspect` performs bounded, read-only decoding of fe2o3 v1
manifests, artifact containers, bundle indexes, and AMDGPU HSACO metadata. Its
output is descriptive only: inspection neither loads code nor grants launch
authority. Auto-detection uses validated wire magic, and `--format` can require
one exact decoder.

`cargo fe2o3 sanitize -- <program>` and `cargo fe2o3 debug -- <program>`
currently print normalized ROCgdb invocation plans without executing them.
Discovery checks `ROCM_PATH`, `HIP_PATH`, supported `/opt/rocm` roots, and
absolute `PATH` entries in a fixed order; `--tool` accepts one explicit absolute
ROCgdb path. The sanitize foundation enables ROCgdb precise-memory mode, which
improves GPU memory-fault location but is not a race, uninitialized-memory, or
synchronization sanitizer. The debug foundation does not itself establish that
source maps or Rust aggregate layouts are complete.

## Pinned rustc executable

The wrapper now contains a private pinned-executable primitive for a future
native `rustc` execution path. On Linux it:

- opens the selected path read-only with `O_NOFOLLOW`, `O_NONBLOCK`, and
  `O_CLOEXEC` so a FIFO or device cannot stall validation;
- requires a non-empty regular file with execute permission and a size no
  larger than 512 MiB;
- hashes exactly the opened object's reported length with SHA-256, rejects
  short reads, growth, and metadata changes during hashing, and rewinds it;
- retains the opened descriptor; and
- constructs commands through a validated `/proc/self/fd/<fd>` reference whose
  lifetime is tied to the retained descriptor.

Compile execution is still disabled. The primitive is not used by bootstrap
passthrough or compile plans yet.

## Pinned codegen-backend object

The wrapper also contains a private Linux primitive for the codegen-backend
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
argument mutation, so the selector cannot be replaced after preparation.
Compile activation and dynamic loading remain disabled.

## Platform and trust limits

Linux with a trustworthy, mounted procfs is the only supported execution
strategy. Other Unix systems and Windows return an unsupported-platform error;
they must not fall back to reopening the selected pathname. The current
strategy is intended for native `rustc` binaries. Interpreter scripts can fail
when the descriptor is close-on-exec and are outside this boundary.

The rustc executable descriptor prevents pathname replacement from redirecting
execution, but does not make its writable inode immutable. The backend memfd
does provide an immutable snapshot after successful capture, so later source
pathname replacement or inode mutation cannot change child-visible bytes.
Parent-directory symlinks are resolved during each initial source open, and a
race before that open can choose which object is measured. SHA-256 becomes
authentication evidence only after an orchestration layer compares it with a
trusted expected digest.

ELF interpreters, transitive shared libraries of either rustc or the codegen
backend, dynamic-loader search and loading behavior, procfs mount/identity
semantics, and the kernel remain outside these primitives' boundary. The
backend descriptor may also remain visible to rustc descendants until an
activation design closes or resets it. Pinning the backend object does not pin
its shared dependencies.
