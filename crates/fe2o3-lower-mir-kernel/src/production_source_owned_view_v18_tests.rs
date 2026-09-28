// These use the original admitted multi-root fixtures and actual source emitter.
include!("production_source_descriptor_propagation_v29_tests.rs");
include!("production_source_descriptor_resources_v29_tests.rs");
type EntranceError = ProductionSourceOwnedViewErrorV18;

#[test]
fn delegated_source_resource_refusal_is_sticky_without_poisoning_raw_budget() {
    for storage in [false, true] {
        for lose_custody in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
            let first = std::cell::Cell::new(None);
            let final_storage = std::cell::Cell::new(0);
            let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
                let before = (budget.work(), budget.storage());
                view.check_query_v18(budget)?;
                assert_eq!((budget.work(), budget.storage()), before);
                let error = if storage {
                    budget.reserve_storage(usize::MAX).unwrap_err()
                } else {
                    budget.charge_work(usize::MAX).unwrap_err()
                };
                first.set(Some(error));
                assert!(matches!(view.retain_query_resource_error_v18(error),
                    EntranceError::Resource(actual) if actual == error));
                budget.charge_work(1)?;
                budget.reserve_storage(1)?;
                budget.release_storage(1)?;
                let later = if storage {
                    budget.charge_work(usize::MAX).unwrap_err()
                } else {
                    budget.reserve_storage(usize::MAX).unwrap_err()
                };
                assert!(matches!(view.retain_query_resource_error_v18(later),
                    EntranceError::Resource(actual) if actual == error));
                let before = (budget.work(), budget.storage());
                assert!(matches!(view.root_count(budget),
                    Err(EntranceError::Resource(actual)) if actual == error));
                assert_eq!((budget.work(), budget.storage()), before);
                if lose_custody {
                    budget.release_storage(1)?;
                    final_storage.set(budget.storage());
                    assert!(matches!(view.check_query_v18(budget),
                        Err(EntranceError::Resource(actual)) if actual == error));
                    return Err(EntranceError::Binding("later consumer failure"));
                }
                Ok(())
            });
            assert!(matches!(result,
                Err(EntranceError::Resource(actual)) if Some(actual) == first.get()));
            assert_eq!(
                budget.storage(),
                if lose_custody {
                    final_storage.get()
                } else {
                    MODULE_FLOOR
                }
            );
        }
    }
}

#[test]
fn delegated_resource_refusal_cannot_replace_an_earlier_source_query_error() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
        assert!(matches!(
            view.root(usize::MAX, budget),
            Err(EntranceError::Binding("root ordinal"))
        ));
        let error = budget.charge_work(usize::MAX).unwrap_err();
        assert!(matches!(
            view.retain_query_resource_error_v18(error),
            EntranceError::Binding("root ordinal")
        ));
        budget.charge_work(1)?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(EntranceError::Binding("root ordinal"))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn unrelated_raw_refusals_inside_source_callback_do_not_poison_queries() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    prepared
        .with_checked_source_v18(&mut budget, |view, budget| {
            assert!(budget.charge_work(usize::MAX).is_err());
            assert!(budget.reserve_storage(usize::MAX).is_err());
            budget.charge_work(1)?;
            view.check_query_v18(budget)?;
            assert_eq!(view.root_count(budget)?, 2);
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

fn original_kernel_abi_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let original = module_fixture_owner(ModuleFixture::Ordinary);
    let semantic = original.source_semantic();
    let functions = semantic
        .functions()
        .iter()
        .map(|function| {
            assert_eq!(function.role(), SemanticFunctionRoleV1::KernelRoot);
            assert_eq!(function.abi().source_input_types(), &[U32]);
            assert_eq!(
                function.abi().source_argument_ownership(),
                &[SemanticSourceArgumentOwnershipV1::Unspecified]
            );
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
                    .with_source_argument_ownership(vec![
                        SemanticSourceArgumentOwnershipV1::ByValue,
                    ])
                    .unwrap(),
                function.locals().to_vec(),
                function.entry(),
                function.blocks().to_vec(),
            )
            .unwrap()
            .with_kernel_entry(function.kernel_entry().unwrap().clone())
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
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
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert_ne!(
        owner.source_semantic_sha256(),
        original.source_semantic_sha256()
    );
    owner
}

fn with_original_kernel_abi_input_v18<'work, R>(
    kind: ModuleFixture,
    preexisting: bool,
    budget: &mut ArgumentBudgetV1<'work>,
    use_input: impl FnOnce(
        ProductionSemanticSsaOwnerV1,
        ProductionSourceLaunchRosterV1,
        ProductionExecutionSourceInputV29<'_>,
        usize,
        &mut ArgumentBudgetV1<'work>,
    ) -> R,
) -> R {
    assert!(matches!(kind, ModuleFixture::Ordinary));
    with_pending_api_owner_v18(
        kind,
        preexisting,
        budget,
        original_kernel_abi_owner_v18,
        use_input,
    )
}

struct OriginalKernelAbiFixtureV18 {
    bindings: Vec<[u8; 32]>,
    exports: Vec<String>,
    arguments: Vec<Vec<ProductionKernelArgumentAbiArgumentV18>>,
}

impl OriginalKernelAbiFixtureV18 {
    fn ordinary(owner: &ProductionSemanticSsaOwnerV1) -> Self {
        use fe2o3_kernel_descriptor as descriptor;
        let semantic = owner.source_semantic();
        let mut fixture = Self {
            bindings: vec![],
            exports: vec![],
            arguments: vec![],
        };
        for &root in semantic.roots() {
            let function = &semantic.functions()[root.index() as usize];
            let entry = function.kernel_entry().unwrap();
            fixture
                .bindings
                .push(*entry.kernel_binding_identity().as_bytes());
            fixture.exports.push(
                std::str::from_utf8(entry.export_symbol().as_bytes())
                    .unwrap()
                    .to_owned(),
            );
            assert_eq!(function.abi().source_input_types(), &[U32]);
            let scalar = descriptor::ScalarTypeV1::U32;
            fixture
                .arguments
                .push(vec![ProductionKernelArgumentAbiArgumentV18 {
                    semantic_type_identity: semantic.types()[U32.index() as usize].identity(),
                    kind: ProductionKernelArgumentAbiKindV18::Descriptor {
                        source: descriptor::SourceTypeDescriptorV3::Scalar(scalar),
                        argument: descriptor::LogicalArgumentV1::scalar(
                            0,
                            descriptor::ValidName::new("seed".to_owned()).unwrap(),
                            &descriptor::SourceTypeRecordV1::new(
                                descriptor::SourceTypeDescriptorV1::scalar(scalar),
                            ),
                            &descriptor::DeviceLayoutRecordV1::new(
                                descriptor::DeviceLayoutDescriptorV1::scalar(scalar),
                            ),
                            0,
                        )
                        .unwrap(),
                    },
                }]);
        }
        fixture
    }

    fn roots(&self) -> Vec<ProductionKernelArgumentAbiRootV18<'_>> {
        self.bindings
            .iter()
            .zip(&self.exports)
            .zip(&self.arguments)
            .map(
                |((binding, export), arguments)| ProductionKernelArgumentAbiRootV18 {
                    kernel_binding: binding,
                    export,
                    arguments,
                    explicit_argument_bytes: 4,
                    kernarg_alignment_bytes: 4,
                },
            )
            .collect()
    }
}

