use super::relation::{scope_identity, update_value};
use super::resolve::{Requirement, Resolver, Role};
use super::session::ProductionScopedMatrixSourceSessionV1 as SourceSession;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticGlobalBf16MatrixLoadV1;

include!("bf16_read_source.rs");

/// Lane and real PolicyMatrixBind custody at one actual Global BF16 terminal.
/// This is NOT a Global constructor, allocation, checked Result, or read proof.
/// Its distinct type cannot satisfy a narrowed Matrix arithmetic consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionScopedBf16LaneUseV1 {
    lane: [u8; 32],
    bound: [u8; 32],
    bind_call: u32,
    bind_instance: SemanticCallInstanceIdV1,
    bound_value: SsaValueV1,
    matrix_value: SsaValueV1,
    context_value: SsaValueV1,
    workgroup_value: SsaValueV1,
    subgroup_value: SsaValueV1,
    lane_reference: SsaValueV1,
    matrix_reference: SsaValueV1,
    contract: SemanticGlobalBf16MatrixLoadV1,
}

impl ProductionScopedBf16LaneUseV1 {
    /// Exact scoped lane custody, not numeric range or convergence evidence.
    pub const fn lane_identity(self) -> [u8; 32] {
        self.lane
    }
    /// Commitment to the real Bind occurrence, without a narrowing occurrence.
    pub const fn bind_identity(self) -> [u8; 32] {
        self.bound
    }
    /// Expanded original caller block of the checked PolicyMatrixBind.
    pub const fn bind_call_block(self) -> u32 {
        self.bind_call
    }
    /// Actual expanded Bind callee instance; never a synthesized instance ID.
    pub const fn bind_instance(self) -> SemanticCallInstanceIdV1 {
        self.bind_instance
    }
    /// Actual Bind return producer that the Global constructor must reuse.
    pub const fn bound_value(self) -> SsaValueV1 {
        self.bound_value
    }
    /// Actual MatrixAccess producer under that Bind.
    pub const fn matrix_value(self) -> SsaValueV1 {
        self.matrix_value
    }
    /// Original checked Context issuer; not an invocation-index value.
    pub const fn context_value(self) -> SsaValueV1 {
        self.context_value
    }
    /// Original checked Workgroup issuer, before any epoch projection.
    pub const fn workgroup_value(self) -> SsaValueV1 {
        self.workgroup_value
    }
    /// Original initialized Subgroup whose field zero supplied this lane.
    /// A consumer still has to prove its numeric lane in the final root.
    pub const fn subgroup_value(self) -> SsaValueV1 {
        self.subgroup_value
    }
    /// Current lane-reference SSA base at the retained load.
    pub const fn lane_reference(self) -> SsaValueV1 {
        self.lane_reference
    }
    /// Current Global-matrix-reference SSA base, NOT an authenticated Global.
    pub const fn matrix_reference(self) -> SsaValueV1 {
        self.matrix_reference
    }
    /// Original typed Global BF16 load contract; no inferred memory theorem.
    pub const fn contract(self) -> SemanticGlobalBf16MatrixLoadV1 {
        self.contract
    }
}

pub(super) struct Row {
    call: SemanticDirectCallV1,
    result: ProductionScopedBf16LaneUseV1,
}

/// Source-only BF16 rows borrowed from one immutable owner and consumed batch.
/// Global constructor lineage and four observed reads remain separate gates.
pub struct ProductionScopedBf16LaneRelationV1<'a> {
    pub(super) owner: &'a ProductionSemanticSsaOwnerV1,
    pub(super) view: &'a SemanticExpandedRootV1,
    pub(super) rows: BTreeMap<u32, Row>,
}

impl ProductionScopedBf16LaneRelationV1<'_> {
    /// Reads a row only for its exact original retained body/call.
    pub fn use_at(
        &self,
        body: &SemanticFunctionDeclV1,
        block: u32,
        call: &SemanticDirectCallV1,
    ) -> Result<Option<ProductionScopedBf16LaneUseV1>> {
        check_call(self.owner, self.view, body, block, call)?;
        self.rows
            .get(&block)
            .map(|row| {
                if row.call != *call {
                    return Err(mismatch());
                }
                Ok(row.result)
            })
            .transpose()
    }

    /// Requires exact, ordered, once-only consumption in the final read replay.
    pub fn check_consumed(&self, body: &SemanticFunctionDeclV1, blocks: &[u32]) -> Result<()> {
        if !std::ptr::eq(body, self.view.body()) {
            return Err(mismatch());
        }
        check_roster(&self.rows, blocks)
    }
}

