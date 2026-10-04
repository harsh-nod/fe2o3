use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resources,
    CanonicalKernelIrWorkBudgetV1 as Work, Signature,
};

fn constant_source(symbol: &str) -> Module {
    let mut block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
    block.operations.push(KirOperation::effect_free(
        fe2o3_kernel_ir::ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(7)),
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let mut source = Module::new("native_bridge");
    source
        .functions
        .push(fe2o3_kernel_ir::Function::internal_helper(
            symbol,
            Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
            vec![],
            vec![block],
        ));
    source
}

fn admit(source: &Module) -> VerifiedCanonicalKernelIrModuleV12 {
    admit_with_storage(source).0
}

fn admit_with_storage(source: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            source,
            &mut budget,
        )
        .unwrap();
    (owner, storage.retained_storage())
}

#[test]
fn independent_structural_censuses_cover_empty_declarations_types_and_results() {
    let empty = Census::source(&Module::new("empty")).unwrap();
    assert_eq!(
        empty,
        Census {
            tree: 3,
            slots: 0,
            functions: 0,
            definitions: 0,
            signature_nodes: 0,
            value_type_nodes: 0,
            edges: 0
        }
    );
    assert_eq!(empty.structural().unwrap(), 3);
    let constant = Census::source(&constant_source("constant")).unwrap();
    assert_eq!(
        constant,
        Census {
            tree: 11,
            slots: 2,
            functions: 1,
            definitions: 1,
            signature_nodes: 1,
            value_type_nodes: 1,
            edges: 0
        }
    );
    assert_eq!(constant.structural().unwrap(), 16);

    let nested = Type::pointer(
        Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut declaration = Module::new("declaration");
    declaration
        .functions
        .push(fe2o3_kernel_ir::Function::declaration(
            "external",
            Signature::new(vec![nested], vec![Type::Scalar(ScalarType::U64)]),
        ));
    let census = Census::source(&declaration).unwrap();
    assert_eq!(
        census,
        Census {
            tree: 3,
            slots: 0,
            functions: 1,
            definitions: 0,
            signature_nodes: 4,
            value_type_nodes: 0,
            edges: 0
        }
    );
    assert_eq!(census.structural().unwrap(), 8);
    declaration.functions[0].signature.parameters = vec![Type::Scalar(ScalarType::U32); 1024];
    assert_eq!(Census::source(&declaration).unwrap().signature_nodes, 1025);
    let mut deep = Type::Scalar(ScalarType::U32);
    for _ in 0..64 {
        deep = Type::pointer(deep, AddressSpace::Global, AccessMode::ReadOnly);
    }
    assert_eq!(source_type_nodes(&deep, 0).unwrap(), 65);
    deep = Type::pointer(deep, AddressSpace::Global, AccessMode::ReadOnly);
    assert!(source_type_nodes(&deep, 0).is_err());

    // Literal algebra, independent of any observed successful work threshold.
    for bytes in [0, 31, 4096] {
        let envelope = native_envelope(bytes, empty).unwrap();
        assert_eq!(envelope.work, 40 * bytes + 96);
        assert_eq!(envelope.storage, 64 * bytes + 4352);
        let envelope = native_envelope(bytes, constant).unwrap();
        assert_eq!(envelope.work, 144 * bytes + 1292);
        assert_eq!(envelope.storage, 64 * bytes + 4992);
        assert_eq!(
            native_envelope(bytes, census).unwrap().work,
            80 * bytes + 396
        );
    }
    assert!(native_envelope(usize::MAX, empty).is_err());
}

#[test]
fn duplicate_empty_edges_and_selector_use_are_counted_independently() {
    let mut entry = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: [0, 1]
            .into_iter()
            .map(|value| fe2o3_kernel_ir::SwitchCase {
                value,
                target: BlockId(1),
                arguments: vec![],
            })
            .collect(),
        default_target: BlockId(1),
        default_arguments: vec![],
    });
    let mut exit = fe2o3_kernel_ir::BasicBlock::new(BlockId(1));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut source = Module::new("edges");
    source
        .functions
        .push(fe2o3_kernel_ir::Function::internal_helper(
            "switch",
            Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
            vec![ValueId(0)],
            vec![entry, exit],
        ));
    let census = Census::source(&source).unwrap();
    assert_eq!(
        census,
        Census {
            tree: 12,
            slots: 2,
            functions: 1,
            definitions: 1,
            signature_nodes: 1,
            value_type_nodes: 0,
            edges: 3
        }
    );
    assert_eq!(census.structural().unwrap(), 19);
    let input = admit(&source);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (graph, witness) = import_native_neutral_v1(&input, &mut budget).unwrap();
    budget
        .charge_work(input.canonical().canonical_bytes().len() + census.tree)
        .unwrap();
    let live = witness.live_census(&graph, &mut budget).unwrap();
    assert_eq!(live.edges, 3);
    assert_eq!(live.slots, 2);
    // Entry argument decoding adds one live value type to the source signature.
    assert_eq!(live.value_type_nodes, 1);
    assert_eq!(live.structural().unwrap(), 20);
    drop(witness);
    drop(graph);
    let release = budget.storage();
    budget.release_storage(release).unwrap();
}

