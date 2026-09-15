# Mutable Context Issuance Checkpoint

Mounted after the checkpoint22b actual Workgroup callback reached canonical
lowering and rejected retained local8/type13. Its diagnostic identifies an
existing compiler-issued capability borrow still classified as storage-observable.
The real source calls `KernelContext::with_workgroup(&mut self)`, which forwards
and reborrows that receiver into the authenticated WorkgroupDerive intrinsic.
The typed context classifier previously covered shared context consumers only.

`context_issue.rs` adds `WorkgroupContextBorrowV1::for_callable` and `accepts`.
Admission requires the actual selected WorkgroupDerive callable, exact binding
identity, `[reference] -> workgroup` signature, UniqueBorrow source ownership,
a mutable thin Reference with the retained owned pointee, and retained brand and
epoch. This is address transparency only, not context issuance or lifetime proof.

The borrowed adapter now admits only a single-consumer mutable forwarding and
reborrow chain for those exact pairs. Shared subgroup/epoch forks remain separate.
Raw/shared/fake substitutions, escapes, duplicate definitions and mutable forks
are not added to the transparent-site set. Ordinary SSA still consumes the real
context reaching definition and StorageDead events. The actual borrowed lowerer
still checks the owned Workgroup SSA issuer, loans, epoch and source occurrence.

Mounted Rust files:
- `adapter.rs`: enable the typed classifier for WorkgroupDerive as well as BF16.
- `adapter/borrowed_workgroup_v1.rs`: exact mutable context facts and closed flow.
- `adapter/borrowed_workgroup_v1/context_issue.rs`: new bounded terminal predicate.
- `adapter/borrowed_workgroup_v1/tests.rs`: mount the complete child tests.
- `adapter/borrowed_workgroup_v1/context_issue_tests.rs`: four focused tests.

No lowerer, borrowed resolver, KIR, schema, source fixture, ranked gate or machine
authority changes. Pascal's WGIndex resolver work and Pauli's shared Context
classifier are untouched. Max's extension stays unmounted and paused; its staged
patch must be revalidated against current anchors before any later mount.

Direct pinned rustfmt syntax checks and git diff --check passed. No Cargo, SSH
or network was run. Parent's reported Pliron23b 219-pass result is not attributed
to these four tests without confirmation that its build included them.

Parent commands:
`cargo test -p fe2o3-pliron --lib context_issue_tests -- --nocapture`
Then rerun the unchanged actual AMD callback
`workgroup_full_import_gfx950_v20` with the cached metadata environment.
No claim that the actual callback or canonical/backend/simulator path now passes.
