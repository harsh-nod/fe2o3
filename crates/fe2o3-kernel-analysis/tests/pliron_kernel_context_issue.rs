use dialect_kernel::{
    CanonicalIdentityAttr, DIALECT_NAME, KernelContextIssueOp, KernelContextType, ReturnOp,
    SourceCoordinateAttr, register_dialect,
};
use fe2o3_kernel_analysis::{
    PlironIrPreservationErrorV1, derive_pliron_ir_structural_identity_v1,
    require_pliron_ir_structural_identity_preserved_v1, run_pliron_ranked_bounds_check_v1,
};
use pliron::{
    builtin::{ops::FuncOp, types::FunctionType},
    context::Context,
    dialect::DialectName,
    op::Op,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(
        &mut context,
        &DialectName::try_new(DIALECT_NAME).expect("valid kernel dialect name"),
    )
    .expect("register kernel dialect");
    context
}

fn context_function(context: &mut Context, issuance: u8) -> FuncOp {
    let function = FuncOp::new(
        context,
        "context_entry".try_into().expect("valid function name"),
        FunctionType::get(context, vec![], vec![]),
    );
    let context_type = KernelContextType::get(
        context,
        "context_entry",
        CanonicalIdentityAttr::from_bytes([1; 32]),
        CanonicalIdentityAttr::from_bytes([2; 32]),
        CanonicalIdentityAttr::from_bytes([3; 32]),
    );
    let issue = KernelContextIssueOp::new(
        context,
        context_type,
        CanonicalIdentityAttr::from_bytes([4; 32]),
        SourceCoordinateAttr::new(0, 0, 0),
        CanonicalIdentityAttr::from_bytes([5; 32]),
        CanonicalIdentityAttr::from_bytes([6; 32]),
        CanonicalIdentityAttr::from_bytes([7; 32]),
        CanonicalIdentityAttr::from_bytes([8; 32]),
        CanonicalIdentityAttr::from_bytes([issuance; 32]),
    );
    let ret = ReturnOp::new(context);
    let entry = function.get_entry_block(context);
    issue.get_operation().insert_at_back(entry, context);
    ret.get_operation().insert_at_back(entry, context);
    function
}

#[test]
fn authenticated_kernel_context_issue_is_in_the_closed_ranked_language() {
    let context = &mut setup();
    let function = context_function(context, 9);

    assert!(run_pliron_ranked_bounds_check_v1(context, &function).is_clean());
    let identity = derive_pliron_ir_structural_identity_v1(context, &function)
        .expect("the exact context issuance participates in structural identity");
    assert!(identity.exactly_matches(&identity));
}

#[test]
fn context_provenance_replays_exactly_and_mutation_invalidates_identity() {
    let original_context = &mut setup();
    let original = context_function(original_context, 9);
    let replay_context = &mut setup();
    let replay = context_function(replay_context, 9);

    let preserved = require_pliron_ir_structural_identity_preserved_v1(
        original_context,
        &original,
        replay_context,
        &replay,
    )
    .expect("an independently rebuilt exact issuance must replay byte-for-byte");
    assert!(preserved.exactly_matches(
        &derive_pliron_ir_structural_identity_v1(replay_context, &replay).expect("replay identity")
    ));

    let hostile_context = &mut setup();
    let hostile = context_function(hostile_context, 10);
    let error = require_pliron_ir_structural_identity_preserved_v1(
        original_context,
        &original,
        hostile_context,
        &hostile,
    )
    .expect_err("substituting context provenance must invalidate exact identity");
    assert!(matches!(
        error,
        PlironIrPreservationErrorV1::IdentityChanged(_)
    ));
    assert!(error.to_string().contains("component attributes"));
}
