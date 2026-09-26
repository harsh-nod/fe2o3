use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_lower_mir_kernel::{
    ProductionSourceOptimizationErrorV18 as Source, ProductionSourceOwnedViewErrorV18 as ViewError,
};
use fe2o3_pliron::{
    KirCheckedNeutralOptimizationErrorV1 as Adoption, KirNeutralOptimizationErrorV18 as Observation,
};
use std::error::Error;

#[test]
fn optimized_pipeline_source_error_retains_original_payload() {
    let error =
        source_optimization_error_v18(Source::Source(ViewError::Binding("original source")));
    assert!(matches!(
        error,
        ProductionPipelineError::SourceOwnedEntrance(ViewError::Binding("original source"))
    ));
    assert!(error.to_string().contains("original source"));
    assert!(error.source().unwrap().is::<ViewError>());
}

#[test]
fn optimized_pipeline_observation_preserves_all_phase_variants() {
    let variants = [
        Observation::Bridge(fe2o3_pliron::KirBridgeErrorV18::Resource(
            Resource::Arithmetic,
        )),
        Observation::Execution(fe2o3_pliron::PlironOptimizationErrorV12::Resources(
            Resource::Accounting,
        )),
        Observation::Pass(fe2o3_pliron::PlironOptimizationErrorV1::GraphAccountingMismatch),
        Observation::Mapping(fe2o3_pliron::KirOptimizationMapErrorV12::Resources(
            Resource::Allocation,
        )),
        Observation::Resource(Resource::Arithmetic),
        Observation::Endpoint,
        Observation::Limit,
        Observation::Panicked,
    ];
    for (index, error) in variants.into_iter().enumerate() {
        let detail = error.to_string();
        let error = source_optimization_error_v18(Source::Observation(error));
        assert!(error.to_string().contains(&detail));
        assert!(error.source().unwrap().is::<Observation>());
        let ProductionPipelineError::SourceOptimizationObservation(error) = error else {
            panic!("observation lost its structured phase");
        };
        match (index, error) {
            (
                0,
                Observation::Bridge(fe2o3_pliron::KirBridgeErrorV18::Resource(
                    Resource::Arithmetic,
                )),
            )
            | (
                1,
                Observation::Execution(fe2o3_pliron::PlironOptimizationErrorV12::Resources(
                    Resource::Accounting,
                )),
            )
            | (
                2,
                Observation::Pass(fe2o3_pliron::PlironOptimizationErrorV1::GraphAccountingMismatch),
            )
            | (
                3,
                Observation::Mapping(fe2o3_pliron::KirOptimizationMapErrorV12::Resources(
                    Resource::Allocation,
                )),
            )
            | (4, Observation::Resource(Resource::Arithmetic))
            | (5, Observation::Endpoint)
            | (6, Observation::Limit)
            | (7, Observation::Panicked) => (),
            _ => panic!("observation payload changed"),
        }
    }
}

#[test]
fn optimized_pipeline_adoption_preserves_non_origin_payloads() {
    let variants = [
        Adoption::Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner),
        Adoption::Transition(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
            "actual output use",
        )),
        Adoption::Resource(Resource::Arithmetic),
        Adoption::OriginAccounting,
        Adoption::Panicked,
    ];
    for (index, error) in variants.into_iter().enumerate() {
        let detail = error.to_string();
        let error = source_optimization_error_v18(Source::Adoption(error));
        assert!(error.to_string().contains(&detail));
        assert!(
            error
                .source()
                .unwrap()
                .is::<Adoption<std::convert::Infallible>>()
        );
        let ProductionPipelineError::SourceOptimizationAdoption(error) = error else {
            panic!("adoption lost its structured phase");
        };
        match (index, error) {
            (
                0,
                Adoption::Inventory(
                    fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner,
                ),
            )
            | (
                1,
                Adoption::Transition(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
                    "actual output use",
                )),
            )
            | (2, Adoption::Resource(Resource::Arithmetic))
            | (3, Adoption::OriginAccounting)
            | (4, Adoption::Panicked) => (),
            _ => panic!("adoption payload changed"),
        }
    }
}

#[test]
fn optimized_pipeline_origin_error_moves_out_without_rewrapping() {
    let error = ProductionPipelineError::SimulationDebugMapCorrespondence("original phase payload");
    let original_display = error.to_string();
    let error = source_optimization_error_v18(Source::Adoption(Adoption::Origin(error)));
    assert!(matches!(
        error,
        ProductionPipelineError::SimulationDebugMapCorrespondence("original phase payload")
    ));
    assert_eq!(error.to_string(), original_display);
}
