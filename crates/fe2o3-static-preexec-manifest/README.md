# fe2o3 static pre-exec manifest

This crate is the canonical safe Rust encoder and structural validator for the
static pre-exec launcher's V1 descriptor manifest. Its 704-byte little-endian
wire format exactly matches
`tools/fe2o3-static-preexec-launcher/include/fe2o3_static_preexec_manifest.h`.

The codec validates all constraints represented by the record: versioning,
reserved bytes, parent identity bounds, descriptor count, ordered source file
descriptors, bounded and unique destinations, required standard descriptors,
zero inactive slots, object-validation classes, and non-aliasing object keys.
Ordinary objects use strict `fstat` identity. Process-pidfd objects additionally
require a live Linux pidfd in the launcher; multiple pidfds may share Linux's
anonymous-inode `fstat` key, so their exact target PID and process start time
must be bound by the receiving service. The launcher remains responsible for
seals, live snapshots, validation classes, descriptor access modes, and closed
unused descriptors.

## Bounded utility storage and work

`StaticPreexecManifestV1::from_descriptors(parent_pid, parent_start_time,
executable, &[StaticPreexecDescriptorV1])` validates and copies a borrowed table
without allocating. The existing `new(..., Vec<StaticPreexecDescriptorV1>)`
delegates to it and releases the caller's vector. `decode`, `encode`, and `clone`
also allocate nothing. There is still one V1 codec and the wire format is
unchanged; no native/V2 wire variant or KIR ledger dependency is introduced.

The retained value is exactly `size_of::<StaticPreexecManifestV1>()` bytes of
inline Rust storage, including the 16-entry backing array and active length.
Its Rust layout is not the wire ABI. Unused descriptor fields are zero (including
the `Fstat = 0` class); Rust padding bytes are not part of this guarantee.
`descriptors()` and all semantic checks use only the active slice. Derived
equality compares canonical backing storage, and cloning copies that storage.

For a caller that prepays utility work, these are logical source-level bounds,
not CPU instruction, compiler-generated stack, allocator, or elapsed-time bounds:

- Typed construction checks executable class, parent PID, parent start time,
  then count before reading or copying any descriptor. Invalid counts, including
  arbitrarily oversized borrowed slices, therefore reject without scanning them.
- Shared semantic validation initializes 128 `Option<usize>` destination slots,
  visits at most 16 active entries, compares at most `16 * 15 / 2 = 120` earlier
  object keys, and checks three required standard destinations. Its fixed
  destination scratch occupies `128 * size_of::<Option<usize>>()` bytes.
- The slice constructor zero-initializes 16 descriptor slots and copies at most
  16 validated descriptors. Decode zero-initializes the same fixed backing,
  decodes at most 16 entries, and examines at most 704 input bytes across header,
  active entries, and inactive-slot checks before shared semantic validation.
  Wrong-length input rejects without reading its contents.
- Encode initializes 704 output bytes and overwrites at most 700 bytes with
  header and active fields. Clone copies one inline value. External manifest
  alias validation compares at most 17 object keys (executable plus active table).

Input slices, output buffers, temporary values and copies, error storage, and
the retained manifest must be accounted for by the caller's own utility budget.
The compatibility vector constructor also drops the supplied vector; its prior
allocation and allocator deallocation are not bounded by this utility contract.
Formatting through caller-provided `Debug`/`Display` writers is likewise outside
it. These functions perform structural validation only, not process or descriptor
admission, I/O, or proof of safe execution.
