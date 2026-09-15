use super::*;
use fe2o3_kernel_ir::{ExecutionCapabilityRoleV1, Type as KirType};
use pliron::{
    derive::pliron_type,
    r#type::{Type, Typed, TypedHandle},
};

#[cfg(test)]
mod tests;

pub(super) fn version(bytes: &[u8]) -> Option<CanonicalKernelIrVersionV1> {
    if bytes.get(..8)? != fe2o3_kernel_ir::KERNEL_IR_MAGIC_V1 {
        return None;
    }
    let tag = bytes.get(8..10)?;
    match u16::from_le_bytes([tag[0], tag[1]]) {
        13 => Some(CanonicalKernelIrVersionV1::V13),
        14 => Some(CanonicalKernelIrVersionV1::V14),
        _ => None,
    }
}

/// Inert, exactly V14 logical phase type. Never a physical ABI type or source issuer.
#[pliron_type(name = "gpu.phase_value_v14", format = "$0")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PhaseValueTypeV14(StringAttr);

impl PhaseValueTypeV14 {
    pub fn get(context: &Context, ty: &KirType) -> Option<TypedHandle<Self>> {
        if !Self::supports(ty) {
            return None;
        }
        let bytes = encode_module_v14(&type_envelope(ty.clone())).ok()?;
        if bytes.len() > MAX_CANONICAL_KIR_OPERATION_BYTES_V1 {
            return None;
        }
        Some(Self::instantiate(
            Self(StringAttr::new(encode_hex(&bytes))),
            context,
        ))
    }

    pub fn supports(ty: &KirType) -> bool {
        match ty {
            KirType::ReusablePhaseToken(token) => token.is_complete(),
            KirType::ExecutionCapability(capability) => {
                capability.is_complete()
                    && matches!(
                        capability.role,
                        ExecutionCapabilityRoleV1::ReusableWorkgroup
                            | ExecutionCapabilityRoleV1::ReusablePhaseCompletion
                    )
            }
            _ => false,
        }
    }

    pub fn kir_type(&self) -> Option<KirType> {
        let bytes = decode_hex(self.0.as_str(), MAX_CANONICAL_KIR_OPERATION_BYTES_V1)?;
        if version(&bytes)? != CanonicalKernelIrVersionV1::V14 {
            return None;
        }
        let module = decode_module_v14(&bytes).ok()?;
        let [function] = module.functions.as_slice() else {
            return None;
        };
        let [ty] = function.signature.parameters.as_slice() else {
            return None;
        };
        if !Self::supports(ty)
            || module != type_envelope(ty.clone())
            || encode_module_v14(&module).ok()? != bytes
        {
            return None;
        }
        Some(ty.clone())
    }
}

fn type_envelope(ty: KirType) -> Module {
    let mut block = KirBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("fe2o3.pliron.phase-type.v14");
    module.functions.push(Function::internal_helper(
        "phase_type",
        Signature::new(vec![ty], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

impl Verify for PhaseValueTypeV14 {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self.kir_type().is_none() {
            return verify_err_noloc!("phase value is not an exact bounded V14 type fragment");
        }
        Ok(())
    }
}

fn logical_type(context: &Context, ty: TypeHandle) -> Option<KirType> {
    let ty = ty.deref(context);
    if let Some(phase) = ty.downcast_ref::<PhaseValueTypeV14>() {
        return phase.kir_type();
    }
    ty.downcast_ref::<dialect_kernel::ExecutionCapabilityType>()
        .and_then(dialect_kernel::ExecutionCapabilityType::capability)
        .map(KirType::ExecutionCapability)
}

pub(super) fn valid_phase_ssa(
    context: &Context,
    raw: &Operation,
    operation: &KirOperation,
) -> bool {
    let OperationKind::ReusablePhase(phase) = &operation.kind else {
        return false;
    };
    if raw.get_num_operands() != phase.operands.len()
        || phase.operands.len() > fe2o3_kernel_ir::MAX_EXECUTION_CAPABILITY_OPERANDS_V1
        || operation.results.len() > fe2o3_kernel_ir::MAX_EXECUTION_CAPABILITY_RESULTS_V1
        || raw.get_num_results() != operation.results.len()
    {
        return false;
    }
    let mut inputs = Vec::new();
    if inputs.try_reserve_exact(phase.operands.len()).is_err() {
        return false;
    }
    for operand in raw.operands() {
        let Some(ty) = logical_type(context, operand.get_type(context)) else {
            return false;
        };
        inputs.push(ty);
    }
    let refs = inputs.iter().collect::<Vec<_>>();
    let Some(results) = phase.checked_result_types(&refs) else {
        return false;
    };
    results.len() == operation.results.len()
        && results.iter().enumerate().all(|(i, ty)| {
            operation.results[i].ty == *ty
                && logical_type(context, raw.get_type(i)).as_ref() == Some(ty)
        })
}