impl SourceSession<'_> {
    /// Resolves the actual lane and a candidate Bind occurrence from live SSA.
    /// The coordinate is not authority: this rechecks the real source Bind,
    /// MatrixAccess, Context, Workgroup, epoch and all referent loans at the use.
    /// A memory consumer must separately link bound_value to the Global's real
    /// constructor receiver on this session's graph before consuming the row.
    pub fn checked_bf16_lane_at(
        &mut self,
        body: &SemanticFunctionDeclV1,
        block: u32,
        call: &SemanticDirectCallV1,
        bind_call: u32,
    ) -> Result<ProductionScopedBf16LaneUseV1> {
        self.require_bf16_scope()?;
        check_call(self.owner, self.view, body, block, call)?;
        self.graph.charge(1)?;
        if self.bf16_rows.contains_key(&block)
            || !self
                .graph
                .reaches(self.view.body().entry().index(), block)?
        {
            return Err(reject("BF16 lane query is duplicate or unreachable"));
        }
        let contract = checked_load(self.owner, self.view, block, call)?;
        let mut selected = None;
        for binding in self.occurrences.binds.values() {
            self.graph.charge(1)?;
            if binding.binding.expanded_call_block().index() == bind_call {
                if selected.replace(binding).is_some() {
                    return Err(mismatch());
                }
            }
        }
        let binding =
            selected.ok_or_else(|| reject("BF16 lane has no exact checked Bind occurrence"))?;
        let identity = binding.record.identity();
        if identity.provenance() != contract.provenance()
            || identity.matrix_brand() != contract.matrix_brand()
        {
            return Err(reject(
                "BF16 lane and Bind changed their source root or Matrix brand",
            ));
        }
        let bound_value = self.graph.use_value(
            binding.returned.block,
            binding.binding.callee_return().index(),
        )?;
        if self.graph.definition(bound_value)? != binding.site {
            return Err(reject(
                "BF16 Bind return is not its original checked constructor",
            ));
        }
        let matrix_reference = operand_value(&mut self.graph, block, &call.arguments()[0])?;
        let lane_reference = operand_value(&mut self.graph, block, &call.arguments()[1])?;
        let mut resolver = Resolver {
            owner: self.owner,
            view: self.view,
            context: &self.context,
            graph: &mut self.graph,
            occurrences: &self.occurrences,
            epochs: &self.epochs,
            requirement: Requirement::Bound(binding.record),
        };
        let bound = resolver.value(
            bound_value,
            &[],
            binding.record.types().bound,
            Role::Bound,
            0,
        )?;
        let lane = resolver.operand(block, &call.arguments()[1], Role::Lane)?;
        if bound.bind != Some(binding.site)
            || bound.narrow.is_some()
            || bound.context != lane.context
            || bound.subgroup != lane.subgroup
            || bound
                .workgroup
                .as_ref()
                .map(|w| (w.issuer, w.contract, w.epoch_projection))
                != lane
                    .workgroup
                    .as_ref()
                    .map(|w| (w.issuer, w.contract, w.epoch_projection))
        {
            return Err(reject(
                "BF16 Bind and lane name different live MatrixAccess owners",
            ));
        }
        let consumer = Site {
            block,
            statement: None,
            local: call
                .destination()
                .ok_or_else(mismatch)?
                .place()
                .local()
                .index(),
        };
        resolver.live(&bound, consumer)?;
        resolver.live(&lane, consumer)?;
        let lane_identity = scope_identity(self.owner, self.view, identity, &lane, false)?;
        let matrix_value = bound.matrix.ok_or_else(mismatch)?;
        let mut digest = Sha256::new();
        digest.update(b"fe2o3.scoped-bf16-bind-use.v1");
        digest.update(lane_identity);
        digest.update(identity.policy().as_bytes());
        update_value(&mut digest, bound_value);
        update_value(&mut digest, matrix_value);
        update_value(&mut digest, bound.policy.ok_or_else(mismatch)?);
        for word in [
            bind_call,
            binding.binding.callee_instance().index(),
            binding.site.block,
            binding.site.statement.ok_or_else(mismatch)?,
            binding.site.local,
            binding.returned.block,
            binding.returned.statement.ok_or_else(mismatch)?,
            binding.returned.local,
        ] {
            digest.update(word.to_le_bytes());
        }
        let result = ProductionScopedBf16LaneUseV1 {
            lane: lane_identity,
            bound: digest.finalize().into(),
            bind_call,
            bind_instance: binding.binding.callee_instance(),
            bound_value,
            matrix_value,
            context_value: lane.context,
            workgroup_value: lane.workgroup.as_ref().ok_or_else(mismatch)?.issuer,
            subgroup_value: lane.subgroup.ok_or_else(mismatch)?,
            lane_reference,
            matrix_reference,
            contract,
        };
        self.graph.charge(
            192 + 3 * std::mem::size_of::<SsaValueV1>() + call.arguments().len() * 8,
        )?;
        for argument in call.arguments() {
            if let SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) = argument {
                self.graph.charge(p.projections().len() * 4)?;
            }
        }
        self.bf16_rows.insert(
            block,
            Row {
                call: call.clone(),
                result,
            },
        );
        Ok(result)
    }
}

