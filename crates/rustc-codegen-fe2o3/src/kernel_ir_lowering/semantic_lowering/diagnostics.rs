//! Authenticated gfx942 inline operations and bounded device diagnostics.
//!
//! The source ABI is exactly lane-local u32 values. Operations are
//! non-convergent, carry no synchronization semantics, and fail closed outside
//! the exact General V3 gfx942 profile.

use super::{FunctionLowerer, HandlerClaim, SessionRecognizedSemanticCall, TranslationDiagnostic};
use crate::kernel_ir_lowering::{LocalBinding, TranslationDiagnosticCode, diagnostic};
use crate::trusted_device_items::{
    TrustedAmdGpuDiagnosticOperation, TrustedAmdGpuInlineOperation, TrustedDeviceItem,
};
use fe2o3_kernel_ir::{
    AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME,
    AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE, AmdGpuDiagnosticOperation,
    AssemblyConstraint, AssemblyOperand, AssemblyOption, AssemblySourceIdentity, BasicBlock,
    InlineAssembly, InlineAssemblyTarget, Operation, OperationKind, ScalarType, TargetCapability,
    Terminator, Type,
};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;

pub(super) fn claim_call(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> HandlerClaim {
    if !matches!(
        call.item.trusted_device_item(),
        TrustedDeviceItem::AmdGpuInline(_) | TrustedDeviceItem::AmdGpuDiagnostic(_)
    ) {
        return HandlerClaim::NotOwned;
    }
    if !lowerer.is_general_v3_profile_context() {
        return HandlerClaim::Reject(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            format!(
                "session-recognized AMDGPU operation `{}` requires the General V3 kernel profile",
                call.callee.identity()
            ),
        ));
    }
    if !lowerer.is_authenticated_general_v3_scalar_context() {
        return HandlerClaim::Reject(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            format!(
                "session-recognized AMDGPU operation `{}` requires an authenticated General V3 scalar kernel context",
                call.callee.identity()
            ),
        ));
    }
    if lowerer.float_target.is_none() {
        return HandlerClaim::Reject(diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            format!(
                "session-recognized AMDGPU operation `{}` requires the exact gfx942 target",
                call.callee.identity()
            ),
        ));
    }
    HandlerClaim::Owned
}

