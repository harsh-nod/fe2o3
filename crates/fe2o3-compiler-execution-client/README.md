# fe2o3 compiler-execution client

This crate owns the bounded Linux `SOCK_SEQPACKET` client state machine for the
protected compiler-execution service. One acquisition first requests exact
subject recovery and, only after a canonical `ReceiptAbsent` response, resumes
the issuer journal from `Ready`, `Prepared`, or `Issued`. It then publishes the
exact signed receipt and returns the complete inert receipt carriage.

The client uses one absolute monotonic deadline, fixed stack packet storage,
strict request/response identity correlation, pinned-policy validation, and no
ancillary descriptors. It grants no compiler, artifact, load, or launch
authority.

The crate also owns the authority-free child-channel handoff used by the direct
rustc parent. Its post-fork callback creates the unnamed `SOCK_SEQPACKET` pair
inside the rustc child, installs only the client endpoint at FD 195, and
transfers only the service endpoint to the parent. Parent admission binds the
transfer to the exact child PID, `SO_PEERCRED`, and a live pidfd under one
absolute deadline. This avoids attributing an outer-Cargo-created socket to
rustc. Binding-wrapper integration, distinct-UID protected service launch,
policy provisioning, HSACO publication, and runtime admission remain outside
this component.
