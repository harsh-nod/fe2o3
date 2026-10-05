use super::super::predicated_v88_tests::predicated_contract;
use super::*;
use fe2o3_kernel_descriptor::mixed_conditional_v86::*;

fn predicated_run<const WRONG_LAYOUT: bool>(
    work_limit: usize,
    storage_limit: usize,
    length: usize,
    mutation: usize,
) -> (
    std::result::Result<(), MixedWorkerV53PreparationError>,
    usize,
    usize,
    usize,
) {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    let bytes = predicated_contract(&table, false, |subjects, _, rows| match mutation {
        1 => subjects.descriptor_identity[0] ^= 1,
        2 => rows[2].formation_envelope = MixedIndexEnvelopeV26::LogicalExtent { argument: 1 },
        3 => {
            let row = &mut rows[0];
            let condition = row.output_guard.condition();
            row.output_guard = MixedAccessGuardV86::CfgEdge {
                edge: MixedEdgeV26 {
                    function: 2,
                    block: 4,
                    successor: 0,
                },
                condition,
            };
            row.path = MixedGuardPathV26::TrueEdge {
                source: 4,
                successor: 0,
                target: 5,
            };
            row.formation_envelope = MixedIndexEnvelopeV26::LogicalExtent { argument: 0 };
        }
        _ => (),
    });
    let contract = decode_mixed_contract_v86(&bytes, &mut free).unwrap();
    let input = [1, 2, 3];
    let mut output = [0; 3];
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    let result = mixed_preparation_v53::prepare_predicated_v89::<MixedKernel, _>(
        &table,
        &contract,
        Arguments::<WRONG_LAYOUT> {
            input: &input[..length],
            output: &mut output[..length],
            count: 9,
        },
        geometry(),
        0,
        1000,
        &mut budget,
    );
    let result = result.map(|(runtime, completion)| {
        assert!(matches!(runtime.invocation_binding(),
            fe2o3_runtime::Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 { contract_identity, .. }
                if contract_identity == *contract.identity()));
        assert_eq!(completion.buffers.len(), if length == 0 { 0 } else { 2 });
        assert!(budget.storage() > 0);
        drop((runtime, completion));
    });
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

#[test]
fn predicated_generated_preparation_reaches_actual_packer_with_empty_and_mixed_guards() {
    for length in [0, 3] {
        for mutation in [0, 3] {
            predicated_run::<false>(usize::MAX, usize::MAX, length, mutation)
                .0
                .unwrap();
        }
    }
}

#[test]
fn predicated_generated_preparation_refuses_layout_contract_and_formation_substitution() {
    for length in [0, 3] {
        assert!(matches!(
            predicated_run::<true>(usize::MAX, usize::MAX, length, 0).0,
            Err(MixedWorkerV53PreparationError::Arguments(_))
        ));
        for mutation in [1, 2] {
            assert!(matches!(
                predicated_run::<false>(usize::MAX, usize::MAX, length, mutation).0,
                Err(MixedWorkerV53PreparationError::Arguments(_))
            ));
        }
    }
}

#[test]
fn predicated_generated_preparation_keeps_original_typed_resource_source() {
    use crate::generated_kfd_arguments::conditional::GeneratedConditionalPremiseErrorV1 as Inner;
    let error = MixedWorkerV53PreparationError::arguments(Inner::from(
        MixedContractErrorV86::Resource(Resource::Accounting),
    ));
    let mut source: &(dyn std::error::Error + 'static) = &error;
    let mut depth = 0;
    while source.downcast_ref::<Resource>().is_none() {
        source = source.source().expect("original typed resource source");
        depth += 1;
        assert!(depth <= 4);
    }
    assert_eq!(
        source.downcast_ref::<Resource>(),
        Some(&Resource::Accounting)
    );
    assert_eq!(depth, 3);
}

#[test]
fn predicated_generated_preparation_has_exact_and_one_short_complete_resources() {
    let measured = predicated_run::<false>(usize::MAX, usize::MAX, 3, 0);
    measured.0.unwrap();
    let exact = predicated_run::<false>(measured.1, measured.3, 3, 0);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for work_short in [false, true] {
        let denied = predicated_run::<false>(
            measured.1 - usize::from(work_short),
            measured.3 - usize::from(!work_short),
            3,
            0,
        );
        let error = denied.0.expect_err("one-short preparation must refuse");
        let mut source: &(dyn std::error::Error + 'static) = &error;
        loop {
            if let Some(resource) = source.downcast_ref::<Resource>() {
                match (resource, work_short) {
                    (Resource::Work(error), true) => {
                        assert_eq!(error.actual(), measured.1);
                        assert_eq!(error.limit(), measured.1 - 1);
                    }
                    (Resource::Storage(error), false) => {
                        assert_eq!(error.actual(), measured.3);
                        assert_eq!(error.limit(), measured.3 - 1);
                    }
                    _ => panic!("unexpected resource: {resource:?}"),
                }
                break;
            }
            source = source.source().expect("resource chain");
        }
    }
}
