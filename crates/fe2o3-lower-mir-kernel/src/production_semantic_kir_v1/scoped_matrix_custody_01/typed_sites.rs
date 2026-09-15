//! Exact source/footer pairing; SSA uses cannot be paired solely by type/value.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticOwnedSourceCallSiteV1, SemanticTransposeOwnedFlowV1,
};
use fe2o3_pliron::{
    ProductionSemanticSsaSourceSiteV1 as SourceSite,
    ProductionSemanticSsaSourceUseV1 as SourceUse,
};

pub(super) fn call<'a>(
    view: &'a SemanticExpandedRootV1,
    block: u32,
) -> Result<&'a SemanticDirectCallV1> {
    match view
        .body()
        .blocks()
        .get(block as usize)
        .ok_or_else(mismatch)?
        .terminator()
        .kind()
    {
        SemanticTerminatorKindV1::Call(call) => Ok(call),
        _ => Err(reject(
            "transpose typed occurrence is not its actual source call",
        )),
    }
}

fn source_site(
    view: &SemanticExpandedRootV1,
    use_: &SourceUse<'_>,
) -> Result<(SemanticCallInstanceIdV1, SemanticOwnedSourceCallSiteV1)> {
    source_site_at(view, use_.site())
}

fn source_site_at(
    view: &SemanticExpandedRootV1,
    site: SourceSite,
) -> Result<(SemanticCallInstanceIdV1, SemanticOwnedSourceCallSiteV1)> {
    let origin = view
        .block_origins()
        .get(site.block().index() as usize)
        .ok_or_else(mismatch)?;
    Ok((
        origin.instance(),
        SemanticOwnedSourceCallSiteV1 {
            function: origin.function(),
            block: origin.block(),
        },
    ))
}

pub(super) fn pair<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    view: &'a SemanticExpandedRootV1,
    graph: &mut Graph<'a>,
    uses: &ProductionTransposeOwnedSourceUsesV1<'a, '_>,
) -> Result<(&'a SemanticTransposeOwnedFlowV1, SemanticCallInstanceIdV1)> {
    let query = owner
        .source_query_for_root(view.root(), view.body())
        .map_err(|_| mismatch())?;
    for use_ in [uses.partition, uses.subgroup, uses.epoch] {
        graph.charge(1)?;
        if !use_.belongs_to(&query) {
            return Err(mismatch());
        }
    }
    graph.charge(1)?;
    if !uses.workgroup.belongs_to(&query) {
        return Err(mismatch());
    }
    for use_ in uses.borrows {
        graph.charge(1)?;
        if !use_.belongs_to(&query) {
            return Err(mismatch());
        }
    }
    let (outer, publish) = source_site_at(view, uses.workgroup.site())?;
    let mut selected = None;
    for row in owner.source_semantic().transpose_owned_flows() {
        graph.charge(1)?;
        if row.sites().publish == publish && selected.replace(row).is_some() {
            return Err(reject(
                "transpose typed source has duplicate Publish footer rows",
            ));
        }
    }
    let row = selected.ok_or_else(|| reject("transpose typed Publish has no exact footer row"))?;
    if source_site(view, uses.partition)? != (outer, row.sites().issue)
        || source_site(view, uses.subgroup)? != (outer, row.sites().matrix_call)
        || source_site(view, uses.epoch)? != (outer, row.sites().matrix_call)
        || uses.borrows.len() != row.workgroup_borrows().len()
    {
        return Err(reject(
            "transpose typed source changed its exact flow or shared-use roster",
        ));
    }
    for (use_, expected) in uses.borrows.iter().zip(row.workgroup_borrows()) {
        graph.charge(1)?;
        if source_site(view, use_)? != (outer, *expected) {
            return Err(reject(
                "transpose typed source changed an ordered shared-use occurrence",
            ));
        }
        argument(view, use_, 0)?;
    }
    argument(view, uses.partition, 0)?;
    argument_at(view, uses.workgroup.site(), uses.workgroup.operand(), 1)?;
    graph.charge(4)?;
    let local = view.local_origins()
        .get(uses.workgroup.local().index() as usize)
        .ok_or_else(mismatch)?;
    if local.instance() != outer || local.function() != publish.function
        || local.local() != row.sites().workgroup_local
    {
        return Err(reject("transpose typed Publish changed its original Workgroup binding"));
    }
    let helper = argument(view, uses.subgroup, 0)?.ok_or_else(mismatch)?;
    if argument(view, uses.epoch, 1)? != Some(helper) {
        return Err(mismatch());
    }
    let instance = view
        .instances()
        .get(helper.index() as usize)
        .ok_or_else(mismatch)?;
    if instance.parent() != Some(outer)
        || instance.function() != row.sites().closure_call.function
        || instance.call_block() != Some(row.sites().matrix_call.block)
    {
        return Err(reject(
            "transpose typed helper changed its original call instance",
        ));
    }
    Ok((row, helper))
}

pub(super) fn argument(
    view: &SemanticExpandedRootV1,
    use_: &SourceUse<'_>,
    argument: u32,
) -> Result<Option<SemanticCallInstanceIdV1>> {
    argument_at(view, use_.site(), use_.operand(), argument)
}

fn argument_at(
    view: &SemanticExpandedRootV1,
    site: SourceSite,
    selected_operand: &SemanticOperandV1,
    argument: u32,
) -> Result<Option<SemanticCallInstanceIdV1>> {
    let origin = view
        .block_origins()
        .get(site.block().index() as usize)
        .ok_or_else(mismatch)?;
    match (site.statement(), origin.terminator()) {
        (None, SemanticExpandedTerminatorOriginV1::Source) => {
            if call(view, site.block().index())?
                .arguments()
                .get(argument as usize)
                .is_none_or(|actual| !std::ptr::eq(actual, selected_operand))
            {
                return Err(mismatch());
            }
            Ok(None)
        }
        (Some(statement), SemanticExpandedTerminatorOriginV1::CallEntry { callee }) => {
            if origin.statements().get(statement as usize)
                != Some(&SemanticExpandedStatementOriginV1::ParameterTransfer { callee, argument })
            {
                return Err(reject(
                    "transpose typed source changed its exact parameter transfer",
                ));
            }
            let actual = view.body().blocks()[site.block().index() as usize]
                .statements()
                .get(statement as usize)
                .ok_or_else(mismatch)?;
            if !matches!(actual.kind(), SemanticStatementKindV1::Assign(a)
                if matches!(a.value().kind(), SemanticRvalueKindV1::Use(operand) if std::ptr::eq(operand, selected_operand)))
            {
                return Err(mismatch());
            }
            Ok(Some(callee))
        }
        _ => Err(reject(
            "transpose typed source use is not its original operand site",
        )),
    }
}
