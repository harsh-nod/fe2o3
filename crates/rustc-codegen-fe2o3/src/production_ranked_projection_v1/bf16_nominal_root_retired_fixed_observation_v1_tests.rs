//! Component/source controls only. Actual source custody is exercised by the
//! separate genuine observer; no raw boolean fixture grants source authority.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn expected<T>(locals: usize) -> Result<usize> {
    let returned = size_of::<T>().checked_mul(2).ok_or_else(arithmetic)?;
    let result = size_of::<Result<T>>()
        .checked_mul(2)
        .ok_or_else(arithmetic)?;
    locals
        .checked_add(returned)
        .and_then(|n| n.checked_add(result))
        .ok_or_else(arithmetic)
}

#[test]
fn retirement_connector_fresh_owner_has_one_empty_slot_and_namespace() {
    let pending = PendingActualRootPrefixIndicesV1::new();
    assert!(slot_vacant_v1(&pending.retired_fixed_proof));
    assert!(pending.arguments.vacant());
    assert!(pending.prefix.entry_operations.is_empty());
    assert_eq!(pending.prefix.next_value, 0);
    assert_eq!(pending.frame_credits, 0);
    assert!(pending.ledger.is_none() && !pending.started && !pending.completed);
}
#[test]
fn retirement_connector_physical_owner_is_lifetime_free() {
    fn static_data<T: 'static>() {}
    static_data::<RetiredLazyProofPayloadsV1>();
    static_data::<PendingActualRootPrefixIndicesV1>();
}
#[test]
fn retirement_connector_slot_rows_match_closed_type_inventory() {
    assert_eq!(slot_rows().unwrap(), expected_slot_rows().unwrap());
    assert_eq!(
        slot_construction_frame_v1().unwrap(),
        expected_slot_rows().unwrap().iter().sum::<usize>()
    );
}
#[test]
fn retirement_connector_private_rows_match_closed_type_inventory() {
    assert_eq!(
        continuation_rows::<(), ()>().unwrap(),
        expected_continuation_rows::<(), ()>().unwrap()
    );
    assert_eq!(
        continuation_rows::<[u8; 4097], [u8; 8193]>().unwrap(),
        expected_continuation_rows::<[u8; 4097], [u8; 8193]>().unwrap()
    );
}
#[test]
fn retirement_connector_compatibility_rows_preserve_noncopy_result_and_consumer_sizes() {
    type NonCopy = Vec<u8>;
    assert_eq!(
        compatibility_rows::<NonCopy, NonCopy>().unwrap(),
        expected_compatibility_rows::<NonCopy, NonCopy>().unwrap()
    );
    assert_eq!(
        compatibility_entry_frame_v1::<NonCopy, NonCopy>().unwrap(),
        expected_compatibility_rows::<NonCopy, NonCopy>()
            .unwrap()
            .iter()
            .sum::<usize>()
    );
}
#[test]
fn retirement_connector_observation_rows_match_closed_type_inventory() {
    assert_eq!(
        observation_rows::<(), ()>().unwrap(),
        expected_observation_rows::<(), ()>().unwrap()
    );
    assert_eq!(
        observation_rows::<[u8; 4097], [u8; 8193]>().unwrap(),
        expected_observation_rows::<[u8; 4097], [u8; 8193]>().unwrap()
    );
}
#[test]
fn retirement_connector_added_frames_do_not_hide_large_captures_or_results() {
    for (small, large) in [
        (
            continuation_frame_v1::<(), ()>().unwrap(),
            continuation_frame_v1::<[u8; 4097], [u8; 8193]>().unwrap(),
        ),
        (
            compatibility_entry_frame_v1::<(), ()>().unwrap(),
            compatibility_entry_frame_v1::<[u8; 4097], [u8; 8193]>().unwrap(),
        ),
        (
            observation_entry_frame_v1::<(), ()>().unwrap(),
            observation_entry_frame_v1::<[u8; 4097], [u8; 8193]>().unwrap(),
        ),
    ] {
        assert!(large >= small + 8193 + 2 * 4097);
    }
}
#[test]
fn retirement_connector_frame_arithmetic_refuses_overflow() {
    assert!(call_frame::<()>(usize::MAX).is_err());
    assert!(sum(&[usize::MAX, 1]).is_err());
    assert!(expected::<()>(usize::MAX).is_err());
}
#[test]
fn retirement_connector_assembly_delta_is_separate_and_exact() {
    type R = [u8; 17];
    type F = [u8; 31];
    let expected = 8192
        + size_of::<PendingActualRootPrefixIndicesV1>()
        + construction_frame_v1().unwrap()
        + slot_construction_frame_v1().unwrap()
        + size_of::<ActualRootPrefixIndicesV1<'static>>()
        + size_of::<ActualSelectedInputsV1<'static>>()
        + size_of::<ActualRootAssemblyPartsV1<'static>>()
        + size_of::<ActualRootGuardedAccessesV1<'static>>()
        + size_of::<ActualRootReferenceOriginsV1<'static>>()
        + 4 * size_of::<F>()
        + 4 * size_of::<Result<R>>();
    assert_eq!(assembly_frame::<R, F>().unwrap(), expected);
}
#[test]
fn retirement_connector_join_rejects_each_independent_mismatch() {
    let mut work = Work::new(1000);
    let budget = Budget::new(&mut work, 1000);
    let expected = (
        &budget as *const Budget<'_> as usize,
        budget.work_ledger_identity_v1(),
    );
    assert!(require_join_v1(true, true, expected, Some(expected), true, false).is_ok());
    let candidates = [
        (false, true, Some(expected), true, false),
        (true, false, Some(expected), true, false),
        (true, true, None, true, false),
        (
            true,
            true,
            Some((expected.0.wrapping_add(1), expected.1)),
            true,
            false,
        ),
        (true, true, Some(expected), false, false),
        (true, true, Some(expected), true, true),
    ];
    for (cfg, rich, observed, belongs, denied) in candidates {
        assert!(matches!(require_join_v1(cfg, rich, expected, observed, belongs, denied),
            Err(Error::CanonicalAssertions(crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Accounting)))));
    }
}
#[test]
fn retirement_connector_join_rejects_distinct_work_ledger() {
    let mut first = Work::new(1000);
    let mut second = Work::new(1000);
    let first_budget = Budget::new(&mut first, 1000);
    let second_budget = Budget::new(&mut second, 1000);
    let expected = (
        &first_budget as *const Budget<'_> as usize,
        first_budget.work_ledger_identity_v1(),
    );
    let foreign = (expected.0, second_budget.work_ledger_identity_v1());
    assert!(expected.1 != foreign.1);
    assert!(require_join_v1(true, true, expected, Some(foreign), true, false).is_err());
}
#[test]
fn retirement_connector_join_does_not_clear_original_denial() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 1000);
    let expected = (
        &budget as *const Budget<'_> as usize,
        budget.work_ledger_identity_v1(),
    );
    assert!(budget.charge_work(1).is_err());
    let denial = budget.failed_work();
    assert!(require_join_v1(true, true, expected, Some(expected), true, denial.is_some()).is_err());
    assert_eq!(budget.failed_work(), denial);
    assert_eq!(budget.storage(), 0);
}
#[test]
fn retirement_connector_original_f1_still_accepts_noncopy_result() {
    // Compile-time signature control only; deliberately no fabricated context.
    fn accepts(
        context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
        checked: &CheckedBf16NominalCallV1<'_>,
        rich: &RichNominalSourceTablesV1<'_>,
        inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
        pending: &mut PendingActualRootPrefixIndicesV1,
    ) -> Result<Vec<u8>> {
        let captured = Vec::<u8>::new();
        context.with_actual_root_argument_initialization_v1(
            checked,
            rich,
            inputs,
            pending,
            move |_, _| Ok(captured),
        )
    }
    let _ = accepts;
}
fn tokens(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii_whitespace()).collect()
}
#[test]
fn retirement_connector_source_keeps_old_callback_and_terminal_before_admission() {
    let source = tokens(include_str!(
        "bf16_nominal_root_argument_initialization_v1.rs"
    ));
    assert!(source.contains("move|view,context,_retired|inspect(view,context)"));
    assert!(source.contains("FnOnce(ActualRootArgumentInitializationV1<'a>,&mutSelf)->Result<R>"));
    let fresh = source.find("letfresh=").unwrap();
    let terminal = source
        .find("pending.arguments.phase=Phase::Terminal;")
        .unwrap();
    let admit = source[terminal..].find("self.with_resources").unwrap() + terminal;
    assert!(fresh < terminal && terminal < admit);
    assert!(
        source[fresh..terminal].contains("retirement_slot_vacant_v1(&pending.retired_fixed_proof)")
    );
    assert!(source.contains("letPendingActualRootPrefixIndicesV1{graph,prefix,arguments,retired_fixed_proof,..}=pending;"));
}
#[test]
fn retirement_connector_old_assembly_rejects_slot_before_its_frame() {
    let source = tokens(include_str!("bf16_nominal_root_prefix_indices_v1.rs"));
    let guard = source
        .find("!argument_initialization::retirement_slot_vacant_v1(&pending.retired_fixed_proof)")
        .unwrap();
    let frame = source[guard..]
        .find("letframe=assembly_frame::<R,F>()?;")
        .unwrap()
        + guard;
    assert!(guard < frame);
    assert!(source.contains("retired_fixed_proof:Option<RetiredLazyProofPayloadsV1>"));
    assert!(source.contains("retired_fixed_proof:None,"));
}
#[test]
fn retirement_connector_source_joins_before_unchanged_retirement_bridge() {
    let source = tokens(include_str!(
        "bf16_nominal_root_retired_fixed_observation_v1.rs"
    ));
    // Select actual method's join, not the helper definition.
    let method = source
        .find("fnwith_actual_root_retired_fixed_proof_observation_v1")
        .unwrap();
    let actual_join = source[method..].find("require_join_v1(").unwrap() + method;
    let bridge = source[actual_join..]
        .find("with_retired_lazy_proof_v1(")
        .unwrap()
        + actual_join;
    assert!(actual_join < bridge);
    assert!(source[method..bridge].contains("letrich=cfg.source_tables().rich();"));
    assert!(
        source[actual_join..bridge].contains("std::ptr::eq(view.source.function,cfg.function())")
    );
    assert!(source[actual_join..bridge].contains("resources.original_ledger_v1()"));
    assert!(source[actual_join..bridge].contains("resources.has_denial()"));
    assert!(!source[method..].contains("with_fixture_v1"));
    assert!(!source[method..].contains("catch_unwind"));
    assert!(!source[method..].contains("release_storage"));
}
#[test]
fn retirement_connector_bridge_keeps_transfer_before_resume() {
    let source = tokens(include_str!("lazy_fixed_proof_retirement_v1.rs"));
    let install = source
        .find("reserved.install(retire_owner(owner));")
        .unwrap();
    let resume = source[install..]
        .find("Err(payload)=>resume_unwind(payload)")
        .unwrap()
        + install;
    assert!(install < resume);
    assert!(!source[install..resume].contains("has_denial"));
    assert!(!source[install..resume].contains("release_storage"));
}
#[test]
fn retirement_connector_original_f1_arrays_survive_outer_unwind_and_refund_order() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut work = Work::new(6);
    let mut budget = Budget::new(&mut work, 6 * size_of::<Option<u32>>());
    let mut owned = 0usize;
    let mut pending = PendingActualRootPrefixIndicesV1::new();
    pending.arguments.phase = Phase::Terminal;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        pending
            .arguments
            .initialize_payload(3, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        panic!("component only: after actual initializer mechanics");
    }));
    assert!(outcome.is_err());
    assert_eq!(pending.arguments.index_arguments.len(), 3);
    assert_eq!(pending.arguments.slice_arguments.len(), 3);
    assert_eq!(pending.arguments.next_argument, 1);
    assert!(slot_vacant_v1(&pending.retired_fixed_proof)); // no bridge was invoked here
    assert_eq!(budget.storage(), owned);
    drop(outcome);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn retirement_connector_actual_debit_exact_work_and_storage_boundary() {
    let mut work = Work::new(26);
    let mut budget = Budget::new(&mut work, 31);
    budget.reserve_storage(5).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let mut owned = 0;
    let pending = PendingActualRootPrefixIndicesV1::new();
    assert_eq!(
        admit_factory_frame_v1(&mut Prep::new(&mut budget, &mut owned), [3, 5, 7, 11]).unwrap(),
        26
    );
    assert_eq!(owned, 26);
    assert_eq!(budget.work(), 26);
    assert_eq!(budget.storage(), 31);
    assert!(ledger == budget.work_ledger_identity_v1());
    assert!(pending.arguments.vacant());
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 5);
}
#[test]
fn retirement_connector_actual_debit_short_work_never_charges_storage() {
    let mut work = Work::new(25);
    let mut budget = Budget::new(&mut work, 1000);
    let mut owned = 0;
    assert!(
        admit_factory_frame_v1(&mut Prep::new(&mut budget, &mut owned), [3, 5, 7, 11]).is_err()
    );
    assert!(budget.failed_work().is_some());
    assert!(budget.failed_storage().is_none());
    assert_eq!((owned, budget.storage()), (0, 0));
}
#[test]
fn retirement_connector_actual_debit_short_storage_retains_work() {
    let mut work = Work::new(26);
    let mut budget = Budget::new(&mut work, 25);
    let mut owned = 0;
    assert!(
        admit_factory_frame_v1(&mut Prep::new(&mut budget, &mut owned), [3, 5, 7, 11]).is_err()
    );
    assert!(budget.failed_work().is_none());
    assert!(budget.failed_storage().is_some());
    assert_eq!((budget.work(), owned, budget.storage()), (26, 0, 0));
}
#[test]
fn retirement_connector_actual_debit_sum_refuses_before_work() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 1000);
    let mut owned = 0;
    assert!(
        admit_factory_frame_v1(
            &mut Prep::new(&mut budget, &mut owned),
            [usize::MAX, 1, 0, 0]
        )
        .is_err()
    );
    assert_eq!((budget.work(), owned, budget.storage()), (0, 0, 0));
    assert!(budget.failed_work().is_none() && budget.failed_storage().is_none());
}

