use super::*;

mod boundary_uses;

pub(in crate::collector::production_importer_v1) fn check_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: &crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
) {
    let roots = closure
        .collection
        .functions
        .iter()
        .filter_map(|function| {
            function
                .kernel_context_contract
                .as_ref()
                .and_then(|contract| contract.authenticated_source.as_ref())
                .map(|source| (function.instance, source))
        })
        .collect::<Vec<_>>();
    let [(root, source)] = roots.as_slice() else {
        panic!("one authenticated real Context root");
    };
    let helpers = closure
        .collection
        .functions
        .iter()
        .filter(|function| {
            crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
                tcx,
                function.instance,
            )
            .function()
            .as_bytes()
                == &source.logical_helper_identity()
        })
        .map(|function| function.instance)
        .collect::<Vec<_>>();
    let [helper] = helpers.as_slice() else {
        panic!("one authenticated logical helper");
    };
    let body = tcx.instance_mir(root.def);
    let issuers = body
        .basic_blocks
        .iter()
        .filter_map(|block| {
            let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                return None;
            };
            let instance = resolve_call(tcx, *root, func).unwrap()?;
            (crate::trusted_device_items::classify(tcx, instance.def_id())
                == Some(crate::trusted_device_items::TrustedDeviceItem::KernelContextIssue))
            .then_some(instance)
        })
        .collect::<Vec<_>>();
    let [issuer] = issuers.as_slice() else {
        panic!("one real trusted source issuer");
    };
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(issuer.def_id()).instantiate(tcx, issuer.args),
    );
    let context = signature.output();
    let mut work = usize::try_from(
        fe2o3_mir_model::semantic_mir_v1::SemanticMirLimitsV1::default()
            .limit(fe2o3_mir_model::semantic_mir_v1::SemanticMirResourceV1::ValidationWork),
    )
    .unwrap();
    let mut checked =
        ContextEntrySourceV1::check(tcx, *root, *helper, *issuer, context, 0, &mut work)
            .expect("actual erased entry must retain its exact source initializer binding");
    checked.replay(tcx, &mut work).unwrap();
    assert!(
        ContextEntrySourceV1::check(tcx, *root, *root, *issuer, context, 0, &mut work).is_err()
    );
    assert!(
        ContextEntrySourceV1::check(tcx, *root, *helper, *issuer, tcx.types.u32, 0, &mut work)
            .is_err()
    );
    assert!(
        ContextEntrySourceV1::check(tcx, *root, *helper, *issuer, context, 1, &mut work).is_err()
    );
    assert!(ContextEntrySourceV1::check(tcx, *root, *helper, *issuer, context, 0, &mut 0).is_err());
    let original = checked.issuer_local;
    checked.issuer_local = mir::RETURN_PLACE;
    assert!(checked.replay(tcx, &mut work).is_err());
    checked.issuer_local = original;
    checked.source_binding = checked.source_use;
    assert!(checked.replay(tcx, &mut work).is_err());
}