#[test]
fn kernel_argument_profile_does_not_infer_unspecified_source_ownership() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    with_pending_api_input(
        ModuleFixture::Ordinary,
        false,
        &mut budget,
        |owner, launch, source, _, budget| {
            let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
            let roots = fixture.roots();
            let floor = budget.storage();
            let result =
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    source,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                );
            assert!(matches!(
                result,
                Err(EntranceError::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            detail: "kernel argument ABI profile differs from the complete original descriptor/source contract",
                            ..
                        }
                    )
                ))
            ));
            assert_eq!(budget.storage(), floor);
        },
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn original_kernel_argument_profile_survives_prepared_capture_and_real_source_replay() {
    for preexisting in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture, original_sha) = with_original_kernel_abi_input_v18(
            ModuleFixture::Ordinary,
            preexisting,
            &mut budget,
            |owner, launch, source, _, budget| {
                let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
                let roots = fixture.roots();
                let sha = *owner.source_semantic_sha256();
                let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner, launch, source, ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(), budget,
                ).unwrap();
                drop(roots);
                assert_eq!(
                    prepared.source.input.retained_storage,
                    owned_input_payload(&prepared.source.input)
                );
                assert!(
                    prepared
                        .source
                        .input
                        .kernel_argument_abi
                        .as_ref()
                        .unwrap()
                        .retained_storage()
                        > 0
                );
                (prepared, fixture, sha)
            },
        );
        assert_eq!(budget.storage(), MODULE_FLOOR + prepared.adopted_storage());
        let called = std::cell::Cell::new(false);
        prepared
            .with_checked_source_v18(&mut budget, |view, budget| {
                called.set(true);
                assert_eq!(
                    view.source_ssa(budget)?.source_semantic_sha256(),
                    &original_sha
                );
                let roots = fixture.roots();
                view.require_kernel_argument_abi_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                for root in 0..2 {
                    assert_eq!(view.kernel_argument_abi_count(root, budget)?, Some(1));
                    assert_eq!(
                        view.kernel_argument_abi_kind(root, 0, budget)?,
                        Some(fe2o3_kernel_descriptor::SourceTypeDescriptorV3::Scalar(
                            fe2o3_kernel_descriptor::ScalarTypeV1::U32
                        ))
                    );
                }
                Ok(())
            })
            .unwrap();
        assert!(
            called.get(),
            "the actual source replay callback must execute"
        );
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn kernel_argument_profile_omission_and_wrong_extent_fail_before_source_emission() {
    for preexisting in [false, true] {
        for fault in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            with_original_kernel_abi_input_v18(
                ModuleFixture::Ordinary,
                preexisting,
                &mut budget,
                |owner, launch, source, capture, budget| {
                    let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
                    let mut roots = fixture.roots();
                    match fault {
                        0 => {
                            roots.pop();
                        }
                        1 => roots[0].explicit_argument_bytes = 3,
                        2 => roots[0].arguments = &[],
                        _ => unreachable!(),
                    }
                    let floor = budget.storage() - capture;
                    let result = ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                        owner, launch, source, ProductionKernelArgumentAbiInputV18 { roots: &roots },
                        ProductionSemanticKirLimitsV1::default(), budget,
                    );
                    assert!(matches!(
                        result,
                        Err(EntranceError::Source(
                            ProductionPendingScopedSourceErrorV29::Source(
                                ProductionSemanticKirErrorV1::Unsupported { .. }
                            )
                        ))
                    ));
                    assert_eq!(
                        budget.storage(),
                        floor,
                        "both fresh and adopted occurrence credit must settle"
                    );
                },
            );
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn caught_kernel_argument_profile_rejoin_failure_remains_the_original_query_error() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = with_original_kernel_abi_input_v18(
        ModuleFixture::Ordinary,
        false,
        &mut budget,
        |owner, launch, source, _, budget| {
            let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
            let roots = fixture.roots();
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                owner,
                launch,
                source,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap()
        },
    );
    let called = std::cell::Cell::new(false);
    let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
        called.set(true);
        assert_eq!(view.kernel_argument_abi_count(0, budget)?, Some(1));
        assert!(matches!(
            view.require_kernel_argument_abi_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &[] },
                budget
            ),
            Err(EntranceError::Binding(_))
        ));
        let work = budget.work();
        assert!(matches!(
            view.kernel_argument_abi_count(0, budget),
            Err(EntranceError::Binding(_))
        ));
        assert_eq!(
            budget.work(),
            work,
            "caught profile failure cannot restore query authority"
        );
        Ok(())
    });
    assert!(called.get());
    assert!(matches!(result, Err(EntranceError::Binding(_))));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn original_as0_shared_slice_root_profile_requires_exact_component_and_type_census() {
    use fe2o3_kernel_descriptor as descriptor;
    let owner = descriptor_source_owner(DescriptorCase::READ);
    let semantic = owner.source_semantic();
    let function = &semantic.functions()[0];
    let entry = function.kernel_entry().unwrap();
    assert_eq!(function.abi().source_input_types().len(), 3);
    for fault in 0..6 {
        let mut arguments = Vec::new();
        for (ordinal, ty) in function.abi().source_input_types().iter().enumerate() {
            let scalar = if ordinal < 2 {
                descriptor::ScalarTypeV1::U32
            } else {
                descriptor::ScalarTypeV1::U64
            };
            let source = if ordinal < 2 {
                descriptor::SourceTypeDescriptorV1::shared_slice(scalar)
            } else {
                descriptor::SourceTypeDescriptorV1::scalar(scalar)
            };
            let layout = if ordinal < 2 {
                descriptor::DeviceLayoutDescriptorV1::shared_slice(scalar)
            } else {
                descriptor::DeviceLayoutDescriptorV1::scalar(scalar)
            };
            let source = descriptor::SourceTypeRecordV1::new(source);
            let layout = descriptor::DeviceLayoutRecordV1::new(layout);
            let name = descriptor::ValidName::new(format!("arg{ordinal}")).unwrap();
            let offset = if fault == 4 && ordinal == 1 {
                0
            } else {
                ordinal as u32 * 16
            };
            let argument = if ordinal < 2 {
                descriptor::LogicalArgumentV1::shared_slice(
                    ordinal as u16,
                    name,
                    &source,
                    &layout,
                    offset,
                )
                .unwrap()
            } else {
                descriptor::LogicalArgumentV1::scalar(
                    ordinal as u16,
                    name,
                    &source,
                    &layout,
                    offset,
                )
                .unwrap()
            };
            arguments.push(ProductionKernelArgumentAbiArgumentV18 {
                semantic_type_identity: semantic.types()[ty.index() as usize].identity(),
                kind: ProductionKernelArgumentAbiKindV18::Descriptor {
                    source: if ordinal < 2 {
                        descriptor::SourceTypeDescriptorV3::SharedSlice(scalar)
                    } else {
                        descriptor::SourceTypeDescriptorV3::Scalar(scalar)
                    },
                    argument,
                },
            });
        }
        match fault {
            1 => {
                arguments.pop();
            }
            2 => {
                arguments[0].semantic_type_identity =
                    semantic.types()[U32.index() as usize].identity()
            }
            3 => {
                let ProductionKernelArgumentAbiKindV18::Descriptor { source, .. } =
                    &mut arguments[0].kind
                else {
                    unreachable!()
                };
                *source =
                    descriptor::SourceTypeDescriptorV3::SharedSlice(descriptor::ScalarTypeV1::F32);
            }
            5 => {
                arguments[0].kind =
                    ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { offset: 0 }
            }
            _ => {}
        }
        let binding = entry.kernel_binding_identity();
        let roots = [ProductionKernelArgumentAbiRootV18 {
            kernel_binding: binding.as_bytes(),
            export: std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
            arguments: &arguments,
            explicit_argument_bytes: 40,
            kernarg_alignment_bytes: 8,
        }];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let floor =
            MODULE_FLOOR + size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>();
        budget.reserve_storage(floor).unwrap();
        let result = kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(
            &owner,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            &mut budget,
        );
        match result {
            Ok(profile) => {
                assert_eq!(fault, 0);
                profile
                    .matches_original_input(
                        &owner,
                        ProductionKernelArgumentAbiInputV18 { roots: &roots },
                        &mut budget,
                    )
                    .unwrap();
                assert_eq!(profile.argument_count(0).unwrap(), 3);
                assert_eq!(
                    profile.argument_kind(0, 0).unwrap(),
                    Some(descriptor::SourceTypeDescriptorV3::SharedSlice(
                        descriptor::ScalarTypeV1::U32
                    ))
                );
                for &ty in &function.abi().source_input_types()[..2] {
                    let SemanticTypeShapeV1::Pointer(pointer) =
                        semantic.types()[ty.index() as usize].shape()
                    else {
                        panic!("original reference shape")
                    };
                    assert_eq!(pointer.address_space(), 0);
                    let Type::Slice(slice) =
                        source_parameter_type_v18(semantic.types(), semantic.callables(), ty)
                            .unwrap()
                    else {
                        panic!("original descriptor representation")
                    };
                    assert_eq!(slice.address_space, AddressSpace::Generic);
                }
                drop(profile);
            }
            Err(ProductionSemanticKirErrorV1::Unsupported { .. }) => assert_ne!(fault, 0),
            Err(error) => panic!("unexpected source profile failure: {error:?}"),
        }
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn kernel_argument_profile_query_meter_denial_is_sticky_without_additional_work() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = with_original_kernel_abi_input_v18(
        ModuleFixture::Ordinary,
        false,
        &mut budget,
        |owner, launch, source, _, budget| {
            let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
            let roots = fixture.roots();
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                owner,
                launch,
                source,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap()
        },
    );
    let called = std::cell::Cell::new(false);
    let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
        called.set(true);
        assert_eq!(view.kernel_argument_abi_count(0, budget)?, Some(1));
        budget.charge_work(MODULE_LIMIT - budget.work())?;
        assert!(matches!(
            view.kernel_argument_abi_kind(0, 0, budget),
            Err(EntranceError::Resource(ArgumentResourceV1::Work(_)))
        ));
        let exhausted = budget.work();
        assert!(matches!(
            view.root_count(budget),
            Err(EntranceError::Resource(ArgumentResourceV1::Work(_)))
        ));
        assert_eq!(budget.work(), exhausted);
        Ok(())
    });
    assert!(called.get());
    assert!(matches!(
        result,
        Err(EntranceError::Resource(ArgumentResourceV1::Work(_)))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn retained_kernel_argument_profile_preserves_foreign_floor_and_unwind_custody() {
    for fault in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        foreign.reserve_storage(97).unwrap();
        let prepared = with_original_kernel_abi_input_v18(
            ModuleFixture::Ordinary,
            false,
            &mut budget,
            |owner, launch, source, _, budget| {
                let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
                let roots = fixture.roots();
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    source,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
                .unwrap()
            },
        );
        let entered = std::cell::Cell::new(None);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_checked_source_v18(&mut budget, |view, budget| {
                assert_eq!(view.kernel_argument_abi_count(0, budget)?, Some(1));
                entered.set(Some(budget.storage()));
                match fault {
                    0 => std::mem::swap(budget, &mut foreign),
                    1 | 2 => budget.release_storage(budget.storage())?,
                    3 => {
                        budget.release_storage(1)?;
                        assert!(matches!(
                            view.kernel_argument_abi_count(0, budget),
                            Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
                        ));
                        budget.reserve_storage(1)?;
                        let work = budget.work();
                        assert!(matches!(
                            view.kernel_argument_abi_count(0, budget),
                            Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
                        ));
                        assert_eq!(budget.work(), work);
                    }
                    _ => unreachable!(),
                }
                if fault == 2 {
                    std::panic::resume_unwind(Box::new(0x1226_1226_u64));
                }
                Err::<(), _>(EntranceError::Binding("original kernel ABI callback error"))
            })
        }));
        let entered = entered
            .get()
            .expect("actual profile-bearing source callback must run");
        match fault {
            0 => {
                assert!(matches!(
                    result.unwrap(),
                    Err(EntranceError::Binding("original kernel ABI callback error"))
                ));
                assert_eq!(budget.storage(), 97);
                assert_eq!(foreign.storage(), entered);
            }
            1 => {
                assert!(matches!(
                    result.unwrap(),
                    Err(EntranceError::Binding("original kernel ABI callback error"))
                ));
                assert_eq!(budget.storage(), 0);
            }
            2 => {
                assert_eq!(
                    *result.unwrap_err().downcast::<u64>().unwrap(),
                    0x1226_1226_u64
                );
                assert_eq!(budget.storage(), 0);
            }
            3 => {
                assert!(matches!(
                    result.unwrap(),
                    Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
                ));
                assert_eq!(
                    budget.storage(),
                    entered,
                    "restoring credit does not restore denied cleanup authority"
                );
            }
            _ => unreachable!(),
        }
    }
}

