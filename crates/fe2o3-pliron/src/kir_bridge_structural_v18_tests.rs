use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Signature};

const SPACE: usize = 20_000_000;
const PREFIX: usize = 11;
const FLOOR: usize = 17;

fn constant_source() -> Module {
    let mut block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
    block.operations.push(KirOperation::effect_free(
        fe2o3_kernel_ir::ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(7)),
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let mut source = Module::new("constant");
    source
        .functions
        .push(fe2o3_kernel_ir::Function::internal_helper(
            "f",
            Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
            vec![],
            vec![block],
        ));
    source
}

#[test]
fn structural_v18_census_and_envelopes_have_independent_literals() {
    let empty = Census::source(&Module::new("m")).unwrap();
    assert_eq!(
        empty,
        Census {
            tree: 3,
            slots: 0,
            functions: 0,
            definitions: 0,
            signature_nodes: 0,
            value_type_nodes: 0,
            edges: 0,
        }
    );
    let constant = Census::source(&constant_source()).unwrap();
    assert_eq!(
        constant,
        Census {
            tree: 11,
            slots: 2,
            functions: 1,
            definitions: 1,
            signature_nodes: 1,
            value_type_nodes: 1,
            edges: 0,
        }
    );
    for bytes in [0, 41, 4096, 16384] {
        let e = native_envelope(bytes, empty).unwrap();
        assert_eq!(e.work, 40 * bytes + 96);
        assert_eq!(e.storage, 64 * bytes + 4352);
        let e = native_envelope(bytes, constant).unwrap();
        assert_eq!(e.work, 144 * bytes + 1292);
        assert_eq!(e.storage, 64 * bytes + 4992);
    }
    assert!(native_envelope(usize::MAX, empty).is_err());
    let nominal = Type::pointer(
        Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(0)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    assert_eq!(source_type_nodes(&nominal, 0).unwrap(), 2);
    assert_eq!(
        source_type_nodes(
            &Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Context),
            0
        )
        .unwrap(),
        1
    );
    let mut deep = nominal;
    for _ in 0..64 {
        deep = Type::pointer(deep, AddressSpace::Private, AccessMode::ReadWrite);
    }
    assert!(source_type_nodes(&deep, 0).is_err());
}

#[test]
fn structural_v18_real_empty_import_exact_short_work_and_storage() {
    // B=41, tree=3, structural volume=4. No successful-run threshold is used.
    const WORK: usize = 5 + 41 + 58 + 41 + (40 * 41 + 96) + 42 + 4;
    let input = super::super::tests::owner(&Module::new("m"));
    assert_eq!(input.canonical_bytes().len(), 41);
    let retained = 6976
        + resources::coordinate_envelope(3).unwrap().storage
        + size_of::<KirPlironGraphV18<'_>>()
        + size_of::<StructuralBridgeWitnessV18>();
    let peak = FLOOR + resources::headers().unwrap() + headers().unwrap() + retained;
    for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
        let mut work = Work::new(PREFIX + WORK - usize::from(short_work));
        work.charge_work(PREFIX).unwrap();
        let mut budget = Budget::new(&mut work, peak - usize::from(short_storage));
        budget.reserve_storage(FLOOR).unwrap();
        let result = import(&input, &mut budget);
        assert_eq!(budget.storage(), FLOOR);
        if short_work {
            assert!(matches!(
                result,
                Err(KirBridgeErrorV18::Resource(ResourceError::Work(_)))
            ));
            assert_eq!(budget.work(), PREFIX + WORK - 4);
            assert_eq!(budget.failed_work(), Some(PREFIX + WORK));
            assert_eq!(budget.failed_storage(), None);
        } else if short_storage {
            assert!(matches!(
                result,
                Err(KirBridgeErrorV18::Resource(ResourceError::Storage(_)))
            ));
            assert_eq!(budget.work(), PREFIX + WORK);
            assert_eq!(budget.failed_storage(), Some(peak));
            assert_eq!(budget.failed_work(), None);
        } else {
            let (graph, witness, receipt) = result.unwrap();
            assert_eq!(receipt.retained_storage(), retained);
            assert_eq!(graph.retained_storage(), retained);
            assert_eq!(budget.work(), PREFIX + WORK);
            assert_eq!(budget.peak_storage(), peak);
            budget.reserve_storage(retained).unwrap();
            drop(witness);
            drop(graph);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn structural_v18_metadata_padding_does_not_pay_byte_squared_import() {
    let mut observations = Vec::new();
    for padding in [0, 1024, 2048] {
        let source = Module::new("m".repeat(padding + 1));
        let input = super::super::tests::owner(&source);
        let mut work = Work::new(500_000_000);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(FLOOR).unwrap();
        let (graph, witness, receipt) = import(&input, &mut budget).unwrap();
        observations.push((input.canonical_bytes().len(), budget.work()));
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        drop(witness);
        drop(graph);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    for pair in observations.windows(2) {
        assert_eq!(pair[1].1 - pair[0].1, 42 * (pair[1].0 - pair[0].0));
    }
}

#[test]
fn structural_v18_live_roundtrip_keeps_nominal_table_and_exact_owner() {
    for source in [constant_source(), super::super::tests::fixture()] {
        let input = super::super::tests::owner(&source);
        let mut work = Work::new(500_000_000);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(FLOOR).unwrap();
        let (mut graph, witness, receipt) = import(&input, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        let (output, report, output_receipt) = graph
            .extract(
                super::super::tests::LIMITS,
                &mut budget,
                true,
                Some(&witness),
            )
            .unwrap();
        assert_eq!(output.canonical_bytes(), input.canonical_bytes());
        assert_eq!(report.table, graph.table_identity());
        assert_eq!(report.correspondence, graph.correspondence());
        assert_eq!(budget.storage(), floor);
        budget
            .reserve_storage(output_receipt.retained_storage())
            .unwrap();
        drop(output);
        drop(report);
        budget
            .release_storage(output_receipt.retained_storage())
            .unwrap();
        drop(witness);
        drop(graph);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn structural_v18_live_empty_export_exact_and_one_short_work() {
    const IMPORT: usize = 1927;
    const EXPORT: usize = 5 + 41 + 3 + (40 * 41 + 96) + 278 + 58 + 41;
    for short in [false, true] {
        let input = super::super::tests::owner(&Module::new("m"));
        let mut work = Work::new(PREFIX + IMPORT + EXPORT - usize::from(short));
        work.charge_work(PREFIX).unwrap();
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(FLOOR).unwrap();
        let (mut graph, witness, receipt) = import(&input, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        let result = graph.extract(
            super::super::tests::LIMITS,
            &mut budget,
            true,
            Some(&witness),
        );
        assert_eq!(budget.storage(), floor);
        if short {
            assert!(matches!(
                result,
                Err(KirBridgeErrorV18::Resource(ResourceError::Work(_)))
            ));
            assert_eq!(budget.work(), PREFIX + IMPORT + EXPORT - 41);
            assert_eq!(budget.failed_work(), Some(PREFIX + IMPORT + EXPORT));
        } else {
            let (output, report, output_receipt) = result.unwrap();
            assert_eq!(budget.work(), PREFIX + IMPORT + EXPORT);
            assert_eq!(output.canonical_bytes(), input.canonical_bytes());
            budget
                .reserve_storage(output_receipt.retained_storage())
                .unwrap();
            drop(output);
            drop(report);
            budget
                .release_storage(output_receipt.retained_storage())
                .unwrap();
        }
        drop(witness);
        drop(graph);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

fn wide_argument_source(width: usize) -> Module {
    let mut block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut source = Module::new("wide-arguments");
    source
        .functions
        .push(fe2o3_kernel_ir::Function::internal_helper(
            "f",
            Signature::new(vec![Type::Scalar(ScalarType::U32); width], vec![]),
            (0..width).map(|index| ValueId(index as u32)).collect(),
            vec![block],
        ));
    source
}

#[test]
fn live_argument_type_census_prepays_exact_index_and_type_bound_before_iteration() {
    // This isolates the private allocation-free census. Full extraction custody
    // and cleanup are exercised above and by the hostile witness controls below.
    for width in [1, 4, 64] {
        let input = super::super::tests::owner(&wide_argument_source(width));
        let mut fixture_work = Work::new(500_000_000);
        let mut fixture_budget = Budget::new(&mut fixture_work, SPACE);
        let (graph, witness, receipt) = import(&input, &mut fixture_budget).unwrap();
        fixture_budget
            .reserve_storage(receipt.retained_storage())
            .unwrap();
        // Pinned Pliron0.17 e054e5b performs one prefix position scan per
        // argument. All scalar type walks fit the unchanged 66-node depth cap.
        assert_eq!(fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1, 64);
        let expected = width * (width + 1) / 2 + 66 * width;
        for short in [false, true] {
            let mut work = Work::new(PREFIX + expected - usize::from(short));
            work.charge_work(PREFIX).unwrap();
            let mut budget = Budget::new(&mut work, FLOOR);
            budget.reserve_storage(FLOOR).unwrap();
            let result = witness.live_census(&graph, &mut budget);
            if short {
                assert!(
                    matches!(result, Err(KirBridgeErrorV18::Resource(ResourceError::Work(error)))
                    if error.actual() == PREFIX + expected && error.limit() == PREFIX + expected - 1)
                );
                assert_eq!(budget.work(), PREFIX);
            } else {
                let census = result.unwrap();
                assert_eq!(census.value_type_nodes, width);
                assert_eq!(budget.work(), PREFIX + expected);
            }
            assert_eq!(budget.storage(), FLOOR);
        }
        drop(witness);
        drop(graph);
        fixture_budget
            .release_storage(receipt.retained_storage())
            .unwrap();
        assert_eq!(fixture_budget.storage(), 0);
    }
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    assert!(matches!(
        precharge_argument_types(usize::MAX, &mut budget),
        Err(KirBridgeErrorV18::Resource(ResourceError::Arithmetic))
    ));
    assert_eq!(budget.work(), 0);
}

#[test]
fn structural_witness_rejects_foreign_root_function_and_signature_before_extraction() {
    let mut source = constant_source();
    let mut second = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
    second.operations.push(KirOperation::effect_free(
        fe2o3_kernel_ir::ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U64)),
        OperationKind::Constant(Constant::U64(9)),
    ));
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    source
        .functions
        .push(fe2o3_kernel_ir::Function::internal_helper(
            "g",
            Signature::new(vec![], vec![Type::Scalar(ScalarType::U64)]),
            vec![],
            vec![second],
        ));
    let input = super::super::tests::owner(&source);
    for mutation in 0..3 {
        let mut work = Work::new(500_000_000);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(FLOOR).unwrap();
        let (mut graph, mut witness, receipt) = import(&input, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let mut foreign = None;
        match mutation {
            0 => {
                let (other, other_witness, other_receipt) = import(&input, &mut budget).unwrap();
                budget
                    .reserve_storage(other_receipt.retained_storage())
                    .unwrap();
                witness.root = other_witness.root.clone();
                foreign = Some((other, other_witness, other_receipt));
            }
            1 => witness.signatures[0].function = witness.signatures[1].function,
            2 => witness.signatures[0].signature = witness.signatures[1].signature,
            _ => unreachable!(),
        }
        let floor = budget.storage();
        assert!(matches!(
            graph.extract(
                super::super::tests::LIMITS,
                &mut budget,
                true,
                Some(&witness)
            ),
            Err(KirBridgeErrorV18::Bridge(
                KirBridgeErrorV1::GraphIdentityMismatch
            ))
        ));
        assert_eq!(budget.storage(), floor);
        assert!(budget.failed_storage().is_none());
        if let Some((other, other_witness, other_receipt)) = foreign {
            drop(other_witness);
            drop(other);
            budget
                .release_storage(other_receipt.retained_storage())
                .unwrap();
        }
        drop(witness);
        drop(graph);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