// Closed typed source inventory; this does not substitute for independent
// runtime/source-oracle review or genuine source-identity qualification.
fn expected_slot_rows() -> Result<[usize; SLOT_ROWS]> {
    Ok([
        // The new literal None and its constructor/return transfers. The whole
        // pending header is separately present in the original assembly row.
        expected::<Slot>(size_of::<Slot>())?,
        expected::<bool>(size_of::<(&Slot, bool)>())?,
        expected::<[usize; SLOT_ROWS]>(size_of::<([usize; SLOT_ROWS], Option<usize>)>())?,
        expected::<usize>(size_of::<(
            Result<[usize; SLOT_ROWS]>,
            [usize; SLOT_ROWS],
            &[usize],
        )>())?,
        expected::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        expected::<usize>(size_of::<(
            usize,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            Error,
        )>())?,
    ])
}

fn expected_continuation_rows<R, F>() -> Result<[usize; SEAM_ROWS]> {
    type P = PendingActualRootPrefixIndicesV1;
    Ok([
        expected::<R>(size_of::<(
            &mut Context,
            &CheckedBf16NominalCallV1<'static>,
            &RichNominalSourceTablesV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut P,
            FrameFn,
            F,
            Result<R>,
            LedgerId,
            bool,
        )>())?,
        // New continuation parameters, including exclusive slot loan.
        expected::<R>(size_of::<(F, View, &mut Context, &mut Slot, Result<R>)>())?,
        // Additional live admission fields, including the actual frame function
        // pointer and all checked component/total/error transfers.
        expected::<()>(size_of::<(
            &mut Prep<'static, 'static>,
            &mut P,
            &FrameFn,
            usize,
            usize,
            usize,
            usize,
            usize,
            Result<usize>,
            Result<usize>,
            Result<usize>,
            Result<usize>,
            [usize; 4],
            &[usize],
        )>())?,
        // Disjoint borrow in the graph continuation, not a second pending.
        expected::<R>(size_of::<(
            &mut Slot,
            &mut PendingArgumentProducersV1,
            &mut RootEntryPrefixV1,
            &mut Context,
            F,
            View,
            Result<R>,
        )>())?,
        expected::<usize>(size_of::<(FrameFn, Result<usize>)>())?,
        expected::<[usize; SEAM_ROWS]>(size_of::<([usize; SEAM_ROWS], Option<usize>)>())?,
        expected::<usize>(size_of::<(
            Result<[usize; SEAM_ROWS]>,
            [usize; SEAM_ROWS],
            &[usize],
        )>())?,
        expected::<usize>(size_of::<(
            [usize; 4],
            &[usize],
            Result<usize>,
            Option<usize>,
            Error,
        )>())?,
        expected::<usize>(size_of::<(
            &mut Prep<'static, 'static>,
            [usize; 4],
            &[usize],
            usize,
            Result<usize>,
        )>())?,
    ])
}

