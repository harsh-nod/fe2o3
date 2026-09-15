//! Runs inside both actual cached-AMD source callbacks. All positive root and
//! entry identities come from the live importer, not synthetic receipt strings.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionKernelContextLoweringInputV1, ProductionScopedMatrixSourceSessionV1 as Session,
    ProductionSemanticKirErrorV1 as LowerError, ProductionTransposeOwnedSourceUsesV1 as Uses,
};

fn reason(result: std::result::Result<(), LowerError>, expected: &'static str) {
    let error = result.expect_err(expected);
    assert!(
        matches!(error, LowerError::Unsupported { detail, .. } if detail == expected),
        "{error:?}"
    );
}

pub(in super::super) fn check(
    contexts: &AuthenticatedProductionKernelContextsV1,
    owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
) {
    let [root] = contexts.roots.as_ref() else {
        panic!("one actual registered source root");
    };
    let entry = root.entry_transfer.map(|transfer| {
        transfer
            .checked_ssa_relation(owner, root.selected_root, 1_048_576)
            .unwrap()
    });
    // SAME production typed driver. A positive must pass the authenticated
    // before_transpose_publish path and post-Publish descendant rejection.
    super::check(contexts, root, owner, entry.as_ref(), 1_048_576)
        .expect("live transpose source, complete SSA roster and exact typed pre-Publish endpoint");
    let mut work = 1_048_576;
    let flows = contexts
        .transpose_source
        .as_ref()
        .unwrap()
        .rebind(owner, root.selected_root, &mut work)
        .unwrap()
        .into_flows(owner)
        .unwrap();
    let [flow] = flows.as_slice() else {
        panic!("one actual outer call instance");
    };
    let mut input = ProductionKernelContextLoweringInputV1::new(
        root.selected_root,
        contexts.frontend_unit_identity,
        root.kernel_marker_identity,
        contexts.target_brand_identity,
        root.launch_brand_identity,
        root.issuance_identity,
    );
    if let Some(transfer) = root.entry_transfer {
        input = input.with_entry_transfer(transfer);
    }
    let exact = || {
        Uses::new(
            &flow.issue_partition,
            &flow.matrix_subgroup,
            &flow.matrix_epoch,
            &flow.workgroup,
            &flow.workgroup_borrows,
        )
    };
    let run =
        |uses| Session::check_transpose_owned_uses(owner, &input, entry.as_ref(), uses, 1_048_576);
    reason(
        run(vec![]),
        "transpose typed source batch omitted a Publish occurrence",
    );
    reason(
        run(vec![exact(), exact()]),
        "transpose typed Publish roster is changed or consumed twice",
    );
    reason(
        run(vec![Uses::new(
            &flow.issue_partition,
            &flow.matrix_subgroup,
            &flow.matrix_epoch,
            &flow.workgroup,
            &flow.workgroup_borrows[..flow.workgroup_borrows.len() - 1],
        )]),
        "transpose typed source changed its exact flow or shared-use roster",
    );

    // A real SourceUse from another expanded callee is not the outer/helper
    // parameter, even though it belongs to this same owner and root query.
    reason(
        run(vec![Uses::new(
            &flow.issue_partition,
            &flow.closure_return,
            &flow.matrix_epoch,
            &flow.workgroup,
            &flow.workgroup_borrows,
        )]),
        "transpose typed source changed its exact flow or shared-use roster",
    );
    reason(
        run(vec![Uses::new(
            &flow.issue_partition,
            &flow.matrix_subgroup,
            &flow.matrix_subgroup,
            &flow.workgroup,
            &flow.workgroup_borrows,
        )]),
        "transpose typed source changed its exact parameter transfer",
    );

    let wrong_root = ProductionKernelContextLoweringInputV1::new(
        SemanticFunctionIdV1::from_index(u32::MAX),
        contexts.frontend_unit_identity,
        root.kernel_marker_identity,
        contexts.target_brand_identity,
        root.launch_brand_identity,
        root.issuance_identity,
    );
    assert!(matches!(
        Session::check_transpose_owned_uses(
            owner,
            &wrong_root,
            entry.as_ref(),
            [exact()],
            1_048_576
        ),
        Err(LowerError::CorrespondenceMismatch)
    ));
    let zero = Session::check_transpose_owned_uses(owner, &input, entry.as_ref(), [exact()], 0)
        .unwrap_err();
    assert!(
        matches!(
            zero,
            LowerError::ResourceLimit {
                resource: fe2o3_lower_mir_kernel::ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 1,
                limit: 0
            }
        ),
        "{zero:?}"
    );
}