fn prepared_source_fixture(
    kind: ModuleFixture,
    preexisting: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> ProductionPreparedSourceV18 {
    with_pending_api_input(
        kind,
        preexisting,
        budget,
        |owner, launch, input, _, budget| {
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                owner,
                launch,
                input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap()
        },
    )
}

#[test]
fn source_owned_entrance_keeps_original_graph_table_and_instance_attachments() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        for preexisting in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = prepared_source_fixture(kind, preexisting, &mut budget);
            assert_eq!(budget.storage(), MODULE_FLOOR + prepared.adopted_storage());
            let digest = *prepared.source.owner.source_semantic_sha256();
            let profile = prepared.source.owner.source_semantic().wire_version();
            let called = std::cell::Cell::new(false);
            prepared
                .with_checked_source_v18(&mut budget, |view, budget| {
                    called.set(true);
                    let original = view.source_ssa(budget)?;
                    assert_eq!(original.source_semantic_sha256(), &digest);
                    assert!(original.occurrence_storage().is_some());
                    view.check_original_source(original, budget)?;
                    assert_eq!(view.source_semantic(budget)?.wire_version(), profile);
                    assert_eq!(view.source_launch(budget)?.semantic_sha256(), &digest);
                    let graph = view.canonical(budget)?;
                    assert!(std::ptr::eq(graph, &view.owner.inner.pending.graph));
                    assert!(!graph.canonical_bytes().is_empty());
                    assert_eq!(
                        graph.module().storage_layouts,
                        view.owner.pending_module().storage_layouts
                    );
                    if matches!(kind, ModuleFixture::Array) {
                        assert!(!graph.module().storage_layouts.is_empty());
                    }
                    let roots = view.root_count(budget)?;
                    assert_eq!(
                        roots,
                        if matches!(kind, ModuleFixture::Ordinary) {
                            2
                        } else {
                            3
                        }
                    );
                    let mut helpers = 0;
                    for root in 0..roots {
                        let (source, function) = view.root(root, budget)?;
                        assert_eq!(
                            source,
                            view.owner.inner.pending.roots[root].coordinates.root
                        );
                        let instances = view.instance_count(root, budget)?;
                        for instance in 0..instances {
                            let (_, incoming) = view.instance(root, instance, budget)?;
                            if instance == 0 {
                                assert!(incoming.is_none());
                            } else {
                                assert!(incoming.is_some());
                                helpers += 1;
                            }
                            let _ = view.invocation_entry(root, instance, budget)?;
                            for anchor in 0..view.memory_anchor_count(root, instance, budget)? {
                                let _ = view.memory_access(root, instance, anchor, budget)?;
                            }
                        }
                        let body = graph.module().functions[function].body.as_ref().unwrap();
                        for ordinal in 0..view.span_count(root, budget)? {
                            for segment in 0..2 {
                                if let Some((instance, block, first, count)) =
                                    view.span_segment(root, ordinal, segment, budget)?
                                {
                                    assert!(instance < instances);
                                    let block =
                                        body.blocks.iter().find(|row| row.id == block).unwrap();
                                    assert!(
                                        u64::from(first) + u64::from(count)
                                            <= block.operations.len() as u64
                                    );
                                }
                            }
                        }
                    }
                    if matches!(kind, ModuleFixture::Ordinary) {
                        assert_eq!(helpers, 0);
                    } else {
                        assert!(
                            helpers >= 2,
                            "original helper instances remain qualified by root"
                        );
                    }
                    assert_eq!(view.assertion_count(budget)?, 2);
                    for ordinal in 0..2 {
                        let _ = view.assertion(ordinal, budget)?;
                    }
                    budget.reserve_storage(37)?;
                    Ok(37)
                })
                .unwrap();
            assert!(called.get());
            assert_eq!(
                budget.storage(),
                MODULE_FLOOR + 37,
                "only consumer growth survives"
            );
            budget.release_storage(37).unwrap();
        }
    }
}

