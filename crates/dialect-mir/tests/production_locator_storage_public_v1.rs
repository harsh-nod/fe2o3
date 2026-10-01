#![cfg(feature = "pliron")]

use std::mem::size_of;

use dialect_mir::pliron::{
    MirProductionBlockLocatorV1, MirProductionFunctionLocatorV1, MirProductionModuleLocatorV1,
    MirProductionSemanticSha256V1, MirProductionStatementLocatorV1, MirProductionSuccessorArcV1,
    MirProductionTerminatorLocatorV1,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticBlockIdV1, SemanticFunctionIdV1};

#[test]
fn production_locator_storage_public_api_uses_original_constructor_capacities() {
    let statements: Vec<MirProductionStatementLocatorV1> = Vec::with_capacity(9);
    let successors: Vec<MirProductionSuccessorArcV1> = Vec::with_capacity(13);
    let mut expected = statements.capacity() * size_of::<MirProductionStatementLocatorV1>()
        + successors.capacity() * size_of::<MirProductionSuccessorArcV1>();
    let block = MirProductionBlockLocatorV1::try_new(
        SemanticBlockIdV1::from_index(0),
        statements,
        MirProductionTerminatorLocatorV1::try_new(successors).unwrap(),
    )
    .unwrap();
    let mut blocks = Vec::with_capacity(5);
    blocks.push(block);
    expected += blocks.capacity() * size_of::<MirProductionBlockLocatorV1>();
    let function = MirProductionFunctionLocatorV1::try_new(
        SemanticFunctionIdV1::from_index(0),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    let mut functions = Vec::with_capacity(3);
    functions.push(function);
    expected += functions.capacity() * size_of::<MirProductionFunctionLocatorV1>();
    let module = MirProductionModuleLocatorV1::try_new(
        MirProductionSemanticSha256V1::from_sha256([0x17; 32]),
        functions,
    )
    .unwrap();
    let mut heap = 0usize;
    let mut visits = 0usize;
    module
        .visit_logical_retained_heap_v1(&mut |n, w| {
            heap = heap
                .checked_add(n.checked_mul(w).ok_or("multiply")?)
                .ok_or("add")?;
            visits = visits.checked_add(1).ok_or("visits")?;
            Ok::<_, &'static str>(())
        })
        .unwrap();
    assert_eq!(heap, expected);
    assert_eq!(visits, 5);
    let mut full = 0usize;
    module
        .visit_logical_retained_storage_v1(&mut |n, w| {
            full = full
                .checked_add(n.checked_mul(w).ok_or("multiply")?)
                .ok_or("add")?;
            Ok::<_, &'static str>(())
        })
        .unwrap();
    assert_eq!(full, expected + size_of::<MirProductionModuleLocatorV1>());
}

#[test]
fn production_locator_storage_public_api_propagates_root_refusal() {
    let block = MirProductionBlockLocatorV1::try_new(
        SemanticBlockIdV1::from_index(0),
        Vec::new(),
        MirProductionTerminatorLocatorV1::try_new(Vec::new()).unwrap(),
    )
    .unwrap();
    let function = MirProductionFunctionLocatorV1::try_new(
        SemanticFunctionIdV1::from_index(0),
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();
    let module = MirProductionModuleLocatorV1::try_new(
        MirProductionSemanticSha256V1::from_sha256([0; 32]),
        vec![function],
    )
    .unwrap();
    let mut calls = 0;
    let refusal = module.visit_logical_retained_heap_v1(&mut |_, _| {
        calls += 1;
        Err("stop")
    });
    assert_eq!(refusal, Err("stop"));
    assert_eq!(calls, 1);
}
