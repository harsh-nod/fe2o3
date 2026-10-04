fn mixed_licm_source_v28(looping: bool) -> ProductionSemanticSsaOwnerV1 {
    mixed_licm_source_with_guard_v28(looping, true)
}

fn mixed_licm_source_with_guard_v28(
    looping: bool,
    ordered_guard: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let base = mixed_pure_cse_source_v26(true);
    if !looping {
        return base;
    }
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let scalar = SemanticTypeIdV1::from_index(1);
    let mut types = source.types().to_vec();
    let boolean = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
    if ordered_guard {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([240; 32]),
            SemanticLayoutIdentityV1::from_sha256([240; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(1),
                1,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 8, 1),
                    SemanticScalarValidityRangeV1::new(0, 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ));
    }
    let counter = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(12), vec![], scalar).unwrap();
    let condition =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(13), vec![], boolean).unwrap();
    let bound = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(14), vec![], scalar).unwrap();
    let constant = |bits| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            scalar,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
        ))
    };
    let assignment = |value| {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                counter.clone(),
                SemanticRvalueV1::new(scalar, value),
            )),
        )
    };
    let mut locals = original.locals().to_vec();
    assert_eq!(locals.len(), 12);
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([241; 32]),
        scalar,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    if ordered_guard {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([242; 32]),
            boolean,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([243; 32]),
            scalar,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let mut blocks = original.blocks().to_vec();
    let header = u32::try_from(blocks.len()).unwrap();
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let term = |kind| SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind);
    let mut preheader = blocks[3].statements().to_vec();
    assert!(preheader.len() >= 6);
    // The real source Load remains before the loop. Its value is invariant;
    // three live bitwise statements and the actual Store execute in the body.
    let mut body = preheader.split_off(preheader.len() - 4);
    preheader.push(assignment(SemanticRvalueKindV1::Use(constant(0))));
    if ordered_guard {
        // Native progress requires the exact bound to dominate loop entry.
        preheader.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                bound.clone(),
                SemanticRvalueV1::new(scalar, SemanticRvalueKindV1::Use(constant(4))),
            )),
        ));
    }
    body.push(assignment(SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::Add,
        left: SemanticOperandV1::Copy(counter.clone()),
        right: constant(1),
    }));
    let exit = blocks[3].terminator().clone();
    blocks[3] = SemanticBasicBlockV1::new(
        blocks[3].identity(),
        blocks[3].source(),
        preheader,
        term(SemanticTerminatorKindV1::Goto(edge(
            SemanticEdgeRoleV1::Goto,
            header,
        ))),
    )
    .unwrap();
    blocks.push(
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([240; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            if ordered_guard {
                // Match the supported Rust `while counter < 4` progress shape.
                // The previous switch-only `counter != 4` is retained below as
                // an explicit refusal, not silently admitted by the checker.
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        condition.clone(),
                        SemanticRvalueV1::new(
                            boolean,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: SemanticOperandV1::Copy(counter.clone()),
                                right: SemanticOperandV1::Copy(bound),
                            },
                        ),
                    )),
                )]
            } else {
                vec![]
            },
            term(SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(if ordered_guard {
                    condition
                } else {
                    counter
                }),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        if ordered_guard { 0 } else { 4 },
                        edge(SemanticEdgeRoleV1::SwitchValue, header + 2),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, header + 1),
                )
                .unwrap(),
            }),
        )
        .unwrap(),
    );
    blocks.push(
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([241; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            body,
            term(SemanticTerminatorKindV1::Goto(edge(
                SemanticEdgeRoleV1::Goto,
                header,
            ))),
        )
        .unwrap(),
    );
    blocks.push(
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([242; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            vec![],
            exit,
        )
        .unwrap(),
    );
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        vec![root],
        source.callables().to_vec(),
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

fn with_mixed_licm_prefix_v28(
    looping: bool,
    run: impl FnMut(
        &ProductionConditionalMixedPureCseOutputHandoffV26<'_, '_>,
        &ProductionSemanticSsaOwnerV1,
        &mut ArgumentBudgetV1<'_>,
    ),
) -> Result<(), ProductionMixedSourceHandoffErrorV26> {
    with_mixed_licm_prefix_custody_v28(looping, false, run)
}

fn with_mixed_licm_prefix_custody_v28(
    looping: bool,
    custody_denied: bool,
    run: impl FnMut(
        &ProductionConditionalMixedPureCseOutputHandoffV26<'_, '_>,
        &ProductionSemanticSsaOwnerV1,
        &mut ArgumentBudgetV1<'_>,
    ),
) -> Result<(), ProductionMixedSourceHandoffErrorV26> {
    let owner = mixed_licm_source_v28(looping);
    with_mixed_licm_prefix_owner_v28(owner, custody_denied, run)
}

fn with_mixed_licm_prefix_owner_v28(
    owner: ProductionSemanticSsaOwnerV1,
    custody_denied: bool,
    mut run: impl FnMut(
        &ProductionConditionalMixedPureCseOutputHandoffV26<'_, '_>,
        &ProductionSemanticSsaOwnerV1,
        &mut ArgumentBudgetV1<'_>,
    ),
) -> Result<(), ProductionMixedSourceHandoffErrorV26> {
    let abi = issued_descriptor_role_abi_v18(&owner);
    let semantic = owner.source_semantic();
    let launch_roots: Vec<_> = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &launch_roots).unwrap();
    let sha = *owner.source_semantic_sha256();
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let roots = abi.roots();
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &sha,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 256 << 20);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
            owner,
            launch,
            input,
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
    let result: Result<(), ProductionMixedSourceHandoffErrorV26> = prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let launches = vec![
                fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [64, 1, 1],
                };
                source.root_count(budget)?
            ];
            let original = source.source_ssa(budget)?;
            let prefix = source.conditional_mixed_pure_cse_output_v26(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            run(&prefix, original, budget);
            let released = prefix.discard(budget);
            released?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        });
    if custody_denied {
        assert!(budget.storage() > MODULE_FLOOR);
    } else {
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
    result
}

