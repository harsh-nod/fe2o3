use super::*;
use crate::collector::TypedArgumentListV1;
use crate::compiler_descriptor::{AccessMode, ScalarTypeV1, TypedDescriptorArgumentV1};
use fe2o3_artifacts::RustcAbiClassV1;
use fe2o3_kernel_ir::{
    AccessMode as KirAccess, AddressSpace, CanonicalKernelIrWorkBudgetV1 as Work, ScalarType,
    SliceType,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use reserved_fe2o3_symbols::KernelBindingIdV1;

mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/defined_helper_semantic_fixture_v1.rs"
    ));
}

// Reuse genuine admitted source for root/semantic validation. These unit tests
// exercise the leaf's pure subchecks, not a forged conditional coverage result.
// Full sealed-owner and retained-wrapper calls belong to actual-source tests.
fn source() -> AdmittedInertSemanticMirV1 {
    let original = fixture::source(vec![fixture::subtract()]);
    let second = fixture::function(
        90,
        true,
        2,
        vec![
            fixture::block(
                91,
                vec![],
                fixture::call(1, vec![fixture::copy(1), fixture::copy(2)], 3, 1),
            ),
            fixture::block(92, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"second_source".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([61; 32]),
        original.functions()[0]
            .kernel_entry()
            .unwrap()
            .source_contract(),
    ));
    let mut functions = original.functions().to_vec();
    functions.push(second);
    InertSemanticMirRequestV1::new(
        original.target(),
        original.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(2),
        ],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn typed_roots(semantic: &AdmittedInertSemanticMirV1) -> Vec<TypedDescriptorRootV1> {
    semantic
        .roots()
        .iter()
        .map(|id| {
            let function = &semantic.functions()[id.index() as usize];
            let entry = function.kernel_entry().unwrap();
            TypedDescriptorRootV1 {
                logical_name: "descriptive_fixture".into(),
                export_name: String::from_utf8(entry.export_symbol().as_bytes().to_vec()).unwrap(),
                kernel_binding: KernelBindingIdV1::from_bytes(
                    *entry.kernel_binding_identity().as_bytes(),
                ),
                arguments: TypedArgumentListV1::new(
                    function
                        .abi()
                        .source_input_types()
                        .iter()
                        .enumerate()
                        .map(|(index, ty)| TypedDescriptorArgumentV1 {
                            name: format!("arg{index}"),
                            kind: DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32),
                            access: AccessMode::ByValue,
                            offset: u32::try_from(index * 4).unwrap(),
                            layout: None,
                            source_size: 4,
                            source_alignment: 4,
                            rustc_abi_class: RustcAbiClassV1::Scalar,
                            semantic_type_identity: semantic.types()[ty.index() as usize]
                                .identity(),
                        })
                        .collect(),
                )
                .unwrap(),
                explicit_argument_bytes: 8,
                kernarg_alignment_bytes: 4,
                source_launch: None,
            }
        })
        .collect()
}

#[test]
fn exact_ordered_roster_selects_the_borrowed_root_not_its_function_index() {
    let semantic = source();
    let roots = typed_roots(&semantic);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 19);
    budget.reserve_storage(19).unwrap();
    for (index, id) in semantic.roots().iter().enumerate() {
        let selected = select_typed_root_v1(&roots, &semantic, *id, &mut budget).unwrap();
        assert!(std::ptr::eq(selected, &roots[index]));
    }
    assert_eq!(semantic.roots()[1].index(), 2);
    // Content agreement is intentionally not same-transaction authentication.
    let copied = roots.clone();
    let selected =
        select_typed_root_v1(&copied, &semantic, semantic.roots()[1], &mut budget).unwrap();
    assert!(std::ptr::eq(selected, &copied[1]));
    assert!(!std::ptr::eq(selected, &roots[1]));
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.peak_storage(), 19);
}

#[test]
fn missing_duplicate_reordered_or_foreign_roots_fail_without_storage_changes() {
    let semantic = source();
    let original = typed_roots(&semantic);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 23);
    budget.reserve_storage(23).unwrap();
    for case in 0..6 {
        let mut roots = original.clone();
        match case {
            0 => roots.clear(),
            1 => {
                roots.pop();
            }
            2 => roots.swap(0, 1),
            3 => roots[1] = roots[0].clone(),
            4 => roots[1].kernel_binding = KernelBindingIdV1::from_bytes([99; 32]),
            5 => roots[1].export_name = roots[0].export_name.clone(),
            _ => unreachable!(),
        }
        assert!(
            select_typed_root_v1(&roots, &semantic, semantic.roots()[0], &mut budget).is_err(),
            "accepted roster mutation {case}",
        );
        assert_eq!(budget.storage(), 23);
    }
    for selected in [1, u32::MAX] {
        assert!(matches!(
            select_typed_root_v1(
                &original,
                &semantic,
                SemanticFunctionIdV1::from_index(selected),
                &mut budget,
            ),
            Err(Error::Root),
        ));
        assert_eq!(budget.storage(), 23);
    }
}