#[test]
fn source_owned_queries_reject_equal_source_reconstruction_and_keep_first_refusal() {
    for foreign_owner in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let substitute = module_fixture_owner(ModuleFixture::Ordinary);
        let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
        let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
            let error = if foreign_owner {
                assert_eq!(
                    substitute.source_semantic_sha256(),
                    view.source_ssa(budget)?.source_semantic_sha256()
                );
                view.check_original_source(&substitute, budget).unwrap_err()
            } else {
                view.root(usize::MAX, budget).unwrap_err()
            };
            let first = error.to_string();
            assert_eq!(view.canonical(budget).err().unwrap().to_string(), first);
            Err::<(), _>(EntranceError::Binding("later callback error"))
        });
        let expected = if foreign_owner {
            "foreign original SSA owner"
        } else {
            "root ordinal"
        };
        assert!(matches!(result, Err(EntranceError::Binding(detail)) if detail == expected));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_owned_query_work_denial_is_sticky_even_if_the_callback_ignores_it() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
        budget.charge_work(MODULE_LIMIT - budget.work())?;
        assert!(matches!(
            view.root_count(budget),
            Err(EntranceError::Resource(ArgumentResourceV1::Work(_)))
        ));
        Ok(())
    });
    assert!(matches!(
        result,
        Err(EntranceError::Resource(ArgumentResourceV1::Work(_)))
    ));
    assert_eq!(budget.storage(), 0);
    assert_eq!(work.failed_work(), Some(MODULE_LIMIT + 1));
}

