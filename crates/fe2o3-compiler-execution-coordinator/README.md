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
