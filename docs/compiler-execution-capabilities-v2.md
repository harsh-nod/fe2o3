# Native Trust Capabilities V2

## Status

`CompilerExecutionPolicyCapabilityV2` and
`CompilerExecutionClientProfileCapabilityV2` transport the native public trust
records in immutable descriptors. They are move-only, expose no `AsFd`, and
never decode or upgrade V1 records. A policy/profile remains public configuration,
not signing custody, protected execution, publication, GPU load, or launch authority.

`CompilerExecutionServiceLaunchCapabilityV2` transports the 112-byte identity-only
launch frame through the same private sealed-image machinery. Its wire deliberately
remains V1: it contains a policy digest, not a policy or subject encoding. Structural
decoding therefore accepts either opaque policy binding. A consumer must match an
independently admitted PolicyV2; structural admission is not a policy upgrade.

`CompilerExecutionSigningKeyCapabilityV2` freshly admits a seed or transferred
secret image under a pinned PolicyV2. It retains that complete typed policy
identity and exposes only revalidation and read-only transfer, not signing.
It cannot be constructed from a V1 key owner.

`ProtectedExternalAnchorServiceAdmissionV2` separately admits the anchor endpoint
and live service pidfd through bounded, allocation-free inspection. It preserves
the shared continuity checks and requires a distinct service UID. See the
[anchor custody contract](compiler-execution-anchor-custody-v2.md) for ownership,
resource accounting and the difference between transport custody and authority.

These APIs do not activate the native producer, service handlers, or launcher.
No protected proof or GPU execution is credited. M0-M7 and 47/47 remain open.
See the [native publication contract](compiler-execution-publication-v2.md)
for the other completed inert protocol boundaries and remaining integration.

## Admission Invariants

The shared sealed-image validator checks metadata, exact regular mode0400,
exact length, exact WRITE/GROW/SHRINK/SEAL seals, and CLOEXEC, in that order.
Revalidation also checks the retained device/inode and compares every byte to
the private immutable native record. Identical bytes in another inode are not
a substitute. Mode and descriptor flags are rechecked because seals do not
freeze them. Validation is an observation, not perpetual metadata protection.

Policy and profile images are 216 and 280 bytes, respectively, just like V1.
Length or a memfd name cannot identify their family: the actual strict native
decoder admits transferred images. Creation instead consumes an already admitted
native record. Revalidation needs no second decode because byte equality preserves
that immutable record's admission invariant. No owner is cloned or projected.

Native reads and writes are positional, fixed-size, single-attempt operations.
Short transfers and EINTR fail; there are no retry loops or dynamically sized
content buffers. Errors retain static operation/reason labels and an errno or
typed decoder error. V1 retains its existing allocating APIs and diagnostic order.

Policy inherited admission borrows an fd >= 3 with CLOEXEC clear and creates a
private CLOEXEC duplicate. It never closes or changes the original descriptor.
The caller must keep that original slot live and unchanged during admission.
Transfer produces another CLOEXEC File referring to the same sealed object.
Raw File interoperability does not meter subsequent arbitrary File operations.

## Production Profile

The only production native profile is
`/etc/fe2o3/compiler-execution/client-profile-v2`. There is no configurable root,
environment override, basename selector, or fallback to `client-profile-v1`.

The fixed walk opens `/`, `etc`, `fe2o3`, and `compiler-execution` relative to
pinned directory descriptors, without following symlinks. All must be owned by
uid/gid 0, owner-traversable, non-group/world-writable directories. The final
RDONLY/NONBLOCK/CLOEXEC object must be a regular, single-link, root-owned,
exact mode0444 file of the expected length. Every directory and both final-file
checks probe `security.capability`, `system.posix_acl_access`, and
`system.posix_acl_default` with a one-byte buffer; presence, including ERANGE,
fails. NODATA and unsupported-xattr filesystems are accepted, as in V1.

The final file's metadata snapshot must match before and after reading. It
includes dev/ino, mode, ownership, links, length, mtime, and ctime, excluding atime.
The result is decoded and sealed. This trusts the protected root-owned tree;
it does not prove exclusion of a privileged writer, immutable ancestors, or
perpetual pathname currentness. Tests use private synthetic trees, never a
public alternate production path.

## Resources And Ownership

All operations use the caller's resource ledger. The shared
`Budget::with_prepaid_scope` prepays outer work and scratch; actual nested native
decoders charge that same ledger. No unlimited child budget is created.
Scope cleanup restores the full entry reservation on success, failure, and
unwind, without refunding work or clearing peaks/first denials. Replaced ledgers
are not released. This is accounting, not an authority gate.

Let `N` be the image length, `S` the capability storage-receipt type, `C` the
capability type, and `Drecord` the native record's retained charge:

```text
Dfile = N + size_of::<(File, S)>()
Dcap  = N + size_of::<(C, S)>()
```

Tuple sizes include alignment padding. Every descriptor conservatively pays
for its complete logical image even when duplicates share one backing inode.

| Operation | Prepaid Input | Additional Returned Charge |
|---|---|---|
| create(record) | consumed Drecord | Dcap - Drecord |
| from_file(file) | consumed Dfile | Dcap - Dfile |
| revalidate | borrowed Dcap | none |
| try_clone_for_transfer | borrowed Dcap | full Dfile |
| from_inherited_at | borrowed Dfile | full Dcap |
| from_production_profile | none | full Dcap |

