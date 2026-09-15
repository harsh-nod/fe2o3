# Global FP4/FP8 Loads: Checkpoint 30

This child and its exact four-helper hooks are mounted for the parent's central
compiler checkpoint. No Cargo, binary build, callback, or qualification was
run by this worker. All repository edits are frozen pending parent release.

## Representation

All four real Global load bodies remain Defined functions. There is no new
terminal tag, MIR/KIR version, BF16 alias, slice-backed view conversion, or
issued fragment authority. The original wrapper receives `&GlobalMatrix`, the
matching `&WaveLane<Wave64, MatrixBrand>`, and two usize tile bases. Its output
is the exact format/role/MatrixBrand fragment with eight u32 registers.

The closed wrapper checker follows the real normal call edges and tracks
copies, moves and shared reborrows. It requires the original ordered chain:
`SubgroupLane::get`, exact format/role `pack_fp*_*`, then
`Gfx950MfmaFragment::from_registers` using the same lane reference. No argument
can be manufactured from a constant or another input. Each callee must have
the exact reviewed provider identity and source ABI.

The full Global view layout and fragment layout retain all nominal fields.
The full subgroup/phase brand and epoch remain in both reference and result
types. The separately retained allocation root must be the exact sealed
MatrixGlobalAccess ancestor. Source collection does not itself prove lane
issuance, liveness, convergence, memory bounds or numerical legalization.

Canonical validation checks original source/function/type/ABI rosters, exact
helper edges and authenticated kernel root. It requires the reachable checked
read to remain the existing `CapabilityGlobalLoad` from the exact
`&Global<u8, ReadOnly, Root>` with `Option<u8>` result and compares its canonical
memory operation against the live source adapter. Original transitive bodies
are replayed under the existing shared body-owner budget. That retains all
packing operations, loop/assertion/zero-fill branches, checked address
arithmetic and Option handling. No new memory proof is granted: ranked and
SSA memory verification must process those retained reads normally.

The existing eight logical-value tests remain relevant: one byte per logical
FP4 value, low-nibble packing, zero upper four dwords, both FP8 depth halves,
all lane/item coordinates, strided addressing, and no read on zero fill.

## Mounted Hooks

1. The `loads` child in `global_views.rs` has importer visibility.
2. `loads::validate_source` runs before the constructor-only source check,
   and `loads::validate_canonical` before the constructor-only canonical check.
3. Only the four exact reviewed Global load markers were added in
   `is_traversed_reviewed_helper_v1`. The exact roster test is now named
   `global_fp4_fp8_helpers_retain_bodies_and_legacy_gates`; legacy slice loads,
   LDS transpose and MFMA handling are unchanged.
4. `Fixture::GlobalLoads` is mounted in the existing registered callback harness,
   using this `fixture.rs`. It reuses live plan/transcript equality, cwd=repo,
   scratch out-dir, all constructor checks and normal full-import checks.
   and invokes `loads::check_import` for the load fixture. Mounted filters:
   `policy_global_fp4_fp8_load_full_import_gfx942` and
   `policy_global_fp4_fp8_load_full_import_gfx950`.

The child tests check source generic substitutions, swapped same-typed tile
bases, broken normal-edge sequence, unwind substitution, exact layout axes,
changed callable identity, erased packing assignments, erased zero-fill/loop
guard and erased checked-read call, retaining labels and ABI where applicable.

## Cross-Owner Dependencies

Actual `ByteMatrix::value_or_zero` calls `Option::zip`. Current core Option
authentication has and_then/unwrap_or, but no zip case was found. Relay to
Turing for a closed original-core-source helper if the mounted callback
confirms that next gate. No cross-crate authorization waiver is appropriate.

Reusable phase canonical Matrix Bind still has the separate phase/root gate
described in `matrix/PHASE_INTEGRATION.md`. LDS transpose remains queued.
MFMA and downstream byte-read/index/memory obligations are not cleared by
this source checkpoint.
