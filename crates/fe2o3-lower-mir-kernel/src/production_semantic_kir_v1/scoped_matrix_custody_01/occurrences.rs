use super::*;

#[derive(Clone)]
pub(super) struct Bind {
    pub(super) binding: SemanticExpandedDefinedCapabilityV1,
    pub(super) record: SemanticPolicyMatrixBindV1,
    pub(super) site: Site,
    pub(super) returned: Site,
}

#[derive(Clone)]
pub(super) struct Narrow {
    pub(super) binding: SemanticExpandedDefinedCapabilityV1,
    pub(super) record: SemanticPolicyGfx950NarrowV1,
    pub(super) site: Site,
    pub(super) returned: Site,
    pub(super) projection: SemanticCallInstanceIdV1,
}

pub(super) struct Occurrences {
    pub(super) binds: BTreeMap<Site, Bind>,
    pub(super) narrows: BTreeMap<Site, Narrow>,
}

impl Occurrences {
    pub(super) fn new(
        owner: &ProductionSemanticSsaOwnerV1,
        view: &SemanticExpandedRootV1,
        context: &RootKernelContextLoweringV1,
        graph: &mut Graph<'_>,
        bindings: &[SemanticExpandedDefinedCapabilityV1],
    ) -> Result<Self> {
        if !owner
            .execution_view_for_root(context.selected_root)
            .is_some_and(|actual| std::ptr::eq(actual, view))
            || !std::ptr::eq(graph.body, view.body())
        {
            return Err(mismatch());
        }
        // Charge retained roster storage as logical words as well as scans.
        graph.charge(bindings.len().checked_mul(64).ok_or_else(mismatch)?)?;
        for binding in bindings {
            graph.charge(
                binding
                    .arguments()
                    .len()
                    .checked_mul(8)
                    .ok_or_else(mismatch)?,
            )?;
            for operand in binding.arguments() {
                if let SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) = operand {
                    graph.charge(p.projections().len().checked_mul(4).ok_or_else(mismatch)?)?;
                }
            }
            graph.charge(
                binding
                    .destination()
                    .projections()
                    .len()
                    .checked_mul(4)
                    .ok_or_else(mismatch)?,
            )?;
        }
        let mut returns = BTreeMap::new();
        let mut aggregates = BTreeMap::<SemanticCallInstanceIdV1, Vec<Site>>::new();
        let mut children =
            BTreeMap::<SemanticCallInstanceIdV1, Vec<SemanticCallInstanceIdV1>>::new();
        for (index, frame) in view.instances().iter().enumerate() {
            graph.charge(16)?;
            if let Some(parent) = frame.parent() {
                let id = view
                    .block_origins()
                    .get(frame.block_start() as usize)
                    .ok_or_else(mismatch)?
                    .instance();
                if id.index() as usize != index {
                    return Err(mismatch());
                }
                children.entry(parent).or_default().push(id);
            }
        }
        for (block, (body, origin)) in view
            .body()
            .blocks()
            .iter()
            .zip(view.block_origins())
            .enumerate()
        {
            graph.charge(1 + body.statements().len())?;
            if body.statements().len() != origin.statements().len() {
                return Err(mismatch());
            }
            for (statement, (value, marker)) in body
                .statements()
                .iter()
                .zip(origin.statements())
                .enumerate()
            {
                let SemanticStatementKindV1::Assign(a) = value.kind() else {
                    continue;
                };
                let site = Site {
                    block: block as u32,
                    statement: Some(statement as u32),
                    local: a.destination().local().index(),
                };
                if let SemanticExpandedStatementOriginV1::ReturnTransfer { callee } = marker {
                    graph.charge(24)?;
                    if returns.insert(*callee, site).is_some() {
                        // Only selected closed constructors require one Return.
                        returns.insert(
                            *callee,
                            Site {
                                statement: None,
                                ..site
                            },
                        );
                    }
                }
                if matches!(marker, SemanticExpandedStatementOriginV1::Source { .. })
                    && matches!(a.value().kind(), SemanticRvalueKindV1::Aggregate(_))
                {
                    graph.charge(24)?;
                    aggregates.entry(origin.instance()).or_default().push(site);
                }
            }
        }
        let mut result = Self {
            binds: BTreeMap::new(),
            narrows: BTreeMap::new(),
        };
        for binding in bindings.iter().cloned() {
            graph.charge(1)?;
            if binding.root() != view.root() {
                continue;
            }
            let (output, identity) = match binding.contract() {
                SemanticDefinedCapabilityContractV1::PolicyMatrixBind(r) => {
                    (r.types().bound, r.identity())
                }
                SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(r) => {
                    (r.types().narrowed, r.identity())
                }
                _ => continue,
            };
            if binding.expansion_identity() != owner.execution_expansion().identity()
                || binding.root_identity() != view.identity()
                || !global_capability_provenance_matches_v1(context, identity.provenance())
                || binding.destination().ty() != output
                || !binding.destination().projections().is_empty()
            {
                return Err(reject(
                    "scoped Matrix constructor changed its root, expansion or result",
                ));
            }
            let [site] = aggregates
                .get(&binding.callee_instance())
                .map(Vec::as_slice)
                .ok_or_else(mismatch)?
            else {
                return Err(reject(
                    "scoped Matrix constructor is not its one checked aggregate",
                ));
            };
            let site = *site;
            let returned = *returns
                .get(&binding.callee_instance())
                .filter(|s| s.statement.is_some())
                .ok_or_else(mismatch)?;
            let assignment = assignment(graph.body, site)?;
            if assignment.destination().local() != binding.callee_return()
                || assignment.destination().ty() != output
                || !assignment.destination().projections().is_empty()
            {
                return Err(mismatch());
            }
            check_return(view, returned, &binding)?;
            match binding.contract() {
                SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record) => {
                    let types = record.types();
                    if binding.arguments().len() != 2
                        || binding.callee_arguments().len() != 2
                        || !children
                            .get(&binding.callee_instance())
                            .is_none_or(Vec::is_empty)
                    {
                        return Err(mismatch());
                    }
                    let SemanticRvalueKindV1::Aggregate(a) = assignment.value().kind() else {
                        return Err(mismatch());
                    };
                    for (index, ty) in [types.matrix_reference, types.policy_reference]
                        .into_iter()
                        .enumerate()
                    {
                        if !matches!(a.operands().get(index), Some(SemanticOperandV1::Copy(p))
                            if p.local() == binding.callee_arguments()[index]
                                && p.ty() == ty && p.projections().is_empty())
                        {
                            return Err(mismatch());
                        }
                    }
                    graph.charge(128)?;
                    if result
                        .binds
                        .insert(
                            site,
                            Bind {
                                binding,
                                record,
                                site,
                                returned,
                            },
                        )
                        .is_some()
                    {
                        return Err(mismatch());
                    }
                }
                SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record) => {
                    if binding.arguments().len() != 1 || binding.callee_arguments().len() != 1 {
                        return Err(mismatch());
                    }
                    let [projection] = children
                        .get(&binding.callee_instance())
                        .map(Vec::as_slice)
                        .ok_or_else(mismatch)?
                    else {
                        return Err(mismatch());
                    };
                    let projection = *projection;
                    let frame = &view.instances()[projection.index() as usize];
                    if frame.function() != record.projection().function()
                        || frame.function_identity() != record.projection().source_identity()
                        || returns
                            .get(&projection)
                            .is_none_or(|site| site.statement.is_none())
                    {
                        return Err(reject(
                            "scoped Matrix narrowing lost its exact projection call and return",
                        ));
                    }
                    let SemanticRvalueKindV1::Aggregate(a) = assignment.value().kind() else {
                        return Err(mismatch());
                    };
                    if !matches!(a.operands().first(), Some(SemanticOperandV1::Copy(p))
                        if p.local() == binding.callee_arguments()[0]
                            && p.ty() == record.types().bound_reference && p.projections().is_empty())
                    {
                        return Err(mismatch());
                    }
                    graph.charge(160)?;
                    if result
                        .narrows
                        .insert(
                            site,
                            Narrow {
                                binding,
                                record,
                                site,
                                returned,
                                projection,
                            },
                        )
                        .is_some()
                    {
                        return Err(mismatch());
                    }
                }
                _ => return Err(mismatch()),
            }
        }
        Ok(result)
    }
}

