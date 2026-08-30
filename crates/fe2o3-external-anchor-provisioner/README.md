# fe2o3 external-anchor provisioner

This package owns the measured transition from root provisioning into the dedicated external-anchor
service identity. The static helper accepts no arguments or environment and consumes only fixed
descriptors: a private root bootstrap endpoint, an existing service-owned durable root, an exact
service-owned daemon image, an independent shared lifecycle lease at FD 6,
sealed deployment and provisioning manifests, and a root-owned
signing-key template.

The helper admits its complete locked process profile and exact running image before reading the
key template. It reissues that template as a service-owned sealed capability, atomically opens or
initializes durable state under one retained lock, creates the unnamed nonblocking service
`SOCK_SEQPACKET` after the credential transition, and transfers only the supervisor endpoint over
the private bootstrap channel. It then installs the daemon's exact descriptor contract and executes
the measured daemon with an empty environment. Bootstrap close-on-exec EOF is not sufficient by
itself: the root coordinator must also revalidate the transferred endpoint against the same live
pidfd before admitting service custody.

The exec transition installs the child lease at daemon FD 5. It never issues
`LOCK_UN`; helper and daemon copies share this child open file description until
the exec boundary closes the helper copy and daemon death performs the last close.

The helper grants no compiler, publication, loading, launch, or GPU authority. The separate
`fe2o3-external-anchor-coordinator` crate now owns endpoint/pidfd admission and process lifecycle;
its supervisor wiring and root-only cross-UID qualification remain open.
