use super::resolve::{Origin, Role};
use super::*;

/// A checked source-use commitment. Bytes extracted from this row are not
/// sufficient to reconstruct a relation or authenticate another use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionScopedMatrixUseV1 {
    pub(super) lane: [u8; 32],
    pub(super) matrix: Option<[u8; 32]>,
    pub(super) operand: usize,
}

impl ProductionScopedMatrixUseV1 {
    /// Exact subgroup/Workgroup/epoch source custody; not convergence proof.
    pub const fn lane_identity(self) -> [u8; 32] {
        self.lane
    }
    /// Policy-bound scoped Matrix owner, when this is a multiply consumer.
    pub const fn matrix_identity(self) -> Option<[u8; 32]> {
        self.matrix
    }
    /// The actual source operand carrying the checked lane or Matrix reference.
    pub const fn operand(self) -> usize {
        self.operand
    }
}

pub(super) struct Row {
    pub(super) call: SemanticDirectCallV1,
    pub(super) receiver: SsaValueV1,
    pub(super) result: ProductionScopedMatrixUseV1,
}

/// Nonserializable source-SSA custody borrowed from one immutable replayed owner.
/// This neither authenticates inert frontend inputs nor proves numerical/memory
/// refinement. Only the private compiler adapter may use it as frontend custody.
pub struct ProductionScopedMatrixUseRelationV1<'a> {
    pub(super) owner: &'a ProductionSemanticSsaOwnerV1,
    pub(super) view: &'a SemanticExpandedRootV1,
    pub(super) rows: BTreeMap<u32, Row>,
}

impl<'a> ProductionScopedMatrixUseRelationV1<'a> {
    /// Resolves actual source issuers using one graph and work allowance.
    /// The input and optional entry relation must come from frontend custody.
    pub fn checked_source_uses(
        owner: &'a ProductionSemanticSsaOwnerV1,
        input: &ProductionKernelContextLoweringInputV1,
        entry: Option<&ProductionKernelContextEntrySsaRelationV1<'a>>,
        max_work: usize,
    ) -> Result<Self> {
        session::ProductionScopedMatrixSourceSessionV1::checked_matrix_uses(
            owner, input, entry, max_work,
        )
    }

    /// Negative-only structural precheck before constructing a custody session.
    /// True grants no source, SSA or reachability fact. The exact row builder
    /// performs all checks; false means this consumer family has no operation.
    pub fn has_potential_source_consumers(
        body: &SemanticFunctionDeclV1,
        callables: &[SemanticCallableDeclV1],
    ) -> bool {
        body.blocks().iter().any(|block| {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                return false;
            };
            matches!(callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. })
                if session::source_operand(operation).is_some())
        })
    }

    /// Read-only during fixed point. The exact borrowed body/call is rechecked;
    /// no row is issued by supplying a digest or a compatible type.
    pub fn use_at(
        &self,
        body: &SemanticFunctionDeclV1,
        block: u32,
        call: &SemanticDirectCallV1,
    ) -> Result<Option<ProductionScopedMatrixUseV1>> {
        if !std::ptr::eq(body, self.view.body())
            || !self
                .owner
                .execution_view_for_root(self.view.root())
                .is_some_and(|v| std::ptr::eq(v, self.view))
        {
            return Err(mismatch());
        }
        let actual = body
            .blocks()
            .get(block as usize)
            .ok_or_else(mismatch)?
            .terminator()
            .kind();
        if !matches!(actual, SemanticTerminatorKindV1::Call(actual) if actual == call) {
            return Err(mismatch());
        }
        self.rows
            .get(&block)
            .map(|row| {
                if row.call != *call {
                    return Err(mismatch());
                }
                // The graph was checked against this immutable owner and exact body.
                let _actual_receiver_ssa = row.receiver;
                Ok(row.result)
            })
            .transpose()
    }

    /// Complete final-replay use roster. Repeated fixed-point queries are not
    /// consumption; duplicate/missing/stale final uses are rejected.
    pub fn check_consumed(&self, body: &SemanticFunctionDeclV1, blocks: &[u32]) -> Result<()> {
        if !std::ptr::eq(body, self.view.body())
            || blocks.len() != self.rows.len()
            || !blocks.iter().copied().eq(self.rows.keys().copied())
        {
            return Err(reject(
                "scoped Matrix final replay did not consume the exact checked use roster",
            ));
        }
        Ok(())
    }
}

