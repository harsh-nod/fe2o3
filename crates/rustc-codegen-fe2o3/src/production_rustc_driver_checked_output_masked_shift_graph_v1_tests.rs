//! Test observations over actual N/O owners, not a source admission rule.
use super::*;
use fe2o3_kernel_ir::{
    BinaryOp, Constant, Function, FunctionRole, LaunchDomain, LaunchExtent, Module, Operation,
    OperationKind, ValueId, WorkgroupSize,
};

fn producer(function: &Function, value: ValueId) -> Option<&Operation> {
    function
        .body
        .as_ref()?
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.results.iter().any(|result| result.id == value))
}

fn result_type(operation: &Operation) -> Option<ScalarType> {
    let [result] = operation.results.as_slice() else {
        return None;
    };
    let Type::Scalar(ty) = result.ty else {
        return None;
    };
    Some(ty)
}

fn mask_constant(function: &Function, value: ValueId, ty: ScalarType, mask: u128) -> bool {
    let Some(operation) = producer(function, value) else {
        return false;
    };
    if result_type(operation) != Some(ty) {
        return false;
    }
    let value = match operation.kind {
        OperationKind::Constant(Constant::U8(v)) => Some(u128::from(v)),
        OperationKind::Constant(Constant::U16(v)) => Some(u128::from(v)),
        OperationKind::Constant(Constant::U32(v)) => Some(u128::from(v)),
        OperationKind::Constant(Constant::U64(v)) => Some(u128::from(v)),
        OperationKind::Constant(Constant::I8(v)) => u128::try_from(v).ok(),
        OperationKind::Constant(Constant::I16(v)) => u128::try_from(v).ok(),
        OperationKind::Constant(Constant::I32(v)) => u128::try_from(v).ok(),
        OperationKind::Constant(Constant::I64(v)) => u128::try_from(v).ok(),
        _ => None,
    };
    value == Some(mask)
}

fn count_path(function: &Function, mut value: ValueId, count: ValueId, integer: Integer) -> bool {
    let mut masked = false;
    // Fixture observations are deliberately bounded. The production proof owns
    // definedness, cast validity, dominance and all source/native correspondence.
    for _ in 0..8 {
        if value == count {
            return masked;
        }
        let Some(operation) = producer(function, value) else {
            return false;
        };
        let Some(ty) = result_type(operation) else {
            return false;
        };
        if !matches!(
            ty,
            ScalarType::I8
                | ScalarType::U8
                | ScalarType::I16
                | ScalarType::U16
                | ScalarType::I32
                | ScalarType::U32
                | ScalarType::I64
                | ScalarType::U64
        ) {
            return false;
        }
        match operation.kind {
            OperationKind::Cast { value: input, .. } => value = input,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs,
                rhs,
            } if mask_constant(function, rhs, ty, u128::from(integer.width() - 1)) => {
                masked = true;
                value = lhs;
            }
            _ => return false,
        }
    }
    false
}

fn operation_function<'a>(
    module: &'a Module,
    batch: Batch,
    kernel: &Kernel,
) -> Result<&'a Function, SourceFailure> {
    let entry = module
        .function(&kernel.entry)
        .ok_or_else(|| failure("masked shift entry missing"))?;
    let [Type::Slice(output), input, count] = entry.signature.parameters.as_slice() else {
        return Err(failure("masked shift output/value/U32-count ABI"));
    };
    if output.address_space != AddressSpace::Global
        || output.element.as_ref() != &Type::Scalar(batch.integer.scalar())
        || output.access != AccessMode::ReadWrite
        || input != &Type::Scalar(batch.integer.scalar())
        || count != &Type::Scalar(ScalarType::U32)
        || !entry.signature.results.is_empty()
        || kernel.domain
            != (LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            })
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
    {
        return Err(failure("masked shift exact fixed-width/full-U32 ABI"));
    }
    let calls = entry
        .body
        .iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call { callee, arguments } => module
                .function(callee)
                .filter(|function| function.role == FunctionRole::InternalHelper)
                .map(|function| (function, arguments, &operation.results)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !batch.retained {
        if !calls.is_empty() {
            return Err(failure(
                "direct masked shift unexpectedly retained a helper",
            ));
        }
        return Ok(entry);
    }
    let [(helper, arguments, results)] = calls.as_slice() else {
        return Err(failure(
            "masked shift requires one retained helper per root",
        ));
    };
    let body = entry
        .body
        .as_ref()
        .ok_or_else(|| failure("masked shift entry body missing"))?;
    if body.parameters.len() != 3
        || arguments.as_slice() != &body.parameters[1..]
        || !matches!(results.as_slice(), [result] if result.ty == Type::Scalar(batch.integer.scalar()))
        || helper.signature.parameters
            != [
                Type::Scalar(batch.integer.scalar()),
                Type::Scalar(ScalarType::U32),
            ]
        || helper.signature.results != [Type::Scalar(batch.integer.scalar())]
    {
        return Err(failure(
            "masked shift retained-helper argument identity/ABI",
        ));
    }
    Ok(helper)
}

