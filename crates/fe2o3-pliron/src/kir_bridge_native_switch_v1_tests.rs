use super::*;
use fe2o3_kernel_ir::{
    BasicBlock as Block, CanonicalKernelIrWorkBudgetV1 as Work, Function, IntegerSwitchCase,
    Signature, SwitchCase, ValueDef,
};

fn source(ty: ScalarType, typed: Option<Vec<Constant>>, keys: Vec<u64>) -> Module {
    let mut entry = Block::new(BlockId(10));
    entry.terminator = Some(match typed {
        Some(cases) => Terminator::IntegerSwitch {
            selector: ValueId(0),
            cases: cases
                .into_iter()
                .enumerate()
                .map(|(i, value)| IntegerSwitchCase {
                    value,
                    target: BlockId(90),
                    arguments: if i % 2 == 0 {
                        vec![ValueId(1), ValueId(2)]
                    } else {
                        vec![ValueId(2), ValueId(1)]
                    },
                })
                .collect(),
            default_target: BlockId(90),
            default_arguments: vec![ValueId(2), ValueId(2)],
        },
        None => Terminator::Switch {
            selector: ValueId(0),
            cases: keys
                .into_iter()
                .enumerate()
                .map(|(i, value)| SwitchCase {
                    value,
                    target: BlockId(90),
                    arguments: if i % 2 == 0 {
                        vec![ValueId(1), ValueId(2)]
                    } else {
                        vec![ValueId(2), ValueId(1)]
                    },
                })
                .collect(),
            default_target: BlockId(90),
            default_arguments: vec![ValueId(2), ValueId(2)],
        },
    });
    let mut join = Block::new(BlockId(90));
    join.parameters = vec![
        ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
    ];
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let mut module = Module::new("native-switch-exact");
    module.functions.push(Function::internal_helper(
        "not_generated",
        Signature::new(
            vec![
                Type::Scalar(ty),
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
            ],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, join],
    ));
    module
}

fn admitted(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = Work::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, storage.retained_storage())
}

fn first_switch(graph: &KirPlironGraphV12<'_>) -> Ptr<Operation> {
    let context = &graph.session.context;
    let block = graph
        .origins
        .blocks
        .iter()
        .find_map(|(block, (_, id))| (*id == BlockId(10)).then_some(*block))
        .unwrap();
    block.deref(context).get_terminator(context).unwrap()
}

fn exact(module: &Module, native: bool) {
    let (input, input_storage) = admitted(module);
    let mut work = Work::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(37 + input_storage).unwrap();
    let (mut graph, storage) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let pointer = first_switch(&graph);
    assert_eq!(
        Operation::is_op::<SwitchOpV3>(pointer, &graph.session.context),
        native
    );
    assert_eq!(
        graph.origins.preserved_terminators.contains_key(&pointer),
        !native
    );
    let (output, report, output_storage) = graph
        .extract_canonical_kir_module_v12_o0(&mut budget)
        .unwrap();
    budget
        .reserve_storage(output_storage.retained_storage())
        .unwrap();
    assert_eq!(output.module(), module);
    assert_eq!(
        output.canonical().canonical_bytes(),
        input.canonical().canonical_bytes()
    );
    drop(output);
    drop(report);
    budget
        .release_storage(output_storage.retained_storage())
        .unwrap();
    drop(graph);
    budget.release_storage(storage.retained_storage()).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn native_switch_exact_legacy_order_wide_selector_and_each_payload_are_preserved() {
    for ty in [
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::I128,
        ScalarType::U128,
        ScalarType::Index,
    ] {
        exact(&source(ty, None, vec![u64::MAX, 0, 1 << 63, 7]), true);
        exact(&source(ty, None, vec![]), true);
    }
    for ty in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
    ] {
        exact(&source(ty, None, vec![255, 0, 7]), true);
    }
}

