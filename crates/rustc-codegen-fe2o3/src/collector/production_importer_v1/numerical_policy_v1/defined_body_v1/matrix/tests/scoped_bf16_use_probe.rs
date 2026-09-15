//! Test-only inspection of existing correspondence. No plan or graph is built.
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SemanticExpandedRootV1, SemanticExpandedStatementOriginV1};
use fe2o3_pliron::{
    ProductionSemanticSsaOwnerV1, ProductionSemanticSsaSourceQueryV1,
    ProductionSemanticSsaSourceSiteV1,
};

pub(super) fn inspect(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    binding: &SemanticExpandedDefinedCapabilityV1,
    load_block: u32,
    call: &SemanticDirectCallV1,
) {
    let Ok(query) = owner.source_query_for_root(view.root(), view.body()) else {
        eprintln!("bf16-use-probe: wrong retained owner/body");
        return;
    };
    // Diagnostic ceiling only: this cannot publish evidence or refund a session.
    let mut remaining = 65_536usize;
    let mut charge = || match remaining.checked_sub(1) {
        Some(next) => {
            remaining = next;
            true
        }
        None => false,
    };
    eprintln!(
        "bf16-use-probe root={} load_bb={} bind_call={} bind_instance={} callee_return={} destination={}",
        view.root().index(),
        load_block,
        binding.expanded_call_block().index(),
        binding.callee_instance().index(),
        binding.callee_return().index(),
        binding.destination().local().index(),
    );
    for (block, (data, origin)) in view
        .body()
        .blocks()
        .iter()
        .zip(view.block_origins())
        .enumerate()
    {
        if !charge() {
            break;
        }
        for (statement, (source, marker)) in data
            .statements()
            .iter()
            .zip(origin.statements())
            .enumerate()
        {
            if !charge() {
                break;
            }
            let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
                continue;
            };
            let site = ProductionSemanticSsaSourceSiteV1::new(
                SemanticBlockIdV1::from_index(block as u32),
                Some(statement as u32),
            );
            match marker {
                SemanticExpandedStatementOriginV1::ReturnTransfer { callee }
                    if *callee == binding.callee_instance() =>
                {
                    if let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() {
                        show(&query, "bind-return", site, operand, &mut charge);
                    }
                }
                SemanticExpandedStatementOriginV1::ParameterTransfer { callee, argument }
                    if *callee == binding.callee_instance() =>
                {
                    eprintln!("bf16-use-probe bind-parameter ordinal={argument}");
                    if let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() {
                        show(&query, "bind-actual", site, operand, &mut charge);
                    }
                }
                SemanticExpandedStatementOriginV1::Source { .. }
                    if origin.instance() == binding.callee_instance()
                        && assignment.destination().local() == binding.callee_return() =>
                {
                    if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() {
                        for (index, operand) in aggregate.operands().iter().take(2).enumerate() {
                            eprintln!("bf16-use-probe bind-aggregate ordinal={index}");
                            show(&query, "bind-formal", site, operand, &mut charge);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for (index, operand) in call.arguments().iter().take(2).enumerate() {
        eprintln!("bf16-use-probe load-argument ordinal={index}");
        show(
            &query,
            "load-reference",
            ProductionSemanticSsaSourceSiteV1::new(SemanticBlockIdV1::from_index(load_block), None),
            operand,
            &mut charge,
        );
    }
    eprintln!("bf16-use-probe remaining={remaining}; diagnostic only");
}

fn show<'a>(
    query: &ProductionSemanticSsaSourceQueryV1<'a>,
    label: &str,
    site: ProductionSemanticSsaSourceSiteV1,
    operand: &'a SemanticOperandV1,
    charge: &mut impl FnMut() -> bool,
) {
    let summary = match operand {
        SemanticOperandV1::Copy(place) => (
            "Copy",
            Some(place.local().index()),
            place.projections().len(),
        ),
        SemanticOperandV1::Move(place) => (
            "Move",
            Some(place.local().index()),
            place.projections().len(),
        ),
        SemanticOperandV1::Constant(_) => ("Constant", None, 0),
    };
    let result = query
        .operand_use(site, operand, charge)
        .map(|usage| (usage.value(), usage.event_range(), usage.agreeing_uses()));
    eprintln!(
        "bf16-use-probe {label} bb={} stmt={:?} kind={} local={:?} projections={} ty={} retained={result:?}",
        site.block().index(),
        site.statement(),
        summary.0,
        summary.1,
        summary.2,
        operand.ty().index(),
    );
}
