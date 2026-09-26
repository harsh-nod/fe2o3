//! Pure closed-profile controls. These inert functions never construct real
//! facts, CheckedBf16NominalCallV1, a source-bound graph view, or admission.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 1_000_000;
fn function(block_count: usize, with_call: bool) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let ty = SemanticTypeIdV1::from_index(0);
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([13; 32]),
        SemanticLayoutIdentityV1::from_sha256([14; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let blocks = (0..block_count)
        .map(|index| {
            let terminal = if index == 0 && with_call {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(0),
                        vec![],
                        None,
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                )
            } else {
                SemanticTerminatorKindV1::Return
            };
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([index as u8 + 1; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, terminal),
            )
            .unwrap()
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([17; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([18; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([19; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([21; 32]),
        source,
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([22; 32]),
            ty,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn call(function: &SemanticFunctionDeclV1) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[0].terminator().kind() else {
        panic!("component call fixture");
    };
    call
}
#[test]
fn closed_profile_requires_the_exact_source_call_pointer_not_equal_bytes() {
    let function = function(1, true);
    let cloned = call(&function).clone();
    let callables = [SemanticCallableDeclV1::defined(
        SemanticFunctionIdV1::from_index(1),
    )];
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
    closed_profile(&function, &callables, call(&function), &mut resources).unwrap();
    assert!(closed_profile(&function, &callables, &cloned, &mut resources).is_err());
    assert_eq!(owned, 0); // Predicate alone does not prepare storage or a view.
}
#[test]
fn closed_profile_refuses_absent_callable_and_missing_actual_call() {
    let calls = function(1, true);
    let no_calls = function(1, false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
    assert!(closed_profile(&calls, &[], call(&calls), &mut resources).is_err());
    assert!(closed_profile(&no_calls, &[], call(&calls), &mut resources).is_err());
    assert_eq!(owned, 0);
}
#[test]
fn closed_profile_refuses_the_unconnected_grid_leader_and_index_transform_families() {
    let ty = SemanticTypeIdV1::from_index(0);
    assert!(!allowed_intrinsic(
        &SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader: ty }
    ));
    assert!(!allowed_intrinsic(
        &SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
            index_witness: ty,
            raw_index: ty,
        }
    ));
    assert!(!allowed_intrinsic(
        &SemanticCompilerIntrinsicOperationV1::ColdPath
    ));
    assert!(allowed_intrinsic(
        &SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness: ty,
            raw_index: ty,
        }
    ));
    assert!(allowed_intrinsic(
        &SemanticCompilerIntrinsicOperationV1::Trap
    ));
}
#[test]
fn closed_profile_shape_and_zero_work_refuse_before_payload_allocation() {
    let oversized = function(33, true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    assert!(
        closed_profile(
            &oversized,
            &[],
            call(&oversized),
            &mut PreparationResourcesV1::new(&mut budget, &mut owned)
        )
        .is_err()
    );
    assert_eq!(owned, 0);
    let function = function(1, true);
    let mut no_work = Work::new(0);
    let mut no_budget = Budget::new(&mut no_work, LIMIT);
    assert!(
        closed_profile(
            &function,
            &[],
            call(&function),
            &mut PreparationResourcesV1::new(&mut no_budget, &mut owned)
        )
        .is_err()
    );
    assert!(no_budget.failed_work().is_some());
    assert_eq!(owned, 0);
}
#[test]
fn fresh_pending_owner_has_no_graph_and_no_readiness() {
    let pending = PendingNominalInitialGraphV1::new();
    assert!(pending.graph.is_none());
}

// S1 component controls inspect only the private source-family predicate/frame.
// No real owner, inventory, canonical facts, or complete graph view is invented.
fn intrinsic(operation: SemanticCompilerIntrinsicOperationV1) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([41; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([42; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([43; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([44; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([45; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            function(1, false).abi().clone(),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([46; 32]),
    }
}
fn function_with_extra_calls(extra: usize) -> SemanticFunctionDeclV1 {
    let original = function(extra + 1, true);
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut blocks = original.blocks().to_vec();
    for (index, block) in blocks.iter_mut().enumerate().skip(1) {
        *block = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([index as u8 + 1; 32]),
            source,
            vec![],
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        vec![],
                        None,
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
        )
        .unwrap();
    }
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([17; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([18; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([19; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([21; 32]),
        source,
        original.abi().clone(),
        original.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

#[test]
fn complete_profile_proof_retains_exact_function_callable_slice_and_call_identity() {
    let function = function(1, true);
    let cloned_function = function.clone();
    let cloned_call = call(&function).clone();
    let callables = [SemanticCallableDeclV1::defined(
        SemanticFunctionIdV1::from_index(1),
    )];
    let cloned_callables = callables.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let proof = closed_profile(
        &function,
        &callables,
        call(&function),
        &mut PreparationResourcesV1::new(&mut budget, &mut owned),
    )
    .unwrap();
    assert!(proof.same_source(&function, &callables, call(&function)));
    assert!(!proof.same_source(&cloned_function, &callables, call(&function)));
    assert!(!proof.same_source(&function, &cloned_callables, call(&function)));
    assert!(!proof.same_source(&function, &callables, &cloned_call));
    assert_eq!(owned, 0);
}

#[test]
fn complete_profile_refuses_actual_grid_leader_calls_before_any_recovery_or_availability() {
    let ty = SemanticTypeIdV1::from_index(0);
    for extra in [1, 2] {
        let function = function_with_extra_calls(extra);
        let callables = [
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            intrinsic(SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader: ty }),
        ];
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let result = closed_profile(
            &function,
            &callables,
            call(&function),
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert!(matches!(
            result,
            Err(Error::Incomplete(
                "nominal initial graph has an unconnected later edge or callable family"
            ))
        ));
        // Both stop at the FIRST seed, independently of a second candidate.
        // Missing/active/ambiguous Option producers never become accepted here.
        assert_eq!(budget.work(), 192);
        assert_eq!(owned, 0);
    }
}

#[test]
fn complete_profile_whitelist_is_unchanged_and_unknown_intrinsics_fail_closed() {
    let ty = SemanticTypeIdV1::from_index(0);
    for operation in [
        SemanticCompilerIntrinsicOperationV1::Trap,
        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness: ty,
            raw_index: ty,
        },
        SemanticCompilerIntrinsicOperationV1::ColdPath,
        SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
            index_witness: ty,
            raw_index: ty,
        },
    ] {
        let allowed = matches!(
            operation,
            SemanticCompilerIntrinsicOperationV1::Trap
                | SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
        );
        let function = function_with_extra_calls(1);
        let callables = [
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            intrinsic(operation),
        ];
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert_eq!(
            closed_profile(
                &function,
                &callables,
                call(&function),
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .is_ok(),
            allowed
        );
        assert_eq!(owned, 0);
    }
}

#[test]
fn complete_profile_work_boundary_refuses_a_partial_scan_without_a_proof() {
    let function = function_with_extra_calls(1);
    let callables = [
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        intrinsic(SemanticCompilerIntrinsicOperationV1::Trap),
    ];
    for limit in [191, 192] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert_eq!(
            closed_profile(
                &function,
                &callables,
                call(&function),
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .is_ok(),
            limit == 192
        );
        assert_eq!(budget.failed_work().is_some(), limit == 191);
        assert_eq!(owned, 0);
    }
}

#[test]
fn complete_graph_both_view_frames_are_paid_at_exact_work_and_storage_boundaries() {
    type Consumer = [u8; 173];
    type Output = [u8; 197];
    let frame = initial_graph_frame::<Output, Consumer>().unwrap();
    assert!(
        frame
            >= size_of::<NominalInitialGraphV1<'static>>()
                + size_of::<NominalCompleteForProfileGraphV1<'static>>()
                + 2 * size_of::<Consumer>()
                + 2 * size_of::<Result<Output>>()
    );
    for (work_limit, storage_limit, succeeds) in [
        (frame, frame + 37, true),
        (frame - 1, frame + 37, false),
        (frame, frame + 36, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(37).unwrap();
        let mut owned = 0;
        let result = admit_initial_graph_frame::<Output, Consumer>(
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(owned, if succeeds { frame } else { 0 });
        assert_eq!(budget.storage(), 37 + owned);
        assert_eq!(budget.failed_work().is_some(), work_limit < frame);
        assert_eq!(
            budget.failed_storage().is_some(),
            storage_limit < frame + 37
        );
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 37); // Foreign floor is not our credit.
    }
}

#[test]
fn complete_graph_wrapper_transfer_is_prepaid_before_the_inner_callback_exists() {
    type Consumer = [u8; 173];
    type Output = [u8; 197];
    let frame = complete_graph_frame::<Output, Consumer>().unwrap();
    assert!(
        frame
            >= size_of::<NominalCompleteForProfileGraphV1<'static>>()
                + 2 * size_of::<Consumer>()
                + 2 * size_of::<Result<Output>>()
    );
    for (work_limit, storage_limit, succeeds) in [
        (frame, frame + 37, true),
        (frame - 1, frame + 37, false),
        (frame, frame + 36, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(37).unwrap();
        let mut owned = 0;
        assert_eq!(
            admit_complete_graph_frame::<Output, Consumer>(&mut PreparationResourcesV1::new(
                &mut budget,
                &mut owned
            ),)
            .is_ok(),
            succeeds
        );
        assert_eq!(owned, if succeeds { frame } else { 0 });
        assert_eq!(budget.storage(), 37 + owned);
        assert_eq!(budget.failed_work().is_some(), work_limit < frame);
        assert_eq!(
            budget.failed_storage().is_some(),
            storage_limit < frame + 37
        );
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 37);
    }
}
