use super::*;

#[test]
fn inventory_growth_charges_actual_capacity_and_old_new_overlap() {
    let mut values: Vec<(u32, u32, u32)> = Vec::new();
    let mut charged = 0;
    reserve(&mut values, 7, &mut |n| {
        charged += n;
        Ok(())
    })
    .unwrap();
    let words = std::mem::size_of::<(u32, u32, u32)>().div_ceil(std::mem::size_of::<usize>());
    assert_eq!(charged, values.capacity() * words);
    values.extend_from_slice(&[(1, 2, 3); 7]);
    let prior_capacity = values.capacity();
    let requested = prior_capacity + 3;
    reserve(&mut values, requested, &mut |n| {
        charged += n;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        charged,
        (prior_capacity + values.capacity() + values.len()) * words
    );
    assert_eq!(values, [(1, 2, 3); 7]);
}

#[test]
fn inventory_growth_rejects_before_allocation_when_shared_debit_fails() {
    let mut values = Vec::<u64>::new();
    let result = reserve(&mut values, 32, &mut |_| {
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual: 32,
            limit: 31,
        })
    });
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::ResourceLimit { limit: 31, .. })
    ));
    assert_eq!(values.capacity(), 0);
    assert!(values.is_empty());
}

#[test]
fn inventory_matrix_rows_retain_instances_source_order_and_duplicates() {
    let mut heads = [NONE; 3];
    let mut tails = [NONE; 3];
    let mut rows = Vec::new();
    let input = [(1, 3), (0, 4), (1, 6), (0, 9), (1, 10), (1, 10)];
    for &(instance, block) in &input {
        append(
            &mut heads,
            &mut tails,
            &mut rows,
            instance,
            block,
            &mut |_| Ok(()),
        )
        .unwrap();
    }
    for (instance, &head) in heads.iter().enumerate() {
        let actual: Vec<_> = MatrixBlocks {
            rows: &rows,
            next: head,
        }
        .collect();
        let expected: Vec<_> = input
            .iter()
            .filter(|&&(found, _)| found == instance)
            .map(|&(_, block)| block)
            .collect();
        assert_eq!(actual, expected);
    }
    let before = rows.clone();
    assert!(matches!(
        append(&mut heads, &mut tails, &mut rows, 3, 11, &mut |_| Ok(())),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(rows, before);
}

#[test]
fn inventory_matrix_cache_keeps_actual_owner_view_and_graph_binding() {
    use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1};
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        super::super::super::resource_tests::noop_semantic_owner(&["matrix_inventory"]),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let other = ProductionSemanticSsaOwnerV1::try_new(
        super::super::super::resource_tests::noop_semantic_owner(&["foreign_matrix_inventory"]),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let plan = owner.execution_plan_for_root(root).unwrap();
    let mut graph = Graph::new(view.body(), plan.plan(), 10_000).unwrap();
    let index = MatrixSites::new(&owner, view, &mut graph).unwrap();
    let instance = view.block_origins()[view.body().entry().index() as usize].instance();
    assert!(
        index
            .blocks(&owner, view, instance, &mut graph)
            .unwrap()
            .next()
            .is_none()
    );
    assert!(matches!(
        index.blocks(&other, view, instance, &mut graph),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    let other_view = other.execution_view_for_root(root).unwrap();
    let other_plan = other.execution_plan_for_root(root).unwrap();
    let mut other_graph = Graph::new(other_view.body(), other_plan.plan(), 10_000).unwrap();
    assert!(matches!(
        index.blocks(&owner, view, instance, &mut other_graph),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(matches!(
        MatrixSites::new(&owner, other_view, &mut graph),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}
