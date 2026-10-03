//! Diagnostic only: retain and inspect the exact active-session compiler owner.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
};
use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticOperandV1, SemanticRvalueKindV1, SemanticStatementKindV1,
};
use fe2o3_verifier::{CheckedU32PrefixOriginV1, check_captured_checked_u32_prefix_v1};
use serde_json::{Value, json};

pub(super) fn extract(tcx: TyCtxt<'_>) -> Result<(), String> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(64_000_000);
    let mut budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 64 * 1024 * 1024);
    let owner = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?
    .extract_checked_u32_prefix_owner_v1(&mut budget)
    .map_err(|error| error.to_string())?;
    let retained = owner
        .checked_u32_add_capture_storage_v1()
        .ok_or("checked-u32 extraction missing retained capture storage")?;
    budget
        .reserve_storage(retained)
        .map_err(|error| error.to_string())?;
    let report = inspect(&owner);
    drop(owner);
    budget
        .release_storage(retained)
        .map_err(|error| error.to_string())?;
    eprintln!("fe2o3 checked-u32-prefix extraction: {}", report?);
    Ok(())
}

fn inspect(owner: &ProductionSemanticKirOwnerV1) -> Result<Value, String> {
    let capture = owner
        .checked_u32_add_capture_v1()
        .ok_or("missing live checked-u32 capture")?;
    let relation =
        check_captured_checked_u32_prefix_v1(capture).map_err(|error| error.to_string())?;
    let source = owner.semantic().semantic();
    let request = capture.request();
    let function = &source.functions()[request.function().index() as usize];
    let block = &function.blocks()[request.block().index() as usize];
    let prefix = block.statements()[..request.statement() as usize]
        .iter()
        .map(|statement| match statement.kind() {
            SemanticStatementKindV1::Nop => Ok("nop"),
            SemanticStatementKindV1::Assign(assign) => match assign.value().kind() {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(_)) => Ok("copy"),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(_)) => Ok("constant"),
                _ => Err("accepted prefix contains unsupported assignment"),
            },
            _ => Err("accepted prefix contains unsupported statement"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let origin = match relation.operand_origin() {
        CheckedU32PrefixOriginV1::Argument(index) => json!({"argument": index}),
        CheckedU32PrefixOriginV1::Constant(value) => json!({"constant": value}),
    };
    let mut evaluations = Vec::new();
    for base in [0u32, u32::MAX, u32::MAX - 16, 0x8000_0000] {
        let arguments = (0..relation.arguments().len())
            .map(|index| base.wrapping_add(index as u32))
            .collect::<Vec<_>>();
        let value = relation
            .evaluate(&arguments)
            .map_err(|error| error.to_string())?;
        evaluations
            .push(json!({"arguments": arguments, "value": value.value, "overflow": value.scc}));
    }
    let kir = owner
        .canonical_kernel_ir_v8_identity()
        .ok_or("missing exact V8 owner")?;
    let arguments = relation
        .arguments()
        .iter()
        .map(|argument| {
            json!({
                "argument": argument.argument(), "semantic_local": argument.semantic_local(),
                "kir_value": argument.kernel_ir_value().0,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema": 1,
        "canonical_kir_version": 8,
        "semantic_mir_sha256": lower_hex_v1(source.semantic_sha256().as_bytes()),
        "kir_identity_sha256": lower_hex_v1(kir.digest()),
        "kir_canonical_length": kir.canonical_length(),
        "root": request.root().index(), "function": request.function().index(),
        "source_block": request.block().index(), "source_statement": request.statement(),
        "source_prefix_kinds": prefix, "arguments": arguments,
        "lhs_local": capture.lhs_local().index(), "tuple_local": capture.tuple_local().index(),
        "lhs_ssa": format!("{:?}", capture.lhs_ssa()), "tuple_ssa": format!("{:?}", capture.tuple_ssa()),
        "use_event": capture.use_event(), "define_event": capture.define_event(),
        "kir_function": capture.kernel_ir_function(), "kir_block": capture.block().0,
        "kir_operation": capture.operation(), "kir_operand": capture.operand().0,
        "kir_value": capture.value().0, "kir_overflow": capture.overflow().0,
        "origin": origin, "literal": capture.literal(), "conditional_evaluations": evaluations,
        "first_entry_only": true, "continuation_verified": false,
        "normalization_adapters_proved": false, "authenticates_compiler_execution": false,
        "artifact_or_launch_authority": false,
    }))
}