pub(super) fn lower_call(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Terminator, TranslationDiagnostic> {
    match call.item.trusted_device_item() {
        TrustedDeviceItem::AmdGpuInline(operation) => {
            lower_inline_operation(lowerer, call, block, operation)
        }
        TrustedDeviceItem::AmdGpuDiagnostic(operation) => {
            lower_diagnostic_operation(lowerer, call, block, operation)
        }
        _ => unreachable!("only claimed AMDGPU operations may be lowered"),
    }
}

fn lower_inline_operation(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
    operation: TrustedAmdGpuInlineOperation,
) -> Result<Terminator, TranslationDiagnostic> {
    let (mnemonic, arity) = inline_contract(operation);
    if call.operands.len() != arity {
        return Err(lowerer.call_arity(
            call.callee,
            arity,
            call.operands.len(),
            call.location.clone(),
        ));
    }
    let source = assembly_source_identity(lowerer, call, mnemonic)?;
    let arguments = lower_u32_operands(lowerer, call, block)?;
    let result_type = Type::Scalar(ScalarType::U32);
    lowerer.require_destination_type(call.destination, &result_type, call.location)?;
    let result = lowerer.fresh_value(result_type, call.location)?;

    let mut operands = Vec::with_capacity(arguments.len() + 1);
    operands.push(AssemblyOperand::output(0, AssemblyConstraint::Vgpr32));
    operands.extend(
        arguments
            .into_iter()
            .map(|value| AssemblyOperand::input(value, AssemblyConstraint::Vgpr32)),
    );
    let assembly = InlineAssembly {
        target: InlineAssemblyTarget::AmdGpuGfx942,
        source,
        mnemonic: mnemonic.to_owned(),
        operands,
        options: BTreeSet::from([
            AssemblyOption::NoMemory,
            AssemblyOption::Pure,
            AssemblyOption::PreservesFlags,
            AssemblyOption::NoStack,
        ]),
        declared_effects: BTreeSet::new(),
    };
    lowerer
        .required_capabilities
        .insert(TargetCapability::Extension {
            namespace: AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME.to_owned(),
        });
    block.operations.push(Operation::new(
        vec![result.clone()],
        OperationKind::InlineAssembly(assembly),
    ));
    lowerer.bind_local(
        call.destination.local,
        LocalBinding::Value(result.id),
        call.location.clone(),
    )?;
    branch(lowerer, call)
}

fn lower_diagnostic_operation(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
    operation: TrustedAmdGpuDiagnosticOperation,
) -> Result<Terminator, TranslationDiagnostic> {
    let arity = diagnostic_arity(operation);
    if call.operands.len() != arity {
        return Err(lowerer.call_arity(
            call.callee,
            arity,
            call.operands.len(),
            call.location.clone(),
        ));
    }
    let arguments = lower_u32_operands(lowerer, call, block)?;
    let diagnostic = match operation {
        TrustedAmdGpuDiagnosticOperation::Print0
        | TrustedAmdGpuDiagnosticOperation::Print1
        | TrustedAmdGpuDiagnosticOperation::Print2 => AmdGpuDiagnosticOperation::Print {
            format_id: arguments[0],
            arguments: arguments[1..].to_vec(),
        },
        TrustedAmdGpuDiagnosticOperation::AssertFail => AmdGpuDiagnosticOperation::AssertFail {
            site_id: arguments[0],
            line: arguments[1],
        },
        TrustedAmdGpuDiagnosticOperation::Clock32 => AmdGpuDiagnosticOperation::Clock32,
        TrustedAmdGpuDiagnosticOperation::Trap => AmdGpuDiagnosticOperation::Trap,
        TrustedAmdGpuDiagnosticOperation::DebugTrap => AmdGpuDiagnosticOperation::DebugTrap,
        TrustedAmdGpuDiagnosticOperation::ProfilingMarker => {
            AmdGpuDiagnosticOperation::ProfilingMarker {
                marker: arguments[0],
            }
        }
    };
    let terminates = diagnostic.is_terminating();

    let declaration = diagnostic.declaration();
    lowerer.register_declaration_identity(
        declaration.id.as_str(),
        declaration.signature.clone(),
        call.location,
    )?;
    if let Some(result_type) = diagnostic.result_type() {
        lowerer.require_destination_type(call.destination, &result_type, call.location)?;
        let result = lowerer.fresh_value(result_type, call.location)?;
        block.operations.push(diagnostic.operation(Some(result.id)));
        lowerer.bind_local(
            call.destination.local,
            LocalBinding::Value(result.id),
            call.location.clone(),
        )?;
    } else {
        lowerer.require_destination_shape(
            call.destination,
            &crate::mir_import::MirTypeShape::Unit,
            call.location,
        )?;
        block.operations.push(diagnostic.operation(None));
    }
    if terminates {
        Ok(Terminator::Unreachable)
    } else {
        branch(lowerer, call)
    }
}

fn lower_u32_operands(
    lowerer: &mut FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    block: &mut BasicBlock,
) -> Result<Vec<fe2o3_kernel_ir::ValueId>, TranslationDiagnostic> {
    let arguments = call
        .operands
        .iter()
        .map(|operand| lowerer.lower_operand(operand, block, call.location))
        .collect::<Result<Vec<_>, _>>()?;
    let expected = Type::Scalar(ScalarType::U32);
    for (index, value) in arguments.iter().copied().enumerate() {
        let actual = lowerer.value_type(value, call.location)?;
        if actual != &expected {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                call.location.clone(),
                format!(
                    "AMDGPU operation `{}` operand {index} must lower to u32; found {actual:?}",
                    call.callee.identity()
                ),
            ));
        }
    }
    Ok(arguments)
}