#[test]
fn import_exact_and_one_under_work_and_late_storage_preserve_nonzero_floor() {
    for source in [Module::new("empty"), constant_source("constant")] {
        let (input, input_storage) = admit_with_storage(&source);
        let bytes = input.canonical().canonical_bytes().len();
        let (envelope_work, envelope_storage, witness_work, definitions) =
            if source.functions.is_empty() {
                (40 * bytes + 96, 64 * bytes + 4352, 4, 0)
            } else {
                (144 * bytes + 1292, 64 * bytes + 4992, 12, 1)
            };
        let witness_storage = std::mem::size_of::<NativeBridgeWitnessV1>()
            + definitions * std::mem::size_of::<SignatureRow>();
        let digest_work = bytes + 12 + KIR_PLIRON_BRIDGE_V12_IDENTITY_DOMAIN_V1.len();
        let exact_work = bytes + envelope_work + digest_work + witness_work;
        const PREFIX: usize = 7;
        let floor = 11 + input_storage;
        for (limit, storage_limit, expected_work, expected_peak, success) in [
            (
                PREFIX + exact_work,
                floor + envelope_storage + witness_storage,
                PREFIX + exact_work,
                floor + envelope_storage + witness_storage,
                true,
            ),
            // The first opaque envelope is denied before allocating the session.
            (
                PREFIX + bytes + envelope_work - 1,
                usize::MAX,
                PREFIX + bytes,
                floor,
                false,
            ),
            // Final witness census denial drops the already-built session.
            (
                PREFIX + exact_work - 1,
                usize::MAX,
                PREFIX + exact_work - witness_work,
                floor + envelope_storage,
                false,
            ),
            // Witness allocation is the final import storage admission.
            (
                PREFIX + exact_work,
                floor + envelope_storage + witness_storage - 1,
                PREFIX + exact_work,
                floor + envelope_storage,
                false,
            ),
        ] {
            let mut work = Work::new(limit);
            work.charge_work(PREFIX).unwrap();
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = import_native_neutral_v1(&input, &mut budget);
            assert_eq!(result.is_ok(), success);
            assert_eq!(budget.work(), expected_work);
            assert_eq!(budget.peak_storage(), expected_peak);
            if limit < PREFIX + exact_work {
                let denied_actual = if limit == PREFIX + exact_work - 1 {
                    PREFIX + exact_work
                } else {
                    PREFIX + bytes + envelope_work
                };
                assert!(
                    matches!(&result, Err(KirBridgeErrorV12::Resource(Resources::Work(error)))
                    if error.actual() == denied_actual && error.limit() == limit)
                );
            }
            drop(result);
            let release = budget.storage() - floor;
            budget.release_storage(release).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(input.module(), &source);
        }
    }
}

