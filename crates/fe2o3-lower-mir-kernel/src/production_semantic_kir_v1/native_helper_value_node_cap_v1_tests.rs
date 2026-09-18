use super::super::helper_source_fixture_v1 as fixture;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_mir_model::semantic_mir_v1::{InertSemanticMirRequestV1, SemanticMirLimitsV1};

const NODES: usize = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
const WORK: usize = 100_000_000;
const STORAGE: usize = 64 << 20;
const FLOOR: usize = 29;

fn doubling(levels: u32, trailing_not: u32) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(0));
    let mut value = ValueId(0);
    for ordinal in 1..=levels {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(ordinal), scalar.clone()),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: value,
                rhs: value,
            },
        ));
        value = ValueId(ordinal);
    }
    for ordinal in levels + 1..=levels + trailing_not {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(ordinal), scalar.clone()),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: value,
            },
        ));
        value = ValueId(ordinal);
    }
    block.terminator = Some(Terminator::Return {
        values: vec![value],
    });
    let mut module = Module::new("temporary_node_limit");
    module.functions.push(Function::internal_helper(
        "doubling",
        Signature::new(vec![scalar.clone()], vec![scalar]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

#[test]
fn actual_normalizer_caps_shallow_dag_expansion_before_tree_allocation() {
    for (levels, trailing_not, expected_nodes, active) in [
        (11, 0, Some(4095), true),
        (12, 0, Some(8191), true),
        (12, 1, Some(8192), true),
        (12, 2, None, true),
        (14, 0, None, true),
        (14, 0, Some(32767), false),
    ] {
        let mut work = Work::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        let module = doubling(levels, trailing_not);
        let (owner, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget
            .reserve_storage(FLOOR + receipt.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let function = &owner.module().functions[0];
        let mut correlation = UnsupportedIndexCorrelationBudgetV1 {
            remaining: 1_000_000,
        };
        let kir =
            build_kir_correlation_index(function.body.as_ref().unwrap(), 64, &mut correlation)
                .unwrap();
        let before = correlation.remaining;
        let reserved_nodes = if active {
            NODES
        } else {
            expected_nodes.unwrap()
        };
        let bytes = reserved_nodes * std::mem::size_of::<NormalizedScalarExpressionV1>();
        let mut meter = NativeValueMeter {
            budget: &mut budget,
            failed: false,
        };
        meter.work(reserved_nodes).unwrap();
        meter.reserve(bytes).unwrap();
        {
            let mut expansion = NativeValueExpansion {
                helpers: None,
                meter: &mut meter,
                reserved: 0,
                temporary_nodes_remaining: active.then_some(NODES),
            };
            let mut visiting = BTreeSet::new();
            let expression = normalize_kir_expression_v1(
                function,
                &kir,
                &BTreeMap::new(),
                ValueId(levels + trailing_not),
                0,
                &mut visiting,
                &mut correlation,
                &mut expansion,
            );
            assert_eq!(expression.is_some(), expected_nodes.is_some());
            assert_eq!(
                expansion.temporary_nodes_remaining,
                if active {
                    Some(NODES - expected_nodes.unwrap_or(NODES))
                } else {
                    None
                }
            );
            assert!(visiting.is_empty());
            assert!(correlation.remaining > 500_000 && correlation.remaining < before);
            drop(expression);
        }
        meter.release(bytes).unwrap();
        assert_eq!(meter.budget.storage(), floor);
        assert!(meter.budget.peak_storage() >= floor + bytes);
    }
}

#[test]
fn per_argument_failure_restores_enclosing_allowance_and_keeps_correlation_limit() {
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let module = doubling(14, 0);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget
        .reserve_storage(FLOOR + receipt.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let function = &owner.module().functions[0];
    let mut correlation = UnsupportedIndexCorrelationBudgetV1 {
        remaining: 1_000_000,
    };
    let kir =
        build_kir_correlation_index(function.body.as_ref().unwrap(), 64, &mut correlation).unwrap();
    let bytes = NODES * std::mem::size_of::<NormalizedScalarExpressionV1>();
    let mut meter = NativeValueMeter {
        budget: &mut budget,
        failed: false,
    };
    meter.reserve(bytes).unwrap();
    {
        let mut expansion = NativeValueExpansion {
            helpers: None,
            meter: &mut meter,
            reserved: 0,
            temporary_nodes_remaining: Some(17),
        };
        let mut visiting = BTreeSet::new();
        assert!(
            expansion
                .argument(
                    function,
                    &kir,
                    &BTreeMap::new(),
                    ValueId(14),
                    0,
                    &mut visiting,
                    &mut correlation
                )
                .is_none()
        );
        assert_eq!(expansion.temporary_nodes_remaining, Some(17));
        assert!(visiting.is_empty());
        let mut exhausted = UnsupportedIndexCorrelationBudgetV1 { remaining: 0 };
        assert!(
            expansion
                .argument(
                    function,
                    &kir,
                    &BTreeMap::new(),
                    ValueId(0),
                    0,
                    &mut visiting,
                    &mut exhausted
                )
                .is_none()
        );
        assert_eq!(expansion.temporary_nodes_remaining, Some(17));
    }
    meter.release(bytes).unwrap();
    assert_eq!(meter.budget.storage(), floor);
}

#[test]
fn argument_allowance_restores_original_state_and_panic_payload() {
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let mut meter = NativeValueMeter {
        budget: &mut budget,
        failed: false,
    };
    for original in [None, Some(0), Some(17)] {
        let mut expansion = NativeValueExpansion {
            helpers: None,
            meter: &mut meter,
            reserved: 0,
            temporary_nodes_remaining: original,
        };
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            expansion.with_argument_allowance(|outer| {
                assert_eq!(outer.temporary_nodes_remaining, Some(NODES));
                outer.charge_normalization_node_v1().unwrap();
                let inner = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    outer.with_argument_allowance(|inner| {
                        inner.charge_normalization_node_v1().unwrap();
                        std::panic::panic_any(31u32);
                    });
                }));
                assert_eq!(outer.temporary_nodes_remaining, Some(NODES - 1));
                std::panic::resume_unwind(inner.unwrap_err());
            });
        }));
        assert_eq!(panic.unwrap_err().downcast_ref::<u32>(), Some(&31));
        assert_eq!(expansion.temporary_nodes_remaining, original);
    }
    assert_eq!(meter.budget.storage(), FLOOR);
}

