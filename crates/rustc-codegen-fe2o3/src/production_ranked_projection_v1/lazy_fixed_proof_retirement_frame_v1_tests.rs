use super::*;

#[test]
fn retirement_inert_owner_is_static_and_contains_no_resource_lifetime() {
    fn require_static<T: 'static>() {}
    require_static::<RetiredLazyProofPayloadsV1>();
    require_static::<SideOwners>();
    require_static::<RetiredAssertionCacheV1>();
}
#[test]
fn retirement_bridge_frame_selects_original_owner_closure_result_and_unwind() {
    type O = LazyFixedProofOwnerV1<'static, 'static, 'static>;
    type P = RetiredLazyProofPayloadsV1;
    type F = [usize; 7];
    type R = [u8; 13];
    let rows = transfer_rows::<F, R>().unwrap();
    let locals = size_of::<(
        FixedGuardInputsV1<'static, 'static>,
        &mut Prep<'static, 'static>,
        &mut Option<P>,
        F,
        usize,
        EmptySlot<'static>,
        Result<O>,
        O,
        Caught<R>,
        P,
        Result<R>,
        Panic,
    )>();
    let independently_typed = locals
        .checked_add(2 * size_of::<R>())
        .unwrap()
        .checked_add(2 * size_of::<Result<R>>())
        .unwrap();
    assert_eq!(rows[1], independently_typed);
    assert_eq!(rows.len(), 21);
}
#[test]
fn retirement_proof_move_frame_selects_every_independently_owned_field() {
    type P = RetiredLazyProofPayloadsV1;
    let rows = transfer_rows::<(), ()>().unwrap();
    let locals = size_of::<(
        SemanticAssertProofsV1<'static>,
        AssertionGraphV1<'static>,
        Option<ProjectedLoopCfgV1>,
        AssertionTableV1<'static, u8>,
        AssertionTableV1<'static, Vec<usize>>,
        AssertionTableV1<'static, bool>,
        AssertionTableV1<'static, Option<ScalarAssignmentSiteV1>>,
        Vec<Vec<usize>>,
        Option<StatementDefinitionIndexV1>,
        AssertionCacheV1<'static>,
        AssertionCacheV1<'static>,
        AssertionResourcesV1<'static>,
        P,
    )>();
    assert_eq!(
        rows[6],
        locals + 2 * size_of::<P>() + 2 * size_of::<Result<P>>()
    );
}
#[test]
fn retirement_exact_selected_row_sum_and_checked_arithmetic_have_no_fixed_allowance() {
    let rows = transfer_rows::<[usize; 23], [u8; 31]>().unwrap();
    let independent = rows
        .into_iter()
        .try_fold(0usize, |n, row| n.checked_add(row))
        .unwrap();
    assert_eq!(
        transfer_frame::<[usize; 23], [u8; 31]>().unwrap(),
        independent
    );
    assert!(frame::<usize>(usize::MAX).is_err());
    assert!(sum(&[usize::MAX, 1]).is_err());
}
#[test]
fn retirement_owned_table_variants_move_pointer_capacity_and_values() {
    let rows = vec![3usize, 5, 7];
    let pointer = rows.as_ptr();
    let capacity = rows.capacity();
    let retired = retire_table(AssertionTableV1::Owned(rows)).unwrap();
    assert_eq!(retired.as_ptr(), pointer);
    assert_eq!(retired.capacity(), capacity);
    assert_eq!(retired, vec![3, 5, 7]);
    assert!(retire_table(AssertionTableV1::Borrowed(&[3usize, 5, 7])).is_none());
}
#[test]
fn retirement_bridge_source_keeps_admission_before_owner_and_install_before_resumed_panic() {
    let source = include_str!("lazy_fixed_proof_retirement_v1.rs");
    let body = source
        .split("fn with_input")
        .nth(1)
        .unwrap()
        .split("pub(in crate::production_ranked_projection_v1) fn with_retired_lazy_proof_v1")
        .next()
        .unwrap();
    let reserve = body.find("resources.reserve_storage(bytes)?").unwrap();
    let new = body
        .find("LazyFixedProofOwnerV1::new(input, resources)?")
        .unwrap();
    let caught = body.find("let outcome = catch_unwind").unwrap();
    let installed = body.find("reserved.install(retire_owner(owner))").unwrap();
    let resumed = body.find("resume_unwind(payload)").unwrap();
    assert!(reserve < new && new < caught && caught < installed && installed < resumed);
    let terminal = &body[installed..];
    assert!(!terminal.contains("reserve_storage"));
    assert!(!terminal.contains("release_storage"));
    assert!(!terminal.contains("available("));
}
