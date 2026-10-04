use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::{
    SsaBlockIdV1, SsaDefinitionIdV1, SsaEventV1, SsaResolvedEventV1, SsaValueV1, SsaVariableIdV1,
};
use fe2o3_pliron::{
    ProductionSemanticSsaEntryOriginV1 as EntryOrigin,
    ProductionSemanticSsaEventRoleV1 as EventRole, ProductionSemanticSsaOccurrenceSiteV1 as Site,
    ProductionSemanticSsaOperandRoleV1 as Operand,
};

#[test]
fn admitted_neutral_source_capture_keeps_implicit_scope_and_transparent_borrow() {
    let mut owner = neutral_ranked_source_for_operation_v1(
        SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
            context: NEUTRAL_CONTEXT_TYPE,
            dynamic_lds: NEUTRAL_DYNAMIC_LDS_TYPE,
            element_storage: NEUTRAL_ELEMENT_TYPE,
            element: NEUTRAL_ELEMENT_TYPE,
        },
        64,
    );
    let function = SemanticFunctionIdV1::from_index(0);
    let scope = SsaVariableIdV1::new(1);
    let reference = SsaVariableIdV1::new(2);
    let scope_value = SsaValueV1::Definition(SsaDefinitionIdV1::new(0));
    let reference_value = SsaValueV1::Definition(SsaDefinitionIdV1::new(1));
    assert_eq!(
        owner
            .plan_for_function(function)
            .unwrap()
            .implicit_entry_variables(),
        &[scope]
    );
    let source = owner.source_semantic().functions().as_ptr();
    let identity = owner.identity();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let storage = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let view = owner.occurrences_v1().unwrap();
    assert_eq!(view.function_count(), 1);
    let rows = view.function(function).unwrap();
    assert!(std::ptr::eq(rows.owner(), &owner));
    assert_eq!(rows.entry_definitions().len(), 1);
    let entry = &rows.entry_definitions()[0];
    assert_eq!(entry.ordinal(), 0);
    assert_eq!(entry.variable(), scope);
    assert_eq!(entry.origin(), EntryOrigin::ImplicitCapability);
    assert_eq!(entry.value(), Some(scope_value));
    // A transparent ambient-scope borrow keeps its source Use. It is not a
    // GridLeader elision and must not acquire ElidedBorrowDestination origin.
    assert!(rows.elisions().is_empty());
    assert_eq!(rows.events().len(), 7);
    let expected = [
        (
            Operand::RvaluePlace,
            EventRole::BaseUse,
            SsaEventV1::Use(scope),
            SsaResolvedEventV1::Use {
                variable: scope,
                value: scope_value,
            },
        ),
        (
            Operand::Destination,
            EventRole::DestinationDefine,
            SsaEventV1::Define(reference),
            SsaResolvedEventV1::Define {
                variable: reference,
                value: reference_value,
            },
        ),
    ];
    for (ordinal, (row, (operand, role, event, resolved))) in
        rows.events().iter().zip(expected).enumerate()
    {
        assert_eq!(
            row.site(),
            Site::Statement {
                block: SsaBlockIdV1::new(0),
                statement: 0
            }
        );
        assert_eq!(row.ordinal(), ordinal as u32);
        assert_eq!(
            (row.operand(), row.role(), row.event()),
            (operand, role, event)
        );
        assert!(row.is_reachable());
        assert!(row.is_promoted());
        assert_eq!(row.resolved(), Some(resolved));
    }
    let call_use = &rows.events()[2];
    assert_eq!(
        call_use.site(),
        Site::Terminator {
            block: SsaBlockIdV1::new(0)
        }
    );
    assert_eq!(
        (call_use.ordinal(), call_use.operand(), call_use.role()),
        (2, Operand::CallArgument(0), EventRole::BaseUse)
    );
    assert_eq!(
        call_use.resolved(),
        Some(SsaResolvedEventV1::Use {
            variable: reference,
            value: reference_value
        })
    );
    assert_eq!(owner.source_semantic().functions().as_ptr(), source);
    assert_eq!(owner.identity(), identity);
    owner.verify_replay().unwrap();
    // NativeSource does not yet account the optional SSA capture. Do not move
    // this owner through materialization or a ranked-projector wrapper here.
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}
