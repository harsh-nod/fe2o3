fn with_backend_private_output_v1(
    program: ProductionRankedSemanticProgramV1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    next: impl FnOnce(
        &fe2o3_lower_mir_kernel::ProductionMaterializedRankedModuleReceiptV1,
        &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        &fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
        &mut Budget<'_>,
    ),
) {
    assert!(program.all_kernel_checks_are_clean());
    let expected_roots = program.root_count();
    let retained = program.materialized.retained_analysis_storage_v1();
    let bound = dialect_amdgcn::bind_production_target_v1(
        program.materialized.executable().module(),
        profile,
    )
    .unwrap();
    assert_eq!(bound.profile(), profile);
    let mut work = Work::new(
        usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap(),
    );
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    const PREFIX: usize = 29;
    budget.reserve_storage(PREFIX + retained).unwrap();
    let (bound_owner, bound_storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        bound.module(), &mut budget,
    ).unwrap();
    budget
        .reserve_storage(bound_storage.retained_storage())
        .unwrap();
    drop(bound);
    let output =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_v1(&bound_owner, &mut budget)
            .unwrap();
    assert_eq!(output.report().passes().len(), 7);
    budget
        .reserve_storage(output.storage().retained_storage())
        .unwrap();
    // These are the real production roster authentication and custody APIs.
    // No source rows, mandatory reports or authority owners are fabricated.
    let verified = program.into_verified_roster_receipt().unwrap();
    let (receipt, _verification) = verified.into_module_verified_receipt().unwrap();
    assert_eq!(receipt.root_count(), expected_roots);
    let live = budget.storage();
    next(&receipt, &bound_owner, &output, &mut budget);
    assert_eq!(budget.storage(), live);
    let output_storage = output.storage().retained_storage();
    drop(output);
    budget.release_storage(output_storage).unwrap();
    drop(bound_owner);
    budget
        .release_storage(bound_storage.retained_storage())
        .unwrap();
    drop(receipt);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), PREFIX);
}

#[test]
fn actual_backend_receipt_joins_all_private_initializer_components_after_seven_passes() {
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    use fe2o3_kernel_ir::OperationKind;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for values in [[11; 8], [0, 1, 2, 3, 7, 31, 255, u32::MAX]] {
            let program = assertion_project(assertion_materialized(
                private_initializer_fixture_v1(values, 2),
            ))
            .unwrap();
            assert_eq!(program.roots[0].access_sources.len(), 17);
            with_backend_private_output_v1(program, profile, |receipt, bound, output, budget| {
                // Caller-owned observations outlive the scope, so reserve
                // their complete actual capacity before its incoming floor.
                let mut observed = Vec::<fe2o3_kernel_ir::CanonicalKirAccessCoordinateV1>::new();
                let header = std::mem::size_of_val(&observed);
                let element =
                    std::mem::size_of::<fe2o3_kernel_ir::CanonicalKirAccessCoordinateV1>();
                let requested = header + 17 * element;
                budget.reserve_storage(requested).unwrap();
                observed.try_reserve_exact(17).unwrap();
                let observed_storage = header + observed.capacity().checked_mul(element).unwrap();
                budget
                    .reserve_storage(observed_storage - requested)
                    .unwrap();
                receipt.with_checked_private_array_output_v1(bound, output, budget, |scope, budget| {
                    assert_eq!(scope.len(budget).unwrap(), 17);
                    assert!(!scope.grants_artifact_or_launch_authority());
                    let mut allocation = None;
                    for ordinal in 0..17 {
                        let row = scope.get(ordinal, budget).unwrap().unwrap();
                        let key = row.source_key();
                        assert_eq!((key[0], key[1], key[2]), (ROOT.index(), ROOT.index(), 0));
                        assert_eq!((key[3], key[6]), if ordinal < 16 {
                            (ordinal as u32 / 8 + 1, ordinal as u32 % 8)
                        } else { (3, 0) });
                        assert_eq!(row.offset(), u64::from(key[6]));
                        let store = row.output().unwrap();
                        assert!(store.executable());
                        assert_eq!(store.access().effect, 0);
                        assert!(!observed.contains(&store.access()));
                        assert!(observed.len() < observed.capacity());
                        observed.push(store.access());
                        if let Some(expected) = allocation { assert_eq!(store.allocation(), expected); }
                        allocation = Some(store.allocation());
                        let coordinate = store.access().operation;
                        let operation = &output.owner().module().functions[coordinate.block.function.0 as usize]
                            .body.as_ref().unwrap().blocks[coordinate.block.block as usize]
                            .operations[coordinate.operation as usize];
                        assert!(matches!(operation.kind, OperationKind::Store { value, .. } if value == store.value()));
                    }
                    Ok(())
                }).unwrap();
                let stores = output
                    .owner()
                    .module()
                    .functions
                    .iter()
                    .filter_map(|f| f.body.as_ref())
                    .flat_map(|b| &b.blocks)
                    .flat_map(|b| &b.operations)
                    .filter(|op| matches!(op.kind, OperationKind::Store { .. }))
                    .count();
                assert_eq!(stores, observed.len());
                drop(observed);
                budget.release_storage(observed_storage).unwrap();
                assert!(!receipt.grants_artifact_or_launch_authority());
                // This remains synthetic admitted-source coverage, not a Rust
                // tutorial run, private R2 proof or target authorization.
            });
        }
    }
}

