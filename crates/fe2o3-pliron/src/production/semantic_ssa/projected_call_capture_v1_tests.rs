use super::*;
use crate::{
    ProductionSemanticSsaEventRoleV1 as CapturedRole,
    ProductionSemanticSsaOccurrenceSiteV1 as CapturedSite,
    ProductionSemanticSsaOperandRoleV1 as CapturedOperand,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

#[test]
fn projected_call_capture_keeps_the_actual_pre_call_address_use() {
    let mut owner = admitted_projected_call_owner();
    let source = owner.source_semantic().functions().as_ptr();
    let identity = owner.identity();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let storage = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let view = owner.occurrences_v1().unwrap();
    assert_eq!(view.function_count(), 2);
    let rows = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
    assert!(std::ptr::eq(rows.owner(), &owner));
    assert_eq!(rows.events().len(), 2);
    let value = SsaValueV1::Definition(SsaDefinitionIdV1::new(0));
    let expected = [
        (
            CapturedSite::Statement {
                block: SsaBlockIdV1::new(0),
                statement: 0,
            },
            CapturedOperand::Destination,
            CapturedRole::DestinationDefine,
            SsaEventV1::Define(variable(2)),
            SsaResolvedEventV1::Define {
                variable: variable(2),
                value,
            },
        ),
        (
            CapturedSite::Terminator {
                block: SsaBlockIdV1::new(1),
            },
            CapturedOperand::CallDestinationAddress,
            CapturedRole::ProjectionIndexUse(0),
            SsaEventV1::Use(variable(2)),
            SsaResolvedEventV1::Use {
                variable: variable(2),
                value,
            },
        ),
    ];
    for (row, (site, operand, role, event, resolved)) in rows.events().iter().zip(expected) {
        assert_eq!(row.ordinal(), 0);
        assert_eq!(
            (row.site(), row.operand(), row.role(), row.event()),
            (site, operand, role, event)
        );
        assert!(row.is_reachable());
        assert!(row.is_promoted());
        assert_eq!(row.resolved(), Some(resolved));
    }
    // The projected destination contributes no aggregate read or whole-local edge definition.
    assert!(rows.edge_definitions().is_empty());
    assert!(rows.entry_definitions().is_empty());
    assert!(rows.elisions().is_empty());
    assert_eq!(rows.constants().len(), 1);
    let constant = &rows.constants()[0];
    assert_eq!(
        constant.site(),
        CapturedSite::Statement {
            block: SsaBlockIdV1::new(0),
            statement: 0
        }
    );
    assert_eq!(constant.operand(), CapturedOperand::RvalueOperand(0));
    assert_eq!(constant.next_event(), 0);
    assert_eq!(constant.ty(), SemanticTypeIdV1::from_index(1));
    assert_eq!(rows.successors().len(), 2);
    assert_eq!(
        rows.successors()[0].id(),
        fe2o3_mir_model::SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0)
    );
    assert_eq!(
        rows.successors()[0].edge(),
        test_edge(SemanticEdgeRoleV1::Goto, 1)
    );
    assert_eq!(
        rows.successors()[1].id(),
        fe2o3_mir_model::SsaEdgeIdV1::new(SsaBlockIdV1::new(1), 0)
    );
    assert_eq!(
        rows.successors()[1].edge(),
        test_edge(SemanticEdgeRoleV1::CallReturn, 2)
    );
    let helper = view.function(SemanticFunctionIdV1::from_index(1)).unwrap();
    assert!(helper.events().is_empty());
    assert!(helper.entry_definitions().is_empty());
    assert!(helper.edge_definitions().is_empty());
    assert_eq!(owner.source_semantic().functions().as_ptr(), source);
    assert_eq!(owner.identity(), identity);
    owner.verify_replay().unwrap();
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}