#[test]
fn live_type_census_exact_and_one_under_are_allocation_free() {
    let input = admit(&constant_source("constant"));
    let mut import_work = Work::new(usize::MAX);
    let mut import_budget = Budget::new(&mut import_work, usize::MAX);
    let (graph, witness) = import_native_neutral_v1(&input, &mut import_budget).unwrap();
    let header = input.canonical().canonical_bytes().len() + 11;
    for suffix in [65, 66] {
        let mut work = Work::new(7 + header + suffix);
        work.charge_work(7).unwrap();
        let mut budget = Budget::new(&mut work, 11);
        budget.reserve_storage(11).unwrap();
        budget.charge_work(header).unwrap();
        let result = witness.live_census(&graph, &mut budget);
        if suffix == 66 {
            assert_eq!(result.unwrap().structural().unwrap(), 16);
            assert_eq!(budget.work(), 7 + header + 66);
        } else {
            assert!(
                matches!(result, Err(KirBridgeErrorV12::Resource(Resources::Work(error)))
                if error.actual() == 7 + header + 66 && error.limit() == 7 + header + 65)
            );
            assert_eq!(budget.work(), 7 + header);
        }
        assert_eq!(budget.storage(), 11);
        assert_eq!(budget.peak_storage(), 11);
    }
    drop(witness);
    drop(graph);
    let release = import_budget.storage();
    import_budget.release_storage(release).unwrap();
}

#[test]
fn native_signature_witness_rejects_foreign_graph_and_changed_function_type() {
    let input = admit(&constant_source("constant"));
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (graph, witness) = import_native_neutral_v1(&input, &mut budget).unwrap();
    let (foreign, foreign_witness) = import_native_neutral_v1(&input, &mut budget).unwrap();
    budget
        .charge_work(input.canonical().canonical_bytes().len() + 11)
        .unwrap();
    assert!(matches!(
        witness.live_census(&foreign, &mut budget),
        Err(KirBridgeErrorV12::Bridge(
            KirBridgeErrorV1::GraphIdentityMismatch
        ))
    ));
    let function = FuncOp::from_operation(witness.signatures[0].function);
    let changed = FunctionType::get(&graph.session.context, vec![], vec![]);
    function.set_attr_func_type(
        &graph.session.context,
        pliron::builtin::attributes::TypeAttr::new(changed.into()),
    );
    budget
        .charge_work(input.canonical().canonical_bytes().len() + 11)
        .unwrap();
    assert!(matches!(
        witness.live_census(&graph, &mut budget),
        Err(KirBridgeErrorV12::Bridge(
            KirBridgeErrorV1::GraphIdentityMismatch
        ))
    ));
    drop(foreign_witness);
    drop(foreign);
    drop(witness);
    drop(graph);
    let release = budget.storage();
    budget.release_storage(release).unwrap();
}

