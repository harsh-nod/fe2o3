# Rooted Native Host Backing V1

This is MEM-DOM native-adapter development above the
[shared-domain engine](runtime-resource-domains-v1.md), not complete memory
closure or HIP/HSA parity. The existing ordinary coherent GTT (N1) cost and
disposal adapter now accepts an opaque root-issued session account.

## Construction And Identity

`Gfx942HostBackingRootV1` owns a fixed-capacity canonical-device registry.
Its capacity is a resource vector; ordinary backing consumes
`ResidentHostAllocationBytes` and `AllocationRecords`. `ControlResidentBytes`
must also accommodate the generic root's fixed arena baseline and the registry
slot payload. `bootstrap_bytes_v1` computes that exact Rust payload; construction
checks the full amount before allocating either arena. The registry consumes
one generic credit record, but no native
`AllocationRecords` unit. `max_records` must include that extra row.
`max_domains` bounds root, registered device parents and live/retained session
leaves together. Registered device entries are not recycled.

The root admits only a borrowed `CheckedGfx942XnackMinusDevice`. Production
UID/PCI extraction uses that retained capability's checked observations, not
caller-provided topology or a standalone observation copy. The underlying
admission remains conditional on the existing kernel/sysfs/ioctl contracts.
Registering another generation of the same UID/PCI reuses the same parent.
Conflicting UID/PCI aliases or changed device limits reject; concurrent
registration is serialized. A new session gets its own N1-only child, not the
Context's logical-request ledger. This keeps native record counts and session
limits distinct, within the existing three-level hierarchy.

The opaque move-only admission binds the root, UID/PCI, exact model device
generation, leaf and session budget. Native intake checks the checked-device
binding before allocating a VM identity or entering `begin_process_vm_attempt`.
The actual adapter additionally checks the session/device/VM identity. Existing
native currentness checks are unchanged; an accounting admission is not a
currentness, launch or disposal certificate.

The exact key binding relies on the current checked-device mint: its admission
generation is process-global and nonwrapping, retained history rejects domain
changes, and the production profile is fixed. Public model-only tokens cannot
be installed into a checked device. Forked inherited capabilities still require
the existing opener-PID/currentness checks; key equality alone does not admit
them. Any future alternate checked-device mint must preserve these guarantees
or bind the complete model admission instead.

Runtime `open_default_with_host_backing_root_v1` and
`from_checked_device_with_host_backing_root_v1` require the typed root, not an
arbitrary account handle. SDMA-first, ordinary compute-first and generated
adoption startup all move the same preissued leaf into native memory acquisition.
Generated DATA initialization and primary construction retain that same session.
Rooted mode remains explicit
after the token is consumed: missing/mismatched admission fails terminally,
never by falling back to local or unconfigured accounting. Late local-budget
replacement is rejected. Existing constructors retain their previous behavior.
Constructor saturation/allocation failures are classified as capacity errors;
invalid binding, corrupted state and exhausted identity generations are terminal
for that construction attempt, not retryable capacity refusals. A conflicting
configuration does not itself poison the shared root.
This preflight admits the identity and account, not a complete native-bootstrap
allocation roster. N1 backing saturation later in construction may occur after
VM acquisition and retains the existing conservative runtime failure handling.
Whole-constructor capacity reservation before effects remains separate work.

## Native Custody

The production N1 allocator reserves the existing exact page-padded cost before
currentness/VA reservation/native allocation. Its retained record keeps that
debit through mapping, queue ownership, pool generation/logical-size changes
and disposal. All ancestors are updated by the shared coordinator; a copied
usage snapshot is not a second physical debit.

Admission and the native account's shared domain retain the typed root.
Uncertain charge destruction anchors that same typed root before generic credit
quarantine, preserving the canonical registry even after outside handles and
native owners are dropped. Only successful confirmed disposal disarms anchoring;
wrong identity, accounting error or unwind cannot do so. Anchoring clones into
an existing Arc slot, without allocating another quarantine record. Successful
disposal followed by final owner destruction frees the registry normally.

## Limits And Verification

The [development evidence](evidence/dev-native-root-admission-2026-09-26/README.md)
distinguishes private registry tests, actual adapter/ownership paths with a fake
backend, runtime mock failures and source-routing assertions. No fake checked
Linux device is introduced. The process's single-VM-attempt-per-GPU restriction
is unchanged: sibling fake sessions do not establish supported native same-GPU
multi-Context concurrency.

Device/session N1 budgets retain their existing 8 GiB/256-record envelope.
The root can aggregate multiple admitted devices, but this is not a new
multi-device execution constructor. N2, executable/kernarg/AQL/userptr backing,
complete VM/Context bootstrap, metadata wrapper and allocator/Arc overhead,
image accounts, scaled tables and closed terminal payloads remain separate.
The registry slot payload is charged, not all typed-root wrapper storage.
Legacy constructors and unrelated roots remain possible, so no process-global
ceiling is established. Registry poison prevents new registrations; already
issued accounts retain their independent native lifetime obligations.

No new formal hierarchy/registry/native correspondence proof or matched
HIP/HSA performance result is established by this implementation. Native
qualification, root-required integration for the remaining resource classes,
whole-profile bootstrap and formal refinement remain open.

The subsequent [compound N1/N2 profile](runtime-compound-native-backing-v1.md)
implements a separate four-level root/device/session/class hierarchy,
preserving both class and combined session ceilings. It supplies all-or-none
adapter installation, all three runtime startup routes and both ordered XGMI
endpoint admissions. It does not upgrade the N1-only API above. Whole-bootstrap
reservation, logical/native composition and correspondence proofs remain open.
