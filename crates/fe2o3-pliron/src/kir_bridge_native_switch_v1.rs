//! Native switch codec. Source templates are never consulted for native keys.
use super::*;
use dialect_gpu::switch_v3::{SwitchEdgeV3, SwitchKeyKindAttrV3 as Kind, SwitchOpV3};
use pliron::common_traits::Verify;

const _: () = {
    assert!(dialect_gpu::switch_v3::MAX_SWITCH_CASES_V3 == fe2o3_kernel_ir::MAX_SWITCH_CASES_V1);
    assert!(
        dialect_gpu::switch_v3::MAX_SWITCH_CASES_V3 == fe2o3_kernel_ir::MAX_INTEGER_SWITCH_CASES_V2
    );
    assert!(
        dialect_gpu::switch_v3::MAX_SWITCH_EDGE_ARGUMENTS_V3
            == fe2o3_kernel_ir::MAX_VALUE_ARGUMENTS_V1
    );
};

fn legacy_key_fits(ty: &Type, key: u64) -> bool {
    match ty {
        Type::Scalar(ScalarType::I8 | ScalarType::U8) => key <= u8::MAX.into(),
        Type::Scalar(ScalarType::I16 | ScalarType::U16) => key <= u16::MAX.into(),
        Type::Scalar(ScalarType::I32 | ScalarType::U32) => key <= u32::MAX.into(),
        Type::Scalar(
            ScalarType::I64
            | ScalarType::U64
            | ScalarType::I128
            | ScalarType::U128
            | ScalarType::Index,
        ) => true,
        _ => false,
    }
}

