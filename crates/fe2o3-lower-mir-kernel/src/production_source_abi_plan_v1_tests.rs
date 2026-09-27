//! Genuine lowerer owners from synthetic admitted MIR, not a rustc capture claim.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::SemanticAbiArgumentRoleV1;

fn plan_fixture_type_closure(
    types: Vec<SemanticTypeDeclV1>,
    roots: [SemanticTypeIdV1; 4],
) -> (Vec<SemanticTypeDeclV1>, [SemanticTypeIdV1; 4]) {
    let mut reached = vec![false; types.len()];
    let mut pending = roots.to_vec();
    while let Some(ty) = pending.pop() {
        let index = ty.index() as usize;
        if std::mem::replace(&mut reached[index], true) {
            continue;
        }
        match types[index].shape() {
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                pending.extend_from_slice(fields.fields());
            }
            SemanticTypeShapeV1::Array { element, .. } => pending.push(*element),
            SemanticTypeShapeV1::Unit | SemanticTypeShapeV1::Scalar(_) => {}
            _ => panic!("unexpected source ABI fixture type"),
        }
    }
    let mut remap = vec![None; types.len()];
    let mut next = 0;
    for (index, present) in reached.iter().enumerate() {
        if *present {
            remap[index] = Some(SemanticTypeIdV1::from_index(next));
            next += 1;
        }
    }
    let remap_type = |ty: SemanticTypeIdV1| remap[ty.index() as usize].unwrap();
    let retained = types
        .iter()
        .enumerate()
        .filter(|(index, _)| reached[*index])
        .map(|(_, ty)| {
            let fields = |value: &SemanticAggregateTypeV1| {
                SemanticAggregateTypeV1::new(
                    value.fields().iter().copied().map(remap_type).collect(),
                )
                .unwrap()
            };
            let shape = match ty.shape() {
                SemanticTypeShapeV1::Tuple(value) => SemanticTypeShapeV1::Tuple(fields(value)),
                SemanticTypeShapeV1::Aggregate(value) => {
                    SemanticTypeShapeV1::Aggregate(fields(value))
                }
                SemanticTypeShapeV1::Array { element, length } => SemanticTypeShapeV1::Array {
                    element: remap_type(*element),
                    length: *length,
                },
                value => value.clone(),
            };
            SemanticTypeDeclV1::new(
                ty.identity(),
                ty.layout_identity(),
                ty.layout().clone(),
                shape,
            )
            .with_rustc_abi_properties(ty.abi_properties())
        })
        .collect();
    (retained, roots.map(remap_type))
}

