# fe2o3-copy-kernel-v1

This crate owns one exact `gfx942:xnack-` byte-copy compiler profile. It
constructs a bounded typed LLVM handoff whose only device effect is:

```text
if global_id_x < byte_len { destination[global_id_x] = source[global_id_x] }
```

The source, destination, and byte count are a closed three-argument ABI. The
source is `readonly`, the destination is `writeonly` and `noalias`. The typed
source encodes both a `256,256` flat-workgroup range and required `256x1x1`
metadata; post-worker admission requires the same metadata and wave64. The
handoff is admitted against the repository's pinned upstream LLVM/LLD 22.1.8
policy, serialized canonically, placed in the existing compiler-FFI handoff,
and rechecked after construction of a sealed Worker V2 request.

The semantic claim is bounded. For a nonempty copy, source and destination
must be valid nonoverlapping ranges of at least `byte_len` bytes, global offset
must be zero, Y/Z grid dimensions must each be one, and the X grid must contain
exactly `ceil(byte_len / 256) * 256` workitems. `byte_len` may not exceed
`0xffff_ff00` bytes, so the padded grid fits the AQL `u32` field and the 32-bit
global-index arithmetic cannot wrap. An empty copy requires no dispatch. This
crate validates no leases and grants no dispatch authority; those preconditions
must be discharged by the runtime capability path.

The post-worker boundary consumes the prepared typed source, a measured pinned
worker, and the existing reproducible-first-build evidence. Before parsing the
artifact it re-decodes both Worker V2 exchanges and rechecks the exact handoff,
LLVM module, compiler envelope, symbol manifest, worker measurement, options,
observed link plan, response, and output lineage. It then checks the exact
one-kernel symbol/ABI/resource profile plus the strict allocation-free COV6
loader envelope. The admitted type retains the typed source, assembly, handoff,
plan, replay-request, worker, and observed output identities. A newly observed
output digest is not an independently approved deployment pin. Likewise, an
externally composed replay plan does not make its predeclared output identity an
approval.

No API in this crate publishes an artifact, loads code, constructs a native
address, validates lease bounds or nonoverlap, packs kernargs, submits an AQL
packet, mints initialized device content, or claims hardware execution. The
repository Worker V2 has no copy-specific post-link semantic profile, and the
pinned LLVM 22 worker executable was unavailable for this change. Therefore
this crate supplies no fixed HSACO digest, successful post-worker fixture, or
machine-code refinement theorem. Qualification must run the exact worker,
retain the output, independently approve its digest, and close the remaining
lease-to-kernarg/dispatch proof before a production copy dispatch can use it.
