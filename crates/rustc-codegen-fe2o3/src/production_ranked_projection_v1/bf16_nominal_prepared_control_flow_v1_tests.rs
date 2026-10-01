//! Accounting and API shape controls; no authenticated owner is fabricated.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 37;

#[test]
fn control_frame_has_checked_callback_and_result_extents() {
    assert!(frame::<()>(0).unwrap() >= 8192);
    assert_eq!(frame::<()>(19).unwrap() - frame::<()>(0).unwrap(), 4 * 19);
    assert!(matches!(
        frame::<()>(usize::MAX),
        Err(QueryError::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        plus(usize::MAX, 1),
        Err(QueryError::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        times(usize::MAX, 2),
        Err(QueryError::Resource(Resource::Arithmetic))
    ));
}
#[test]
fn checkpoint_accepts_only_original_slot_and_retained_floor() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let before = Checkpoint::take(&budget).unwrap();
    budget.reserve_storage(13).unwrap();
    budget.charge_work(7).unwrap();
    before.require(&budget, 13).unwrap();
    budget.release_storage(1).unwrap();
    assert!(matches!(
        before.require(&budget, 13),
        Err(QueryError::Resource(Resource::Accounting))
    ));
}
#[test]
fn checkpoint_detects_replaced_ledger_not_merely_equal_numbers() {
    let mut first = Work::new(LIMIT);
    let mut second = Work::new(LIMIT);
    let mut budget = Budget::new(&mut first, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let before = Checkpoint::take(&budget).unwrap();
    budget = Budget::new(&mut second, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        before.require(&budget, 0),
        Err(QueryError::Resource(Resource::Accounting))
    ));
}
#[test]
fn sticky_denial_cannot_open_a_control_scope() {
    let mut work = Work::new(1);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(budget.charge_work(2).is_err());
    assert!(matches!(
        Checkpoint::take(&budget),
        Err(QueryError::Resource(Resource::Accounting))
    ));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, 1);
    assert!(budget.reserve_storage(2).is_err());
    assert!(matches!(
        Checkpoint::take(&budget),
        Err(QueryError::Resource(Resource::Accounting))
    ));
}
#[test]
fn resource_and_original_static_projection_refusals_are_preserved() {
    assert!(matches!(
        query_error(ranked_projection_source_v1::resource(Resource::Accounting)),
        QueryError::Resource(Resource::Accounting)
    ));
    assert!(matches!(
        query_error(ProductionRankedProjectionErrorV1::Incomplete(
            "actual reason"
        )),
        QueryError::Unavailable("actual reason")
    ));
    assert!(matches!(
        query_error(ProductionRankedProjectionErrorV1::Unsupported(
            "other reason"
        )),
        QueryError::Unavailable("other reason")
    ));
}
#[test]
fn production_entry_is_original_owner_bound_and_normal_gate_stays_closed() {
    // Compile-time signature/loan guard only. This function is never called and
    // is not evidence of an authenticated constructor or real frontend run.
    fn signature<'g, 'w>(
        owner: &'g ProductionPreRankedKirOwnerV1,
        inventory: &CanonicalKirInventoryV1<'g>,
        root: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        budget: &mut Budget<'w>,
    ) -> Result<()> {
        with_nominal_prepared_control_flow_v1(
            owner,
            inventory,
            root,
            block,
            call,
            budget,
            |view, _| {
                let _ = view.effects().original().candidate().permutation();
                let _ = view.assertions().cfg().function();
                let _ = view.terminators().len();
                Ok(())
            },
        )
    }
    let _ = signature;
    assert!(matches!(
        bf16_nominal_call_routing_v1::require_defined_call_access_ready_v1(
            bf16_nominal_call_routing_v1::DefinedCallAccessRouteV1::NominalPending
        ),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            bf16_nominal_call_routing_v1::NOMINAL_PENDING_V1
        ))
    ));
}
