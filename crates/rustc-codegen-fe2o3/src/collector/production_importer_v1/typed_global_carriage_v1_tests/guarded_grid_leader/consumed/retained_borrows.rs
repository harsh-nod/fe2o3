//! Failure-only bounded observation. No classification or event changes.
use fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionIdV1, SemanticOperandV1, SemanticRvalueKindV1,
    SemanticStatementKindV1, SemanticTerminatorKindV1,
};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

pub(super) fn report(
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    error: &ProductionSemanticKirErrorV1,
) {
    let ProductionSemanticKirErrorV1::RetainedLocalStorage { retained_locals, .. } = error else {
        return;
    };
    let view = owner.execution_view_for_root(root).expect("same observed root");
    let plan = owner.execution_plan_for_root(root).expect("same observed plan");
    // One total work counter and bounded borrowed-reference roster for this
    // test diagnostic. Truncation never changes the original failure.
    let mut remaining = 8192_usize;
    let mut references = Vec::new();
    for &(local, ty, _) in retained_locals.iter().take(8) {
        eprintln!("GRID_RETAINED owner={local} type={ty} local_origin={:?}",
            view.local_origins().get(local as usize));
    }
    for receipt in plan.guarded_grid_leader_results().iter().take(8) {
        eprintln!("GRID_RETAINED contract_types={:?} source={:?}",
            receipt.contract().types(), receipt.contract().source());
    }
    'borrows: for (bi, (block, origin)) in view.body().blocks().iter().zip(view.block_origins()).enumerate() {
        let Some(next) = remaining.checked_sub(1) else { break };
        remaining = next;
        for (si, statement) in block.statements().iter().enumerate() {
            let Some(next) = remaining.checked_sub(1) else { break 'borrows };
            remaining = next;
            let SemanticStatementKindV1::Assign(a) = statement.kind() else { continue };
            let SemanticRvalueKindV1::Borrow { kind, place } = a.value().kind() else { continue };
            if !retained_locals.iter().take(8).any(|r| r.0 == place.local().index()) { continue; }
            eprintln!("GRID_RETAINED borrow bb{bi}s{si} owner={} owner_type={} reference={} reference_type={} kind={kind:?} projection_count={} source_function={} source_instance={} source_block={} source_statement={:?}",
                place.local().index(), place.ty().index(), a.destination().local().index(),
                a.destination().ty().index(), place.projections().len(), origin.function().index(),
                origin.instance().index(), origin.block().index(), origin.statements().get(si));
            if references.len() < 16 { references.push(a.destination().local().index()); }
        }
    }
    let direct = |o: &SemanticOperandV1| match o {
        SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => Some(p.local().index()),
        SemanticOperandV1::Constant(_) => None,
    };
    let mut rows = 0_usize;
    'uses: for (bi, (block, origin)) in view.body().blocks().iter().zip(view.block_origins()).enumerate() {
        let Some(next) = remaining.checked_sub(1) else { break };
        remaining = next;
        for (si, statement) in block.statements().iter().enumerate() {
            let Some(next) = remaining.checked_sub(1) else { break 'uses };
            remaining = next;
            let SemanticStatementKindV1::Assign(a) = statement.kind() else { continue };
            let source = match a.value().kind() {
                SemanticRvalueKindV1::Use(o) => direct(o),
                SemanticRvalueKindV1::Load(load) => Some(load.source().local().index()),
                _ => None,
            };
            if source.is_none_or(|local| !references.contains(&local)) || rows == 32 { continue; }
            rows += 1;
            eprintln!("GRID_RETAINED use bb{bi}s{si} reference={source:?} destination={} destination_type={} source_function={} source_instance={} source_block={} source_statement={:?}",
                a.destination().local().index(), a.destination().ty().index(), origin.function().index(),
                origin.instance().index(), origin.block().index(), origin.statements().get(si));
            if references.len() < 16 && !references.contains(&a.destination().local().index()) {
                references.push(a.destination().local().index());
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
            for (ai, argument) in call.arguments().iter().enumerate() {
                let Some(next) = remaining.checked_sub(1) else { break 'uses };
                remaining = next;
                if direct(argument).is_some_and(|local| references.contains(&local)) && rows < 32 {
                    rows += 1;
                    eprintln!("GRID_RETAINED call bb{bi} arg={ai} callable={} source_function={} source_instance={} source_block={}",
                        call.callee().index(), origin.function().index(), origin.instance().index(), origin.block().index());
                }
            }
        }
    }
    eprintln!("GRID_RETAINED bounded_observation remaining={remaining} references={} rows={rows} source_plan_function={}",
        references.len(), plan.function().index());
}
