use super::super::borrowed_workgroup_01::{EpochOccurrence, checked_epoch_occurrences_v1};
use super::relation::{Row, scope_identity, select_record};
use super::resolve::{Resolver, Role};
use super::*;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Scope {
    MatrixOnly,
    MatrixAndBf16,
    TransposeOnly,
}

include!("typed_driver.rs");

/// Non-clonable source-custody work owner shared with typed memory consumers.
/// No row is a numerical, allocation, or read proof. The public input remains
/// inert until authenticated by the compiler's private frontend adapter.
pub struct ProductionScopedMatrixSourceSessionV1<'a> {
    pub(super) owner: &'a ProductionSemanticSsaOwnerV1,
    pub(super) view: &'a SemanticExpandedRootV1,
    pub(super) context: RootKernelContextLoweringV1,
    pub(super) graph: Graph<'a>,
    pub(super) occurrences: occurrences::Occurrences,
    pub(super) epochs: Vec<EpochOccurrence>,
    pub(super) bf16_rows: BTreeMap<u32, super::bf16::Row>,
    expected_bf16: Option<BTreeSet<u32>>,
}

impl<'a> ProductionScopedMatrixSourceSessionV1<'a> {
    /// Replays the retained owner and reuses its exact checked Context entry.
    /// All subsequent lane and sibling memory queries share this work allowance.
    pub fn new(
        owner: &'a ProductionSemanticSsaOwnerV1,
        input: &ProductionKernelContextLoweringInputV1,
        entry: Option<&ProductionKernelContextEntrySsaRelationV1<'a>>,
        max_work: usize,
    ) -> Result<Self> {
        Self::new_scoped(owner, input, entry, max_work, Scope::MatrixAndBf16)
    }

