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