#[test]
fn genuine_mixed_source_licm_preserves_unproved_equality_loop_progress_refusal() {
    let mut calls = 0;
    let error = with_mixed_licm_prefix_owner_v28(
        mixed_licm_source_with_guard_v28(true, false),
        false,
        |_, _, _| {
            calls += 1;
        },
    )
    .unwrap_err();
    assert_eq!(calls, 0, "unproved progress cannot reach LICM");
    let ProductionMixedSourceHandoffErrorV26::Optimization(
        ProductionSourceOptimizationErrorV18::Adoption(
            fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                ProductionMixedSourceCheckErrorV26::Native(
                    ProductionSourceNativeLifecycleErrorV18::Native(native),
                ),
            ),
        ),
    ) = &error
    else {
        panic!("expected native origin refusal, got {error:?}");
    };
    let fe2o3_pliron::CanonicalRankedPolicyFailureV1::Analysis {
        function: 0,
        cause: fe2o3_pliron::ProductionPlironPreloweringErrorV2::Semantic(semantic),
    } = native.failure()
    else {
        panic!("expected typed progress refusal, got {error:?}");
    };
    assert!(
        semantic
            .report()
            .progress()
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                fe2o3_pliron::PlironProgressFindingV1::ProgressIncomplete { .. }
            ))
    );
}