    // This entry returns only Matrix rows. It cannot publish a BF16 relation
    // or abandon a caller's pending BF16 query/consumption batch.
    pub(super) fn checked_matrix_uses(
        owner: &'a ProductionSemanticSsaOwnerV1,
        input: &ProductionKernelContextLoweringInputV1,
        entry: Option<&ProductionKernelContextEntrySsaRelationV1<'a>>,
        max_work: usize,
    ) -> Result<ProductionScopedMatrixUseRelationV1<'a>> {
        let mut session = Self::new_scoped(owner, input, entry, max_work, Scope::MatrixOnly)?;
        let rows = session.narrow_rows()?;
        Ok(ProductionScopedMatrixUseRelationV1 {
            owner: session.owner,
            view: session.view,
            rows,
        })
    }

    fn new_scoped(
        owner: &'a ProductionSemanticSsaOwnerV1,
        input: &ProductionKernelContextLoweringInputV1,
        entry: Option<&ProductionKernelContextEntrySsaRelationV1<'a>>,
        max_work: usize,
        scope: Scope,
    ) -> Result<Self> {
        owner
            .verify_replay()
            .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
        let view = owner
            .execution_view_for_root(input.selected_root())
            .ok_or_else(mismatch)?;
        let plan = owner
            .execution_plan_for_root(input.selected_root())
            .ok_or_else(mismatch)?;
        if plan.function_identity() != view.body().identity() {
            return Err(mismatch());
        }
        let context = checked_root_context_source_v1(owner, input, |ty| {
            match (input.entry_transfer, entry) {
                (None, None) => Ok(None),
                (Some(expected), Some(entry))
                    if expected == entry.input
                        && entry.matches_body(view.body())
                        && entry.context() == ty =>
                {
                    Ok(Some(entry.plan.clone()))
                }
                _ => Err(reject(
                    "scoped Matrix lost its exact checked Context entry transfer",
                )),
            }
        })?;
        let mut graph = Graph::new(view.body(), plan.plan(), max_work)?;
        // Source-query construction performs at most three root-index binary
        // searches and three binding checks; charge before borrowing its index.
        let roots = owner.execution_expansion().roots().len();
        graph.charge(3 * (1 + (usize::BITS - roots.leading_zeros()) as usize) + 3)?;
        let definition_source = owner.source_query_for_root(input.selected_root(), view.body())
            .map_err(|_| mismatch())?;
        let mut graph = graph.with_definition_source(&definition_source)?;
        // Header/source inventory work is charged once, before any use queries.
        for function in owner.source_semantic().functions() {
            graph.charge(1 + function.blocks().len())?;
        }
        let bindings = owner
            .execution_expansion()
            .defined_capability_bindings(owner.source_semantic())
            .map_err(|_| mismatch())?;
        let occurrences =
            occurrences::Occurrences::new(owner, view, &context, &mut graph, &bindings)?;
        let epochs = checked_epoch_occurrences_v1(owner, view, bindings)?;
        let mut expected_bf16 = (scope == Scope::MatrixAndBf16).then(BTreeSet::new);
        if let Some(expected_bf16) = &mut expected_bf16 {
            for (index, block) in view.body().blocks().iter().enumerate() {
                graph.charge(1)?;
                if matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                if matches!(owner.source_semantic().callables().get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { .. }, ..
                    })))
                    && graph.reaches(view.body().entry().index(), index as u32)?
                {
                    graph.charge(8)?;
                    expected_bf16.insert(index as u32);
                }
            }
        }
        Ok(Self {
            owner,
            view,
            context,
            graph,
            occurrences,
            epochs,
            bf16_rows: BTreeMap::new(),
            expected_bf16,
        })
    }

    pub(super) fn require_bf16_scope(&self) -> Result<()> {
        if self.expected_bf16.is_none() {
            return Err(reject(
                "Matrix-only source session cannot publish BF16 custody",
            ));
        }
        Ok(())
    }

    // Source-only sibling consumers receive the SAME graph/cache/remaining work.
    // This is intentionally not a public API for injecting a graph or a witness.
    pub(in crate::production_semantic_kir_v1) fn with_source_graph<T>(
        &mut self,
        consume: impl FnOnce(
            &'a ProductionSemanticSsaOwnerV1,
            &'a SemanticExpandedRootV1,
            &RootKernelContextLoweringV1,
            &mut Graph<'a>,
        ) -> Result<T>,
    ) -> Result<T> {
        consume(self.owner, self.view, &self.context, &mut self.graph)
    }

    // A BF16 sibling cannot select a partial roster or borrow a restricted
    // session's graph. Capturing source inputs does not mark a read consumed.
    pub(in crate::production_semantic_kir_v1) fn with_bf16_source_graph<T>(
        &mut self,
        consume: impl FnOnce(
            &'a ProductionSemanticSsaOwnerV1,
            &'a SemanticExpandedRootV1,
            &RootKernelContextLoweringV1,
            &mut Graph<'a>,
            &BTreeSet<u32>,
        ) -> Result<T>,
    ) -> Result<T> {
        self.require_bf16_scope()?;
        let expected = self.expected_bf16.as_ref().ok_or_else(mismatch)?;
        consume(self.owner, self.view, &self.context, &mut self.graph, expected)
    }

    /// Closes the complete source batch. Every reachable Global BF16 terminal
    /// needs a checked lane and once-only sibling consumer; omission rejects.
    /// The returned rows still cannot grant Global memory or numeric authority.
    pub fn finish(
        mut self,
        bf16_consumed: &[u32],
    ) -> Result<(
        ProductionScopedMatrixUseRelationV1<'a>,
        ProductionScopedBf16LaneRelationV1<'a>,
    )> {
        // A queried lane must be joined by its storage/geometry consumer before
        // the source batch can publish. This check is not a memory proof.
        self.require_bf16_scope()?;
        let expected_bf16 = self.expected_bf16.as_ref().ok_or_else(mismatch)?;
        self.graph.charge(1 + bf16_consumed.len())?;
        if self.bf16_rows.len() != expected_bf16.len()
            || !self.bf16_rows.keys().eq(expected_bf16.iter())
        {
            return Err(reject(
                "BF16 source session omitted a reachable Global load lane",
            ));
        }
        super::bf16::check_roster(&self.bf16_rows, bf16_consumed)?;
        let rows = self.narrow_rows()?;
        Ok((
            ProductionScopedMatrixUseRelationV1 {
                owner: self.owner,
                view: self.view,
                rows,
            },
            ProductionScopedBf16LaneRelationV1 {
                owner: self.owner,
                view: self.view,
                rows: self.bf16_rows,
            },
        ))
    }

    /// Source-only matrix custody from this same graph and remaining allowance.
    /// The non-clonable BF16 session and all pending reads remain owned here.
    pub fn matrix_uses_while_bf16_pending(&mut self) -> Result<ProductionScopedMatrixUseRelationV1<'a>> {
        self.require_bf16_scope()?;
        let rows = self.narrow_rows()?;
        Ok(ProductionScopedMatrixUseRelationV1 { owner: self.owner, view: self.view, rows })
    }

    fn narrow_rows(&mut self) -> Result<BTreeMap<u32, Row>> {
        let Self {
            owner,
            view,
            context,
            graph,
            occurrences,
            epochs,
            ..
        } = self;
        let (owner, view) = (*owner, *view);
        let mut rows = BTreeMap::new();
        for (block, body) in view.body().blocks().iter().enumerate() {
            graph.charge(1)?;
            let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) = owner
                .source_semantic()
                .callables()
                .get(call.callee().index() as usize)
            else {
                continue;
            };
            let Some((operand, role, ty)) =
                reachable_source_operand(graph, block as u32, operation)?
            else {
                continue;
            };
            let Some(record) = select_record(owner, occurrences, graph, role, ty)? else {
                continue;
            };
            let source = view.block_origins().get(block).ok_or_else(mismatch)?;
            if source.terminator() != SemanticExpandedTerminatorOriginV1::Source
                || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
                || call
                    .destination()
                    .is_none_or(|d| !d.place().projections().is_empty())
            {
                return Err(reject(
                    "scoped Matrix consumer is not a normal original source terminal",
                ));
            }
            let original = &owner.source_semantic().functions()[source.function().index() as usize]
                .blocks()[source.block().index() as usize];
            if !matches!(original.terminator().kind(), SemanticTerminatorKindV1::Call(c) if c.callee() == call.callee())
            {
                return Err(mismatch());
            }
            let actual = call.arguments().get(operand).ok_or_else(mismatch)?;
            if resolve::shared_pointee(owner.source_semantic().types(), actual.ty()) != Some(ty) {
                return Err(reject(
                    "scoped Matrix consumer changed its exact shared source operand",
                ));
            }
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = actual else {
                return Err(reject(
                    "scoped Matrix consumer cannot use a constant reference",
                ));
            };
            let receiver = graph.use_value(block as u32, place.local().index())?;
            let mut resolver = Resolver {
                owner,
                view,
                context,
                graph: graph,
                occurrences: occurrences,
                epochs,
                requirement: resolve::Requirement::Narrow(record),
            };
            let origin = resolver.operand(block as u32, actual, role)?;
            resolver.live(
                &origin,
                Site {
                    block: block as u32,
                    statement: None,
                    local: call
                        .destination()
                        .ok_or_else(mismatch)?
                        .place()
                        .local()
                        .index(),
                },
            )?;
            let lane = scope_identity(owner, view, record.identity(), &origin, false)?;
            let matrix = if role == Role::Narrow {
                Some(scope_identity(
                    owner,
                    view,
                    record.identity(),
                    &origin,
                    true,
                )?)
            } else {
                None
            };
            graph.charge(128 + call.arguments().len() * 8)?;
            for argument in call.arguments() {
                if let SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) = argument {
                    graph.charge(p.projections().len() * 4)?;
                }
            }
            if rows
                .insert(
                    block as u32,
                    Row {
                        call: call.clone(),
                        receiver,
                        result: ProductionScopedMatrixUseV1 {
                            lane,
                            matrix,
                            operand,
                        },
                    },
                )
                .is_some()
            {
                return Err(mismatch());
            }
        }
        Ok(rows)
    }
}

pub(super) fn source_operand(
    operation: &SemanticCompilerIntrinsicOperationV1,
) -> Option<(usize, Role, SemanticTypeIdV1)> {
    Some(match *operation {
        SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate { context, .. } => {
            (0, Role::Narrow, context)
        }
        SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero { lane, .. } => {
            (0, Role::Lane, lane)
        }
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 { lane, .. }
        | SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixLoadM16K128 { lane, .. }
        | SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixLoadM16K128 { lane, .. } => {
            (1, Role::Lane, lane)
        }
        // Global BF16 remains at its independent complete source/memory gate.
        _ => return None,
    })
}

fn reachable_source_operand(
    graph: &mut Graph<'_>,
    block: u32,
    operation: &SemanticCompilerIntrinsicOperationV1,
) -> Result<Option<(usize, Role, SemanticTypeIdV1)>> {
    let Some(candidate) = source_operand(operation) else {
        return Ok(None);
    };
    Ok(graph
        .reaches(graph.body.entry().index(), block)?
        .then_some(candidate))
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;
    include!("session_selection_tests.rs");
}
