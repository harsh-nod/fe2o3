use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBasicBlockV1, SemanticBlockIdentityV1, SemanticTerminatorV1,
};
use global_enum_transport_v1::tests::{build_owner, build_owner_from_function};

#[test]
fn empty_goto_census_successful_private_owner_has_no_diagnostic_or_changed_reads() {
    let owner = build_owner();
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let reads = analysis.run().unwrap();
    assert!(!reads.is_empty());
    assert!(analysis.observation.empty_goto_census.is_none());
    let published = PrivateScalarReads::for_root(&owner, root).unwrap();
    assert_eq!(published.reads, reads);
    assert!(published.observation.completed);
    assert!(published.observation.empty_goto_census.is_none());
}

#[test]
fn empty_goto_census_private_owner_raw_block_failure_publishes_no_reads_or_fuel() {
    let base = build_owner();
    let original = &base.source_semantic().functions()[0];
    let mut blocks = original.blocks().to_vec();
    while blocks.len() <= MAX_RANKED_BOUNDS_BLOCKS {
        let mut identity = [247; 32];
        identity[24..].copy_from_slice(&(blocks.len() as u64).to_be_bytes());
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256(identity),
                original.source(),
                vec![],
                SemanticTerminatorV1::new(original.source(), SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        );
    }
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let owner = build_owner_from_function(function);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let body_before = view.body().clone();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let old_preflight_work = analysis.conditions.as_ref().unwrap().work_units()
        + view.body().locals().len()
        + view.body().blocks().len()
        + view
            .body()
            .blocks()
            .iter()
            .map(|b| b.statements().len() + 1)
            .sum::<usize>();
    assert!(old_preflight_work < MAX_WORK);
    assert!(analysis.run().is_err());
    assert_eq!(analysis.observation.phase, Phase::CfgPreflight);
    assert_eq!(
        analysis.observation.failure,
        Some(Failure::BlockLimit {
            blocks: view.body().blocks().len(),
            limit: MAX_RANKED_BOUNDS_BLOCKS,
        })
    );
    assert_eq!(
        analysis.observation.remaining_work,
        MAX_WORK - old_preflight_work
    );
    assert_eq!(analysis.observation.admitted_reads, 0);
    assert!(!analysis.observation.completed);
    assert!(!analysis.observation.work_exhausted);
    let census = analysis.observation.empty_goto_census.as_ref().unwrap();
    assert!(format!("{census:?}").contains("source_only_not_ranked_eligibility"));
    let published = PrivateScalarReads::for_root(&owner, root).unwrap();
    assert!(published.reads.is_empty());
    assert!(published.observation.empty_goto_census.is_some());
    assert_eq!(view.body(), &body_before);
}