fn assembly_source_identity(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
    mnemonic: &str,
) -> Result<AssemblySourceIdentity, TranslationDiagnostic> {
    let authenticated = lowerer
        .kernel_context
        .and_then(|context| context.frontend_contract.as_ref())
        .ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                call.location.clone(),
                "typed AMDGPU inline operation requires an authenticated frontend contract",
            )
        })?;
    let source = call.location.source.as_deref().ok_or_else(|| {
        diagnostic(
            TranslationDiagnosticCode::UnsupportedCall,
            call.location.clone(),
            "typed AMDGPU inline operation requires a concrete source location",
        )
    })?;
    let target_hash = authenticated.target_def_path_hash();
    let line = (source.line as u64).to_le_bytes();
    let column = (source.column as u64).to_le_bytes();
    Ok(AssemblySourceIdentity::new(
        identity_digest(
            b"fe2o3.amdgpu.frontend-unit.v1",
            &[
                authenticated.registration_path().as_bytes(),
                &target_hash,
                authenticated.target_symbol().as_bytes(),
                authenticated.canonical_bytes(),
            ],
        ),
        identity_digest(
            b"fe2o3.amdgpu.function.v1",
            &[
                lowerer.function.rust_path.as_bytes(),
                lowerer.function.export_name.as_bytes(),
            ],
        ),
        identity_digest(
            b"fe2o3.amdgpu.contract.v1",
            &[authenticated.canonical_bytes()],
        ),
        identity_digest(
            b"fe2o3.amdgpu.statement.v1",
            &[
                source.file.as_bytes(),
                &line,
                &column,
                call.callee.identity().as_bytes(),
                mnemonic.as_bytes(),
            ],
        ),
    ))
}

fn identity_digest(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_le_bytes());
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    let mut identity: [u8; 32] = digest.finalize().into();
    if identity == [0; 32] {
        identity[0] = 1;
    }
    identity
}

const fn inline_contract(operation: TrustedAmdGpuInlineOperation) -> (&'static str, usize) {
    match operation {
        TrustedAmdGpuInlineOperation::VMovB32 => ("v_mov_b32", 1),
        TrustedAmdGpuInlineOperation::VAddU32 => ("v_add_u32", 2),
        TrustedAmdGpuInlineOperation::VSubU32 => ("v_sub_u32", 2),
        TrustedAmdGpuInlineOperation::VAndB32 => ("v_and_b32", 2),
        TrustedAmdGpuInlineOperation::VOrB32 => ("v_or_b32", 2),
        TrustedAmdGpuInlineOperation::VXorB32 => ("v_xor_b32", 2),
    }
}

const fn diagnostic_arity(operation: TrustedAmdGpuDiagnosticOperation) -> usize {
    match operation {
        TrustedAmdGpuDiagnosticOperation::Print0
        | TrustedAmdGpuDiagnosticOperation::ProfilingMarker => 1,
        TrustedAmdGpuDiagnosticOperation::Print1 | TrustedAmdGpuDiagnosticOperation::AssertFail => {
            2
        }
        TrustedAmdGpuDiagnosticOperation::Print2 => 3,
        TrustedAmdGpuDiagnosticOperation::Clock32
        | TrustedAmdGpuDiagnosticOperation::Trap
        | TrustedAmdGpuDiagnosticOperation::DebugTrap => 0,
    }
}

fn branch(
    lowerer: &FunctionLowerer<'_, '_>,
    call: SessionRecognizedSemanticCall<'_>,
) -> Result<Terminator, TranslationDiagnostic> {
    Ok(Terminator::Branch {
        target: lowerer.block_id(call.target, call.location.clone())?,
        arguments: lowerer.edge_arguments(call.target, call.location)?,
    })
}
