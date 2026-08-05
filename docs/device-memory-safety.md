# Device memory type safety

Safe `DeviceBuffer<T>` construction and transfer require `T: DeviceCopy`.
`DeviceCopy` is an unsafe structural host representation contract: host values
have no padding, every byte is initialized, every bit pattern is a valid host
Rust value, and the type satisfies `Copy + Send + Sync + 'static`. This keeps
downloaded bytes from becoming invalid Rust values and prevents uninitialized
padding from being uploaded.

fe2o3 provides audited implementations for fixed-width integer types, `f32`,
`f64`, const-generic arrays of `DeviceCopy` elements, and structs accepted by
`#[derive(DeviceCopy)]`. The derive is re-exported from `fe2o3_core`, so the
same `use fe2o3_core::DeviceCopy;` import brings both the trait and derive macro
into their respective namespaces.

The derive accepts only non-generic structs with exactly `#[repr(C)]` or
`#[repr(transparent)]`. Every field must implement `DeviceCopy`. Generated
compile-time obligations use checked addition for field sizes and require the
sum to equal the complete struct size, ruling out internal and trailing
padding. The unsafe implementation also requires the complete type to satisfy
`Copy + Send + Sync + 'static`. Packed, explicitly aligned, duplicate, and
conflicting representations are rejected conservatively.

This is intentionally structural host-side byte-copy evidence, not semantic or
cross-target ABI evidence. A `DeviceCopy` integer or integer newtype may encode
a host address, resource handle, or any other application-defined value; the
derive cannot infer that meaning. `DeviceCopy` also does not claim that a device
compiler uses the same size, alignment, field offsets, scalar representation,
or calling convention. Safe transfer may treat device memory as opaque bytes,
but safe typed launch or device interpretation must additionally validate
manifest-derived type and ABI identities together with provenance,
address-space, and capability requirements. Raw unsafe launches leave those
obligations with the caller.

The trait remains deliberately unimplemented for `bool`, `char`, `usize`,
`isize`, pointers, references, `NonZero` types, and standard owning containers
such as `String` and `Vec<T>`. These structural exclusions do not classify the
semantic meaning of otherwise valid integer fields.

Buffers record the numeric HIP device that owns their allocation. A safe
buffer-to-host transfer rejects a stream from another device before calling
HIP. Numeric device identity is used because separately created wrappers for a
HIP primary context on the same device refer to the same device domain.

`DeviceCopy` is only a structural host representation and Rust-validity
guarantee. It does not prove device type or ABI identity, provenance, required
capabilities, kernel bounds, buffer access modes, absence of data races, stream
ordering, or kernel ABI field ordering. Those require the manifest-gated typed
launch, artifact-contract, and kernel-verification layers.
