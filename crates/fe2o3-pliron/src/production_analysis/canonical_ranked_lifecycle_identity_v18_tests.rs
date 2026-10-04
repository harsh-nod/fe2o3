use super::tests::{LAYOUTS, with_checked};
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ExecutionOperationV15 as E,
    ExecutionRoleV15 as R, Function, Kernel, LaunchDomain, LaunchExtent, Module, Operation,
    OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};

fn lifecycle() -> Module {
    let mut module = Module::new("native-lifecycle-identity");
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(0), Type::Execution(R::Context))],
            OperationKind::Execution(E::ContextIssue),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(1), Type::Execution(R::Workgroup))],
            OperationKind::Execution(E::WorkgroupDerive {
                context: ValueId(0),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Execution(E::ScopeEnd {
                workgroup: ValueId(1),
                discarded: vec![],
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "lifecycle",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "lifecycle",
        "lifecycle",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

#[test]
fn exact_imported_lifecycle_runs_all_nine_stages_without_completing_source_roles() {
    with_checked(&lifecycle(), |checked, budget| {
        let floor = budget.storage();
        let completed = Cell::new(false);
        with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
            assert_eq!(pending.obligations(budget)?.len(), 3);
            pending.with_native_observations(budget, |observed, budget| {
                let report = observed.report(0, budget)?.unwrap();
                assert!(report.is_clean());
                assert_eq!(report.pass_order(), &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                assert_eq!(observed.obligations(budget)?.len(), 3);
                assert!(!observed.source_roles_are_complete());
                assert!(!observed.ranked_verification_is_complete());
                assert!(!observed.grants_artifact_or_launch_authority());
                assert!(observed.history(0, budget)?.is_some());
                completed.set(true);
                Ok(())
            }).unwrap();
            Ok(())
        }).unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), floor);
        let error = with_canonical_ranked_policy_checks_v18(
            checked,
            LAYOUTS,
            budget,
            |_, _| -> Result<(), Failure> {
                panic!("identity extension completed the strict source-role gate")
            },
        )
        .unwrap_err();
        assert!(matches!(error.failure(), Failure::SourceRequirementV18 {
            coordinate, requirement: CanonicalRankedSourceRequirementV18::Execution,
        } if coordinate.operation == 0));
        assert!(error.last_invocation().is_none());
    });
}

#[test]
fn unrelated_preserved_operation_still_refuses_native_identity() {
    let mut module = lifecycle();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![ValueDef::new(
                ValueId(2),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ));
    with_checked(&module, |checked, budget| {
        let completed = Cell::new(false);
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                assert_eq!(pending.obligations(budget)?.len(), 4);
                let error = pending
                    .with_native_observations(budget, |_, _| -> Result<(), Failure> {
                        panic!("private allocation acquired a lifecycle identity extension")
                    })
                    .unwrap_err();
                assert!(matches!(
                    error.failure(),
                    Failure::Analysis {
                        function: 0,
                        cause: ProductionPlironPreloweringErrorV2::Preservation(
                            crate::PlironPassPreservationErrorV1::IdentityUnavailable {
                                source_code: "FE2O3-PRESERVE-001",
                                ..
                            }
                        )
                    }
                ));
                assert!(error.last_invocation().is_some());
                completed.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(completed.get());
    });
}

#[test]
fn exact_imported_unreachable_runs_all_nine_native_stages_without_source_completion() {
    let mut module = lifecycle();
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Unreachable);
    with_checked(&module, |checked, budget| {
        let exact = checked.inventory(budget).unwrap().owner();
        let floor = budget.storage();
        let completed = Cell::new(false);
        with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
            assert_eq!(pending.obligations(budget)?.len(), 3);
            pending.with_native_observations(budget, |observed, budget| {
                assert!(std::ptr::eq(observed.owner(budget)?, exact));
                assert_eq!(observed.owner(budget)?.module(), &module);
                let report = observed.report(0, budget)?.unwrap();
                assert!(report.is_clean());
                assert_eq!(report.pass_order(), &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                assert_eq!(observed.obligations(budget)?.len(), 3);
                assert!(!observed.source_roles_are_complete());
                assert!(!observed.ranked_verification_is_complete());
                assert!(!observed.grants_artifact_or_launch_authority());
                assert!(observed.history(0, budget)?.is_some());
                completed.set(true);
                Ok(())
            }).unwrap();
            Ok(())
        }).unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), floor);
        let error = with_canonical_ranked_policy_checks_v18(
            checked,
            LAYOUTS,
            budget,
            |_, _| -> Result<(), Failure> {
                panic!("terminal identity completed source Execution");
            },
        )
        .unwrap_err();
        assert!(matches!(error.failure(), Failure::SourceRequirementV18 {
            coordinate, requirement: CanonicalRankedSourceRequirementV18::Execution,
        } if coordinate.operation == 0));
        assert!(error.last_invocation().is_none());
    });
}
