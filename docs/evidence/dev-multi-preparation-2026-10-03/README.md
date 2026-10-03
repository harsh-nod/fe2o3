# Exact-Child Multi-Device Preparation

This checkpoint extends `74fdecca17c62a5d87f873522b55c52252c42bdd` with inert
preparation on `RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>` and its owned
async progress handle. It closes the first multi-device application bridge,
not generated execution or A3.

## Implementation

The backend resolves the exact UID, checks child/index/occupancy consistency,
checks selected-child liveness before occupancy, and delegates to the unchanged
retained-device preparation scope. Unknown or occupied selections reject before
the callback; an unrelated occupied pair does not block a free child. Corrupt
routes, terminal child results and callback unwind seal the router. The existing
Context boundary quarantines submission writers on terminal failure or unwind.

Only inert preparation and validation are shared between concrete Context
implementations. Binding retains Context generation, logical device, backend
UID and exact native admission. The immutable checked-device borrow cannot
escape. Payloads remain owner-local and non-extractable. Plain async tickets
reuse existing custody, cancellation, capacity, discard and shutdown behavior;
they have no generated reservation/adoption/issue/completion hooks.

## Qualification

- 2359 runtime tests pass, with 32 hardware tests explicitly ignored.
- 170 example tests and 71 doctests pass, including borrow nonescape, immutable
  owner access, non-Send capture rejection and absent multi-device generated APIs.
- Strict library and witness Clippy, no-default library check, formatting and
  all 33 source-control workflow commands pass.
- Seven new runtime tests cover exact routing, identity and route corruption,
  terminal-versus-busy precedence, unrelated-child isolation, callback nonentry,
  error/unwind propagation, Context binding and synthetic async-owner rejection.

The native no-queue witness passes on GPUs 6/7 in both selected UID orders:
`0x10a254ce4987e716` and `0x53691ef168a0147d`. Each process uses the production
deny-all kernel constructor and one owned engine. It prepares both retained
devices, checks exact UIDs, rejects callback errors without poisoning the Context,
and revalidates both original wrappers in reverse order. Two async `Rc` payloads
remain on the owner thread: unsupported reservation preserves the first ticket,
explicit discard drops it once, and shutdown drops the second once after ticket
loss. The inspected shutdown report requires complete cleanup and release.

The controller pins the ELF by open descriptor and SHA256, bounds each process,
and records 18 command receipts with process-group closure. Eight fresh endpoint
observations show the selected pair idle and unattached before/after each case.
These are shared-host point observations, not exclusive reservations. No kernel,
VM, queue or GPU allocation is created by this witness.

Native ELF SHA256:
`c38cc15c0b28b2d7b8b2bafd862156f75eba9917d385ed49f297e93405c09b38`
(20834280 bytes). Results were retrieved before the hash-bound cleanup helper
confirmed no owned processes and removed only
`/tmp/fe2o3-multi-preparation-20261003.9YSpUqgJ`. No foreign resources were changed.

## Evidence Limits

The archive retains command/environment/source receipts, all native observations,
test output and roster, executable identities, source patch, metadata audit and
qualification tools. The accepted source inventory has 6370 files. Nine source
guards change only 16 hash literals; inventory counts are unchanged and all 76
listed executable proof files remain baseline-identical. This is not a new
solver run or formal refinement of the multi-device adapter. Native preparation
after queue materialization, generated launch authority and performance are not
qualified here. The initial build receipt spans metadata refresh and is diagnostic
only; the final build and all acceptance runs bind the same unchanged source.

The next bounded milestone is reservation, global/child-local shell routing,
nonpublishing DATA adoption and exact retirement. Pristine adopted DATA must
exclude peer owner extraction even before it appears in generated-submission
indexes. Issue/completion routing, authenticated generated-storage peer handoff,
and deployment-approved application-kernel authority remain separate work.
