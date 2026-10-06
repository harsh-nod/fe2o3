use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;

#[test]
fn original_mir_scalar_congruence_indexes_are_deterministic_and_subquadratic() {
    let scalar = ScalarV30::Integer {
        width: 32,
        signed: false,
    };
    let mut previous = 0;
    for count in [64usize, 256, 1024] {
        let mut nodes = vec![NodeV30 {
            scalar,
            expression: ExpressionV30::Argument(0),
        }];
        for index in 1..count {
            nodes.push(NodeV30 {
                scalar,
                expression: ExpressionV30::Binary {
                    operation: OperatorV30::Xor,
                    left: index - 1,
                    right: 0,
                },
            });
        }
        let run = || {
            let mut work = Work::new(10_000_000);
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            budget
                .reserve_storage(super::super::super::SOURCE_LIMIT)
                .unwrap();
            let mut writer = Writer::new(&mut budget).unwrap();
            let classes = relation::classes([&nodes, &nodes], &mut writer).unwrap();
            assert_eq!(classes[0], classes[1]);
            assert_eq!(classes[0].len(), count);
            let actual = writer.budget.work();
            assert!(
                actual <= count * (usize::BITS as usize - count.leading_zeros() as usize + 1) * 100
            );
            (classes, actual)
        };
        let first = run();
        let second = run();
        assert_eq!(first, second);
        if previous != 0 {
            assert!(first.1 <= previous * 6);
        }
        previous = first.1;
    }
}

