# Superseded Context Transfer Experiment

UNMOUNTED after coordination with Ram. The original semantic MIR must remain
unchanged. Production now retains Ram's HIR initializer/use evidence through
`workgroup_source_v1/context_entry_source_v1.rs`, `context_entry_transport_v1.rs`,
and the lowerer's `kernel_context_entry_01.rs`. Do not remount this rewriting
experiment: it would erase the input to that source-custody check. The seven
standalone tests below describe this rejected normalization approach, not the
current production path. The authenticated logical-helper identity and custody
mutation regression remain shared infrastructure used by Ram's path.

## Historical Experiment

Mounted before semantic MIR admission, after all rustc bodies/callables are
constructed. No MIR schema change, terminal substitution, or ranked-origin
exception. Existing nonconstant source operands remain unchanged.

The collector now retains the canonical identity of the already-authenticated
logical helper. It participates in move-only context custody (digest domain v2).
The importer rederives the existing issuance digest from the exact root identity,
original rustc body digest, retained issuer block, terminal identity, and nominal
kernel marker before constructing a private source-boundary record.

Only the generated wrapper's unique direct helper call may replace its exact
ignored-ABI context ZST operand with a Move of the retained issuer result. The
issuer must be at entry, have the exact no-argument ignored return ABI, and lead
directly to the helper on its sole normal incoming edge. The result must be an
exact whole temporary; the helper must take that exact context at ordinal zero.
Remaining operands must forward physical arguments in order. Bypasses,
backedges, duplicate issuers/helper calls, and intervening result lifetime or
value changes reject. This is not an arbitrary helper/constant origin resolver.

Original function/block/local/call identities, source coordinates, other
operands, and the entire defined helper body are retained. The canonical Move
is then checked by ordinary direct-call replay, SSA move/lifetime accounting,
ranked shared-borrow origin transfer, and lowerer source attribution. No constant
by itself acquires authority. A later source use of the moved result is still
subject to the ordinary move checker.

## Checkpoint

- Seven component regressions pass via standalone rustc against cached MIR.
  The positive admits both source and normalized documents and replays the exact
  expanded Move followed by the retained shared borrow.
- Added a central-only custody helper-identity substitution regression.
- Existing actual AMD `policy_math_all13_source_kir_gfx942` and `gfx950` callbacks
  now additionally assert that the real generated helper receives the exact
  retained issuer destination as a Move. All ranked, SSA, KIR, and mutation
  assertions remain intact. These callbacks still require the parent's rerun;
  the standalone component is not production qualification.
- No Cargo, dependency rebuild, or source-snapshot edits by this worker.