#[test]
fn genuine_mixed_source_licm_relocates_live_loop_values_and_rebuilds_exact_memory() {
    for looping in [false, true] {
        with_mixed_licm_prefix_v28(looping, |prefix, source, budget| {
            let inspected = (|| -> Result<(), ProductionMixedLicmRelocationErrorV28> {
                let prefix_floor = budget.storage();
                let relocated = prefix.prepare_mixed_licm_v28(budget)?;
                let inspected = (|| -> Result<(), ProductionMixedLicmRelocationErrorV28> {
                    relocated.check_original_source(source, budget)?;
                    relocated.replay(budget)?;
                    assert!(std::ptr::eq(relocated.prefix(budget)?, prefix));
                    let tail = relocated.tail(budget)?;
                    let moved = tail
                        .origins()
                        .iter()
                        .filter(|row| row.hoist.is_some())
                        .count();
                    assert_eq!(moved > 0, looping);
                    assert!(
                        relocated
                            .definition_projection(budget)?
                            .iter()
                            .any(|row| row.input != row.output)
                            || !looping
                    );
                    assert_eq!(prefix.output(budget)?.execution().policy_version(), 10);
                    assert!(!relocated.final_native_completion_is_complete());
                    assert!(!relocated.grants_artifact_or_launch_authority());
                    Ok(())
                })();
                let released = relocated.discard(budget);
                inspected?;
                released?;
                assert_eq!(budget.storage(), prefix_floor);
                Ok(())
            })();
            inspected.unwrap();
        })
        .unwrap();
    }
}

#[test]
fn genuine_mixed_source_licm_rejects_omitted_rebound_and_foreign_projection_rows() {
    with_mixed_licm_prefix_v28(true, |prefix, _, budget| {
        let floor = budget.storage();
        let relocated = prefix.prepare_mixed_licm_v28(budget).unwrap();
        let inspected = relocated.check_projection_refusals_v28(budget);
        let released = relocated.discard(budget);
        inspected.unwrap();
        released.unwrap();
        assert_eq!(budget.storage(), floor);
    })
    .unwrap();
}