fn expected_compatibility_rows<R, F>() -> Result<[usize; COMPAT_ROWS]> {
    Ok([
        // Preserved public entry and its actual, non-Copy consumer.
        expected::<R>(size_of::<(
            &mut Context,
            &CheckedBf16NominalCallV1<'static>,
            &RichNominalSourceTablesV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut PendingActualRootPrefixIndicesV1,
            F,
            FrameFn,
            Result<R>,
        )>())?,
        // Compatibility adapter captures exactly F; the ignored slot is borrowed.
        expected::<R>(size_of::<(F, View, &mut Context, &mut Slot, Result<R>)>())?,
        expected::<[usize; COMPAT_ROWS]>(size_of::<([usize; COMPAT_ROWS], Option<usize>)>())?,
        expected::<usize>(size_of::<(
            Result<[usize; COMPAT_ROWS]>,
            [usize; COMPAT_ROWS],
            &[usize],
        )>())?,
        expected::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        expected::<usize>(size_of::<(
            usize,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            Error,
        )>())?,
    ])
}

fn expected_observation_rows<R, F>() -> Result<[usize; OBSERVATION_ROWS]> {
    type Cfg = NominalRootCfgSourceV1<'static>;
    type Rich = RichNominalSourceTablesV1<'static>;
    type Resources = Prep<'static, 'static>;
    Ok([
        expected::<R>(size_of::<(
            &mut Context,
            &CheckedBf16NominalCallV1<'static>,
            &Cfg,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut PendingActualRootPrefixIndicesV1,
            F,
            &Rich,
            FrameFn,
            Result<R>,
        )>())?,
        // Actual F1 continuation closure and its captured cfg + F.
        expected::<R>(size_of::<(
            &Cfg,
            &Rich,
            F,
            View,
            &mut Context,
            &mut Slot,
            Result<R>,
        )>())?,
        // Actual original-resource callback: all captured source/view/slot/F
        // fields, source getters and boolean/ledger result transfers are named.
        expected::<R>(size_of::<(
            &mut Resources,
            &Cfg,
            F,
            View,
            &mut Slot,
            &Rich,
            &SemanticFunctionDeclV1,
            &SemanticFunctionDeclV1,
            LedgerId,
            Option<LedgerId>,
            bool,
            bool,
            bool,
            bool,
            Result<()>,
            Result<R>,
        )>())?,
        // Bridge's actual callback captures View + F, not context/resources.
        expected::<R>(size_of::<(View, F, &mut Owner, Result<R>)>())?,
        expected::<()>(size_of::<(
            bool,
            bool,
            LedgerId,
            Option<LedgerId>,
            bool,
            bool,
            Result<()>,
            Error,
        )>())?,
        expected::<&NominalRootSourceTablesV1<'static>>(size_of::<&Cfg>())?,
        expected::<&Rich>(size_of::<&NominalRootSourceTablesV1<'static>>())?,
        expected::<&SemanticFunctionDeclV1>(size_of::<&Cfg>())?,
        expected::<&SemanticFunctionDeclV1>(size_of::<&Rich>())?,
        expected::<bool>(size_of::<(&Rich, LedgerId, bool)>())?,
        expected::<Option<LedgerId>>(size_of::<(
            &Resources,
            &&mut Budget<'static>,
            &Budget<'static>,
            LedgerId,
        )>())?,
        expected::<bool>(size_of::<(
            &Resources,
            &&mut Budget<'static>,
            Option<usize>,
            Option<usize>,
        )>())?,
        expected::<()>(size_of::<(&mut Resources, &mut &mut Budget<'static>, usize)>())?,
        expected::<[usize; OBSERVATION_ROWS]>(
            size_of::<([usize; OBSERVATION_ROWS], Option<usize>)>(),
        )?,
        expected::<usize>(size_of::<(
            Result<[usize; OBSERVATION_ROWS]>,
            [usize; OBSERVATION_ROWS],
            &[usize],
        )>())?,
        expected::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        expected::<usize>(size_of::<(
            usize,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            Error,
        )>())?,
    ])
}
