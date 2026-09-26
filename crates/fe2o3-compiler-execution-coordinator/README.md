# fe2o3 compiler-execution coordinator

The production composition below currently uses V1 owners. Native V2/V3 trust
and preparation now compose genuine deployment/policy/key custody, three freshly
sealed images, listener/root inputs, root-bound lifecycle leases and a managed
anchor on the original resource ledger. Preparation creates no process: native
inherited composition, supervisor startup and consuming launch remain unfinished.
These APIs do not establish 47/47 safe GPU launch. See the
[native preparation checkpoint](../../docs/evidence/conditional-native-compiler-preparation-20260926.md).

## Native Preparation

`CompilerExecutionSupervisorTrustV2/V3` bind actual same-family deployment,
policy and signing-key owners. `PreparedCompilerExecutionSupervisorV2/V3`
consume this trust plus the native service inputs, two native lifecycle leases
and a genuine managed anchor. Both leases must protect the canonical sibling
of the retained service root; independent validity alone is insufficient.
Preparation pins exact root credentials, process and namespaces, and the three
sealed supervisor/launcher/issuer images against their actual contexts.

The input-storage and quota queries include complete source ownership, cumulative
image growth and nested scratch. Reserve returned growth before retaining an
owner, keep all consumed charges live, and retire the full retained charge only
after Drop. Calls restore entry storage without resetting work or denial history.
No V1 authority conversion, raw-descriptor accessor or public signing operation
is exposed. The V1 program-source bundle is reused only as three untrusted Files.

## Existing V1 Deployment

This package owns the sole root-to-protected-supervisor deployment transition.
It admits and pins the exact supervisor, static pre-exec launcher, and issuer
images against the canonical deployment and issuer policy; retains the exact
production listener, service-owned root, root signing-key template, and live
independently administered external anchor; and launches through
`fe2o3-protected-service-spawn` without a second credential or child-lifecycle
implementation.

The child establishes the complete locked non-root profile before root releases
its gate. Root independently revalidates credentials, namespaces, every pinned
input, the anchor, and all retained object identities before release. The
supervisor receives only its fixed descriptor contract, one argument, and an
empty environment. Success requires the canonical private-bootstrap readiness
record, exact child PID and deployment identity, bootstrap EOF, and continued
pidfd liveness.

The returned move-only service retains exact supervisor pidfd/reaping custody,
all deployment continuity inputs, and root custody of the anchor occurrence.
Dropping it terminates the supervisor before the retained anchor is dropped.

`InheritedCompilerExecutionDeploymentV1` is the sole production input
composition. It requires exact UID/GID 0, must run before any second process
thread exists, and takes this dense descriptor set:

| FD | Content |
|---:|---|
| 3 | Root-owned, service-group mode-`0660` production listener |
| 4 | Existing supervisor-service-owned mode-`0700` state root |
| 5 | Existing anchor-service-owned mode-`0700` state root |
| 6 | Root-provisioned static supervisor image |
| 7 | Root-provisioned static issuer pre-exec launcher |
| 8 | Root-provisioned static issuer image |
| 9 | Root-provisioned static external-anchor helper |
| 10 | Root-provisioned static external-anchor daemon |
| 11 | Canonical supervisor deployment |
| 12 | Canonical issuer policy |
| 13 | Canonical external-anchor deployment |
| 14 | Canonical external-anchor provisioning manifest |
| 15 | Root-owned issuer signing-key seed |
| 16 | Root-owned external-anchor signing-key seed |

Executable sources must be root-owned, root-group, single-link mode `0555`
read-only regular files. Public records use the same constraints with exact mode
`0444` and canonical lengths. Seeds are exact 32-byte mode-`0400` files. Every
source excludes file capabilities and POSIX ACLs. Records and seeds are read
twice under repeated metadata validation; seeds are compared in constant time
and every temporary copy is zeroized. The records are then copied into the
existing sealed capabilities, executable measurements are enforced by the
existing coordinators, and launch remains anchor-first and supervisor-second.
No inherited source descriptor is transferred directly to either service.

Before either key seed is read, admission opens three separate shared-lock open
file descriptions for the canonical lifecycle file. The coordinator retains
one, transfers one to supervisor FD 12, and transfers one to anchor-helper FD 6.
No lease uses explicit `LOCK_UN`; last close releases each description. A killed
coordinator therefore cannot admit the exclusive provisioner while either
protected child remains alive.

`fe2o3-compiler-execution-coordinator` is the sole root process entrypoint. It
accepts no arguments, requires exact systemd activation metadata for the dense
descriptor table, clears its environment, binds the fixed Unix socket without
calling `listen(2)`, and synchronously handles termination while revalidating
service continuity. The bound descriptor is transferred to the protected
supervisor, whose distinct UID activates it. `deployment/` defines the matching
service, account, and directory policy; no socket unit exists. The release build gate emits a
static `ET_EXEC` image with no interpreter, dynamic section, runtime dependency,
RPATH, RUNPATH, undefined symbol, or executable stack.

`fe2o3-compiler-execution-provision` is the sole same-host reference
provisioning command. It takes one nonzero canonical decimal policy generation
and otherwise uses only fixed service names, image paths, output names, and
filesystem policy. It creates missing signing seeds, derives the complete
policy/supervisor/anchor/provisioning record graph and Cargo-facing client
profile, and uses durable no-replace publication. The public profile is derived
only from the exact graph, is never supplied to the coordinator, and is
published root-owned, single-link, and mode `0444` at
`/etc/fe2o3/compiler-execution/client-profile-v1`. Before reading or publishing
mutable deployment state it retains an
exclusive lease on the dedicated root-only lifecycle file and its root-owned
parent pathname. The production coordinator derives that sibling from its
existing state-root descriptor, takes the corresponding shared lease before key
admission, and gives both protected children independent shared custody. This leaves the
issuer's independent state-root singleton lock unchanged. An exact rerun is
accepted; substitution is not overwritten.