#[test]
fn native_switch_exact_typed_full_contract_and_empty_wide_variants_are_preserved() {
    for cases in [
        vec![
            Constant::I8(i8::MIN),
            Constant::I8(0),
            Constant::I8(i8::MAX),
        ],
        vec![
            Constant::I16(i16::MIN),
            Constant::I16(0),
            Constant::I16(i16::MAX),
        ],
        vec![
            Constant::I32(i32::MIN),
            Constant::I32(0),
            Constant::I32(i32::MAX),
        ],
        vec![
            Constant::I64(i64::MIN),
            Constant::I64(0),
            Constant::I64(i64::MAX),
        ],
        vec![Constant::U8(0), Constant::U8(u8::MAX)],
        vec![Constant::U16(0), Constant::U16(u16::MAX)],
        vec![Constant::U32(0), Constant::U32(u32::MAX)],
        vec![
            Constant::U64(0),
            Constant::U64(1 << 63),
            Constant::U64(u64::MAX),
        ],
        vec![Constant::Index(0), Constant::Index(u64::MAX)],
    ] {
        let Type::Scalar(ty) = cases[0].ty() else {
            unreachable!()
        };
        for value in &cases {
            let (kind, bits) = encode(value).unwrap();
            assert_eq!(decode(kind, bits).unwrap(), *value);
        }
        exact(&source(ty, Some(cases), vec![]), true);
        exact(&source(ty, Some(vec![]), vec![]), true);
    }
    for ty in [ScalarType::I128, ScalarType::U128] {
        exact(&source(ty, Some(vec![]), vec![]), true);
    }
    assert!(decode(Kind::U8, 256).is_err());
    assert!(decode(Kind::I16, 1 << 16).is_err());
}

#[test]
fn native_switch_general_bridge_retains_out_of_range_legacy_keys_without_narrowing() {
    for (ty, key) in [
        (ScalarType::U8, 256),
        (ScalarType::I8, u64::MAX),
        (ScalarType::U16, 1 << 16),
        (ScalarType::I32, u64::MAX),
    ] {
        exact(&source(ty, None, vec![key, 0]), false);
    }
}

#[test]
fn native_switch_extraction_reads_live_keys_instead_of_an_original_template() {
    let module = source(ScalarType::U64, None, vec![2, 9]);
    let (input, input_storage) = admitted(&module);
    let mut work = Work::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(input_storage).unwrap();
    let (mut graph, storage) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let pointer = first_switch(&graph);
    let context = &graph.session.context;
    let switch = SwitchOpV3::from_operation(pointer);
    // A typed U64 carrier has a different canonical terminator variant even
    // when key bits and every physical use are byte-for-byte unchanged.
    switch.set_attr_gpu_switch_kind(context, Kind::U64);
    assert!(
        graph
            .extract_canonical_kir_module_v12_o0(&mut budget)
            .is_err()
    );
    drop(graph);
    budget.release_storage(storage.retained_storage()).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
}

#[test]
fn native_switch_same_byte_foreign_graph_handle_cannot_borrow_the_original_session() {
    let module = source(ScalarType::U64, None, vec![2, 9]);
    let (input, input_storage) = admitted(&module);
    let (foreign, foreign_storage) = admitted(&module);
    assert!(!std::ptr::eq(&input, &foreign));
    assert_eq!(
        input.canonical().canonical_bytes(),
        foreign.canonical().canonical_bytes()
    );
    let mut work = Work::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget
        .reserve_storage(input_storage + foreign_storage)
        .unwrap();
    let (mut graph, storage) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (donor, donor_storage) = KirPlironGraphV12::import(&foreign, &mut budget).unwrap();
    budget
        .reserve_storage(donor_storage.retained_storage())
        .unwrap();
    let original = graph.root.clone();
    graph.root = donor.root.clone();
    assert!(matches!(
        graph.validate_custody_v12(),
        Err(KirBridgeErrorV12::Bridge(
            KirBridgeErrorV1::GraphIdentityMismatch
        ))
    ));
    assert!(
        graph
            .extract_canonical_kir_module_v12_o0(&mut budget)
            .is_err()
    );
    graph.root = original;
    drop(donor);
    budget
        .release_storage(donor_storage.retained_storage())
        .unwrap();
    drop(graph);
    budget.release_storage(storage.retained_storage()).unwrap();
    drop(input);
    drop(foreign);
    budget
        .release_storage(input_storage + foreign_storage)
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn native_switch_representability_lookup_pays_actual_definition_and_key_visits() {
    let module = source(ScalarType::U8, None, vec![255, 0]);
    let function = &module.functions[0];
    let Terminator::Switch {
        selector, cases, ..
    } = function.body.as_ref().unwrap().blocks[0]
        .terminator
        .as_ref()
        .unwrap()
    else {
        unreachable!()
    };
    for allowance in 0..=3 {
        let mut work = Work::new(11 + allowance);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 37);
        budget.charge_work(11).unwrap();
        budget.reserve_storage(37).unwrap();
        let result = source_legacy_representable(function, *selector, cases, &mut budget);
        if allowance == 3 {
            assert_eq!(result, Ok(true));
        } else {
            assert!(
                matches!(result, Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error)) if error.actual() == 12 + allowance)
            );
        }
        assert_eq!(budget.work(), 11 + allowance);
        assert_eq!(
            (
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (37, 37, None)
        );
    }
}

