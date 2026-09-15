# Checked Execution Source Carrier Checkpoint

## Mounted Terminal Path

`lower_one_semantic_function_v1` now takes `&ProductionSemanticSsaOwnerV1`
as its first argument, replacing the inert semantic argument. It constructs
`CheckedExecutionSourceCarrierV1::new(owner, plan.correspondence_owner, function)`
and installs it in `SemanticFunctionLoweringV1::execution_source_carrier`.

`execution_expansion_identity` retains its existing per-root view meaning.
`execution_source_expansion_identity` separately carries the whole expansion
identity from `execution_expansion_identity_v1(owner)`. Both are exact lookup
inputs. Original caller function/block remain the legacy source fields.

`lower_execution_capability_v1` now requires `source_for_call`; absence is an
error for both expanded and unexpanded functions. It does not construct a source
record from local block numbers. Existing signature, authenticated context,
root provenance, resource and operation checks remain in place.

The carrier maps surviving ExecutionCapability/PolicyMathF32 compiler intrinsic
calls only, after owner replay, exact execution-function comparison, Source
origin/original callee match, caller-instance match and exact source/call ABI
type checks. Synthetic entries/returns cannot use this method.

## Pauli: Defined Math Attribution

`CheckedExecutionSourceCarrierV1::source_for_defined_statement` takes:

1. `owner: &ProductionSemanticSsaOwnerV1`
2. `root: SemanticFunctionIdV1`
3. `function: &SemanticFunctionDeclV1`
4. whole `expansion_identity: [u8; 32]`
5. per-root `expanded_root_identity: [u8; 32]`
6. `expected: &SemanticExpandedDefinedCapabilityV1`
7. expanded `block: SemanticBlockIdV1`
8. `statement: u32`

It returns an inert `ExecutionCapabilitySourceV1`. Owner replay rederives and
exactly matches the full binding, original caller/callee/ABI, both instances,
the specific CallEntry origin, and the actual emission statement. Accepted
sites are KernelMathDerive's exact ReturnTransfer and PolicyMathBind's retained
entry/statement-zero aggregate assignment. WorkgroupEpochProjection and generic
CallEntry/Return sites reject. The wire uses original caller coordinates plus
the actual emission block and caller instance. It does not encode a fabricated
intrinsic call or use the callee instance as the caller instance.

This factory is not Math authority and is not connected to Math dispatch.
Pauli must retain its separate same-context SSA issuer/loan/dominance checks and
compare the expected binding to its plan. Positive defined-Math owner tests
remain to be integrated with Pauli's real SSA fixtures; no source-to-KIR Math
success is claimed by this checkpoint.

## Ram: Wire Coordination

Some(occurrence) selects payload revision 5, with a mandatory 104-byte suffix
and u32 obligations. None preserves old revisions 1/2/4 byte-for-byte. Ram's
borrowed operation keeps tag 25 and revision 3 for None; Math keeps tag 26.
Revision 5 must bypass the legacy operation-family/revision equivalence checks,
but never its bounded payload, required occurrence, completeness or trailing
byte checks. The 2048-byte ceiling is unchanged. Current maximal scoped raw
binding is 1030 bytes legacy / 1136 expanded; four-argument Math is 1011 / 1115.

## Parent Test Filters

- Kernel IR: `source_occurrence::tests` (six tests).
- Lowerer: `execution_source_occurrence` (four owner tests and one ABI test).
- Compiler: `final_write_contract_requests_v1::tests` (the four failing fixtures
  corrected; integer overflow/domain rejection assertions retained).

No Cargo, builds, network or hardware execution was run by this worker.
