use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, CanonicalKernelIrWorkBudgetV1 as Work, Function, Signature, StorageFieldV1,
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1, StoragePointerV1,
};
use std::mem::size_of;

const FLOOR: usize = 19;

fn cyclic_fixture() -> Module {
    let mut module = Module::new("cyclic-table-bridge");
    module.storage_layouts = vec![
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: StorageLayoutIdV1(1),
                value_space: AddressSpace::Global,
                encoded_space: AddressSpace::Global,
                access: AccessMode::ReadWrite,
                stored_bits: 64,
            }),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                }]
                .into_boxed_slice(),
            ),
        },
    ];
    // Row1 contains row0 by value; row0 refers back to row1 through a pointer.
    // The live signature contains one finite nominal view, not an unfolded type.
    let address = Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(1)),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    module.functions.push(Function::internal_helper(
        "identity",
        Signature::new(vec![address.clone()], vec![address]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

fn roundtrip(source: &Module, declarations_only: bool) {
    let mut work = Work::new(tests::AMPLE);
    let mut budget = Budget::new(&mut work, tests::AMPLE);
    budget.reserve_storage(FLOOR).unwrap();
    let (input, input_credit) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            source,
            tests::LIMITS,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    budget
        .reserve_storage(input_credit.retained_storage())
        .unwrap();
    let (mut graph, graph_credit) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    budget
        .reserve_storage(graph_credit.retained_storage())
        .unwrap();
    let floor = budget.storage();
    assert_eq!(graph.origins.functions.is_empty(), declarations_only);
    assert_eq!(graph.coordinates.is_empty(), declarations_only);
    for exact in [true, false] {
        let (output, report, credit) = if exact {
            graph.extract_canonical_v18_o0(tests::LIMITS, &mut budget)
        } else {
            graph.extract_canonical_v18(tests::LIMITS, &mut budget)
        }
        .unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(credit.retained_storage()).unwrap();
        assert_eq!(output.module(), source);
        assert_eq!(output.canonical_bytes(), input.canonical_bytes());
        assert_eq!(report.input, report.output);
        assert_eq!(report.table, graph.table_identity());
        assert_eq!(report.correspondence, graph.correspondence());
        assert_eq!(report.correspondence.is_empty(), declarations_only);
        assert!(std::ptr::eq(
            output.verified_storage_module_ref_v1().module(),
            output.module()
        ));
        assert!(!std::ptr::eq(
            output.module().storage_layouts.as_ptr(),
            input.module().storage_layouts.as_ptr()
        ));
        drop((output, report));
        budget.release_storage(credit.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(input.module(), source);
    drop(graph);
    budget
        .release_storage(graph_credit.retained_storage())
        .unwrap();
    drop(input);
    budget
        .release_storage(input_credit.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(budget.work_budget_v1().failed_work(), None);
}

#[test]
fn storage_v18_cyclic_pointer_table_copy_and_live_roundtrip_use_the_finite_roster() {
    let source = cyclic_fixture();
    let payload = 2 * size_of::<StorageLayoutV1>() + size_of::<StorageFieldV1>();
    // Two physical rows plus one record field. The pointer's referent is not
    // another copy traversal, and the back-edge must not expand the record.
    let mut work = Work::new(3);
    let mut budget = Budget::new(&mut work, FLOOR + payload);
    budget.reserve_storage(FLOOR).unwrap();
    assert_eq!(
        resources::table_bytes(&source, &mut budget).unwrap(),
        payload
    );
    assert_eq!(budget.work(), 3);
    budget.reserve_storage(payload).unwrap();
    let copy = resources::copy_table(&source).unwrap();
    assert_eq!(copy, source.storage_layouts);
    assert_eq!(copy.capacity(), 2);
    assert_ne!(copy.as_ptr(), source.storage_layouts.as_ptr());
    match (&copy[1].kind, &source.storage_layouts[1].kind) {
        (StorageLayoutKindV1::Record(actual), StorageLayoutKindV1::Record(original)) => {
            assert_eq!(actual.len(), 1);
            assert_ne!(actual.as_ptr(), original.as_ptr());
        }
        _ => panic!("cyclic table's record row changed"),
    }
    drop(copy);
    budget.release_storage(payload).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + payload);
    assert_eq!(budget.failed_storage(), None);
    let mut short_work = Work::new(2);
    let mut short = Budget::new(&mut short_work, FLOOR + payload);
    short.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        resources::table_bytes(&source, &mut short),
        Err(ResourceError::Work(error)) if error.actual() == 3 && error.limit() == 2
    ));
    assert_eq!(short.work(), 2);
    assert_eq!(short.storage(), FLOOR);
    assert_eq!(short.work_budget_v1().failed_work(), Some(3));
    roundtrip(&source, false);
}

#[test]
fn storage_v18_declaration_only_nominal_signatures_survive_both_public_exports() {
    let mut source = Module::new("declarations-only-storage-bridge");
    source.storage_layouts = vec![
        tests::scalar(),
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Scalar(ScalarType::I64),
        },
    ];
    let pointer = Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(1)),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let slice = Type::slice(
        Type::StorageObject(StorageLayoutIdV1(0)),
        AddressSpace::Constant,
        AccessMode::ReadOnly,
    );
    source.functions.push(Function::declaration(
        "external_pointer",
        Signature::new(vec![slice.clone(), pointer.clone()], vec![pointer]),
    ));
    source.functions.push(Function::declaration(
        "external_slice",
        Signature::new(vec![slice.clone()], vec![slice]),
    ));
    assert!(
        source
            .functions
            .iter()
            .all(|function| function.body.is_none())
    );
    assert!(matches!(
        module_metadata_v12(&source),
        Err(KirBridgeErrorV1::UnsupportedType)
    ));
    let metadata = module_metadata_without_storage(&source);
    assert!(metadata.storage_layouts.is_empty());
    assert_eq!(metadata.functions, source.functions);
    roundtrip(&source, true);
}
