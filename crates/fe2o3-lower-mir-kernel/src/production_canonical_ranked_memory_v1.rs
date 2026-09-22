/// Entry-carrier coverage only, never a current pointer-provenance proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCanonicalRankedArgumentCoverageV1 {
    /// Source node is retained although it has no physical ABI component.
    Zero,
    /// Half-open physical signature component interval.
    Components {
        /// First signature slot.
        first: usize,
        /// Exclusive signature slot.
        end: usize,
    },
    /// One actual parameter value and signature slot.
    Parameter {
        /// Signature slot.
        slot: usize,
        /// Actual graph value.
        value: ValueId,
    },
    /// Structural child of an atomic carrier, not a separately passed parameter.
    WithinAtomicParameter {
        /// Carrier signature slot.
        slot: usize,
        /// Actual carrier value.
        value: ValueId,
    },
}
fn cr_argument_coverage_v1(
    node: ProductionArgumentNodeV1<'_>,
) -> ProductionCanonicalRankedArgumentCoverageV1 {
    use ProductionCanonicalRankedArgumentCoverageV1 as Out;
    match node.coverage() {
        ProductionArgumentCoverageV1::Zero => Out::Zero,
        ProductionArgumentCoverageV1::Components { first, end } => Out::Components { first, end },
        ProductionArgumentCoverageV1::Parameter(p) => Out::Parameter {
            slot: p.slot(),
            value: p.value(),
        },
        ProductionArgumentCoverageV1::WithinAtomicParameter(p) => Out::WithinAtomicParameter {
            slot: p.slot(),
            value: p.value(),
        },
    }
}
/// One complete structural source argument node, including zero-sized envelopes.
/// Source type declarations retain pointer metadata/layout; no pointee walk or
/// current dynamic extent is inferred from an entry declaration.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionCanonicalRankedArgumentV1 {
    association: usize,
    source: u32,
    adjusted: Option<u32>,
    ty: SemanticTypeIdV1,
    ownership: SemanticSourceArgumentOwnershipV1,
    local: Option<SemanticLocalIdV1>,
    local_offset: usize,
    path: std::ops::Range<usize>,
    ignored: bool,
    coverage: ProductionCanonicalRankedArgumentCoverageV1,
}
impl ProductionCanonicalRankedArgumentV1 {
    /// Complete root-qualified function association.
    pub const fn association(&self) -> usize {
        self.association
    }
    /// Original logical source argument ordinal.
    pub const fn source_argument(&self) -> u32 {
        self.source
    }
    /// Adjusted ABI ordinal, absent for an enclosing RustCall tuple.
    pub const fn adjusted_argument(&self) -> Option<u32> {
        self.adjusted
    }
    /// Exact source node type, resolved against this scope's source type table.
    pub const fn semantic_type(&self) -> SemanticTypeIdV1 {
        self.ty
    }
    /// Source argument ownership, not derived runtime alias freedom.
    pub const fn source_ownership(&self) -> SemanticSourceArgumentOwnershipV1 {
        self.ownership
    }
    /// Bound semantic entry local when a physical or packed local exists.
    pub const fn local(&self) -> Option<SemanticLocalIdV1> {
        self.local
    }
    /// Whether the genuine correspondence retains an ignored local binding.
    pub const fn is_ignored_local(&self) -> bool {
        self.ignored
    }
    /// Actual physical entry coverage; not permission to access memory.
    pub const fn coverage(&self) -> ProductionCanonicalRankedArgumentCoverageV1 {
        self.coverage
    }
}
struct CrArgumentRowsV1<'a> {
    rows: Vec<ProductionCanonicalRankedArgumentV1>,
    paths: Vec<ProductionArgumentProjectionV1>,
    associations: Vec<std::ops::Range<usize>>,
    frames: Vec<Option<ProductionHelperLocalFrameV1<'a>>>,
}
fn cr_with_arguments_v1<'w, R>(
    owner: &ProductionPreRankedKirOwnerV1,
    group: &CanonicalCallGroupV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl for<'s> FnOnce(
        &mut ProductionArgumentViewV1<'s, 'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    with_parameter_correspondence_v1(
        owner.semantic_ssa.source_semantic(),
        group.function.source,
        group.function.canonical.function,
        group.parameters(),
        budget,
        run,
    )
}
fn cr_argument_snapshot_v1(
    association: usize,
    node: ProductionArgumentNodeV1<'_>,
    path: std::ops::Range<usize>,
) -> ProductionCanonicalRankedArgumentV1 {
    ProductionCanonicalRankedArgumentV1 {
        association,
        source: node.source_argument(),
        adjusted: node.adjusted_argument(),
        ty: node.semantic_type(),
        ownership: node.source().source_ownership(),
        local: node.local_binding().map(|row| row.0),
        local_offset: node
            .local_binding()
            .map_or(0, |(_, local)| node.source_path().len() - local.len()),
        path,
        ignored: node.ignored_local_binding().is_some(),
        coverage: cr_argument_coverage_v1(node),
    }
}
fn cr_build_arguments_v1<'a>(
    owner: &'a ProductionPreRankedKirOwnerV1,
    calls: &ProductionCanonicalCallsV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<CrArgumentRowsV1<'a>> {
    let (mut count, mut projections) = (0usize, 0usize);
    for group in &calls.groups {
        cr_with_arguments_v1(owner, group, budget, |view| {
            view.visit_nodes_with_budget_v1(|node, budget| {
                budget.charge_work(2)?;
                count = argument_sum_v1(&[count, 1])?;
                projections = argument_sum_v1(&[projections, node.source_path().len()])?;
                Ok(())
            })
        })?;
    }
    // All retained backing precedes BOTH correspondence and traversal scratch.
    let mut out = CrArgumentRowsV1 {
        rows: cr_vec_v1(count, budget)?,
        paths: cr_vec_v1(projections, budget)?,
        associations: cr_vec_v1(calls.groups.len(), budget)?,
        frames: cr_vec_v1(calls.groups.len(), budget)?,
    };
    for (association, group) in calls.groups.iter().enumerate() {
        let frame = if group.function.source.role == SemanticKirFunctionRoleV1::InternalHelper {
            owner.helper_memory_view_v1(association, budget)?
        } else {
            None
        };
        cr_push_v1(&mut out.frames, frame, budget)?;
        let start = out.rows.len();
        cr_with_arguments_v1(owner, group, budget, |view| {
            view.visit_nodes_with_budget_v1(|node, budget| {
                budget.charge_work(argument_sum_v1(&[2, node.source_path().len()])?)?;
                if out.rows.len() == count
                    || node.source_path().len() > projections - out.paths.len()
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let first = out.paths.len();
                out.paths.extend_from_slice(node.source_path());
                out.rows.push(cr_argument_snapshot_v1(
                    association,
                    node,
                    first..out.paths.len(),
                ));
                Ok(())
            })
        })?;
        cr_push_v1(&mut out.associations, start..out.rows.len(), budget)?;
    }
    if out.rows.len() != count || out.paths.len() != projections {
        return Err(cr_invalid_v1("argument traversal count"));
    }
    Ok(out)
}
fn cr_check_arguments_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    calls: &ProductionCanonicalCallsV1<'_>,
    rows: &CrArgumentRowsV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<()> {
    if rows.associations.len() != calls.groups.len() || rows.frames.len() != calls.groups.len() {
        return Err(cr_invalid_v1("argument association roster"));
    }
    let (mut next, mut path) = (0usize, 0usize);
    for (association, group) in calls.groups.iter().enumerate() {
        let expected = if group.function.source.role == SemanticKirFunctionRoleV1::InternalHelper {
            owner.helper_memory_view_v1(association, budget)?
        } else {
            None
        };
        budget.charge_work(6)?;
        let same = match (&rows.frames[association], &expected) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                std::ptr::eq(a.source(), b.source())
                    && std::ptr::eq(a.function(), b.function())
                    && std::ptr::eq(a.allocations(), b.allocations())
                    && std::ptr::eq(a.accesses(), b.accesses())
                    && std::ptr::eq(a.control(), b.control())
                    && std::ptr::eq(a.edge_bindings(), b.edge_bindings())
            }
            _ => false,
        };
        if !same {
            return Err(cr_invalid_v1("helper memory/control custody"));
        }
        let start = next;
        cr_with_arguments_v1(owner, group, budget, |view| {
            view.visit_nodes_with_budget_v1(|node, budget| {
                budget.charge_work(argument_sum_v1(&[13, node.source_path().len()])?)?;
                let actual = rows
                    .rows
                    .get(next)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let end = argument_sum_v1(&[path, node.source_path().len()])?;
                // Independently compare every typed field; do not call the builder
                // or its snapshot function during admission.
                if actual.association != association
                    || actual.source != node.source_argument()
                    || actual.adjusted != node.adjusted_argument()
                    || actual.ty != node.semantic_type()
                    || actual.ownership != node.source().source_ownership()
                    || actual.local != node.local_binding().map(|row| row.0)
                    || actual.local_offset
                        != node
                            .local_binding()
                            .map_or(0, |(_, local)| node.source_path().len() - local.len())
                    || actual.path != (path..end)
                    || actual.ignored != node.ignored_local_binding().is_some()
                    || rows.paths.get(path..end) != Some(node.source_path())
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let same = match (actual.coverage, node.coverage()) {
                    (
                        ProductionCanonicalRankedArgumentCoverageV1::Zero,
                        ProductionArgumentCoverageV1::Zero,
                    ) => true,
                    (
                        ProductionCanonicalRankedArgumentCoverageV1::Components {
                            first: a,
                            end: b,
                        },
                        ProductionArgumentCoverageV1::Components { first, end },
                    ) => (a, b) == (first, end),
                    (
                        ProductionCanonicalRankedArgumentCoverageV1::Parameter { slot, value },
                        ProductionArgumentCoverageV1::Parameter(p),
                    )
                    | (
                        ProductionCanonicalRankedArgumentCoverageV1::WithinAtomicParameter {
                            slot,
                            value,
                        },
                        ProductionArgumentCoverageV1::WithinAtomicParameter(p),
                    ) => (slot, value) == (p.slot(), p.value()),
                    _ => false,
                };
                if !same {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                next += 1;
                path = end;
                Ok(())
            })
        })?;
        if rows.associations[association] != (start..next) {
            return Err(cr_invalid_v1("argument range"));
        }
    }
    if next != rows.rows.len() || path != rows.paths.len() {
        return Err(cr_invalid_v1("extra argument metadata"));
    }
    Ok(())
}