// Called only for legacy Switch, before native projection. The actual source
// definition is found without allocating a second definition index.
pub(crate) fn source_legacy_representable(
    function: &fe2o3_kernel_ir::Function,
    selector: ValueId,
    cases: &[fe2o3_kernel_ir::SwitchCase],
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    let Some(body) = &function.body else {
        return Ok(false);
    };
    let mut ty = None;
    for (id, parameter_type) in body.parameters.iter().zip(&function.signature.parameters) {
        budget.charge_work(1)?;
        if *id == selector {
            ty = Some(parameter_type);
            break;
        }
    }
    if ty.is_none() {
        'blocks: for block in &body.blocks {
            budget.charge_work(1)?;
            for value in &block.parameters {
                budget.charge_work(1)?;
                if value.id == selector {
                    ty = Some(&value.ty);
                    break 'blocks;
                }
            }
            for operation in &block.operations {
                budget.charge_work(1)?;
                for value in &operation.results {
                    budget.charge_work(1)?;
                    if value.id == selector {
                        ty = Some(&value.ty);
                        break 'blocks;
                    }
                }
            }
        }
    }
    let Some(ty) = ty else { return Ok(false) };
    for case in cases {
        budget.charge_work(1)?;
        if !legacy_key_fits(ty, case.value) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn encode(value: &Constant) -> Result<(Kind, u64), KirBridgeErrorV1> {
    Ok(match *value {
        Constant::I8(n) => (Kind::I8, u64::from(n as u8)),
        Constant::I16(n) => (Kind::I16, u64::from(n as u16)),
        Constant::I32(n) => (Kind::I32, u64::from(n as u32)),
        Constant::I64(n) => (Kind::I64, n as u64),
        Constant::U8(n) => (Kind::U8, n.into()),
        Constant::U16(n) => (Kind::U16, n.into()),
        Constant::U32(n) => (Kind::U32, n.into()),
        Constant::U64(n) => (Kind::U64, n),
        Constant::Index(n) => (Kind::Index, n),
        _ => return Err(KirBridgeErrorV1::MalformedGraph),
    })
}

fn decode(kind: Kind, bits: u64) -> Result<Constant, KirBridgeErrorV1> {
    let malformed = |_| KirBridgeErrorV1::MalformedGraph;
    Ok(match kind {
        Kind::I8 => Constant::I8(u8::try_from(bits).map_err(malformed)? as i8),
        Kind::I16 => Constant::I16(u16::try_from(bits).map_err(malformed)? as i16),
        Kind::I32 => Constant::I32(u32::try_from(bits).map_err(malformed)? as i32),
        Kind::I64 => Constant::I64(bits as i64),
        Kind::U8 => Constant::U8(bits.try_into().map_err(malformed)?),
        Kind::U16 => Constant::U16(bits.try_into().map_err(malformed)?),
        Kind::U32 => Constant::U32(bits.try_into().map_err(malformed)?),
        Kind::U64 => Constant::U64(bits),
        Kind::Index => Constant::Index(bits),
        Kind::LegacyU64 | Kind::EmptyTyped => return Err(KirBridgeErrorV1::MalformedGraph),
    })
}

// Existing bridge admission prepays construction, key validation and vectors.
// Only the legacy representability mismatch selects the preserved fallback;
// malformed typed keys, payloads or a native constructor refusal never do.
pub(super) fn build(
    context: &mut Context,
    function: usize,
    terminator: &Terminator,
    values: &BTreeMap<ValueId, Value>,
    blocks: &BTreeMap<BlockId, Ptr<BasicBlock>>,
) -> Result<Option<Ptr<Operation>>, KirBridgeErrorV1> {
    let (selector, kind, keys, edges) = match terminator {
        Terminator::Switch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            let selector = value_for(values, function, *selector)?;
            let ty = type_from_pliron(context, selector.get_type(context))?;
            if cases.iter().any(|case| !legacy_key_fits(&ty, case.value)) {
                return Ok(None);
            }
            let keys = cases.iter().map(|case| case.value).collect();
            let edges = cases
                .iter()
                .map(|case| {
                    Ok(SwitchEdgeV3::new(
                        block_for(blocks, function, case.target)?,
                        values_for(values, function, &case.arguments)?,
                    ))
                })
                .chain(std::iter::once_with(|| {
                    Ok(SwitchEdgeV3::new(
                        block_for(blocks, function, *default_target)?,
                        values_for(values, function, default_arguments)?,
                    ))
                }))
                .collect::<Result<Vec<_>, KirBridgeErrorV1>>()?;
            (selector, Kind::LegacyU64, keys, edges)
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            let mut kind = Kind::EmptyTyped;
            let mut keys = Vec::with_capacity(cases.len());
            let mut edges = Vec::with_capacity(cases.len() + 1);
            for case in cases {
                let (next_kind, bits) = encode(&case.value)?;
                if kind != Kind::EmptyTyped && kind != next_kind {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
                kind = next_kind;
                keys.push(bits);
                edges.push(SwitchEdgeV3::new(
                    block_for(blocks, function, case.target)?,
                    values_for(values, function, &case.arguments)?,
                ));
            }
            edges.push(SwitchEdgeV3::new(
                block_for(blocks, function, *default_target)?,
                values_for(values, function, default_arguments)?,
            ));
            (value_for(values, function, *selector)?, kind, keys, edges)
        }
        _ => return Ok(None),
    };
    SwitchOpV3::try_new(context, selector, kind, keys, edges)
        .map(|switch| Some(switch.get_operation()))
        .map_err(|_| KirBridgeErrorV1::MalformedGraph)
}

pub(super) fn extract(
    context: &Context,
    switch: SwitchOpV3,
    values: &HashMap<Value, ValueId>,
    blocks: &HashMap<Ptr<BasicBlock>, BlockId>,
) -> Result<Terminator, KirBridgeErrorV1> {
    switch
        .verify(context)
        .and_then(|()| switch.verify_interfaces(context))
        .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
    let kind = switch
        .kind(context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    let keys = switch
        .cases(context)
        .ok_or(KirBridgeErrorV1::MalformedGraph)?;
    let raw = switch.get_operation().deref(context);
    let selector = id_for(
        values,
        switch
            .selector(context)
            .ok_or(KirBridgeErrorV1::MalformedGraph)?,
    )?;
    let edge = |ordinal| {
        let range = switch
            .successor_operand_range(context, ordinal)
            .ok_or(KirBridgeErrorV1::MalformedGraph)?;
        Ok::<_, KirBridgeErrorV1>((
            block_id_for(blocks, raw.get_successor(ordinal))?,
            ids_for(values, range.map(|index| raw.get_operand(index)))?,
        ))
    };
    let (default_target, default_arguments) = edge(keys.bits().len())?;
    if kind == Kind::LegacyU64 {
        let mut cases = Vec::with_capacity(keys.bits().len());
        for (ordinal, &value) in keys.bits().iter().enumerate() {
            let (target, arguments) = edge(ordinal)?;
            cases.push(fe2o3_kernel_ir::SwitchCase {
                value,
                target,
                arguments,
            });
        }
        Ok(Terminator::Switch {
            selector,
            cases,
            default_target,
            default_arguments,
        })
    } else {
        let mut cases = Vec::with_capacity(keys.bits().len());
        for (ordinal, &bits) in keys.bits().iter().enumerate() {
            let (target, arguments) = edge(ordinal)?;
            cases.push(fe2o3_kernel_ir::IntegerSwitchCase {
                value: decode(kind, bits)?,
                target,
                arguments,
            });
        }
        Ok(Terminator::IntegerSwitch {
            selector,
            cases,
            default_target,
            default_arguments,
        })
    }
}

#[cfg(test)]
#[path = "kir_bridge_native_switch_v1_tests.rs"]
mod tests;