#[test]
fn actual_backend_private_initializer_plus_slice_read_is_not_a_closed_private_output_batch() {
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    use fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1;
    let ssa = assertion_ssa_functions(
        private_slice_composition_types_v1(),
        vec![private_slice_composition_source_v1(
            PrivateSliceCompositionV1::InitializerRead,
        )],
    );
    let source =
        materialize_ranked_fixture_v1(ssa, &[ranked_root_input_1d(A_NAME, 247, 64)]).unwrap();
    let program = assertion_project(source).unwrap();
    assert_eq!(program.roots[0].access_sources.len(), 10);
    assert!(program.roots[0].executable_effect_sources.is_empty());
    with_backend_private_output_v1(
        program,
        Profile::Gfx942,
        |receipt, bound, output, budget| {
            let mut called = false;
            let result =
                receipt.with_checked_private_array_output_v1(bound, output, budget, |_, _| {
                    called = true;
                    Ok(())
                });
            assert!(!called);
            assert!(matches!(
                result,
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "private output forward census is incomplete"
                ))
            ));
        },
    );
}

fn actual_backend_two_private_initializer_roots_v1() -> ProductionRankedSemanticProgramV1 {
    // Reuse the admitted two-root launch fixture, preserving its complete
    // identities, ABI ownership, root order and distinct exported symbols.
    let original = source_launch_test_semantic_with_access_v1(70, 0xa1, true);
    let scalar = SemanticTypeIdV1::from_index(1);
    let array = SemanticTypeIdV1::from_index(2);
    let functions = original
        .functions()
        .iter()
        .map(|function| {
            let mut statements = function.blocks()[0].statements().to_vec();
            statements.insert(
                1,
                typed_assignment(
                    2,
                    array,
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Array,
                        (0..8).map(|_| typed_constant(scalar, 41, 4)).collect(),
                    )
                    .unwrap(),
                ),
            );
            private_write_statements_v1(function, statements)
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        original.target(),
        original.types().to_vec(),
        original.allocations().to_vec(),
        original.statics().to_vec(),
        original.vtables().to_vec(),
        functions,
        original.callables().to_vec(),
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let inputs = source_launch_test_inputs_v1();
    let materialized = materialize_ranked_fixture_v1(ssa, &inputs).unwrap();
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        materialized,
        &inputs,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
    )
    .unwrap();
    assert_eq!(program.root_count(), 2);
    for root in &program.roots {
        assert_eq!(root.access_sources.len(), 9);
        assert!(root.executable_effect_sources.is_empty());
    }
    for index in 0..9 {
        let left = program.roots[0].access_sources[index];
        let right = program.roots[1].access_sources[index];
        assert_eq!(
            (
                left.semantic_block(),
                left.semantic_statement(),
                left.semantic_access_ordinal()
            ),
            (
                right.semantic_block(),
                right.semantic_statement(),
                right.semantic_access_ordinal()
            )
        );
        let operation =
            |root: usize, source: fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1| {
                &program.roots[root].lowering.kernel().blocks()[source.ranked_block() as usize]
                    .operations()[source.ranked_operation() as usize]
            };
        let ProductionRankedOperationV1::Access {
            view: left_view,
            indices: left_indices,
            ..
        } = operation(0, left)
        else {
            panic!("first root ordinary access");
        };
        let ProductionRankedOperationV1::Access {
            view: right_view,
            indices: right_indices,
            ..
        } = operation(1, right)
        else {
            panic!("second root ordinary access");
        };
        assert_eq!((left_view, left_indices), (right_view, right_indices));
    }
    program
}

