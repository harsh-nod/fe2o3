# gfx942 Launch/HSACO Bridge V2

The launch/HSACO bridge is an inert compatibility check for current, recovered Worker V2
artifacts. It supports only `gfx942:xnack-`, code object V6, Wave64, exact workgroup geometry,
zero dynamic LDS, canonical scalar/slice descriptor semantics, and the mandatory COV6 implicit
ABI profile with an exact 256-byte implicit suffix.

The returned value contains three deliberately narrow projections:

- a physical kernel signature that commits every explicit argument and every mandatory implicit
  ABI record in canonical order, including kind, offset, size, alignment semantics, and the full
  implicit span;
- launch geometry with explicit provenance in each accessor: source rank and grid ceilings remain
  artifact/descriptor declarations, while required block size, wave width, maximum flat size, and
  per-axis maximum workgroups are inspected physical facts;
- matched static LDS, observed private-segment bytes, and a zero-dynamic-LDS profile that requires
  both a zero artifact/descriptor declaration and absence of a physical dynamic-LDS argument.

Artifact reference declarations must use exactly the descriptor V1 canonical source-type and
device-layout identities. Slice element size and alignment must match the scalar encoded by those
identities, and physical pointee alignment must be present and equal. Standalone pointers and
nested reference elements are rejected because descriptor V1 cannot express their semantic kind.
The deprecated physical `.value_type` field is normalized into a closed scalar enum. Omission
remains explicit unknown metadata because canonical LLVM 22 output omits the field; any declaration
that is present must exactly match scalar, slice-element, or slice-length semantics. Presence and
the canonical value enter the domain-separated physical-signature identity.

Optional hidden ABI records are rejected. Hostcall, printf, multigrid, heap, default-queue,
completion, private/shared-base, queue-pointer, and dynamic-LDS records therefore cannot disappear
from the physical signature or acquire unmodeled typed semantics.

Occupancy-dependent fields do not participate in the bridge. Minimum/maximum waves per execution
unit, occupancy witnesses and subjects, free-form variant names, variant tuple and policy
identities, capabilities, and proof records are neither validated nor retained. Matching scans the
bounded family using only occupancy-independent physical and artifact facts. The bridge exposes no
variant label or occupancy identity and cannot admit occupancy-dependent execution.

The value retains cooperative publication currentness but grants no load, dispatch, compiler,
Rust-type, Verus, proof, policy, or parity authority. It is not GPU execution evidence.