#[test]
fn existing_semantic_validator_errors_are_preserved() {
    let semantic = source();
    let original = typed_roots(&semantic);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 0);
    for case in 0..4 {
        let mut roots = original.clone();
        let mut arguments = roots[0].arguments.as_slice().to_vec();
        match case {
            0 => {
                arguments[0].semantic_type_identity = SemanticTypeIdentityV1::from_sha256([99; 32]);
            }
            1 => arguments[0].source_size = 8,
            2 => arguments[0].source_alignment = 8,
            3 => arguments[0].kind = DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
            _ => unreachable!(),
        }
        roots[0].arguments = TypedArgumentListV1::new(arguments).unwrap();
        let existing = validate_production_v1_semantic_root_ownership_evidence(
            &roots[0],
            &semantic,
            &semantic.functions()[0],
        )
        .unwrap_err();
        let Err(Error::Descriptor(actual)) =
            select_typed_root_v1(&roots, &semantic, semantic.roots()[0], &mut budget)
        else {
            panic!("expected the existing semantic validator's error");
        };
        assert_eq!(actual.to_string(), existing.to_string());
        assert_eq!(budget.storage(), 0);
    }
}

fn abi(arguments: Vec<SemanticAbiArgumentV1>) -> SemanticFunctionAbiV1 {
    let original = fixture::abi(false);
    let fixed = arguments
        .iter()
        .filter(|argument| argument.is_source())
        .count();
    SemanticFunctionAbiV1::from_rustc(
        original.identity(),
        original.layout_identity(),
        original.canon_abi(),
        original.extern_abi(),
        false,
        false,
        u32::try_from(fixed).unwrap(),
        arguments,
        original.return_value().clone(),
    )
    .unwrap()
}

#[test]
fn ignored_hidden_and_expanded_source_arguments_are_not_compacted() {
    let semantic = source();
    let original = fixture::abi(false);
    let scalar = original.arguments()[0].clone();
    let ignored = abi(vec![
        SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            fixture::UNIT,
            SemanticAbiPassModeV1::Ignore,
        )),
        scalar.clone(),
    ]);
    let hidden = abi(vec![
        scalar.clone(),
        SemanticAbiArgumentV1::hidden(
            SemanticAbiHiddenArgumentRoleV1::CallerLocation,
            scalar.value().clone(),
        ),
    ]);
    let expanded = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        original.identity(),
        original.layout_identity(),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        1,
        vec![fixture::WORD, SemanticTypeIdV1::from_index(2)],
        original.source_output_type(),
        vec![
            scalar.clone(),
            SemanticAbiArgumentV1::rust_call_tuple_field(0, scalar.value().clone()),
        ],
        original.return_value().clone(),
    )
    .unwrap();
    let mut types = semantic.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([90; 32]),
        SemanticLayoutIdentityV1::from_sha256([90; 32]),
        semantic.types()[1].layout().clone(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![fixture::WORD]).unwrap()),
    ));
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 0);
    require_flat_abi_v1(&original, &types, &mut budget).unwrap();
    for abi in [ignored, hidden, expanded] {
        assert!(matches!(
            require_flat_abi_v1(&abi, &types, &mut budget),
            Err(Error::UnsupportedAbi),
        ));
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn direct_tuple_and_nominal_context_inputs_are_not_flat_generated_fields() {
    let semantic = source();
    let original = fixture::abi(false);
    let tuple = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([91; 32]),
        SemanticLayoutIdentityV1::from_sha256([91; 32]),
        semantic.types()[1].layout().clone(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![fixture::WORD]).unwrap()),
    );
    let context =
        semantic.types()[1]
            .clone()
            .with_rust_type_kind(SemanticRustTypeKindV1::Execution(
                SemanticExecutionRoleV29::KernelContext,
            ));
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 0);
    // Hostile standalone type inputs test rejection, not semantic admission.
    for replacement in [tuple, context] {
        let mut types = semantic.types().to_vec();
        types[1] = replacement;
        assert!(matches!(
            require_flat_abi_v1(&original, &types, &mut budget),
            Err(Error::UnsupportedAbi),
        ));
    }
}