pub(crate) fn fixture(first: SemanticBinaryOpV1, moved: bool) -> (Vec<Type>, Function) {
    let word = SemanticTypeIdV1::from_index(0);
    let types = vec![SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )];
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let value = || SemanticAbiValueV1::new(word, SemanticAbiPassModeV1::Direct(attributes));
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([4; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![value(), value()],
        value(),
    )
    .unwrap();
    let source = SemanticSourceProvenanceV1::unavailable();
    let place =
        |local| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], word).unwrap();
    let copy = |local| SemanticOperandV1::Copy(place(local));
    let statement = |local, operation, left, right| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local),
                SemanticRvalueV1::new(
                    word,
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left,
                        right,
                    },
                ),
            )),
        )
    };
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([5; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([6; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([7; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([8; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([9; 32]),
        source,
        abi,
        [
            LocalRole::Return,
            LocalRole::Argument(0),
            LocalRole::Argument(1),
            LocalRole::Temporary,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, role)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([10 + i as u8; 32]),
                word,
                role,
                source,
            )
        })
        .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([20; 32]),
                source,
                vec![
                    statement(
                        3,
                        first,
                        if moved {
                            SemanticOperandV1::Move(place(1))
                        } else {
                            copy(1)
                        },
                        copy(2),
                    ),
                    statement(0, SemanticBinaryOpV1::BitOr, copy(3), copy(1)),
                ],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    (types, function)
}

fn run(
    first: SemanticBinaryOpV1,
    moved: bool,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize) {
    let (types, function) = fixture(first, moved);
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget
        .reserve_storage(super::super::super::SOURCE_LIMIT)
        .unwrap();
    let result = (|| {
        let mut writer = Writer::new(&mut budget)?;
        let program = SourceProgramV30::derive(&types, &function, &mut writer)?;
        assert_eq!(program.arguments, 2);
        assert_eq!(program.statements, 2);
        assert_eq!(program.assignments.len(), 2);
        assert_eq!(
            program.assignments[0],
            AssignmentV30 {
                statement: 0,
                destination: 3,
                value: 2
            }
        );
        assert_eq!(
            program.assignments[1],
            AssignmentV30 {
                statement: 1,
                destination: 0,
                value: 3
            }
        );
        assert_eq!(program.returned, Some(3));
        program.emit_trace(0, &mut writer)?;
        let text = writer.finish()?;
        assert!(text.contains("((m0 as u32) ^ (m1 as u32)) as int"));
        assert!(text.contains("((m2 as u32) | (m0 as u32)) as int"));
        assert!(text.contains("seq![m2,m3,m3,]"));
        assert!(!text.contains("assume("));
        drop(text);
        drop(program);
        Ok(())
    })();
    let actual = budget.work();
    let peak = budget.peak_storage();
    budget.release_storage(budget.storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    (result, actual, peak)
}

#[test]
fn original_mir_scalar_model_preserves_each_assignment_and_return() {
    run(
        SemanticBinaryOpV1::BitXor,
        false,
        1_000_000,
        64 * 1024 * 1024,
    )
    .0
    .unwrap();
}

#[test]
fn original_mir_scalar_model_refuses_move_reuse_and_unmodeled_arithmetic() {
    assert!(matches!(
        run(
            SemanticBinaryOpV1::BitXor,
            true,
            1_000_000,
            64 * 1024 * 1024
        )
        .0,
        Err(Error::Statement("original MIR scalar use is undefined"))
    ));
    assert!(matches!(
        run(SemanticBinaryOpV1::Add, false, 1_000_000, 64 * 1024 * 1024).0,
        Err(Error::Statement(
            "original MIR arithmetic/effect contract is not modeled"
        ))
    ));
}

#[test]
fn original_mir_scalar_model_exact_and_one_short_resources() {
    let (result, work, storage) = run(
        SemanticBinaryOpV1::BitXor,
        false,
        1_000_000,
        64 * 1024 * 1024,
    );
    result.unwrap();
    let exact = run(SemanticBinaryOpV1::BitXor, false, work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(
        matches!(run(SemanticBinaryOpV1::BitXor, false, work - 1, storage).0,
        Err(Error::Resource(Resource::Work(error))) if error.actual() == work && error.limit() == work - 1)
    );
    assert!(
        matches!(run(SemanticBinaryOpV1::BitXor, false, work, storage - 1).0,
        Err(Error::Resource(Resource::Storage(error))) if error.actual() == storage && error.limit() == storage - 1)
    );
}

#[test]
fn original_mir_target_model_reads_actual_canonical_operator_and_operands() {
    use fe2o3_kernel_analysis::CanonicalKirInventoryV18;
    use fe2o3_kernel_ir::{
        BasicBlock, BinaryOp, BlockId, Function as KirFunction, Module, Operation, OperationKind,
        ScalarType, Signature, StorageLayoutLimitsV1, Terminator as KirTerminator, Type as KirType,
        ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV18,
    };
    for changed in [false, true] {
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        let scalar = KirType::Scalar(ScalarType::U32);
        let operation = |id, op, left, right| {
            Operation::effect_free(
                ValueDef::new(ValueId(id), scalar.clone()),
                OperationKind::Binary {
                    op,
                    lhs: ValueId(left),
                    rhs: ValueId(right),
                },
            )
        };
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            operation(2, BinaryOp::BitXor, 0, 1),
            operation(
                3,
                if changed {
                    BinaryOp::BitAnd
                } else {
                    BinaryOp::BitOr
                },
                2,
                0,
            ),
        ];
        block.terminator = Some(KirTerminator::Return {
            values: vec![ValueId(3)],
        });
        let mut module = Module::new("original-mir-scalar-model");
        module.functions.push(KirFunction::internal_helper(
            "scalar",
            Signature::new(vec![scalar.clone(), scalar.clone()], vec![scalar]),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        ));
        let layouts = StorageLayoutLimitsV1 {
            rows: 1,
            edges: 1,
            containment_depth: 1,
            object_bytes: 16,
        };
        let (owner, storage) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                layouts,
                &mut budget,
            )
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (inventory, storage) =
            CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        budget
            .reserve_storage(super::super::super::SOURCE_LIMIT)
            .unwrap();
        let mut writer = Writer::new(&mut budget).unwrap();
        let mut target =
            canonical::CanonicalProgramV30::derive(&inventory, 0, &mut writer).unwrap();
        let (types, function) = fixture(SemanticBinaryOpV1::BitXor, false);
        let source = SourceProgramV30::derive(&types, &function, &mut writer).unwrap();
        assert_eq!(source.nodes == target.nodes, !changed);
        assert_eq!(target.arguments, 2);
        assert_eq!(target.returned, Some(3));
        assert_eq!(target.value(target.definition_start + 3).unwrap(), 3);
        let endpoints = [
            relation::EndpointV30::Definition(target.definition_start + 2),
            relation::EndpointV30::Definition(target.definition_start + 3),
        ];
        let compared = relation::check(&source, &target, &endpoints, &mut writer);
        if changed {
            assert!(matches!(
                compared,
                Err(Error::Statement(
                    "original MIR scalar step result differs from actual canonical value"
                ))
            ));
            drop(writer);
        } else {
            compared.unwrap();
            assert!(matches!(
                relation::check(&source, &target, &endpoints[..1], &mut writer),
                Err(Error::Statement(
                    "original MIR scalar relation has incomplete endpoints"
                ))
            ));
            let exchanged = [endpoints[1], endpoints[0]];
            assert!(matches!(
                relation::check(&source, &target, &exchanged, &mut writer),
                Err(Error::Statement(
                    "original MIR scalar step result differs from actual canonical value"
                ))
            ));
            relation::check(&source, &target, &endpoints, &mut writer).unwrap();
            let original_scalar = target.nodes[1].scalar;
            target.nodes[1].scalar = ScalarV30::Integer {
                width: 32,
                signed: true,
            };
            assert!(matches!(
                relation::check(&source, &target, &endpoints, &mut writer),
                Err(Error::Statement(
                    "original MIR scalar argument representation differs"
                ))
            ));
            target.nodes[1].scalar = original_scalar;
            relation::emit_prelude(&mut writer).unwrap();
            relation::emit(&source, &target, &endpoints, 0, &mut writer).unwrap();
            let text = writer.finish().unwrap();
            assert!(text.contains("spec fn original_mir_trace_0_v30"));
            assert!(text.contains("spec fn original_kir_trace_0_v30"));
            assert!(text.contains("proof fn original_mir_refines_canonical_0_v30"));
            assert!(text.contains("0int <= base[1] < 4294967296int"));
            assert!(!text.contains("assume("));
            assert!(!text.contains("external_body"));
            drop(text);
        }
        drop(source);
        drop(target);
        drop(inventory);
        drop(owner);
        budget.release_storage(budget.storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