#[test]
fn source_owned_scope_preserves_semantic_errors_and_raw_panics_after_floor_loss() {
    for undercut in [false, true] {
        for panic in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
            let held = std::cell::Cell::new(0);
            let payload = Box::new(0x1829_u64);
            let address = std::ptr::from_ref(payload.as_ref());
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                prepared.with_checked_source_v18(&mut budget, |_, budget| {
                    if undercut {
                        budget.release_storage(1)?;
                    }
                    held.set(budget.storage());
                    if panic {
                        std::panic::resume_unwind(payload);
                    }
                    Err::<(), _>(EntranceError::Binding("original callback error"))
                })
            }));
            if panic {
                let payload = result.unwrap_err().downcast::<u64>().unwrap();
                assert_eq!(std::ptr::from_ref(payload.as_ref()), address);
                assert_eq!(*payload, 0x1829);
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(EntranceError::Binding("original callback error"))
                ));
            }
            assert_eq!(
                budget.storage(),
                if undercut { held.get() } else { MODULE_FLOOR }
            );
        }
    }
}

#[test]
fn source_owned_scope_never_refunds_a_substituted_ledger() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    other.reserve_storage(97).unwrap();
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let original = budget.work_ledger_identity_v1();
    let retained = std::cell::Cell::new(0);
    let result = prepared.with_checked_source_v18(&mut budget, |_, budget| {
        retained.set(budget.storage());
        std::mem::swap(budget, &mut other);
        Err::<(), _>(EntranceError::Binding("selected error before postflight"))
    });
    assert!(matches!(
        result,
        Err(EntranceError::Binding("selected error before postflight"))
    ));
    assert_eq!(budget.storage(), 97);
    assert_eq!(other.storage(), retained.get());
    assert!(other.work_ledger_identity_v1() == original);
}

#[test]
fn prepared_source_rejects_under_reserved_occurrences_before_header_credit() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    with_pending_api_input(
        ModuleFixture::Ordinary,
        true,
        &mut budget,
        |owner, launch, input, capture, budget| {
            assert!(capture > 0);
            budget.release_storage(1).unwrap();
            let storage = budget.storage();
            let before = budget.work();
            let result = ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                owner,
                launch,
                input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            );
            assert!(matches!(
                result,
                Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
            ));
            assert_eq!(budget.storage(), storage);
            assert_eq!(budget.work(), before);
        },
    );
}

