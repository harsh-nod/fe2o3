//! Invoked by the single-session ordinary-source callback, not an invented tcx.
use super::*;

pub(crate) fn check_actual_sealed_trait_chain_paths_v1(
    tcx: TyCtxt<'_>,
    primitive: DefId,
    scalar: Wave64ShuffleScalarV1,
) {
    let origin = reviewed_provider_semantic_definition_v1(tcx, primitive).unwrap();
    validate_definition(tcx, primitive, scalar, &origin).unwrap();
    let implementation = tcx.impl_of_assoc(primitive).unwrap();
    let mut parent = tcx.impl_trait_id(implementation);
    for (expected, reexport_spelling) in [
        (
            "fe2o3_device::collective::WorkgroupCollectiveElement",
            "fe2o3_device::WorkgroupCollectiveElement",
        ),
        (
            "fe2o3_device::collective::sealed::CollectiveElement",
            "fe2o3_device::CollectiveElement",
        ),
    ] {
        // Independently enumerate actual predicates and use their authenticated
        // definition observations, not the production selector's display text.
        let candidates: Vec<_> = tcx.explicit_super_predicates_of(parent)
            .iter_identity_copied()
            .filter_map(|(clause, _)| {
                let ClauseKind::Trait(predicate) = clause.kind().skip_binder() else {
                    return None;
                };
                let candidate = predicate.trait_ref.def_id;
                if candidate.krate != parent.krate {
                    return None;
                }
                let observed = reviewed_provider_semantic_definition_v1(tcx, candidate).unwrap();
                if observed.canonical_definition_path != expected {
                    return None;
                }
                validate_safe_execution_provider_definition_v1(&observed).unwrap();
                assert!(observed.provider == origin.provider);
                assert_eq!(observed.source_closure_identity, origin.source_closure_identity);
                assert_eq!(observed.cargo_metadata_build_observation, origin.cargo_metadata_build_observation);
                assert!(matches!(predicate.trait_ref.self_ty().kind(), TyKind::Param(parameter) if parameter.index == 0));
                Some(candidate)
            })
            .collect();
        let [actual] = candidates.as_slice() else {
            panic!("actual provider has no unique declared supertrait {expected}: {candidates:?}");
        };
        assert_eq!(
            exact_supertrait(tcx, parent, expected, &origin).unwrap(),
            *actual
        );
        assert!(exact_supertrait(tcx, parent, reexport_spelling, &origin).is_err());
        assert!(
            exact_supertrait(tcx, parent, "fe2o3_device::collective::WrongTrait", &origin).is_err()
        );
        eprintln!(
            "WAVE64 ACTUAL TRAIT structural={expected}; display={}",
            tcx.def_path_str(*actual)
        );
        parent = *actual;
    }
    assert_ne!(parent, tcx.impl_trait_id(implementation));
}
