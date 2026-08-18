# fe2o3 compiler driver

`fe2o3-compiler-driver` owns explicit routing for the three
`PipelineSelectorV1` values. Each request invokes exactly one configured
backend. A failed or malformed backend transaction is converted to a bounded,
canonical rejected output; the driver never attempts another selector.

Backend outputs are revalidated against the actual request before they cross
the driver boundary. This preserves receipt and obligation chains and prevents
an output validated for one request from being replayed for another.
`PlironShadow` is inspect-only, and any executable candidate returned from its
slot is rejected before output revalidation.

## Authority boundary

This crate routes in-memory compiler API contracts. It does not invoke COMGR,
publish artifacts, read or write artifact stores, load modules, dispatch work,
or launch kernels. An output candidate remains opaque and grants none of those
authorities.
