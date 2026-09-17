# Production Safe Launch Integration

This checkpoint adds prerequisites for the ordinary Rust-source to safe GPU
launch path. It does **not** complete that path, qualify any tutorial kernel,
or replace a protected semantic proof with simulation or structural inspection.
The working target is gfx942; gfx950 needs its own admission and validation.

## What Is Connected

- The scalar-output obligation census joins the exact Policy3 output owner,
  source/output occurrence custody, and a fresh formal memory report. It checks
  forward and reverse effect coverage and retains assertion/trap obligations.
  Conditional assertions remain unresolved. The closed initial subset is one
  rank-one entry with scalar global memory effects and source assertion traps;
  private memory, helper bodies, synchronization, and advanced GPU operations
  are rejected. Source replay and occurrence correspondence are structural
  checks, not numerical equivalence with an arbitrary Rust reference. This is
  not final production admission.
- The gfx942 EXEC interpreter checks exact instruction bytes, decoded operands,
  effects, branches, and explicitly supplied register live-ins. Work and trace
  storage are bounded. It uses the existing machine trace, not another IR.
  A runtime-machine adapter associates this analysis with a Worker V3 request
  by complete artifact bytes, selected symbol, code range, and selected ISA.
  Observations are conditional on live-ins and grant no launch authority.
- The inherited compiler-current-record auditor requires an independently
  supplied issuer/anchor policy pin. A mismatch rejects before sending and
  consumes the one-shot endpoint. A caller-supplied pin is not itself proof of
  administrator-controlled deployment or protected key custody.
- Generated KFD preparation now checks source/physical launch constraints and
  consistency of packed buffers, slice lengths, fixups, and completion storage.
  Each slice length must match its own allocation. This does not establish
  kernel-dependent relationships between different argument lengths.
- Explicit reviewed-host tests open the real protected Verus runtime and test
  both acceptance of a true proof and rejection of a false proof. Missing
  provisioning fails a selected test; there is no alternate test-runtime path.

The EXEC transition expressions are shared with six conditional Verus lemmas.
These lemmas concern mask arithmetic, SCC, and branch senses. They do not prove
decoding, ABI live-ins, memory behavior, floating-point results, termination,
compiler refinement, or agreement with physical hardware.

## Remaining Critical Path

1. Admit the actual optimized Policy3 graph as the final production owner.
   Preserve source/ranked custody and discharge all source/control/effect
   obligations on that same graph. Version downstream descriptor and lineage
   consumers together; do not relabel the historical pre-optimization receipt.
2. Prove input/output bounds, assertion outcomes, indexing, and value semantics
   for the ordinary vecadd source. Extend coverage to helper/private-memory,
   wave/workgroup synchronization, and advanced operations with explicit
   invariants. Memory-analysis completeness is not absence of traps.
3. Prove the selected emitted machine code refines that admitted graph,
   including ABI initial state, active lanes, addresses, IEEE behavior, and
   completion. Numerical approximation needs explicit proved bounds. An EXEC
   observation or artifact digest is not this theorem.
4. Implement the concrete protected-verifier and semantic-machine-refinement
   backends, retaining independent deployment policy and current-record
   authority. Keep all existing failure gates until their premises are met.
5. Connect ordinary compilation, finalization, generated host binding, proof
   admission, and completion without fixture substitution. Run vecadd, then a
   second differently shaped kernel, then the full tutorial roster.
6. Provision the pinned protected runtime in an administrator-controlled
   compatible environment and archive exact-source proof and GPU evidence.

As checked on 2026-09-17, both MI300X and MI350 are reachable, but neither has
the required protected runtime installation. User-owned Verus binaries do not
meet that deployment requirement. Provisioning alone also does not implement
the missing compiler/refinement backends above.

## Validation

Validation commands and checkpoint results are recorded with the branch's
integration report. Unit fixtures, offline machine interpretation, development
Verus proofs, protected proofs, and GPU runs must be reported separately.
