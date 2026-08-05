# Device memory type safety

Safe `DeviceBuffer<T>` construction and transfer require `T: DeviceCopy`.
`DeviceCopy` is an unsafe plain-data contract: values have no padding, every
bit pattern is valid, and host and device use the same stable representation.
The type must not contain pointers, references, resource ownership, or interior
mutability. This keeps raw device bytes from becoming invalid Rust values and
keeps host-only addresses or uninitialized padding from being copied to a GPU.

fe2o3 provides audited implementations only for fixed-width integer types,
`f32`, `f64`, and const-generic arrays of `DeviceCopy` elements. It deliberately
does not implement the trait for `bool`, `char`, `usize`, `isize`, pointers,
references, `NonZero` types, or user-defined structures. A future derive macro
and layout checker will cover eligible structures.

Buffers record the numeric HIP device that owns their allocation. A safe
buffer-to-host transfer rejects a stream from another device before calling
HIP. Numeric device identity is used because separately created wrappers for a
HIP primary context on the same device refer to the same device domain.

`DeviceCopy` is only a representation and Rust-validity guarantee. It does not
prove kernel bounds, buffer access modes, absence of data races, stream
ordering, or kernel ABI field ordering. Those require the typed launch,
artifact-contract, and kernel-verification layers.