#[test]
fn native_switch_import_literal_envelope_has_exact_success_and_each_work_storage_boundary() {
    use std::mem::{align_of, size_of};
    const DOMAIN: &[u8] = b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V12/V1\0";
    assert_eq!(KIR_PLIRON_BRIDGE_V12_IDENTITY_DOMAIN_V1, DOMAIN);
    assert_eq!(size_of::<usize>(), 8);
    // Private-layout premise: SignatureRow contains exactly these two handles.
    // No field is observed from a successful production receipt to infer it.
    let row = size_of::<(Ptr<Operation>, TypeHandle)>();
    assert_eq!(
        row,
        (size_of::<Ptr<Operation>>() + size_of::<TypeHandle>())
            .next_multiple_of(align_of::<(Ptr<Operation>, TypeHandle)>())
    );
    let witness_storage = size_of::<NativeBridgeWitnessV1>() + row;
    for count in [0, 1, 16, 17] {
        for typed in [false, true] {
            let module = if typed {
                source(
                    ScalarType::I64,
                    Some(
                        (0..count)
                            .map(|i| Constant::I64(i as i64 - count as i64))
                            .collect(),
                    ),
                    vec![],
                )
            } else {
                source(
                    ScalarType::U64,
                    None,
                    (0..count).map(|i| u64::MAX - i as u64).collect(),
                )
            };
            let (input, input_storage) = admitted(&module);
            let bytes = input.canonical().canonical_bytes().len();
            let edges = count + 1;
            // root3+function3+blocks2+terminators4 = tree12;
            // slots3+2+(1+2E)+1 = 7+2E; signature4, value types2,
            // function1 and edgeE give structural26+3E, then volume+1.
            let volume = 27 + 3 * edges;
            let envelope_work = 4 * volume * volume + 8 * bytes * volume + 8 * (bytes + volume);
            let envelope_storage = 64 * (bytes + 20 + 2 * edges) + 4096;
            let digest_work = bytes + 12 + DOMAIN.len();
            let charges = [bytes, envelope_work, digest_work, 13];
            let exact_work: usize = charges.iter().sum();
            let floor = 37 + input_storage;
            let exact_storage = envelope_storage + witness_storage;
            for cut in 0..7 {
                let (work_limit, storage_limit, accepted_work, accepted_peak, denied_storage) =
                    match cut {
                        0..=3 => {
                            let before: usize = charges[..cut].iter().sum();
                            (
                                11 + before + charges[cut] - 1,
                                usize::MAX,
                                11 + before,
                                floor + if cut >= 2 { envelope_storage } else { 0 },
                                None,
                            )
                        }
                        4 => (
                            usize::MAX,
                            floor + envelope_storage - 1,
                            11 + bytes + envelope_work,
                            floor,
                            Some(floor + envelope_storage),
                        ),
                        5 => (
                            usize::MAX,
                            floor + exact_storage - 1,
                            11 + exact_work,
                            floor + envelope_storage,
                            Some(floor + exact_storage),
                        ),
                        _ => (
                            11 + exact_work,
                            floor + exact_storage,
                            11 + exact_work,
                            floor + exact_storage,
                            None,
                        ),
                    };
                let mut work = Work::new(work_limit);
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
                budget.charge_work(11).unwrap();
                budget.reserve_storage(floor).unwrap();
                let result = import_native_neutral_v1(&input, &mut budget);
                assert_eq!(
                    result.is_ok(),
                    cut == 6,
                    "typed={typed} count={count} cut={cut}"
                );
                assert_eq!(budget.work(), accepted_work);
                assert_eq!(budget.peak_storage(), accepted_peak);
                assert_eq!(budget.failed_storage(), denied_storage);
                if cut < 4 {
                    assert!(
                        matches!(&result, Err(KirBridgeErrorV12::Resource(CanonicalKernelIrVerificationResourceErrorV1::Work(error))) if error.actual() == work_limit + 1 && error.limit() == work_limit)
                    );
                } else if cut < 6 {
                    assert!(
                        matches!(&result, Err(KirBridgeErrorV12::Resource(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))) if Some(error.actual()) == denied_storage && error.limit() == storage_limit)
                    );
                }
                drop(result);
                budget.release_storage(budget.storage() - floor).unwrap();
                assert_eq!(budget.storage(), floor);
            }
        }
    }
}