#[test]
fn preparation_errors_restore_only_adopted_source_credit_without_module_emission() {
    for preexisting in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        with_pending_api_input(
            ModuleFixture::Ordinary,
            preexisting,
            &mut budget,
            |owner, launch, input, _, budget| {
                let mut digest = *input.semantic_sha256;
                digest[0] ^= 1;
                let input = ProductionExecutionSourceInputV29 {
                    semantic_sha256: &digest,
                    ..input
                };
                let result = ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                );
                assert!(matches!(
                    result,
                    Err(EntranceError::Source(
                        ProductionPendingScopedSourceErrorV29::Source(_)
                    ))
                ));
                assert_eq!(budget.storage(), MODULE_FLOOR);
            },
        );
    }
}

fn entrance_resource(error: EntranceError) -> ArgumentResourceV1 {
    match error {
        EntranceError::Resource(error) => error,
        EntranceError::Source(error) => owning_resource(match error {
            ProductionPendingScopedSourceErrorV29::Source(error) => {
                ScopedModuleErrorV29::Source(error)
            }
            ProductionPendingScopedSourceErrorV29::Canonical(error) => {
                ScopedModuleErrorV29::Canonical(error)
            }
            ProductionPendingScopedSourceErrorV29::Occurrences(error) => {
                ScopedModuleErrorV29::Occurrences(error)
            }
        }),
        other => panic!("not a resource refusal: {other:?}"),
    }
}

fn source_entrance_probe(
    allowance: Option<(usize, usize)>,
) -> (
    Result<(), EntranceError>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let (result, used, peak, denied_storage) = {
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
        let retained = prepared.adopted_storage();
        budget
            .reserve_storage(budget.peak_storage() + 1 - budget.storage())
            .unwrap();
        if let Some((left_work, left_storage)) = allowance {
            budget
                .charge_work(MODULE_LIMIT - budget.work() - left_work)
                .unwrap();
            budget
                .reserve_storage(MODULE_LIMIT - budget.storage() - left_storage)
                .unwrap();
        }
        let entry = budget.storage();
        let before = budget.work();
        let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
            assert_eq!(view.root_count(budget)?, 2);
            assert_eq!(view.assertion_count(budget)?, 2);
            Ok(())
        });
        assert_eq!(budget.storage(), entry - retained);
        (
            result,
            budget.work() - before,
            budget.peak_storage() - entry,
            budget.failed_storage(),
        )
    };
    (result, used, peak, work.failed_work(), denied_storage)
}

#[test]
fn source_owned_consuming_scope_obeys_exact_and_one_short_work_and_storage() {
    let (result, work, peak, _, _) = source_entrance_probe(None);
    result.unwrap();
    assert!(work > 0 && peak > 0);
    let exact = source_entrance_probe(Some((work, peak)));
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3, exact.4),
        (work, peak, None, None)
    );
    let short_work = source_entrance_probe(Some((work - 1, peak)));
    assert!(matches!(
        entrance_resource(short_work.0.unwrap_err()),
        ArgumentResourceV1::Work(_)
    ));
    assert!(short_work.3.is_some());
    let short_storage = source_entrance_probe(Some((work, peak - 1)));
    assert!(matches!(
        entrance_resource(short_storage.0.unwrap_err()),
        ArgumentResourceV1::Storage(_)
    ));
    assert!(short_storage.4.is_some());
}

