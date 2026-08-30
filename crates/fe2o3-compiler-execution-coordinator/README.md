# fe2o3 compiler-execution coordinator

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

`fe2o3-compiler-execution-coordinator` is the sole root process entrypoint. It
accepts no arguments, requires exact systemd activation metadata for the dense
descriptor table, clears its environment, and synchronously handles termination
while revalidating service continuity. `deployment/` defines the matching
socket, service, account, and directory policy. The release build gate emits a
static `ET_EXEC` image with no interpreter, dynamic section, runtime dependency,
RPATH, RUNPATH, undefined symbol, or executable stack.