fn plan_owner(shape: u8) -> ProductionPreRankedKirOwnerV1 {
    let original = argument_owner_shape(false, ArgumentTupleShape::Mixed, false, true);
    let semantic = original.source_semantic();
    let old = &semantic.functions()[1];
    let mut types = semantic.types().to_vec();
    if shape == 1 {
        let old = &types[PAIR.index() as usize];
        let SemanticTypeShapeV1::Tuple(fields) = old.shape() else {
            panic!("tuple fixture");
        };
        types[PAIR.index() as usize] = SemanticTypeDeclV1::new(
            old.identity(),
            old.layout_identity(),
            old.layout().clone(),
            SemanticTypeShapeV1::Aggregate(fields.clone()),
        )
        .with_rustc_abi_properties(old.abi_properties());
    }
    let array = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            12,
            4,
            SemanticFieldsShapeV1::array(4, 3),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            12,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: U32,
            length: 3,
        },
    ));
    let selected = match shape {
        0 | 1 => PAIR,
        2 => array,
        3 => ZERO,
        4 => TUPLE,
        _ => unreachable!(),
    };
    // The helper fixture has additional types, but this request has one root.
    // Keep its exact closure, preserving every source layout and ABI property.
    let (types, [selected, scalar, zero, unit]) =
        plan_fixture_type_closure(types, [selected, U32, ZERO, UNIT]);
    assert_eq!((scalar, unit), (U32, UNIT));
    let arguments = [selected, scalar, zero];
    let SemanticAbiPassModeV1::Direct(attributes) = old.abi().fixed_arguments()[0].mode() else {
        panic!("known-valid initialized scalar ABI");
    };
    let attributes = *attributes;
    let value = |ty: SemanticTypeIdV1| {
        let declaration = &types[ty.index() as usize];
        SemanticAbiValueV1::new(
            ty,
            if declaration.layout().size_bytes() == Some(0) {
                SemanticAbiPassModeV1::Ignore
            } else if matches!(
                declaration.layout().backend_repr(),
                SemanticBackendReprV1::ScalarPair { .. }
            ) {
                SemanticAbiPassModeV1::Pair {
                    first: attributes,
                    second: attributes,
                }
            } else if matches!(
                declaration.layout().backend_repr(),
                SemanticBackendReprV1::Memory { sized: true }
            ) {
                SemanticAbiPassModeV1::Indirect {
                    attributes: SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(
                            true,
                            Some(SemanticAbiPointerCaptureV1::CapturesNone),
                            true,
                            false,
                            false,
                            true,
                        ),
                        SemanticAbiExtensionV1::None,
                        declaration.layout().rustc_size_bytes(),
                        Some(declaration.layout().alignment_bytes()),
                    )
                    .unwrap(),
                    metadata_attributes: None,
                    on_stack: false,
                }
            } else {
                SemanticAbiPassModeV1::Direct(attributes)
            },
        )
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        old.abi().identity(),
        old.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        3,
        arguments
            .iter()
            .copied()
            .map(|ty| SemanticAbiArgumentV1::source(value(ty)))
            .collect(),
        value(UNIT),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 3])
    .unwrap();
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut locals = vec![SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([210; 32]),
        UNIT,
        SemanticLocalRoleV1::Return,
        source,
    )];
    for (index, ty) in arguments.into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([211 + index as u8; 32]),
            ty,
            SemanticLocalRoleV1::Argument(index as u32),
            source,
        ));
    }
    let function = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([220; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(old.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    materialize_argument_view(
        ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap(),
    )
}

const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

#[test]
fn source_abi_plan_fixture_preserves_complete_source_and_adjusted_abi_contract() {
    for (shape, type_count) in [(0, 7), (1, 7), (2, 7), (3, 6), (4, 8)] {
        let owner = plan_owner(shape);
        let semantic = owner.semantic_ssa.source_semantic();
        assert_eq!(semantic.roots(), &[ROOT]);
        assert_eq!(
            semantic.callables(),
            &[SemanticCallableDeclV1::defined(ROOT)]
        );
        assert_eq!(semantic.types().len(), type_count);
        let function = &semantic.functions()[0];
        let abi = function.abi();
        assert_eq!(function.role(), SemanticFunctionRoleV1::KernelRoot);
        assert!(function.kernel_entry().is_some());
        assert_eq!(abi.fixed_count(), 3);
        assert_eq!(abi.source_input_types().len(), 3);
        assert_eq!(abi.adjusted_arguments().len(), 3);
        assert_eq!(
            abi.source_argument_ownership(),
            &[SemanticSourceArgumentOwnershipV1::ByValue; 3]
        );
        assert!(abi.hidden_arguments().is_empty());
        assert!(!abi.can_unwind() && !abi.c_variadic());
        assert_eq!(abi.source_output_type(), UNIT);
        assert!(matches!(
            abi.return_value().mode(),
            SemanticAbiPassModeV1::Ignore
        ));
        assert!(matches!(
            abi.arguments()[2].mode(),
            SemanticAbiPassModeV1::Ignore
        ));
        for (index, argument) in abi.adjusted_arguments().iter().enumerate() {
            assert_eq!(argument.role(), SemanticAbiArgumentRoleV1::Source);
            assert_eq!(argument.ty(), abi.source_input_types()[index]);
            assert_eq!(function.locals()[index + 1].ty(), argument.ty());
            assert_eq!(
                function.locals()[index + 1].role(),
                SemanticLocalRoleV1::Argument(index as u32)
            );
        }
        let SemanticAbiPassModeV1::Direct(scalar) = abi.arguments()[1].mode() else {
            panic!("scalar companion must remain direct");
        };
        assert!(scalar.regular().no_undef());
        match (shape, abi.arguments()[0].mode()) {
            (0 | 1, SemanticAbiPassModeV1::Pair { first, second }) => {
                assert_eq!((first, second), (scalar, scalar));
            }
            (
                2 | 4,
                SemanticAbiPassModeV1::Indirect {
                    on_stack: false, ..
                },
            ) => {}
            (3, SemanticAbiPassModeV1::Ignore) => {}
            _ => panic!("wrong adjusted aggregate ABI"),
        }
    }
}

#[test]
fn source_abi_plan_replays_tuple_struct_array_nested_and_explicit_zero_coverage() {
    for (shape, expected) in [
        (0, vec![0, 4]),
        (1, vec![0, 4]),
        (2, vec![0, 4, 8]),
        (3, vec![]),
        (4, vec![0, 4, 8]),
    ] {
        let owner = plan_owner(shape);
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        owner
            .with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
                plan.check_subject(
                    owner.semantic_ssa.source_semantic(),
                    owner.executable.module(),
                )?;
                assert_eq!(plan.counts()?, (3, expected.len() + 1));
                assert_eq!(plan.argument(0)?.physical_slots(), 0..expected.len());
                assert_eq!(
                    plan.argument(1)?.physical_slots(),
                    expected.len()..expected.len() + 1
                );
                assert_eq!(
                    plan.argument(2)?.physical_slots(),
                    expected.len() + 1..expected.len() + 1
                );
                assert_eq!(plan.argument_type(2)?.layout().size_bytes(), Some(0));
                assert!(!plan.grants_authority());
                for (slot, offset) in expected.iter().enumerate() {
                    let row = plan.component(slot)?;
                    assert_eq!(row.source_argument(), 0);
                    assert_eq!(row.source_offset_bytes(), *offset);
                    assert_eq!(row.physical().slot(), slot);
                    assert_eq!(row.physical().ty(), &Type::Scalar(ScalarType::U32));
                }
                assert_eq!(plan.component(expected.len())?.source_offset_bytes(), 0);
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn source_abi_plan_foreign_same_shaped_owner_is_sticky_before_query_debit() {
    let owner = plan_owner(1);
    let foreign = plan_owner(1);
    for foreign_semantic in [true, false] {
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let error = owner
            .with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
                let stopped = (plan.budget.work(), plan.budget.storage());
                let semantic = if foreign_semantic {
                    foreign.semantic_ssa.source_semantic()
                } else {
                    owner.semantic_ssa.source_semantic()
                };
                let module = if foreign_semantic {
                    owner.executable.module()
                } else {
                    foreign.executable.module()
                };
                assert!(plan.check_subject(semantic, module).is_err());
                assert!(plan.counts().is_err());
                assert_eq!((plan.budget.work(), plan.budget.storage()), stopped);
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::CorrespondenceMismatch
        ));
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn source_abi_plan_rejects_wrong_root_function_and_same_typed_field_transports() {
    for mutation in 0..6 {
        let mut owner = plan_owner(1);
        match mutation {
            0 => {}
            1 => {
                owner.correspondence.parameter_component_bindings[0].semantic_component_type = UNIT
            }
            2 => {
                owner.correspondence.parameter_component_bindings[0].projection[0] =
                    SemanticKirParameterProjectionV1::Field(2)
            }
            3 => {
                let first = owner.correspondence.parameter_component_bindings[0].kernel_ir_value;
                owner.correspondence.parameter_component_bindings[0].kernel_ir_value =
                    owner.correspondence.parameter_component_bindings[1].kernel_ir_value;
                owner.correspondence.parameter_component_bindings[1].kernel_ir_value = first;
            }
            4 => owner.correspondence.ignored_parameter_bindings[0].semantic_type = UNIT,
            5 => {}
            _ => unreachable!(),
        }
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let root = if mutation == 0 {
            SemanticFunctionIdV1::from_index(1)
        } else {
            ROOT
        };
        let function = if mutation == 5 {
            SemanticFunctionIdV1::from_index(1)
        } else {
            ROOT
        };
        assert!(
            owner
                .with_checked_source_abi_plan_v1(
                    root,
                    function,
                    &mut budget,
                    |_| -> Result<(), ProductionSemanticKirErrorV1> {
                        panic!("a substituted source map reached the plan")
                    }
                )
                .is_err()
        );
        assert_eq!(budget.storage(), 0);
    }
}

fn measured_plan(
    owner: &ProductionPreRankedKirOwnerV1,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result = owner.with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
        assert_eq!(plan.counts()?, (3, 4));
        for slot in 0..4 {
            let _ = plan.component(slot)?;
        }
        Ok(())
    });
    assert_eq!(budget.storage(), 0);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn source_abi_plan_whole_work_and_storage_exact_and_one_short() {
    let owner = plan_owner(4);
    let (full, work, peak) = measured_plan(&owner, usize::MAX, usize::MAX);
    full.unwrap();
    measured_plan(&owner, work, peak).0.unwrap();
    assert!(matches!(
        measured_plan(&owner, work - 1, usize::MAX).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        measured_plan(&owner, usize::MAX, peak - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

#[test]
fn source_abi_plan_swallowed_resource_remains_first_even_with_later_callback_error() {
    for selected_error in [false, true] {
        let owner = plan_owner(1);
        let mut work = Work::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let result = owner.with_checked_source_abi_plan_v1(ROOT, ROOT, &mut budget, |plan| {
            assert!(plan.charge_work(1_000_001).is_err());
            let stopped = plan.budget.work();
            assert!(plan.counts().is_err());
            assert_eq!(plan.budget.work(), stopped);
            if selected_error {
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            )
        ));
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn source_abi_plan_header_equation_and_overflow_are_independent() {
    use std::mem::{align_of, size_of};
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, source_arguments_v1::ProductionSourceArgumentErrorV1>>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + size_of::<Result<T, ArgumentResourceV1>>()
    }
    let expected = 17
        + 2 * 8
        + size_of::<SourceAbiConstructorRefsV1<'_, '_, '_>>()
        + 2 * align_of::<SourceAbiConstructorRefsV1<'_, '_, '_>>()
        + size_of::<ProductionSourceAbiPlanV1<'_, '_, '_>>()
        + h::<ProductionSourceAbiArgumentV1>()
        + h::<Option<ProductionSourceAbiArgumentV1>>()
        + h::<ProductionSourceAbiComponentV1<'_>>()
        + h::<Option<ProductionSourceAbiComponentV1<'_>>>()
        + h::<Option<&ProductionSourceAbiArgumentV1>>()
        + h::<Option<&ProductionSourceAbiComponentV1<'_>>>()
        + size_of::<std::ops::Range<usize>>()
        + h::<source_arguments_v1::KernelParameterShapeV1>()
        + size_of::<&source_arguments_v1::KernelParameterShapeV1>()
        + size_of::<&Vec<source_arguments_v1::ByValueKernelParameterComponentV1>>()
        + size_of::<std::slice::Iter<'_, source_arguments_v1::ByValueKernelParameterComponentV1>>()
        + size_of::<&source_arguments_v1::ByValueKernelParameterComponentV1>()
        + h::<Option<&source_arguments_v1::ByValueKernelParameterComponentV1>>()
        + h::<ProductionPhysicalArgumentV1<'_>>()
        + h::<Option<ProductionPhysicalArgumentV1<'_>>>()
        + h::<ProductionArgumentTraceV1<'_>>()
        + size_of::<&SemanticKirParameterBindingV1>()
        + size_of::<&SemanticKirParameterComponentBindingV1>()
        + size_of::<&Vec<SemanticKirParameterProjectionV1>>()
        + size_of::<&[SemanticKirParameterProjectionV1]>()
        + size_of::<&SemanticTypeIdV1>()
        + size_of::<&Type>()
        + size_of::<&u64>()
        + h::<Option<&SemanticKirIgnoredParameterBindingV1>>()
        + size_of::<&SemanticKirIgnoredParameterBindingV1>()
        + h::<&SemanticKirFunctionCorrespondenceV1>()
        + h::<&SemanticFunctionDeclV1>()
        + h::<Option<&SemanticFunctionDeclV1>>()
        + h::<&fe2o3_kernel_ir::Function>()
        + h::<Option<&fe2o3_kernel_ir::Function>>()
        + size_of::<&[SemanticFunctionDeclV1]>()
        + size_of::<&[SemanticTypeDeclV1]>()
        + size_of::<&[ProductionSourceAbiArgumentV1]>()
        + size_of::<&[ProductionSourceAbiComponentV1<'_>]>()
        + h::<&SemanticTypeDeclV1>()
        + h::<Option<&SemanticTypeDeclV1>>()
        + h::<(usize, usize)>()
        + h::<()>()
        + h::<()>()
        + h::<usize>()
        + h::<Option<usize>>()
        + h::<Vec<ProductionSourceAbiArgumentV1>>()
        + h::<Vec<ProductionSourceAbiComponentV1<'_>>>()
        + h::<SourceAbiIteratorFrameV1<'_>>()
        + 2 * align_of::<SourceAbiIteratorFrameV1<'_>>()
        + h::<fe2o3_mir_model::SemanticSourceArgumentV1<'_>>()
        + h::<Option<fe2o3_mir_model::SemanticSourceArgumentV1<'_>>>()
        + size_of::<fe2o3_mir_model::SemanticSourceArgumentBindingV1<'_>>()
        + 12 * size_of::<usize>()
        + 4 * size_of::<u64>()
        + size_of::<&mut ProductionSourceAbiPlanV1<'_, '_, '_>>()
        + size_of::<&mut ArgumentBudgetV1<'_>>()
        + size_of::<SourceAbiPlanFailureV1>()
        + 2 * size_of::<Option<SourceAbiPlanFailureV1>>();
    assert_eq!(source_abi_plan_headers_v1::<()>(17, 8).unwrap(), expected);
    assert_eq!(
        source_abi_plan_headers_v1::<()>(usize::MAX, 8),
        Err(ArgumentResourceV1::Arithmetic)
    );
    assert_eq!(
        source_abi_plan_headers_v1::<()>(17, usize::MAX),
        Err(ArgumentResourceV1::Arithmetic)
    );
    let wrapper = 17
        + 2 * 8
        + size_of::<Result<(), ProductionSemanticKirErrorV1>>()
        + size_of::<Result<(), ArgumentResourceV1>>()
        + size_of::<Result<usize, ArgumentResourceV1>>()
        + size_of::<Option<usize>>()
        + size_of::<&mut ArgumentBudgetV1<'_>>()
        + 2 * size_of::<SemanticFunctionIdV1>()
        + 2 * size_of::<usize>();
    assert_eq!(
        source_abi_plan_wrapper_headers_v1::<()>(17, 8).unwrap(),
        wrapper
    );
    assert_eq!(
        source_abi_plan_wrapper_headers_v1::<()>(usize::MAX, 8),
        Err(ArgumentResourceV1::Arithmetic)
    );
    assert_eq!(
        source_abi_plan_wrapper_headers_v1::<()>(17, usize::MAX),
        Err(ArgumentResourceV1::Arithmetic)
    );
}
