# Persistent Generated Projection V1

R77 implements the consuming, nonexecuting projection portion of GEN-2B-3.
It builds on [R76 Context preparation](runtime-context-generated-preparation-v1.md)
and [R73 charged storage](runtime-owned-arguments-and-credits-v1.md). Operation
adoption, publication-time authority, native completion and public async submission
are not implemented by this packet.

## Source And Format

`Gfx942KfdDispatchRequestV1::into_parts_v1` moves all ten mechanics coordinates:
image, descriptor offset, full kernarg, alignment, buffers, fixups, geometry,
private/group segment sizes and timeout. Parts are public inert data; reassembly
must validate again and neither form grants execution authority.

`PreparedGfx942RuntimeDispatchV1::into_persistent_projection_v1` consumes the
prepared request and borrows the original HSACO for independent validation. It
checks the complete object and selected closure identity, descriptor, canonical
materialized image, geometry/resources, complete policy roster and the original
dispatch digest. The canonical image comparison scans the loader's ordered copy
segments and zero-filled gaps without allocating a second image. Rehashing uses
borrowed buffer bytes, not reconstructed buffer copies.

Every original buffer remains in order, including unused storage and untouched
prefixes/tails. Fixups retain their order, buffer indices, interior offsets and
alignment. They map to inspected global-argument ordinals, not source-language
argument numbers. Because one-shot fixups carry no access length, the fixed
binding conservatively covers the remaining buffer suffix. Unsupported aliases
reject; they are never split into distinct allocations.

The projection reconciles a descriptive ABI using the original dispatch digest
as the source-contract identity. This is not a compiler-signature witness or a
new admission decision. No validated HSACO envelope is retained by the projection;
the host's private application authority still owns the authenticated artifact.
Adoption must reconcile against that exact retained source and current native
incarnations again.

## Native Policy Reuse

`project_gfx942_fixed_host_packet_v1` accepts an initialized kernarg packet and
descriptive future coherent-host lengths. It runs the actual fixed-dispatch
metadata policy and independently derives all declared COV6 implicit values.
The complete 256-byte initialized suffix, including padding, must match before
normalization to the native queue's caller-zero template. Native construction
uses the same initializer to restore the values inside retained custody.

Every used and unused length passes the existing ordinary coherent-host layout
routine, including page rounding, maximum size and overflow checks. The existing
fixed planner then validates the complete data roster, ABI, geometry, ordering,
pointer fields and alias policy. These checks reserve no backing and produce no
initialization witness, VM, queue, address or dispatch authority.

The existing fixed profile still requires 1 through 16 retained buffers and one
binding per inspected global argument. Scalar-only, empty/null-buffer, larger
rosters, overlapping aliases and runtime-service hidden-field profiles remain
unsupported by this path. Closing those compatibility gaps needs native/profile
implementation and qualification, not a projection workaround.

## Host Custody

Only Context-bound generated preparation applies the projection. It remains
inside R76's opening/closing currentness scope, after protected application
admission. Standalone preparation retains its existing one-shot format.

The closed `GeneratedRuntimeStorageV1::project_persistent` transition preserves:

```text
projected image, encoded buffers and policies
private decoder and existing R73 result credits
application authority
```

Image, buffer, fixup and policy vectors move without full-byte duplication.
Errors cannot return storage outside the carrier; success, rejection and unwind
dispose payload before decoder credits. No output becomes observable. Existing
host metadata, kernarg, image and policy-copy accounting gaps are not closed.

The exact invocation timeout is retained in the inert recipe. No persistent
deadline is started or enforced here. Adoption/completion must define one
publication-relative deadline, keep observer timeouts distinct, and never renew
the execution deadline on pending polls or deferred publication retries.

## Acceptance And Next Work

The [local evidence](evidence/local-r77-persistent-projection-2026-09-10/README.md)
separates CPU/type gates from native and formal acceptance. Structural fixtures
contain deliberately nonexecutable entry bytes and grant no production authority.
Twelve new unit tests and two compile-fail examples cover consuming storage,
exact source mutations, all hidden-suffix bytes, unused/invalid layouts, aliases,
dynamic LDS/timeout and charged disposal. No new Verus theorem or solver run is
claimed; existing property proofs do not prove this transformation or executor.

GEN-2B-3 still needs Context-bound one-shot adoption and exact publication
authority, including deferred paths, timeout identity and native custody. Before
issue, freeze B4's complete readback and reply reservations. Reuse the encoded
initial buffers as readback destinations or admit an additional overlap peak;
do not use the copying/cropping ordinary allocation-snapshot adapter. B4 then
validates every returned buffer, including read-only storage, before complete
charged decoding/commit. Public APIs and generated graph/drain follow that
composition. Compiler-backed construction, Linux qualification, whole-executor
refinement and matched performance remain open.