pub(super) fn assignment(
    body: &SemanticFunctionDeclV1,
    site: Site,
) -> Result<&fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1> {
    let statement = site.statement.ok_or_else(mismatch)?;
    match body
        .blocks()
        .get(site.block as usize)
        .and_then(|b| b.statements().get(statement as usize))
        .map(|s| s.kind())
    {
        Some(SemanticStatementKindV1::Assign(a))
            if a.destination().local().index() == site.local =>
        {
            Ok(a)
        }
        _ => Err(mismatch()),
    }
}

fn check_return(
    view: &SemanticExpandedRootV1,
    site: Site,
    binding: &SemanticExpandedDefinedCapabilityV1,
) -> Result<()> {
    let origin = view
        .block_origins()
        .get(site.block as usize)
        .ok_or_else(mismatch)?;
    let a = assignment(view.body(), site)?;
    if origin.instance() != binding.callee_instance()
        || origin
            .statements()
            .get(site.statement.ok_or_else(mismatch)? as usize)
            != Some(&SemanticExpandedStatementOriginV1::ReturnTransfer {
                callee: binding.callee_instance(),
            })
        || origin.terminator()
            != (SemanticExpandedTerminatorOriginV1::CallReturn {
                callee: binding.callee_instance(),
            })
        || a.destination() != binding.destination()
        || !matches!(a.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
            if p.local() == binding.callee_return() && p.projections().is_empty()
                && p.ty() == binding.destination().ty())
    {
        return Err(reject(
            "scoped Matrix constructor return lost its checked occurrence",
        ));
    }
    Ok(())
}
