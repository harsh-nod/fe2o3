//! A hidden caller location can be omitted from an execution frame only when
//! the closed admitted body cannot observe it. The original FnAbi is retained.

use super::*;

mod literal_divisor_v1;

pub(super) fn require_unobserved(
    source: &AdmittedInertSemanticMirV1,
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    call: &SemanticDirectCallV1,
    site: (SemanticFunctionIdV1, SemanticBlockIdV1),
    budget: &mut Budget,
) -> Result<()> {
    let hidden = function.abi().hidden_arguments();
    if hidden.is_empty() {
        return Ok(());
    }
    // Admission has already checked this value's exact pointee/layout/physical
    // attributes. No source argument or MIR local represents this ABI-only input.
    if function.abi().extern_abi() != SemanticExternAbiV1::Rust
        || hidden.len() != 1
        || hidden[0].role()
            != SemanticAbiArgumentRoleV1::Hidden(SemanticAbiHiddenArgumentRoleV1::CallerLocation)
    {
        return Err(unsupported(
            function_id,
            None,
            "defined call has unsupported hidden ABI arguments",
        ));
    }

    // Check every block, including unreachable blocks. A defined callee that
    // also receives caller location must pass this check when its frame is
    // expanded; recursion/depth/work bounds remain the ordinary expander's.
    let mut certificate = None;
    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.work(1 + block.statements().len())?;
        let location = Some(SemanticBlockIdV1::from_index(block_index as u32));
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                if !matches!(
                    source.callables().get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::Defined { .. })
                ) {
                    return Err(unsupported(
                        function_id,
                        location,
                        "caller-location frame reaches an opaque call",
                    ));
                }
            }
            // Only a complete per-call proof established here can omit the
            // hidden input. Later assertion discharge is not an exemption.
            SemanticTerminatorKindV1::Assert { .. } => {
                if certificate.is_none() {
                    certificate = literal_divisor_v1::certify(
                        source,
                        function_id,
                        function,
                        call,
                        site,
                        budget,
                    )?;
                }
                if certificate
                    .as_ref()
                    .is_some_and(|proof| proof.covers(source, function, call, site, block_index))
                {
                    continue;
                }
                return Err(unsupported(
                    function_id,
                    location,
                    "caller-location frame contains an observing assertion",
                ));
            }
            SemanticTerminatorKindV1::TailCall(_)
            | SemanticTerminatorKindV1::Drop { .. }
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate => {
                return Err(unsupported(
                    function_id,
                    location,
                    "caller-location frame contains unsupported implicit control flow",
                ));
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::SwitchInt { .. }
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
    }
    Ok(())
}