pub(in super::super::super) fn check_graph(
    module: &Module,
    batch: Batch,
) -> Result<std::collections::BTreeMap<&str, String>, SourceFailure> {
    let ordinals = unique_root_ordinals(module.kernels.iter().map(|kernel| kernel.id.as_str()))?;
    let helpers = module
        .functions
        .iter()
        .filter(|function| function.role == FunctionRole::InternalHelper)
        .count();
    if helpers != if batch.retained { ROOTS.len() } else { 0 } {
        return Err(failure("masked shift exact retained helper roster"));
    }
    let mut owners = std::collections::BTreeMap::new();
    for (kernel, ordinal) in module.kernels.iter().zip(ordinals) {
        let function = operation_function(module, batch, kernel)?;
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| failure("masked shift operation body missing"))?;
        let value_ordinal = usize::from(!batch.retained);
        if body.parameters.len() != value_ordinal + 2 {
            return Err(failure("masked shift exact actual parameters"));
        }
        let shifts = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::ShiftLeft | BinaryOp::ShiftRight,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        let [shift] = shifts.as_slice() else {
            return Err(failure("masked shift exact dynamic shift operation"));
        };
        let OperationKind::Binary { op, lhs, rhs } = shift.kind else {
            unreachable!()
        };
        let expected = if ordinal == 0 {
            BinaryOp::ShiftLeft
        } else {
            BinaryOp::ShiftRight
        };
        if op != expected
            || result_type(shift) != Some(batch.integer.scalar())
            || lhs != body.parameters[value_ordinal]
            || !count_path(
                function,
                rhs,
                body.parameters[value_ordinal + 1],
                batch.integer,
            )
        {
            return Err(failure(
                "masked shift actual direction/value/masked-U32-count identity",
            ));
        }
        owners.insert(kernel.id.as_str(), function.id.as_str().to_owned());
    }
    Ok(owners)
}

#[test]
fn masked_shift_graph_preserves_observed_order_and_rejects_wrong_actual_inputs() {
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, LaunchDomain, Operation, Signature, Terminator, ValueDef,
    };
    let batch = Batch {
        integer: Integer::U32,
        spelling: Spelling::ExplicitMask,
        retained: false,
    };
    let mut module = Module::new("masked-shift-roster-test");
    for (ordinal, root) in ROOTS.into_iter().enumerate() {
        let entry = format!("entry_{root}");
        let mut block = BasicBlock::new(BlockId(0));
        for (result, kind) in [
            (3, OperationKind::Constant(Constant::U32(31))),
            (
                4,
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(2),
                    rhs: ValueId(3),
                },
            ),
            (
                5,
                OperationKind::Binary {
                    op: if ordinal == 0 {
                        BinaryOp::ShiftLeft
                    } else {
                        BinaryOp::ShiftRight
                    },
                    lhs: ValueId(1),
                    rhs: ValueId(4),
                },
            ),
        ] {
            block.operations.push(Operation::new(
                vec![ValueDef::new(
                    ValueId(result),
                    Type::Scalar(ScalarType::U32),
                )],
                kind,
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            entry.clone(),
            Signature::new(
                vec![
                    Type::slice(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    ),
                    Type::Scalar(ScalarType::U32),
                    Type::Scalar(ScalarType::U32),
                ],
                vec![],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            root,
            entry,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
    }
    module.kernels.reverse();
    let actual_order = module
        .kernels
        .iter()
        .map(|kernel| kernel.id.clone())
        .collect::<Vec<_>>();
    let owners = check_graph(&module, batch).unwrap();
    for root in ROOTS {
        assert_eq!(owners[root], format!("entry_{root}"));
    }
    assert_eq!(
        module
            .kernels
            .iter()
            .map(|kernel| kernel.id.clone())
            .collect::<Vec<_>>(),
        actual_order
    );

    let mut bad_mask = module.clone();
    bad_mask.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::Constant(Constant::U32(63));
    assert!(check_graph(&bad_mask, batch).is_err());
    let mut wrong_input = module.clone();
    wrong_input.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind =
        OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs: ValueId(1),
            rhs: ValueId(3),
        };
    assert!(check_graph(&wrong_input, batch).is_err());
    let mut raw = module.clone();
    raw.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind = OperationKind::Binary {
        op: BinaryOp::ShiftLeft,
        lhs: ValueId(1),
        rhs: ValueId(2),
    };
    assert!(check_graph(&raw, batch).is_err());
    let mut wrong_abi = module.clone();
    wrong_abi.functions[0].signature.parameters[2] = Type::Scalar(ScalarType::U64);
    assert!(check_graph(&wrong_abi, batch).is_err());
    let mut duplicate = module.clone();
    duplicate.kernels[1] = duplicate.kernels[0].clone();
    assert!(check_graph(&duplicate, batch).is_err());
    let mut missing = module.clone();
    missing.kernels.pop();
    assert!(check_graph(&missing, batch).is_err());
    for mutation in 0..5 {
        let mut changed = module.clone();
        match mutation {
            0 => changed.kernels[0].workgroup_size = None,
            1 => changed.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1)),
            2 => changed.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 2, 1)),
            3 => {
                changed.kernels[0].domain = LaunchDomain::D1 {
                    x: LaunchExtent::Static(128),
                }
            }
            4 => {
                changed.kernels[0].domain = LaunchDomain::D2 {
                    x: LaunchExtent::Dynamic,
                    y: LaunchExtent::Dynamic,
                }
            }
            _ => unreachable!(),
        }
        assert!(
            check_graph(&changed, batch).is_err(),
            "launch mutation {mutation}"
        );
    }
    module.kernels[0].id = "foreign_root".into();
    assert!(check_graph(&module, batch).is_err());
}