fn operand_value(
    graph: &mut Graph<'_>,
    block: u32,
    operand: &SemanticOperandV1,
) -> Result<SsaValueV1> {
    match operand {
        SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => {
            graph.use_value(block, p.local().index())
        }
        SemanticOperandV1::Constant(_) => {
            Err(reject("BF16 reference cannot originate from a constant"))
        }
    }
}

fn check_call(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    body: &SemanticFunctionDeclV1,
    block: u32,
    call: &SemanticDirectCallV1,
) -> Result<()> {
    if !std::ptr::eq(body, view.body())
        || !owner
            .execution_view_for_root(view.root())
            .is_some_and(|v| std::ptr::eq(v, view))
        || !matches!(body.blocks().get(block as usize).map(|b| b.terminator().kind()),
            Some(SemanticTerminatorKindV1::Call(actual)) if actual == call)
    {
        return Err(mismatch());
    }
    Ok(())
}

fn checked_load(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    block: u32,
    call: &SemanticDirectCallV1,
) -> Result<SemanticGlobalBf16MatrixLoadV1> {
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract },
        ..
    }) = owner
        .source_semantic()
        .callables()
        .get(call.callee().index() as usize)
    else {
        return Err(reject(
            "BF16 lane query requires an actual Global BF16 load terminal",
        ));
    };
    let source = view
        .block_origins()
        .get(block as usize)
        .ok_or_else(mismatch)?;
    let original = owner
        .source_semantic()
        .functions()
        .get(source.function().index() as usize)
        .and_then(|f| f.blocks().get(source.block().index() as usize))
        .ok_or_else(mismatch)?;
    let types = owner.source_semantic().types();
    if source.terminator() != SemanticExpandedTerminatorOriginV1::Source
        || !matches!(original.terminator().kind(), SemanticTerminatorKindV1::Call(c) if c.callee() == call.callee())
        || binding.identity() != contract.source_identity()
        || contract.provenance().root() != view.root()
        || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
        || call.arguments().len() != 4
        || call.destination().is_none_or(|d| {
            d.place().ty() != contract.types().fragment || !d.place().projections().is_empty()
        })
        || resolve::shared_pointee(types, call.arguments()[0].ty()) != Some(contract.types().matrix)
        || resolve::shared_pointee(types, call.arguments()[1].ty()) != Some(contract.types().lane)
        || call.arguments()[2].ty() != contract.types().index
        || call.arguments()[3].ty() != contract.types().index
    {
        return Err(reject(
            "BF16 lane query changed the original load ABI or source identity",
        ));
    }
    Ok(*contract)
}

pub(super) fn check_roster(rows: &BTreeMap<u32, Row>, blocks: &[u32]) -> Result<()> {
    if blocks.len() != rows.len() || !blocks.iter().copied().eq(rows.keys().copied()) {
        return Err(reject(
            "BF16 source replay did not consume the exact checked lane roster",
        ));
    }
    Ok(())
}