Keep consumed inputs' reservations and reserve the returned delta before
retaining the result. On consuming failure, retire the dropped input's old
reservation after return. On eventual drop or transfer, retire the full owner's
charge, not its construction delta. Unrelated entry owners remain prepaid.

Outer work is `8 + 32*1024 + 32*N`: at most 32 descriptor syscalls with a fixed
logical weight, plus byte processing. The trusted-root walk adds `64*1024`.
The exported `IO_STORAGE` and `PRODUCTION_STORAGE` constants cover outer frames;
admission additionally peaks at the larger of native decoder scratch and its
returned record retention. That record is reserved immediately and kept through
outer cleanup. Actual work totals are:

| Operation | Policy | Profile |
|---|---:|---:|
| create, revalidate, transfer | 39688 | 41736 |
| from_file; policy from_inherited_at | 54800 | 65808 |
| from_production_profile | n/a | 131344 |

For the launch capability, create/revalidate/transfer cost 36360 units and
file/inherited admission costs 39952 units including the native manifest decoder.
The manifest API costs 3592 units for construction, decoding, or policy matching.
Its fixed scratch is exported as `COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2`;
borrowed policy/manifest inputs stay separately prepaid.

These logical quotas do not bound syscall latency, kernel allocation, generated
instructions/stack, allocator behavior, page cache, or process RSS.

## Native Signing-Key Custody

The key image is exactly 32 bytes. In addition to the shared mode, length, seal,
CLOEXEC and retained-inode checks, it must be anonymous, owned by the current
effective UID/GID, RDONLY and not O_PATH. Creation seals a writable memfd, then
reopens that same retained inode read-only using a fixed stack path buffer.
These secret-specific restrictions do not change the public-record images.

Fresh admission derives one Ed25519 key using pinned Dalek and checks the policy's
public key. Revalidation compares the guarded image bytes to the retained seed
with `subtle::ConstantTimeEq`, without deriving a second key, and requires the
complete retained PolicyIdentityV2. Same-key changes to generation, executable,
runtime or anchor key therefore reject. Raw seed bytes have no protocol-family
tag: fresh V2 admission can use the same seed as V1 without upgrading a V1 owner.

The caller seed is borrowed by a wiping guard before entry-work or storage
admission. Read scratch is guarded before I/O, including partial/error reads and
post-read refusal. Guards run on ordinary return and unwind; Dalek's owned key
has `ZeroizeOnDrop`. This covers the explicit owned userspace buffers, not prior
caller copies, compiler-generated temporaries, abort/termination, or erasure of
kernel pages when the sealed memfd closes. Debug reveals no seed, fd or path.
The transferred File contains readable secret material. Its recipient must be
trusted and must protect any copies it makes; this API is not a secrecy boundary
against an owner of that File. Read-only transport prevents writes, not reads.

Let `Dkey = 32 + size_of::<(KeyCapabilityV2, StorageV2)>()`, `Dfile` use the same
formula with `File`, and `P` be the borrowed native policy's retained charge:

| Operation | Prepaid Input | Additional Returned Charge |
|---|---|---|
| create_and_zeroize | borrowed 32-byte seed + P | full Dkey |
| from_file | consumed Dfile + borrowed P | Dkey - Dfile |
| from_inherited_at | borrowed Dfile + P | full Dkey |
| revalidate | borrowed Dkey + P | none |
| try_clone_for_transfer | borrowed Dkey | full Dfile |

Key `IO_WORK` is 66568 logical units, allowing at most 64 descriptor, credential
and cleanup calls plus fixed byte processing. Fresh admission prepays one named
65536-unit crypto derivation allowance, for `ADMISSION_WORK = 132104`.
All methods prepay `IO_STORAGE` for fixed owners, guarded seed buffers, metadata,
control frames and crypto scratch. These are named logical admission quotas,
not instruction, generated-stack or physical-memory bounds. Returned deltas
and consumed inputs follow the same ledger discipline as public capabilities.
The raw seed wire and destination slot 7 remain unchanged; custody alone does
not authenticate an issuer service or activate a native launch.

## Remaining Integration

The issuer's explicit native input reader now independently admits policy and
launch capabilities from fixed slots 6/8 and revalidates both before requiring
their native policy match. It borrows the installed sources, returning private
CLOEXEC owners and their full retained charge. Nested admission and comparison
use the same ledger; partial failures drop private duplicates and restore the
entry floor. The caller remains responsible for closing the original sources.
Agreement is not an external policy pin: a consistently replaced native pair
also agrees. Trusted installation and native program admission must supply that
pin before activation.
The reader grants no client/anchor/process/key authentication or readiness, and
the serving entrypoint does not call it yet. Tests exercise a real process exec
with native, mismatched, mixed-family and corrupt inputs, without invoking the
protected launcher or crediting a protected service occurrence.

Native child installation needs an owned launcher that retains descriptor
capture charges and prepays each spawn. Attaching a callback to a reusable
external `Command` would not meter its future spawns; no such native adapter
is exposed here. Service handlers, native durable-state families, broker V4
observation, anchor/Worker/ACK ordering, Cargo restart paths, runtime/host joins,
and coherent provisioning remain required before producer activation.

Fresh native program custody now consumes the pinned policy with independently
sealed launcher/issuer images through bounded shared executable mechanics. It
does not yet bind the separately admitted native key and anchor transport into
service authority or a consuming launch.
See the [program status](../crates/fe2o3-compiler-execution-supervisor/README.md)
and [image accounting](../crates/fe2o3-protected-static-executable/README.md).
