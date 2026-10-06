use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_mir_model::semantic_mir_v1::SemanticAssertMessageV1;

fn terminal_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = admitted_single_function_semantic();
    let mut types = scalar_types(&base);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(230)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(231)),
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
    let condition = SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(2),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
    ));
    let root = root_with(
        &base.functions()[0],
        vec![
            test_local(232, 0, SemanticLocalRoleV1::Return),
            test_local(233, 1, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            test_block(
                234,
                vec![test_assign_to(test_typed_place(1, 1), number(7))],
                SemanticTerminatorKindV1::Assert {
                    condition,
                    expected: true,
                    message: SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Move(
                        test_typed_place(1, 1),
                    )),
                    target: test_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            test_block(
                235,
                vec![test_assign_to(
                    test_typed_place(1, 1),
                    SemanticOperandV1::Copy(test_typed_place(1, 1)),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    admit_owner(
        InertSemanticMirRequestV1::new_with_callables(
            base.target(),
            types,
            vec![],
            vec![],
            vec![],
            vec![root],
            vec![SemanticCallableDeclV1::defined(
                SemanticFunctionIdV1::from_index(0),
            )],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap(),
    )
}

#[test]
fn capture_rederives_failure_boundary_and_preserves_success_definition() {
    with_capture(terminal_owner(), |owner| {
        let rows = owner
            .occurrences_v1()
            .unwrap()
            .function(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        assert_eq!(rows.terminal_failure_start(SsaBlockIdV1::new(0)), Some(1));
        assert_eq!(rows.terminal_failure_start(SsaBlockIdV1::new(1)), None);
        assert_eq!(rows.terminal_failure_start(SsaBlockIdV1::new(2)), None);
        let events = rows.events();
        assert_eq!(events.len(), 5);
        assert_eq!(events[1].resolved(), events[3].resolved());
        assert!(!owner.grants_proof_or_artifact_authority());
    });
}

#[test]
fn capture_terminal_boundary_exact_and_short_keep_owner_and_caller_floor() {
    let mut owner = terminal_owner();
    let mut measured_work = Work::new(1_000_000);
    let mut measured = Budget::new(&mut measured_work, 1_000_000);
    measured.reserve_storage(17).unwrap();
    let receipt = owner
        .try_capture_occurrences_with_budget_v1(&mut measured)
        .unwrap();
    let work = measured.work();
    let peak = measured.peak_storage();
    assert_eq!(measured.storage(), 17);
    assert!(receipt.retained_storage() > 0);
    for (work_limit, storage_limit) in [(work, peak), (work - 1, peak), (work, peak - 1)] {
        let mut owner = terminal_owner();
        let identity = owner.identity();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let result = owner.try_capture_occurrences_with_budget_v1(&mut budget);
        assert_eq!(owner.identity(), identity);
        assert_eq!(budget.storage(), 17);
        if work_limit == measured.work() && storage_limit == peak {
            assert_eq!(result.unwrap(), receipt);
            assert!(owner.occurrences_v1().is_some());
        } else {
            match result {
                Err(CaptureError::Resource(Resource::Work(error)))
                    if work_limit < measured.work() =>
                {
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (measured.work(), work_limit)
                    );
                    assert_eq!(budget.work(), measured.work() - 3);
                    assert_eq!(budget.failed_storage(), None);
                }
                Err(CaptureError::Resource(Resource::Storage(error))) if storage_limit < peak => {
                    assert_eq!((error.actual(), error.limit()), (peak, storage_limit));
                    assert_eq!(budget.failed_storage(), Some(peak));
                }
                other => panic!("expected the exact short resource refusal, got {other:?}"),
            }
            assert!(owner.occurrences_v1().is_none());
            owner.verify_replay().unwrap();
        }
    }
}
