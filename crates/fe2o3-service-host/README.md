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
  coherent allocations obtained from a real checked gfx942 KFD device/VM; and
- addressless fixed-batch descriptions and linear prepared, published,
  completed, recycled, and unbound ownership of one long-lived KFD queue.

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

The Linux x86_64 composition path can initialize complete device-local extents
from owned bytes through KFD's checked public-device-local mapping path. It can
also consume inspected executable envelopes, complete kernarg byte images, and
checked addressless device-local or host-visible coherent ranges into a fixed
batch. Native addresses are
substituted only inside KFD. A batch of 1 through 8192 packets uses one ring
reservation and monotonic per-packet doorbell publication. Exact completion and signal
recycle are required before the same native queue can detach its current batch,
replace a complete initialized allocation, bind a different fixed batch, or
return allocation custody for explicit release. An owned full-extent coherent
initialization path is distinct from the arbitrary scoped host-write path, so
only the former can satisfy an inspected read or read-write argument.

For an admitted metadata-derived subset, callers supply a complete kernarg
image whose exact trailing 256-byte COV6 implicit suffix is zero. The retained
KFD owner privately fills block counts, group sizes, partial-group remainders,
zero global offsets, grid dimensions, and dynamic LDS before GPU mapping. Queue
pointers and all runtime-service or address fields remain rejected. This
service layer does not ask callers to fill or assert implicit values.

After exact completion and signal recycle, the retained owner can create a
request bound to that dispatch generation and one owner-checked coherent range.
The lower owner copies bytes only when the requested range lies within exactly
one metadata-inspected write or read-write binding. It rejects device-local,
read-only, unwritten, out-of-range, overlapping, stale-generation,
pre-completion, and pre-recycle access. The result owns a byte copy; it exposes
no mapped pointer or GPU address and grants no full-allocation initialization.

This layer does not establish executable correctness, effect correctness beyond
inspected metadata, full write coverage, content interpretation, numerical
correctness, hardware execution, or performance. Generic completion preserves
only whether a complete allocation was already initialized, never its stale
pre-dispatch content digest. An allocation admitted uninitialized remains
uninitialized after generic completion. Queue and allocation failures after
ambiguous native effects are terminal and expose no retry. `NeverPublished` release remains
available only before queue composition and is not a guarantee about unsafe
later use of a raw CPU pointer safely retained from a scoped callback.

Live typestate values retain Rust borrows of queue, state, input, and output
storage. Only stopped or quiesced-failure typestates expose the conversion that
returns those borrows. This is a source-level ownership shape, not runtime
evidence: external admission must still reject forgetting or dropping live
descriptions and must establish the applicable shutdown/failure policy.
