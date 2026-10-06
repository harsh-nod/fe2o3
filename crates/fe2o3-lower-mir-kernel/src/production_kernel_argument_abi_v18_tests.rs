use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

mod inline_v29 {
    include!("production_kernel_inline_argument_abi_v29_tests.rs");
}

mod inline_resources_v29 {
    include!("production_kernel_inline_argument_abi_resources_v29_tests.rs");
}

const FLOOR: usize = 47 + size_of::<CapturedKernelArgumentAbiV18>();
const LIMIT: usize = 8_000_000;

// Test inputs model a complete original compiler descriptor. They are submitted
// to the production capture checks; this helper cannot manufacture a profile.
pub(in super::super) struct FixtureKernelAbiV18 {
    bindings: Vec<[u8; 32]>,
    exports: Vec<String>,
    arguments: Vec<Vec<ProductionKernelArgumentAbiArgumentV18>>,
    extents: Vec<(u32, u32)>,
}

impl FixtureKernelAbiV18 {
    pub(in super::super) fn new(owner: &ProductionSemanticSsaOwnerV1) -> Self {
        let semantic = owner.source_semantic();
        let mut input = Self {
            bindings: vec![],
            exports: vec![],
            arguments: vec![],
            extents: vec![],
        };
        for root in semantic.roots() {
            let function = &semantic.functions()[root.index() as usize];
            let entry = function.kernel_entry().unwrap();
            input
                .bindings
                .push(*entry.kernel_binding_identity().as_bytes());
            input.exports.push(
                std::str::from_utf8(entry.export_symbol().as_bytes())
                    .unwrap()
                    .to_owned(),
            );
            let mut arguments = Vec::new();
            let mut offset = 0u64;
            let mut alignment = 1u64;
            for (ordinal, &ty) in function.abi().source_input_types().iter().enumerate() {
                let declaration = &semantic.types()[ty.index() as usize];
                let layout = declaration.layout();
                let align = layout.alignment_bytes();
                assert!(align.is_power_of_two());
                offset = offset.checked_add(align - 1).unwrap() & !(align - 1);
                alignment = alignment.max(align);
                let physical_offset = u32::try_from(offset).unwrap();
                let kind = if shared_slice_leaf_v1(semantic.types(), ty) {
                    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
                        unreachable!()
                    };
                    let SemanticTypeShapeV1::Slice { element } =
                        semantic.types()[pointer.pointee().index() as usize].shape()
                    else {
                        unreachable!()
                    };
                    let element = fixture_descriptor_scalar_v18(
                        semantic.types()[element.index() as usize].shape(),
                    )
                    .unwrap();
                    let name = ValidName::new(format!("arg{ordinal}")).unwrap();
                    let source =
                        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(element));
                    let layout =
                        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(element));
                    ProductionKernelArgumentAbiKindV18::Descriptor {
                        source: SourceTypeDescriptorV3::SharedSlice(element),
                        argument: LogicalArgumentV1::shared_slice(
                            u16::try_from(ordinal).unwrap(),
                            name,
                            &source,
                            &layout,
                            physical_offset,
                        )
                        .unwrap(),
                    }
                } else if let Some(scalar) = fixture_descriptor_scalar_v18(declaration.shape()) {
                    let name = ValidName::new(format!("arg{ordinal}")).unwrap();
                    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(scalar));
                    let layout =
                        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(scalar));
                    ProductionKernelArgumentAbiKindV18::Descriptor {
                        source: SourceTypeDescriptorV3::Scalar(scalar),
                        argument: LogicalArgumentV1::scalar(
                            u16::try_from(ordinal).unwrap(),
                            name,
                            &source,
                            &layout,
                            physical_offset,
                        )
                        .unwrap(),
                    }
                } else {
                    ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue {
                        offset: physical_offset,
                    }
                };
                arguments.push(ProductionKernelArgumentAbiArgumentV18 {
                    semantic_type_identity: declaration.identity(),
                    kind,
                });
                offset = offset.checked_add(layout.size_bytes().unwrap()).unwrap();
            }
            input.arguments.push(arguments);
            input.extents.push((
                u32::try_from(offset).unwrap(),
                u32::try_from(alignment).unwrap(),
            ));
        }
        input
    }

    pub(in super::super) fn arguments_mut(
        &mut self,
        root: usize,
    ) -> &mut Vec<ProductionKernelArgumentAbiArgumentV18> {
        &mut self.arguments[root]
    }

    pub(in super::super) fn roots(&self) -> Vec<ProductionKernelArgumentAbiRootV18<'_>> {
        self.bindings
            .iter()
            .zip(&self.exports)
            .zip(&self.arguments)
            .zip(&self.extents)
            .map(|(((binding, export), arguments), &(extent, alignment))| {
                ProductionKernelArgumentAbiRootV18 {
                    kernel_binding: binding,
                    export,
                    arguments,
                    explicit_argument_bytes: extent,
                    kernarg_alignment_bytes: alignment,
                }
            })
            .collect()
    }
}