fn entrance_control_owner(cyclic: bool) -> ProductionSemanticSsaOwnerV1 {
    let baseline = module_fixture_owner(ModuleFixture::Ordinary);
    let source = baseline.source_semantic();
    let mut functions = source.functions().to_vec();
    let original = &functions[0];
    let blocks = if cyclic {
        vec![
            block(
                201,
                original.blocks()[0].statements().to_vec(),
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(1, U32)),
                    targets: fe2o3_mir_model::semantic_mir_v1::SemanticSwitchTargetsV1::new(
                        vec![
                            fe2o3_mir_model::semantic_mir_v1::SemanticSwitchTargetV1::new(
                                0,
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::SwitchValue,
                                    SemanticBlockIdV1::from_index(1),
                                ),
                            ),
                        ],
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(0),
                        ),
                    )
                    .unwrap(),
                },
            ),
            block(202, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        let call = |tag, target| {
            block(
                tag,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(2),
                        vec![SemanticOperandV1::Copy(place(1, U32))],
                        Some(SemanticCallDestinationV1::new(
                            place(0, UNIT),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(target),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            )
        };
        vec![
            call(201, 1),
            call(202, 2),
            block(203, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    functions[0] = function(
        60,
        original.role(),
        original.abi().clone(),
        original.locals().to_vec(),
        blocks,
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut callables = source.callables().to_vec();
    if !cyclic {
        functions.push(function(
            210,
            SemanticFunctionRoleV1::InternalHelper,
            abi(211, false, &[U32]),
            vec![
                local(212, UNIT, SemanticLocalRoleV1::Return),
                local(213, U32, SemanticLocalRoleV1::Argument(0)),
            ],
            vec![block(214, vec![], SemanticTerminatorKindV1::Return)],
        ));
        callables.push(SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(2),
        ));
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn source_owned_entrance_retains_repeated_helper_and_cyclic_invocation_coordinates() {
    for cyclic in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let projection = entrance_control_owner(cyclic);
        let owner = entrance_control_owner(cyclic);
        let (_, launch) =
            with_module_fixture_view(&owner, ModuleFixture::Ordinary, &mut budget, |_, _| ())
                .unwrap();
        let (prepared, _) = with_module_fixture_view(
            &projection,
            ModuleFixture::Ordinary,
            &mut budget,
            |source, budget| {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    source.input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
                .unwrap()
            },
        )
        .unwrap();
        let called = std::cell::Cell::new(false);
        prepared
            .with_checked_source_v18(&mut budget, |view, budget| {
                called.set(true);
                if cyclic {
                    let invocation = view
                        .invocation_entry(0, 0, budget)?
                        .expect("actual source-entry backedge requires invocation preheader");
                    assert_eq!(
                        invocation.semantic_function(),
                        SemanticFunctionIdV1::from_index(0)
                    );
                    assert_eq!(view.instance_count(0, budget)?, 1);
                } else {
                    assert_eq!(view.instance_count(0, budget)?, 3);
                    let first = view.instance(0, 1, budget)?;
                    let second = view.instance(0, 2, budget)?;
                    assert_eq!(first.0, second.0);
                    assert_ne!(
                        first.1, second.1,
                        "same helper has distinct original call occurrences"
                    );
                }
                assert!(view.span_count(0, budget)? > 0);
                Ok(())
            })
            .unwrap();
        assert!(
            called.get(),
            "the actual repeated/cyclic source reached its checked view"
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn source_preparation_boundary_header_has_independent_exact_and_short_costs() {
    let header = size_of::<ScopedSourceCleanupBoundaryV29>()
        + size_of::<std::thread::Result<SourceOwnedResultV18<ProductionPreparedSourceV18>>>();
    type Capture<'a> = (
        ProductionSemanticSsaOwnerV1,
        crate::ProductionSourceLaunchRosterV1,
        crate::ProductionExecutionSourceInputV29<'a>,
        Option<ProductionKernelArgumentAbiInputV18<'a>>,
        ProductionSemanticKirLimitsV1,
        usize,
        &'a std::cell::Cell<bool>,
    );
    let callback = cleanup_callback_header_oracle_v1766::<
        ProductionPreparedSourceV18,
        ProductionSourceOwnedViewErrorV18,
        Capture<'_>,
    >();
    for short in [false, true] {
        for preexisting in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            with_pending_api_input(
                ModuleFixture::Ordinary,
                preexisting,
                &mut budget,
                |owner, launch, input, capture, budget| {
                    let available = header - usize::from(short);
                    budget
                        .reserve_storage(MODULE_LIMIT - budget.storage() - available)
                        .unwrap();
                    let entry = budget.storage();
                    let before = budget.work();
                    let result =
                        ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                            owner,
                            launch,
                            input,
                            ProductionSemanticKirLimitsV1::default(),
                            budget,
                        );
                    assert!(matches!(
                        result,
                        Err(EntranceError::Resource(ArgumentResourceV1::Storage(_)))
                    ));
                    assert_eq!(budget.storage(), entry - capture);
                    let next = if short { header } else { header + callback };
                    assert_eq!(budget.failed_storage(), Some(entry + next));
                    assert_eq!(
                        budget.work(),
                        before,
                        "headers are checked before occurrence/source work"
                    );
                },
            );
        }
    }
}

#[test]
fn prepared_source_callback_header_refusal_drops_capture_and_restores_adopted_credit() {
    struct Capture<'a>(&'a std::cell::Cell<usize>);
    impl Drop for Capture<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for preexisting in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_source_fixture(ModuleFixture::Ordinary, preexisting, &mut budget);
        let retained = prepared.adopted_storage();
        let header = size_of::<ScopedSourceCleanupBoundaryV29>()
            + size_of::<std::thread::Result<Result<(), SourceConsumerErrorV18<EntranceError>>>>();
        let filler = MODULE_LIMIT - budget.storage() - header;
        budget.reserve_storage(filler).unwrap();
        let floor = budget.storage() - retained;
        let before = budget.work();
        let drops = std::cell::Cell::new(0);
        let captured = Capture(&drops);
        let result = prepared.with_checked_source_v18(
            &mut budget,
            move |_, _| -> SourceOwnedResultV18<()> {
                std::hint::black_box(&captured);
                panic!("callback frame refusal must precede source replay and consumer entry");
            },
        );
        assert!(matches!(
            result,
            Err(EntranceError::Resource(ArgumentResourceV1::Storage(error)))
                if error.actual() > MODULE_LIMIT && error.limit() == MODULE_LIMIT
        ));
        assert_eq!(drops.get(), 1);
        assert_eq!((budget.storage(), budget.work()), (floor, before));
        assert_eq!(budget.peak_storage(), MODULE_LIMIT);
        budget.release_storage(filler).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn prepared_source_checks_incoming_floor_before_adding_its_consuming_header() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    budget.release_storage(1).unwrap();
    let storage = budget.storage();
    let before = budget.work();
    let result =
        prepared.with_checked_source_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            panic!("under-reserved prepared owner must not enter the callback")
        });
    assert!(matches!(
        result,
        Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
    ));
    assert_eq!((budget.storage(), budget.work()), (storage, before));
}

#[test]
fn source_entrance_preserves_prior_work_and_storage_denial_history() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    assert!(budget.charge_work(MODULE_LIMIT + 1).is_err());
    assert!(budget.reserve_storage(MODULE_LIMIT + 1).is_err());
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    prepared
        .with_checked_source_v18(&mut budget, |view, budget| {
            assert_eq!(view.root_count(budget)?, 2);
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.failed_storage(), Some(MODULE_LIMIT + 1));
    assert_eq!(work.failed_work(), Some(MODULE_LIMIT + 1));
}

#[test]
fn borrowed_pending_view_keeps_legacy_owner_credits_and_narrow_caller_policy() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let limits = ProductionSemanticKirLimitsV1::default().with_storage_layout_limits(
        fe2o3_kernel_ir::StorageLayoutLimitsV1 {
            rows: 128,
            edges: 512,
            containment_depth: 32,
            object_bytes: 4096,
        },
    );
    let (pending, capture) = with_pending_api_input(
        ModuleFixture::Ordinary,
        true,
        &mut budget,
        |owner, launch, input, capture, budget| {
            (
                ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                    owner, launch, input, limits, budget,
                )
                .unwrap(),
                capture,
            )
        },
    );
    let floor = budget.storage();
    pending
        .with_checked_source_v18(&mut budget, |view, budget| {
            assert_eq!(view.limits(budget)?, limits);
            assert_eq!(view.root_count(budget)?, 2);
            budget.reserve_storage(17)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), floor + 17);
    let retained = pending.adopted_storage();
    drop(pending);
    budget.release_storage(retained + capture + 17).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn source_query_error_precedes_later_callback_error_even_after_custody_loss() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let held = std::cell::Cell::new(0);
    let result = prepared.with_checked_source_v18(&mut budget, |view, budget| {
        assert!(matches!(
            view.root(usize::MAX, budget),
            Err(EntranceError::Binding("root ordinal"))
        ));
        budget.release_storage(1)?;
        held.set(budget.storage());
        Err::<(), _>(EntranceError::Binding("later callback error"))
    });
    assert!(matches!(
        result,
        Err(EntranceError::Binding("root ordinal"))
    ));
    assert_eq!(
        budget.storage(),
        held.get(),
        "the first diagnostic does not clear no-refund"
    );
}

