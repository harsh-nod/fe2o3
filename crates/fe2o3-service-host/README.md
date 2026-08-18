# fe2o3-service-host

`fe2o3-service-host` is the authority-free P1 host/model adapter boundary for
issue #135. It consumes `fe2o3-service-model` and `fe2o3-host-api` and defines:

- exact service run, service epoch, queue allocation epoch, slot, and logical
  and encoded generation bindings;
- borrow-retaining prepared, starting, running, draining, stopping, stopped,
  and classified failure typestates;
- structurally validated persistent-task submission tickets and terminal wait
  observations; and
- independent access to cancellation, quiescence, and progress property
  classifications without implication or promotion.

The crate is `no_std` and contains no raw handles or unsafe code. It performs
no HSA/HIP allocation, artifact load, kernel launch, queue publication or
execution, runtime wait, persistence, authentication, proof, quiescence
attestation, progress inference, or storage release. A successful check means
only that caller-supplied inert records have the required V1 structure.

Live typestate values retain Rust borrows of queue, state, input, and output
storage. Only stopped or quiesced-failure typestates expose the conversion that
returns those borrows. This is a source-level ownership shape, not runtime
evidence: external admission must still reject forgetting or dropping live
descriptions and must establish the applicable shutdown/failure policy.