fn fixture_descriptor_scalar_v18(shape: &SemanticTypeShapeV1) -> Option<DescriptorScalar> {
    match shape {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) => {
            match (*signed, *bits) {
                (false, 8) => Some(DescriptorScalar::U8),
                (true, 8) => Some(DescriptorScalar::I8),
                (false, 16) => Some(DescriptorScalar::U16),
                (true, 16) => Some(DescriptorScalar::I16),
                (false, 32) => Some(DescriptorScalar::U32),
                (true, 32) => Some(DescriptorScalar::I32),
                (false, 64) => Some(DescriptorScalar::U64),
                (true, 64) => Some(DescriptorScalar::I64),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(in super::super) fn fixture_descriptor_ownership_v18(
    original: ProductionSemanticSsaOwnerV1,
) -> ProductionSemanticSsaOwnerV1 {
    let semantic = original.source_semantic();
    let functions = semantic
        .functions()
        .iter()
        .map(|function| {
            if function.role() != SemanticFunctionRoleV1::KernelRoot {
                return function.clone();
            }
            let ownership = function
                .abi()
                .source_input_types()
                .iter()
                .map(|&ty| {
                    if shared_slice_leaf_v1(semantic.types(), ty) {
                        SemanticSourceArgumentOwnershipV1::SharedBorrow
                    } else {
                        SemanticSourceArgumentOwnershipV1::ByValue
                    }
                })
                .collect();
            SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                function
                    .abi()
                    .clone()
                    .with_source_argument_ownership(ownership)
                    .unwrap(),
                function.locals().to_vec(),
                function.entry(),
                function.blocks().to_vec(),
            )
            .unwrap()
            .with_kernel_entry(function.kernel_entry().unwrap().clone())
        })
        .collect();
    let admitted = fe2o3_mir_model::semantic_mir_v1::InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(fe2o3_mir_model::semantic_mir_v1::SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn owner() -> ProductionSemanticSsaOwnerV1 {
    super::super::source_storage_demands_v29_tests::owner(2, false, false, false)
}

fn bindings(owner: &ProductionSemanticSsaOwnerV1) -> Vec<[u8; 32]> {
    let source = owner.source_semantic();
    source
        .roots()
        .iter()
        .map(|root| {
            *source.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap()
                .kernel_binding_identity()
                .as_bytes()
        })
        .collect()
}

fn roots<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    bindings: &'a [[u8; 32]],
) -> Vec<ProductionKernelArgumentAbiRootV18<'a>> {
    let source = owner.source_semantic();
    assert_eq!(source.roots().len(), bindings.len());
    source
        .roots()
        .iter()
        .zip(bindings)
        .map(|(root, binding)| {
            let function = &source.functions()[root.index() as usize];
            assert!(function.abi().source_input_types().is_empty());
            let entry = function.kernel_entry().unwrap();
            ProductionKernelArgumentAbiRootV18 {
                kernel_binding: binding,
                export: std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                arguments: &[],
                explicit_argument_bytes: 0,
                kernarg_alignment_bytes: 8,
            }
        })
        .collect()
}

#[test]
fn complete_original_kernel_abi_census_preserves_source_profile_and_exact_root_order() {
    let owner = owner();
    let bindings = bindings(&owner);
    let roots = roots(&owner, &bindings);
    let original = *owner.source_semantic_sha256();
    let version = owner.source_semantic().wire_version();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let floor = FLOOR;
    budget.reserve_storage(floor).unwrap();
    let profile = CapturedKernelArgumentAbiV18::capture(
        &owner,
        ProductionKernelArgumentAbiInputV18 { roots: &roots },
        &mut budget,
    )
    .unwrap();
    assert_eq!(profile.source, original);
    assert_eq!(owner.source_semantic().wire_version(), version);
    assert_eq!(
        profile
            .roots
            .iter()
            .map(|row| row.function)
            .collect::<Vec<_>>(),
        owner.source_semantic().roots()
    );
    assert_eq!(profile.retained_storage(), roots.len() * size_of::<Root>());
    assert_eq!(budget.storage(), floor + profile.retained_storage());
    profile
        .matches_original_input(
            &owner,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &mut budget,
        )
        .unwrap();
    for ordinal in 0..roots.len() {
        assert_eq!(profile.argument_count(ordinal).unwrap(), 0);
    }
    let credit = profile.retained_storage();
    drop(profile);
    budget.release_storage(credit).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn original_abi_root_omission_reordering_and_foreign_profile_are_not_empty_contracts() {
    let owner = owner();
    let foreign = [244; 32];
    let bindings = bindings(&owner);
    let mut roots = roots(&owner, &bindings);
    for fault in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        if fault == 1 {
            roots.swap(0, 1);
        }
        let original = roots[0].kernel_binding;
        if fault == 2 {
            roots[0].kernel_binding = &foreign;
        }
        let extent = roots[0].kernarg_alignment_bytes;
        if fault == 3 {
            roots[0].kernarg_alignment_bytes = 3;
        }
        let result = CapturedKernelArgumentAbiV18::capture(
            &owner,
            ProductionKernelArgumentAbiInputV18 {
                roots: if fault == 0 { &roots[..1] } else { &roots },
            },
            &mut budget,
        );
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
        roots[0].kernel_binding = original;
        roots[0].kernarg_alignment_bytes = extent;
        if fault == 1 {
            roots.swap(0, 1);
        }
        budget.release_storage(budget.storage() - FLOOR).unwrap();
        let profile = CapturedKernelArgumentAbiV18::capture(
            &owner,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &mut budget,
        )
        .unwrap();
        let credit = profile.retained_storage();
        drop(profile);
        budget.release_storage(credit).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn retained_profile_replay_rejects_source_root_and_component_census_substitution() {
    let owner = owner();
    let bindings = bindings(&owner);
    let roots = roots(&owner, &bindings);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut profile = CapturedKernelArgumentAbiV18::capture(
        &owner,
        ProductionKernelArgumentAbiInputV18 { roots: &roots },
        &mut budget,
    )
    .unwrap();
    profile.check(&owner, &mut budget).unwrap();
    profile.source[0] ^= 1;
    assert!(matches!(
        profile.check(&owner, &mut budget),
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
    profile.source[0] ^= 1;
    profile.roots.swap(0, 1);
    assert!(matches!(
        profile.check(&owner, &mut budget),
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
    profile.roots.swap(0, 1);
    profile.roots[0].arguments.end = 1;
    assert!(matches!(
        profile.check(&owner, &mut budget),
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
    profile.roots[0].arguments.end = 0;
    profile.roots[0].explicit_argument_bytes = 8;
    assert!(matches!(
        profile.matches_original_input(
            &owner,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &mut budget
        ),
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
    profile.roots[0].explicit_argument_bytes = 0;
    profile
        .matches_original_input(
            &owner,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &mut budget,
        )
        .unwrap();
    let credit = profile.retained_storage();
    drop(profile);
    budget.release_storage(credit).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn kernel_abi_root_capture_has_independent_exact_and_one_short_work_and_storage() {
    let owner = owner();
    let bindings = bindings(&owner);
    let roots = roots(&owner, &bindings);
    // Count header, two prepaid vector constructions, and one root identity
    // comparison per original root. These zero-argument fixtures cannot hide
    // descriptor query allocations in the boundary oracle.
    let work_required = roots.len()
        + 4
        + 3
        + 3
        + roots
            .iter()
            .map(|root| root.export.len() + 40)
            .sum::<usize>();
    let storage_required = FLOOR + roots.len() * size_of::<Root>();
    for (work_limit, storage_limit, fault) in [
        (work_required, storage_required, 0),
        (work_required - 1, storage_required, 1),
        (work_required, storage_required - 1, 2),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        match CapturedKernelArgumentAbiV18::capture(
            &owner,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &mut budget,
        ) {
            Ok(profile) => {
                assert_eq!(fault, 0);
                assert_eq!(budget.work(), work_required);
                assert_eq!(budget.peak_storage(), storage_required);
                let credit = profile.retained_storage();
                drop(profile);
                budget.release_storage(credit).unwrap();
            }
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(_),
            )) => assert_eq!(fault, 1),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_),
            )) => assert_eq!(fault, 2),
            Err(other) => panic!("unexpected profile failure: {other:?}"),
        }
        budget.release_storage(budget.storage() - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