fn nested_owner() -> ProductionPreRankedKirOwnerV1 {
    let binary = |left, right| SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::BitXor,
        left: fixture::copy(left),
        right: fixture::copy(right),
    };
    let mut statements = vec![fixture::assign(
        3,
        SemanticRvalueKindV1::Use(fixture::copy(1)),
    )];
    for _ in 0..11 {
        statements.push(fixture::assign(3, binary(3, 3)));
    }
    let root = fixture::function(
        30,
        true,
        3,
        vec![
            fixture::block(
                31,
                statements,
                fixture::call(1, vec![fixture::copy(3), fixture::copy(2)], 4, 1),
            ),
            fixture::block(
                32,
                vec![fixture::assign(3, binary(2, 4))],
                fixture::call(3, vec![fixture::copy(3), fixture::copy(2)], 5, 2),
            ),
            fixture::block(33, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let nested = fixture::function(
        60,
        false,
        0,
        vec![
            fixture::block(
                61,
                vec![],
                fixture::call(2, vec![fixture::copy(1), fixture::copy(2)], 0, 1),
            ),
            fixture::block(62, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let duplicate = fixture::function(
        70,
        false,
        0,
        vec![fixture::block(
            71,
            vec![fixture::assign(0, binary(1, 1))],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let identity = fixture::function(
        80,
        false,
        0,
        vec![fixture::block(
            81,
            vec![fixture::assign(
                0,
                SemanticRvalueKindV1::Use(fixture::copy(1)),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let base = fixture::source(vec![fixture::subtract()]);
    let semantic = InertSemanticMirRequestV1::new(
        base.target(),
        base.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root, nested, duplicate, identity],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        &semantic,
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "helper_value_source",
            [31; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = Work::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

#[test]
fn genuine_nested_helper_result_is_limited_before_emit_with_exact_ledger_boundaries() {
    let owner = nested_owner();
    let module = owner.executable().module();
    let function = module
        .functions
        .iter()
        .find(|f| f.id == module.kernels[0].entry)
        .unwrap();
    let value = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|b| &b.operations)
        .rfind(|op| matches!(op.kind, OperationKind::Call { .. }))
        .unwrap()
        .results[0]
        .id;
    let floor = FLOOR + owner.unit_local_source_storage_floor_v1().unwrap();
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let mut reached_preflight = false;
        let result = with_native_value_expansion_v1(
            Some(owner.semantic_ssa().source_semantic()),
            module,
            &owner.correspondence,
            module.kernels[0].id.as_str(),
            &mut budget,
            |expansion| {
                let mut correlation = UnsupportedIndexCorrelationBudgetV1 {
                    remaining: 1_000_000,
                };
                let kir = build_kir_correlation_index(
                    function.body.as_ref().unwrap(),
                    4096,
                    &mut correlation,
                )
                .unwrap();
                let mut visiting = BTreeSet::new();
                let result = normalize_kir_expression_v1(
                    function,
                    &kir,
                    &BTreeMap::new(),
                    value,
                    0,
                    &mut visiting,
                    &mut correlation,
                    expansion,
                );
                // The inner result is 8191 nodes, but only 8190 enclosing
                // nodes remain. No helper result payload may have been emitted.
                reached_preflight =
                    result.is_none() && expansion.reserved == 0 && !expansion.meter.exhausted();
                assert!(visiting.is_empty());
                assert!(correlation.remaining > 900_000);
                Err(ProductionMirPlironTranslationErrorV1::KernelShape)
            },
        );
        assert_eq!(budget.storage(), floor);
        (
            result,
            reached_preflight,
            budget.work(),
            budget.peak_storage(),
        )
    };
    let (result, reached, spent, peak) = run(WORK, STORAGE);
    assert!(matches!(
        result,
        Err(ProductionMirPlironTranslationErrorV1::KernelShape)
    ));
    assert!(reached);
    assert!(peak > floor);
    let (result, reached, _, _) = run(spent, peak);
    assert!(matches!(
        result,
        Err(ProductionMirPlironTranslationErrorV1::KernelShape)
    ));
    assert!(reached);
    for (work, storage) in [(spent - 1, peak), (spent, peak - 1)] {
        assert!(matches!(
            run(work, storage).0,
            Err(ProductionMirPlironTranslationErrorV1::ResourceLimit)
        ));
    }
}