#[test]
fn genuine_mixed_source_licm_observed_custody_loss_never_refunds_restored_tail_credit() {
    for foreign in [false, true] {
        let mut completed = false;
        let error = with_mixed_licm_prefix_custody_v28(true, true, |prefix, _, budget| {
            let relocated = prefix.prepare_mixed_licm_v28(budget).unwrap();
            let paid = budget.storage();
            let selected = if foreign {
                let mut other_work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
                let mut other = ArgumentBudgetV1::new(&mut other_work, 256 << 20);
                other.reserve_storage(paid).unwrap();
                let before = (
                    other.work(),
                    other.storage(),
                    budget.work(),
                    budget.storage(),
                );
                let error = relocated
                    .tail(&other)
                    .err()
                    .expect("foreign ledger must refuse");
                assert_eq!(
                    (
                        other.work(),
                        other.storage(),
                        budget.work(),
                        budget.storage()
                    ),
                    before
                );
                error
            } else {
                budget.release_storage(1).unwrap();
                let error = relocated
                    .tail(budget)
                    .err()
                    .expect("observed one-byte loss must refuse");
                budget.reserve_storage(1).unwrap();
                error
            };
            assert!(matches!(
                selected,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            assert!(matches!(
                relocated.tail(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert!(matches!(
                relocated.discard(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!(
                budget.storage(),
                paid,
                "sticky denial retains every borrowed and tail credit"
            );
            completed = true;
        })
        .unwrap_err();
        assert!(
            completed,
            "authentic LICM owner must reach custody control: {error:?}"
        );
        assert!(matches!(
            error,
            ProductionMixedSourceHandoffErrorV26::Check(
                ProductionMixedSourceCheckErrorV26::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                )
            )
        ));
    }
}

fn mixed_licm_resource_v28(error: ProductionMixedLicmRelocationErrorV28) -> ArgumentResourceV1 {
    use ProductionMixedLicmRelocationErrorV28 as Error;
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryErrorV1 as Inventory, CanonicalKirLicmErrorV1 as Pair,
        CanonicalKirLoopErrorV1 as Loops, CanonicalKirMemorySsaErrorV1 as Memory,
    };
    use fe2o3_kernel_ir::{
        BorrowedKernelIrVerificationErrorV1 as Verification,
        CanonicalKernelIrReplayAdmissionErrorV18 as Admission,
        CanonicalKirControlFlowScopeErrorV1 as Flow, KernelIrDecodeError as Decode,
        KernelIrEncodeError as Encode, StorageLayoutErrorV1 as Layout,
    };
    use fe2o3_kernel_opt::OwnedLicmErrorV1 as Motion;
    match error {
        Error::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))
        | Error::Inventory(Inventory::Resource(resource))
        | Error::Memory(Memory::Resource(resource))
        | Error::Motion(Motion::Resource(resource))
        | Error::Motion(Motion::Inventory(Inventory::Resource(resource)))
        | Error::Motion(Motion::Loops(Loops::Resource(resource)))
        | Error::Motion(Motion::ControlFlow(Flow::Resource(resource)))
        | Error::Motion(Motion::Pair(Pair::Resource(resource)))
        | Error::Motion(Motion::Pair(Pair::Inventory(Inventory::Resource(resource))))
        | Error::Motion(Motion::Pair(Pair::Loops(Loops::Resource(resource))))
        | Error::Motion(Motion::Pair(Pair::ControlFlow(Flow::Resource(resource))))
        | Error::Motion(Motion::AdmissionV18(Admission::Resource(resource)))
        | Error::Motion(Motion::AdmissionV18(Admission::Layout(Layout::Resource(resource))))
        | Error::Motion(Motion::AdmissionV18(Admission::Verification(Verification::Resource(
            resource,
        ))))
        | Error::Motion(Motion::AdmissionV18(Admission::Decode(Decode::Resource(resource)))) => {
            resource
        }
        Error::Motion(Motion::AdmissionV18(Admission::Encode(Encode::WorkLimit(error))))
        | Error::Motion(Motion::AdmissionV18(Admission::Decode(Decode::WorkLimit(error))))
        | Error::Motion(Motion::AdmissionV18(Admission::Decode(Decode::Encode(
            Encode::WorkLimit(error),
        )))) => ArgumentResourceV1::Work(error),
        other => panic!("expected typed LICM resource refusal, got {other:?}"),
    }
}

#[test]
fn genuine_mixed_source_licm_has_exact_and_one_short_post_prefix_limits() {
    const WORK: usize = 1_000_000_000;
    const SPACE: usize = 256 << 20;
    let run = |work_headroom: usize, storage_headroom: usize| {
        let mut observation = None;
        let outer = with_mixed_licm_prefix_v28(true, |prefix, _, budget| {
            let floor = budget.storage();
            budget
                .charge_work(WORK - budget.work() - work_headroom)
                .unwrap();
            let padding = SPACE - floor - storage_headroom;
            let old_peak = budget.peak_storage();
            budget.reserve_storage(padding).unwrap();
            assert!(budget.storage() > old_peak);
            let start = (budget.work(), budget.storage());
            let result = (|| -> Result<(), ProductionMixedLicmRelocationErrorV28> {
                let relocated = prefix.prepare_mixed_licm_v28(budget)?;
                let inspected = relocated.replay(budget);
                let released = relocated.discard(budget);
                inspected?;
                released?;
                Ok(())
            })();
            assert_eq!(budget.storage(), start.1);
            observation = Some((
                result.err().map(mixed_licm_resource_v28),
                budget.work() - start.0,
                budget.peak_storage() - start.1,
            ));
            budget.release_storage(padding).unwrap();
            assert_eq!(budget.storage(), floor);
        });
        let observed = observation.expect("genuine prefix reached LICM continuation");
        if observed.0.is_none() {
            outer.unwrap();
        }
        observed
    };
    let full = run(WORK / 2, SPACE / 2);
    assert!(full.0.is_none());
    let exact = run(full.1, full.2);
    assert!(exact.0.is_none());
    assert_eq!((exact.1, exact.2), (full.1, full.2));
    let ArgumentResourceV1::Work(denied) = run(full.1 - 1, full.2).0.unwrap() else {
        panic!("one-short LICM work must preserve typed Work");
    };
    assert_eq!(denied.limit(), WORK);
    assert!(denied.actual() > denied.limit());
    let ArgumentResourceV1::Storage(denied) = run(full.1, full.2 - 1).0.unwrap() else {
        panic!("one-short LICM storage must preserve typed Storage");
    };
    assert_eq!(denied.limit(), SPACE);
    assert!(denied.actual() > denied.limit());
}

include!("production_source_mixed_licm_native_v28_tests.rs");
