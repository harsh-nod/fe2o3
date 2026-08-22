# fe2o3-service-host

`fe2o3-service-host` is the P1 host/model adapter boundary for issue #135. It
consumes `fe2o3-service-model` and `fe2o3-host-api` and defines:

- exact service run, service epoch, queue allocation epoch, slot, and logical
  and encoded generation bindings;
- borrow-retaining prepared, starting, running, draining, stopping, stopped,
  and classified failure typestates;
- structurally validated persistent-task submission tickets and terminal wait
  observations; and
- independent access to cancellation, quiescence, and progress property
  classifications without implication or promotion; and
- on Linux x86_64, non-Clone ownership of bounded device-local and host-visible
  coherent allocations obtained from a real checked gfx942 KFD device/VM.

The crate is `no_std` and contains no raw handles or unsafe code. The allocation
owner composes `fe2o3-kfd` production APIs; the underlying KFD adapter owns the
native handles, GPU virtual addresses, selected device/VM, and native allocation
records. Service-host adds process-local owner, device, VM, and allocation
labels retained beside those private native tokens; it does not expose or claim
a native KFD identity join. Public service-host range views expose only sealed
roles, kinds, offsets, extents, and alignments. They expose no native handle,
GPU address, descriptor, or direct persistent-pointer accessor and cannot mint
allocation authority. Before GPU mapping, `with_host_bytes_mut` intentionally
lends a scoped mutable CPU slice. A safe callback can derive and retain or
return a raw CPU pointer or numerical address from that slice. Rust provides no
safe dereference after the callback borrow ends; unsafe later use is outside
this owner's guarantees.

This crate performs no artifact load, kernel launch, queue publication,
execution, runtime wait, authentication, device-content initialization, copy,
completion, proof, quiescence attestation, or progress inference. The current
allocation owner has only a `NeverPublished` release path: because this owner
exposes no GPU address or queue bridge, explicit reverse-order unmap and free
is GPU-quiescent by construction. That is not a guarantee about unsafe later
use of a raw CPU pointer safely retained from a scoped callback. Any future
in-flight transition must be composed with exact completion authority inside
the private KFD dispatch boundary.

Live typestate values retain Rust borrows of queue, state, input, and output
storage. Only stopped or quiesced-failure typestates expose the conversion that
returns those borrows. This is a source-level ownership shape, not runtime
evidence: external admission must still reject forgetting or dropping live
descriptions and must establish the applicable shutdown/failure policy.
