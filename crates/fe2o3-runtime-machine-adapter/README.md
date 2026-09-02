# fe2o3-runtime-machine-adapter

This integration-only crate joins a move-only authenticated gfx942
machine-structure receipt from `fe2o3-kernel-analysis` to the exact prepared
dispatch owned by `fe2o3-runtime`. Keeping the join here prevents the host
runtime layer from depending on Pliron/compiler implementation code.

The adapter retains both inputs, checks exact artifact, length, kernel,
descriptor, and entry identities, and delegates execution to the existing
Worker V3-authorized runtime transition. It grants no load or launch authority
and does not establish instruction semantics, compiler refinement, atomic
ordering, collective convergence, KFD behavior, or hardware coherence.