#[test]
fn prepared_source_does_not_reacquire_its_reservation_from_a_different_budget_slot() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
    let original_slot = prepared.slot;
    let storage = budget.storage();
    let mut moved = Box::new(budget);
    assert_ne!(original_slot, std::ptr::from_ref(moved.as_ref()) as usize);
    let result = prepared.with_checked_source_v18(&mut moved, |_, _| -> SourceOwnedResultV18<()> {
        panic!("changed slot must not reacquire source custody")
    });
    assert!(matches!(
        result,
        Err(EntranceError::Resource(ArgumentResourceV1::Accounting))
    ));
    assert_eq!(moved.storage(), storage);
}

thread_local! {
    static ENTRANCE_EMITTER_OBSERVED_V18: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn entrance_emitter_error(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    _: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    ENTRANCE_EMITTER_OBSERVED_V18.set(true);
    budget.release_storage(budget.storage())?;
    Err(unsupported(
        0,
        None,
        None,
        "original source entrance emitter error",
    ))
}

fn entrance_emitter_panic(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    _: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    ENTRANCE_EMITTER_OBSERVED_V18.set(true);
    budget.release_storage(budget.storage())?;
    std::panic::resume_unwind(Box::new(0x1829_1829_u64))
}

#[test]
fn prepared_source_does_not_refund_inner_emitter_custody_loss_or_replace_its_failure() {
    for panic in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = prepared_source_fixture(ModuleFixture::Ordinary, false, &mut budget);
        let observer: ScopedSlotObserverV29 = if panic {
            entrance_emitter_panic
        } else {
            entrance_emitter_error
        };
        let previous_observed = ENTRANCE_EMITTER_OBSERVED_V18.replace(false);
        let previous = SCOPED_SLOT_OBSERVER_V29.replace(Some(observer));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_checked_source_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
                panic!("failed original source emission must not reach a checked view")
            })
        }));
        SCOPED_SLOT_OBSERVER_V29.set(previous);
        assert!(ENTRANCE_EMITTER_OBSERVED_V18.replace(previous_observed));
        if panic {
            // C1 deliberately converts its inner callback panic into a typed
            // diagnostic. The separate outer callback test preserves raw Boxes.
            assert!(matches!(
                result.expect("inner C1 panic must use its established typed contract"),
                Err(EntranceError::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            function: 0,
                            block: None,
                            statement: None,
                            detail: "source reference callback panicked",
                        }
                    )
                ))
            ));
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(EntranceError::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            function: 0,
                            block: None,
                            statement: None,
                            detail: "original source entrance emitter error",
                        }
                    )
                ))
            ));
        }
        assert_eq!(
            budget.storage(),
            0,
            "no enclosing scope may recreate lost credit"
        );
    }
}

#[test]
fn source_callback_diagnostic_does_not_replace_an_earlier_c2_query_failure() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let called = std::cell::Cell::new(false);
    let result = with_module_fixture(ModuleFixture::Ordinary, &mut budget, |source, budget| {
        with_scoped_source_test_layouts_v29(
            source,
            ProductionSemanticKirLimitsV1::default(),
            budget,
            |_, layouts, budget| {
                production_call_instances_v1::with_production_call_instances_v1(
                    source.owner,
                    SemanticFunctionIdV1::from_index(0),
                    budget,
                    |instances, budget| {
                        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                            source_storage_v29::with_source_storage_root_v29(
                                layouts,
                                instances,
                                budget,
                                |plan, root, budget| {
                                    called.set(true);
                                    let prior = root
                                        .snapshot_local(
                                            instances.root(),
                                            SemanticLocalIdV1::from_index(u32::MAX),
                                            false,
                                            budget,
                                        )
                                        .unwrap_err();
                                    assert!(plan.failure.matches_recorded(&prior));
                                    let before = (budget.work(), budget.storage());
                                    let later = root.record_callback_failure(unsupported(
                                        0,
                                        None,
                                        None,
                                        "later callback must not replace C2",
                                    ));
                                    assert!(plan.failure.matches_recorded(&later));
                                    assert_eq!((budget.work(), budget.storage()), before);
                                    Err::<(), _>(later.into())
                                },
                            ),
                        )
                    },
                )
                .map_err(scoped_root_instance_error_v29)?
            },
        )
    })
    .unwrap();
    assert!(
        called.get(),
        "the actual original C1/C2 scope ran: {result:?}"
    );
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            function: 0,
            block: None,
            statement: None,
            detail: "source snapshot local is outside its original instance",
        })
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

include!("production_source_owned_entry_callbacks_v18_tests.rs");
