# fe2o3-rustc-front

This standalone crate defines the first version of a bounded, target-neutral
record exchanged between a rustc-facing collector and later fe2o3 compiler
stages. It intentionally has no `rustc_private` dependency, so its model and
wire protocol can be tested on stable Rust independently of a particular
compiler build.

`FrontendUnitV1` contains a canonical set of collected monomorphized
functions. Each function records:

- an opaque stable function identity and a kernel or helper role;
- a bounded diagnostic name and source location;
- a typed signature whose parameter and return types are opaque stable type
  identities; and
- a dense set of CFG block identities, source locations, and successor sets.

Constructors canonicalize set-like collections and reject duplicates. Block
identities must be dense from zero, entry blocks and successors must exist,
all opaque identities must be nonzero, and every unit must contain at least
one kernel. The decoder checks lengths and counts before allocation, rejects
unknown flags, versions, tags, and reserved fields, then requires
byte-identical canonical re-encoding.

## Wire V1

All integers are unsigned little-endian. The fixed 24-byte header is:

```text
offset  size  field
0       8     magic = "FE2O3RF\0"
8       2     version = 1
10      2     flags = 0
12      4     total byte length
16      4     function count
20      4     reserved = 0
```

Functions are ordered by function identity. Blocks are ordered by block ID,
and each block's successors are ordered by block ID. Text is UTF-8 with a
`u16` byte length. Collection counts use `u16` or `u32` according to their
published bounds. Every extensibility slot is zero in V1.

Each function record has this order:

```text
size      field
32        opaque function identity
1         role: 1 = kernel, 2 = helper
1 + 2     reserved = 0
2 + N     diagnostic-name byte length and UTF-8 bytes
32 + 4+4  source-file identity, one-based line, one-based column
2 + 2     parameter count, reserved = 0
32        opaque return-type identity
32 each   ordered opaque parameter-type identities
4 + 4     entry block ID, block count
variable  block records in ascending ID order
```

Each block record has this order:

```text
size      field
4         dense block ID
32 + 4+4  source-file identity, one-based line, one-based column
2 + 2     successor count, reserved = 0
4 each    unique successor block IDs in ascending order
```

V1 limits a unit to 4 MiB, 4096 functions, 131072 blocks in total, 65535
blocks per function, 128 parameters per function, 256 successors per block,
and 512 UTF-8 bytes per diagnostic name.

## Kernel contract wire V1

Launch and unsafe-assembly declarations use a separate wire domain, so the
`FrontendUnitV1` bytes above remain unchanged. `KernelFrontendContractV1` has
magic `FE2O3KF\0`, version 1, a maximum length of 64 bytes, and explicit flags
for its launch and assembly records. It models:

- optional required and maximum 3D workgroup dimensions, each with a volume no
  larger than 1024;
- an optional minimum workgroups per compute unit in `1..=64`, valid only with
  maximum dimensions; and
- a gfx942 assembly declaration with bounded operand, option, and effect bits.

The decoder rejects unknown bits, targets, nonzero reserved bytes,
noncanonical absent fields, contradictory dimensions, and incompatible
assembly options and effects. It then requires byte-identical re-encoding.

The proc macro emits this wire in an immutable `#[used]` sidecar whose final
path segment starts with `__fe2o3_kernel_frontend_contract_v1_`. The sidecar
also binds the logical kernel name and exact function pointer while preserving
the existing kernel registration tuple. Collection, reachable-assembly
comparison, lowering to AMDGPU attributes, and executable inspection remain
compiler responsibilities.

## Control-flow sidecar V1

Source control flow uses a third, independent wire domain. It does not modify
either wire above. `ControlFlowContractV1` has magic `FE2O3CF\0`, version 1,
and a 1 MiB limit. Its fixed 28-byte header records the total length, node
count, dense entry-node ID, and zero flags and reserved fields.

Each node records a dense ID, a complete source-file/start/end span, and one
of these closed terminators:

- entry or ordinary single-successor block;
- exit or two-successor branch;
- a loop header with a nonzero maximum iteration count, body, and exit;
- `break` or `continue` with an exact enclosing-loop identity and target; or
- a fixed-width signed or unsigned integer switch with canonical case bit
  patterns, case targets, and a default target.

Construction validates all references and source spans, requires every node
to be reachable and at least one exit to exist, and checks break/continue
targets against their declared loops. Dominator-derived backedges must target
bounded loop headers. Removing those backedges must leave a DAG, which rejects
unbounded and irreducible cycles. Every declared loop must have a structural
backedge.

Switch cases cover the complete `i8` through `i128` and `u8` through `u128`
domains. `isize` and `usize` are deliberately absent because their width is
target-dependent. Cases are sorted by their signed or unsigned semantic value
and duplicates are rejected. The exact source-span-independent CFG projection
is exposed as `CanonicalCfgIdentityV1`; its canonical bytes are a collision-free
structural identity rather than a cryptographic digest.

The decoder bounds every count before allocation, rejects unknown tags,
versions, flags, nonzero reserved fields, invalid UTF-8, truncation, trailing
bytes, malformed graphs, and noncanonical ordering, then requires exact
re-encoding. Proc-macro emission and authenticated MIR reconciliation are
separate producer responsibilities.

## rustc producer

`rustc-codegen-fe2o3` contains a fail-closed producer for this wire format. It
reads collected `Instance` and optimized MIR data from the active `TyCtxt`,
requires ordinary fully monomorphized items with available MIR, and records
instantiated signatures, definition and block source locations, dense MIR
block IDs, and canonical successor sets. Before later backend stages consume
the record, the producer encodes and decodes it through this crate and requires
an identical canonical model.

Producer identities are domain-separated SHA-256 digests. Function identities
bind rustc's monomorphized symbol, type identities bind the normalized
untrimmed Rust type, and source-file identities bind the remapped source name.
They are deterministic for the same bound rustc inputs; they are not claimed
to be compiler-version-independent semantic identities. Tool and invocation
identity must be bound separately when records become durable evidence.

## Trust boundary and non-goals

This crate validates structure and canonical representation only. In
particular, it does **not**:

- collect MIR or prove that a record came from rustc;
- authenticate the rustc producer's function, type, or source-file identity
  derivations, or require other producers to use those derivations;
- prove that a function was monomorphized, that helpers are reachable from a
  kernel, or that names and source locations are truthful;
- encode MIR statements, terminators, edge conditions, call sites, constants,
  layouts, ABI details, ownership, or general target information outside the
  separately versioned control-flow sidecar;
- prove source loop bounds, authenticate source spans, or prove that a source
  control-flow sidecar is equivalent to optimized MIR;
- establish type safety, memory safety, bounds safety, initialization, alias
  safety, race freedom, convergence, or functional correctness;
- bind a record to source inputs, a compiler invocation, Kernel IR, a proof,
  an artifact, a loaded code object, a launch, or a device; or
- grant compilation or launch authority.

Those properties require separately authenticated compiler stages and exact
identity bindings. Consumers must treat decoded records as untrusted input.
