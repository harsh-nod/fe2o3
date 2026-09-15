//! Full classifier boundary, not separate per-edge allowances.
use super::*;

fn classify(
    source: &AdmittedInertSemanticMirV1,
    expansion: &SemanticCallExpansionV1,
    limit: usize,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    execution_sites(
        source,
        expansion,
        expansion.root(source.roots()[0]).unwrap(),
        limit,
    )
}

fn work_failure(error: ProductionSemanticSsaErrorV1, limit: usize) -> (&'static str, [usize; 11]) {
    let ProductionSemanticSsaErrorV1::BorrowFlowWork {
        stage,
        phase_work_units,
        remaining_work_units,
        requested_work_units,
        error,
        ..
    } = error
    else {
        panic!("expected the original borrow-flow resource error: {error:?}")
    };
    assert_eq!(
        *error,
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            required: limit + 1,
            limit,
        }
    );
    assert_eq!(
        phase_work_units.iter().sum::<usize>() + remaining_work_units,
        limit
    );
    assert!(requested_work_units > remaining_work_units);
    (stage, phase_work_units)
}

#[test]
fn shared_carrier_full_classification_one_short_keeps_one_work_owner_and_no_publication() {
    let source = fixture::admitted();
    let original_bytes = source.canonical_encoding().to_vec();
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let expected = classify(&source, &expansion, MAX_FLOW_WORK).unwrap();
    let original = fixture::original_borrows(view.body());
    assert_eq!(original.len(), 2);
    assert!(original.iter().all(|(site, _)| expected.contains(site)));

    // At most 19 full runs for the unchanged 262144-unit ceiling. Each run is
    // independent; within it the real production classifier owns one Budget.
    let (mut low, mut high) = (0, MAX_FLOW_WORK);
    while low < high {
        let middle = low + (high - low) / 2;
        match classify(&source, &expansion, middle) {
            Ok(sites) => {
                assert_eq!(sites, expected);
                high = middle;
            }
            Err(error) => {
                work_failure(error, middle);
                low = middle + 1;
            }
        }
    }
    let required = low;
    assert!(required > 1);
    assert_eq!(classify(&source, &expansion, required).unwrap(), expected);
    let (stage, work) = work_failure(
        classify(&source, &expansion, required - 1).unwrap_err(),
        required - 1,
    );
    assert_eq!(stage, "components");
    assert!(work[FlowWorkStage::Facts as usize] > 0);
    assert!(work[FlowWorkStage::Candidates as usize] > 0);
    assert!(work[FlowWorkStage::Uses as usize] > 0);
    assert!(work[FlowWorkStage::Components as usize] > 0);
    // Thus constructor/edge/use work has already run, yet the Result publishes
    // no partial sites. A retry cannot reuse a partially successful cache.
    assert_eq!(classify(&source, &expansion, required).unwrap(), expected);
    assert_eq!(source.canonical_encoding(), original_bytes.as_slice());
}
