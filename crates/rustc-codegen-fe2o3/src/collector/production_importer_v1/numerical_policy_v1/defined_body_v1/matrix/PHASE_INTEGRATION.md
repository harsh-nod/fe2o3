# Reusable phase and policy root

The source adapter now reuses `bind::matrix_global_brand` for the sealed
`MatrixGlobalAccess` ancestry relation. It does not equate the full reusable
execution brand with the numerical policy root. The source facts retain the
full subgroup brand (including its execution brand), the exact epoch, the
separate root policy brand, and the original shared-reference type edges.
Kernel, target, and launch coordinates must agree. Width remains exactly 64
and the accepted policy remains exactly StrictIeee.

Central callback filters (new, not run by this worker):

- `policy_reusable_matrix_source_contract_gfx942`
- `policy_reusable_matrix_source_contract_gfx950`

These now run full canonical V23 import and live source replay, rather than
stop after source validation. They check root and phase substitutions,
including a well-shaped foreign subgroup from another kernel, exact issuer
E/M/epoch, V22 rejection, and canonical rejection of substituted/flattened E.
The upgraded callbacks have not been run. Their earlier source-only versions
passed centrally; that is not evidence for the new canonical checks.

## Canonical contract handoff

Mounted V23 stores optional exact E alongside existing policy root R. No
ancestry hash graph is serialized: hashes cannot authenticate Rust sealed
impls. The existing source-authenticated Bind/narrowing owner revalidates the
bounded sealed relation against actual source ADTs. E comes from the Matrix
receiver, never a consumer. MatrixAccess and execution claims require E;
policy claims keep R. Full M/epoch, both references and strict policy remain.

Tags 3/4 retain their exact old payloads and direct-root meaning. Tags 5/6
append only E (32 bytes) and require V23. Zero, redundant R, aliased nominal
axes, and changing an already specified E are rejected. The absence of E
continues to mean E=R. No fresh runtime epoch or SSA dominance is inferred
from these type identities.

Pascal's separate KernelMatrixDerive recipe is mounted at V23 tag 7 with its
520-byte payload, closed body validators, and dedicated Matrix consistency
claim. It cannot satisfy Math, scoped MatrixAccess, policy or epoch issuance.
Source attachment and SSA consumption for that issuer remain outstanding.

Common V23 registration, exact admission/decode, type decoder version list,
closed enum/codec dispatch, MIR exclusions and source test exclusions are
mounted. Parent lowerer hooks still needed: add KernelMatrixDerive to the
existing exclusion groups in numerical_policy_math_01/occurrences.rs
(continue) and execution_source_occurrence_01/defined.rs (false).

The mixed29 Flash diagnostic confirms the old source equality was reached.
Mixed30 reaches PointerCoercion before canonical validation. It establishes
neither canonical Flash import nor legalization.

## Next frontier

Mixed30 has eleven low-precision cases at core Option<usize>::zip, including
FP4 attention before its previous LDS terminal. Turing owns the zip gate.
The mounted Global load body/packing/replay checks await an actual full import
past that gate. No LDS clearance is claimed; transpose remains queued.