#[test]
fn closed_native_optimizer_runs_seven_passes_with_long_metadata_and_transfers_output() {
    let short = constant_source("constant");
    let long = constant_source(&"s".repeat(4000));
    let short_census = Census::source(&short).unwrap();
    assert_eq!(Census::source(&long).unwrap(), short_census);
    let mut byte_lengths = Vec::new();
    let mut envelope_work = Vec::new();
    for source in [short, long] {
        let (input, input_storage) = admit_with_storage(&source);
        let bytes = input.canonical().canonical_bytes().len();
        byte_lengths.push(bytes);
        envelope_work.push(native_envelope(bytes, short_census).unwrap().work);
        let mut work = Work::new(1_000_000_000);
        work.charge_work(7).unwrap();
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        let floor = 11 + input_storage;
        budget.reserve_storage(floor).unwrap();
        let output = crate::optimize_native_neutral_kernel_ir_v1(&input, &mut budget).unwrap();
        assert!(std::ptr::eq(output.input(), &input));
        assert_eq!(output.owner().module(), &source);
        assert_eq!(output.report().passes().len(), 7);
        assert!(output.map().matches_execution(output.report()));
        assert_eq!(budget.storage(), floor);
        let retained = output.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        drop(output);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(
        envelope_work[1] - envelope_work[0],
        144 * (byte_lengths[1] - byte_lengths[0])
    );
}

#[test]
fn live_nested_signature_and_multi_result_type_occurrences_are_not_omitted() {
    let nested = Type::pointer(
        Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let mut nested_source = Module::new("nested");
    nested_source
        .functions
        .push(fe2o3_kernel_ir::Function::internal_helper(
            "nested",
            Signature::new(vec![nested.clone()], vec![nested]),
            vec![ValueId(0)],
            vec![block],
        ));

    let mut block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
    block.operations.push(KirOperation::checked_binary(
        fe2o3_kernel_ir::ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        fe2o3_kernel_ir::ValueDef::new(ValueId(3), Type::BOOL),
        fe2o3_kernel_ir::CheckedBinaryOperator::Add,
        ValueId(0),
        ValueId(1),
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2), ValueId(3)],
    });
    let mut pair_source = Module::new("pair");
    pair_source
        .functions
        .push(fe2o3_kernel_ir::Function::internal_helper(
            "pair",
            Signature::new(
                vec![Type::Scalar(ScalarType::U32); 2],
                vec![Type::Scalar(ScalarType::U32), Type::BOOL],
            ),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        ));
    let mut block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Unreachable);
    let mut no_return_source = Module::new("no_return");
    no_return_source
        .functions
        .push(fe2o3_kernel_ir::Function::internal_helper(
            "no_return",
            Signature::new(
                vec![],
                vec![Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                )],
            ),
            vec![],
            vec![block],
        ));

    for (source, source_structure, live_structure, live_types) in [
        (nested_source, 18, 21, 3),
        (pair_source, 26, 28, 4),
        (no_return_source, 12, 12, 0),
    ] {
        let census = Census::source(&source).unwrap();
        assert_eq!(census.structural().unwrap(), source_structure);
        let (input, input_storage) = admit_with_storage(&source);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(input_storage).unwrap();
        let (graph, witness) = import_native_neutral_v1(&input, &mut budget).unwrap();
        budget
            .charge_work(input.canonical().canonical_bytes().len() + census.tree)
            .unwrap();
        let live = witness.live_census(&graph, &mut budget).unwrap();
        assert_eq!(live.structural().unwrap(), live_structure);
        assert_eq!(live.value_type_nodes, live_types);
        drop(witness);
        drop(graph);
        let release = budget.storage() - input_storage;
        budget.release_storage(release).unwrap();
        assert_eq!(budget.storage(), input_storage);
    }
}

#[test]
fn closed_native_entry_restores_floor_after_initial_work_denial_and_import_unwind() {
    let (input, input_storage) = admit_with_storage(&constant_source("constant"));
    let floor = 11 + input_storage;
    let mut work = Work::new(7);
    work.charge_work(7).unwrap();
    {
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        let error = match crate::optimize_native_neutral_kernel_ir_v1(&input, &mut budget) {
            Err(error) => error,
            Ok(_) => panic!("expected initial work denial"),
        };
        assert!(
            matches!(error, crate::KirNeutralOptimizationErrorV1::Bridge(
        KirBridgeErrorV12::Resource(Resources::Work(error)))
        if error.actual() == 7 + input.canonical().canonical_bytes().len() && error.limit() == 7)
        );
        assert_eq!(budget.work(), 7);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
    }
    assert_eq!(work.failed_work(), Some(usize::MAX));

    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000_000);
    budget.reserve_storage(floor).unwrap();
    super::super::connected_v12_tests::FAIL_IMPORT_AFTER_BUILD.with(|fail| fail.set(true));
    assert!(matches!(
        crate::optimize_native_neutral_kernel_ir_v1(&input, &mut budget),
        Err(crate::KirNeutralOptimizationErrorV1::Bridge(
            KirBridgeErrorV12::Bridge(KirBridgeErrorV1::UpstreamPanicked)
        ))
    ));
    assert_eq!(budget.storage(), floor);
    assert!(budget.peak_storage() > floor);
}