#[test]
fn actual_backend_two_private_roots_keep_identical_local_coordinates_separate() {
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_backend_private_output_v1(
            actual_backend_two_private_initializer_roots_v1(),
            profile,
            |receipt, bound, output, budget| {
                assert_eq!(receipt.root_count(), 2);
                // All observations remain inside the callback; no output buffer
                // or borrowed fact survives its lifetime.
                receipt
                    .with_checked_private_array_output_v1(bound, output, budget, |scope, budget| {
                        assert_eq!(scope.len(budget).unwrap(), 18);
                        for index in 0..9 {
                            let left = scope.get(index, budget).unwrap().unwrap();
                            let right = scope.get(index + 9, budget).unwrap().unwrap();
                            let left_key = left.source_key();
                            let right_key = right.source_key();
                            assert_eq!((left_key[0], left_key[1]), (0, 0));
                            assert_eq!((right_key[0], right_key[1]), (1, 1));
                            assert_eq!(&left_key[2..], &right_key[2..]);
                            assert_eq!(
                                (left_key[2], left_key[3], left_key[6]),
                                if index < 8 {
                                    (0, 1, index as u32)
                                } else {
                                    (0, 2, 0)
                                }
                            );
                            assert_eq!(left.offset(), right.offset());
                            assert_ne!(
                                left.original_effect_and_slot().1,
                                right.original_effect_and_slot().1
                            );
                            let left_ranked = left.ranked_access();
                            let right_ranked = right.ranked_access();
                            assert_eq!((left_ranked.0, right_ranked.0), (0, 1));
                            assert_eq!(
                                (left_ranked.1, left_ranked.2),
                                (right_ranked.1, right_ranked.2)
                            );
                            let left = left.output().unwrap();
                            let right = right.output().unwrap();
                            assert!(left.executable() && right.executable());
                            assert_ne!(
                                left.access().operation.block.function,
                                right.access().operation.block.function
                            );
                            assert_ne!(
                                left.allocation().block.function,
                                right.allocation().block.function
                            );
                            assert_eq!(
                                (
                                    left.access().operation.block.block,
                                    left.access().operation.operation,
                                    left.access().effect
                                ),
                                (
                                    right.access().operation.block.block,
                                    right.access().operation.operation,
                                    right.access().effect
                                )
                            );
                            assert_eq!(left.value(), right.value());
                        }
                        Ok(())
                    })
                    .unwrap();
            },
        );
    }
}

#[test]
fn actual_backend_two_root_substitution_is_rejected_before_output_scope() {
    let mut program = actual_backend_two_private_initializer_roots_v1();
    assert!(program.all_kernel_checks_are_clean());
    // This is an earlier authenticated-roster refusal, not a private mutation
    // of a lowerer authority owner or an exercise of the output row getter.
    program.roots[1].semantic_root = program.roots[0].semantic_root;
    assert!(matches!(
        program.into_verified_roster_receipt(),
        Err(ProductionRankedVerificationErrorV1::RosterMetadata(
            "a reordered, duplicate, or substituted semantic root"
        ))
    ));
}