pub(super) fn select_record(
    owner: &ProductionSemanticSsaOwnerV1,
    occurrences: &occurrences::Occurrences,
    graph: &mut Graph<'_>,
    role: Role,
    ty: SemanticTypeIdV1,
) -> Result<Option<SemanticPolicyGfx950NarrowV1>> {
    let mut selected = None;
    for row in occurrences.narrows.values() {
        graph.charge(1)?;
        let record = row.record;
        let matches = if role == Role::Narrow {
            record.types().narrowed == ty
        } else {
            let mut matched = false;
            for callable in owner.source_semantic().callables() {
                graph.charge(1)?;
                if let SemanticCallableDeclV1::CompilerIntrinsic {
                    operation:
                        SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                    ..
                } = callable
                    && let SemanticExecutionCapabilityOperationV1::MatrixAccess {
                        matrix,
                        subgroup,
                        subgroup_brand,
                        width: 64,
                        ..
                    } = contract.operation()
                {
                    if matrix == record.types().bind.matrix
                        && subgroup_brand == record.identity().matrix_brand()
                        && contract.provenance() == record.identity().provenance()
                    {
                        matched |=
                            resolve::shared_pointee(owner.source_semantic().types(), subgroup)
                                .and_then(|s| {
                                    resolve::aggregate_fields(owner.source_semantic().types(), s)
                                })
                                .and_then(|f| f.first())
                                .copied()
                                == Some(ty);
                    }
                }
            }
            matched
        };
        if matches {
            if selected.is_some_and(|old| old != record) {
                return Err(reject(
                    "scoped Matrix use has ambiguous nominal source constraints",
                ));
            }
            selected = Some(record);
        }
    }
    Ok(selected)
}

pub(super) fn scope_identity(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    identity: SemanticDefinedMatrixIdentityV1,
    origin: &Origin,
    matrix: bool,
) -> Result<[u8; 32]> {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.scoped-matrix-source-use.v1");
    digest.update([u8::from(matrix)]);
    digest.update(owner.source_semantic().semantic_sha256().as_bytes());
    digest.update(owner.execution_expansion().identity());
    digest.update(view.identity());
    digest.update(
        owner.source_semantic().functions()[view.root().index() as usize]
            .identity()
            .as_bytes(),
    );
    let p = identity.provenance();
    digest.update(p.root().index().to_le_bytes());
    for id in [
        p.kernel_binding().as_bytes(),
        p.frontend_unit().as_bytes(),
        p.kernel_marker().as_bytes(),
        p.target_brand().as_bytes(),
        p.launch_brand().as_bytes(),
        p.issuance().as_bytes(),
        identity.kernel_brand().as_bytes(),
        identity.execution_brand().as_bytes(),
        identity.matrix_brand().as_bytes(),
        identity.epoch().as_bytes(),
    ] {
        digest.update(id);
    }
    update_value(&mut digest, origin.context);
    update_value(
        &mut digest,
        origin.workgroup.as_ref().ok_or_else(mismatch)?.issuer,
    );
    update_value(&mut digest, origin.subgroup.ok_or_else(mismatch)?);
    if matrix {
        digest.update(identity.policy().as_bytes());
        update_value(&mut digest, origin.matrix.ok_or_else(mismatch)?);
        update_value(&mut digest, origin.policy.ok_or_else(mismatch)?);
        for site in [
            origin.bind.ok_or_else(mismatch)?,
            origin.narrow.ok_or_else(mismatch)?,
        ] {
            let source = view
                .block_origins()
                .get(site.block as usize)
                .ok_or_else(mismatch)?;
            for word in [
                site.block,
                site.statement.ok_or_else(mismatch)?,
                site.local,
                source.instance().index(),
                source.function().index(),
                source.block().index(),
            ] {
                digest.update(word.to_le_bytes());
            }
        }
    }
    Ok(digest.finalize().into())
}

pub(super) fn update_value(digest: &mut Sha256, value: SsaValueV1) {
    match value {
        SsaValueV1::Definition(id) => {
            digest.update([0]);
            digest.update(id.get().to_le_bytes());
        }
        SsaValueV1::BlockArgument { block, variable } => {
            digest.update([1]);
            digest.update(block.get().to_le_bytes());
            digest.update(variable.get().to_le_bytes());
        }
    }
}