fn output_fields() -> (TypedDescriptorRootV1, SemanticTypeDeclV1, Type) {
    let semantic = source();
    let mut root = typed_roots(&semantic).remove(0);
    let ty = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([92; 32]),
        SemanticLayoutIdentityV1::from_sha256([92; 32]),
        SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    root.arguments = TypedArgumentListV1::new(
        (0..2)
            .map(|index| TypedDescriptorArgumentV1 {
                name: format!("arg{index}"),
                kind: DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::U32),
                access: AccessMode::ReadWrite,
                offset: index * 16,
                layout: None,
                source_size: 16,
                source_alignment: 8,
                rustc_abi_class: RustcAbiClassV1::ScalarPair,
                semantic_type_identity: ty.identity(),
            })
            .collect(),
    )
    .unwrap();
    root.explicit_argument_bytes = 32;
    root.kernarg_alignment_bytes = 8;
    let physical = Type::Slice(SliceType::new(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        KirAccess::ReadWrite,
    ));
    (root, ty, physical)
}

#[test]
fn same_type_output_fields_are_selected_by_certified_position_not_shape() {
    let (root, ty, physical) = output_fields();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 0);
    for index in 0..2 {
        let selected = checked_output_field_v1(&root, index, &ty, &physical, &mut budget).unwrap();
        assert_eq!(selected, index);
        assert_eq!(
            root.arguments.as_slice()[selected].offset,
            (index * 16) as u32
        );
    }
    for index in [2, usize::from(u16::MAX) + 1, usize::MAX] {
        assert!(matches!(
            checked_output_field_v1(&root, index, &ty, &physical, &mut budget),
            Err(Error::OutputArgument),
        ));
    }
}

#[test]
fn output_lookup_rejects_identity_access_kind_and_physical_type_substitutions() {
    let (original, ty, physical) = output_fields();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 29);
    budget.reserve_storage(29).unwrap();
    for case in 0..4 {
        let mut root = original.clone();
        let mut arguments = root.arguments.as_slice().to_vec();
        match case {
            0 => {
                arguments[1].semantic_type_identity = SemanticTypeIdentityV1::from_sha256([93; 32]);
            }
            1 => arguments[1].access = AccessMode::ReadOnly,
            2 => arguments[1].kind = DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
            3 => arguments[1].kind = DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32),
            _ => unreachable!(),
        }
        root.arguments = TypedArgumentListV1::new(arguments).unwrap();
        assert!(checked_output_field_v1(&root, 1, &ty, &physical, &mut budget).is_err());
        assert_eq!(budget.storage(), 29);
    }
    for physical in [
        Type::Scalar(ScalarType::U32),
        Type::Slice(SliceType::new(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Workgroup,
            KirAccess::ReadWrite,
        )),
        Type::Slice(SliceType::new(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            KirAccess::ReadOnly,
        )),
        Type::Slice(SliceType::new(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            KirAccess::ReadWrite,
        )),
    ] {
        assert!(checked_output_field_v1(&original, 1, &ty, &physical, &mut budget).is_err());
        assert_eq!(budget.storage(), 29);
    }
}

#[test]
fn output_lookup_preserves_write_only_access() {
    let (mut root, ty, read_write) = output_fields();
    let mut arguments = root.arguments.as_slice().to_vec();
    arguments[1].access = AccessMode::WriteOnly;
    root.arguments = TypedArgumentListV1::new(arguments).unwrap();
    let write_only = Type::Slice(SliceType::new(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        KirAccess::WriteOnly,
    ));
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 0);
    assert_eq!(
        checked_output_field_v1(&root, 1, &ty, &write_only, &mut budget).unwrap(),
        1,
    );
    assert!(matches!(
        checked_output_field_v1(&root, 1, &ty, &read_write, &mut budget),
        Err(Error::OutputArgument),
    ));
}

#[test]
fn new_scans_use_inherited_work_and_preserve_the_exact_storage_floor() {
    let semantic = source();
    let roots = typed_roots(&semantic);
    let (output, ty, physical) = output_fields();
    let run = |limit| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 31);
        budget.reserve_storage(31).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let outcome = select_typed_root_v1(&roots, &semantic, semantic.roots()[1], &mut budget)
            .and_then(|_| checked_output_field_v1(&output, 1, &ty, &physical, &mut budget));
        assert_eq!(budget.storage(), 31);
        assert_eq!(budget.peak_storage(), 31);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (outcome, budget.work())
    };
    let (result, exact) = run(1_000_000);
    assert_eq!(result.unwrap(), 1);
    assert_eq!(run(exact).0.unwrap(), 1);
    for limit in [17, exact - 1] {
        assert!(matches!(
            run(limit).0,
            Err(Error::Resource(Resource::Work(_)))
        ));
    }
    assert_eq!(sum(&[usize::MAX, 1]), Err(Resource::Arithmetic));
    assert_eq!(product(usize::MAX, 2), Err(Resource::Arithmetic));
}
