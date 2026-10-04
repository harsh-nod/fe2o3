# fe2o3-host-api

`fe2o3-host-api` defines the inert, target-neutral boundary used to describe
compile, admission, load, dispatch, and wait operations. It contains data
contracts and structural validation only. It does not compile, admit, load,
dispatch, wait, authenticate identities, or grant authority to do so.

The V1 contracts provide:

- distinct request, result, state, and event identity domains for every flow;
- bounded payload descriptors, diagnostics, claims, bindings, dependencies,
  wait sets, observations, and event batches;
- exact request/result and predecessor/state binding across each operation;
- explicit causal event sets and dispatch completion dependencies, allowing
  independent operations to proceed in parallel without a global serial
  cursor; and
- finite and issue #135 persistent-task dispatch descriptions without target,
  runtime, queue, or device handles.

All identities are caller-supplied, fixed-width commitments. The crate exposes
canonical, domain-separated identity preimages so an integration-owned digest
profile can derive them, but it neither hashes nor authenticates those
preimages. Shape-valid records are not proof, publication, admission, load,
launch, dispatch, completion, cancellation, quiescence, or progress evidence.

The crate deliberately defines no wire decoder. A future wire format must be
separately versioned, reject oversized lengths before allocation, reject
unknown mandatory fields and trailing bytes, and receive compatibility tests.
