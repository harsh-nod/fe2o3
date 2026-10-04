use super::*;

pub(super) fn role(operation: &SemanticCompilerIntrinsicOperationV1) -> Result<Option<Role>> {
    use SemanticCompilerIntrinsicOperationV1 as O;
    let selected = match operation {
        O::MatrixContextCurrent { .. } => Role::Context,
        O::WaveLaneCurrent { wave_width: 64, .. } => Role::Lane,
        O::Bf16MatrixLoadZeroFilledV2 {
            contract,
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            ..
        } if contract.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
            && contract.register_distribution == SemanticMfmaRegisterDistributionV1::Tile16x16
            && contract.wave_width == 64 =>
        {
            match contract.role {
                SemanticMfmaOperandRoleV1::A => Role::Lhs,
                SemanticMfmaOperandRoleV1::B => Role::Rhs,
            }
        }
        O::F32MatrixAccumulatorZero { contract, .. }
            if contract.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
                && contract.wave_width == 64
                && contract.distribution == SemanticMfmaAccumulatorDistributionV1::RowMajor =>
        {
            Role::Zero
        }
        O::MatrixMultiplyAccumulate {
            lhs,
            rhs,
            accumulator,
            ..
        } if lhs.role == SemanticMfmaOperandRoleV1::A
            && rhs.role == SemanticMfmaOperandRoleV1::B
            && lhs.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
            && rhs.profile == lhs.profile
            && lhs.register_distribution == SemanticMfmaRegisterDistributionV1::Tile16x16
            && rhs.register_distribution == lhs.register_distribution
            && lhs.wave_width == 64
            && rhs.wave_width == 64
            && accumulator.wave_width == 64
            && accumulator.profile == lhs.profile
            && accumulator.distribution == SemanticMfmaAccumulatorDistributionV1::RowMajor =>
        {
            Role::Result
        }
        O::WaveLaneCurrent { .. }
        | O::Bf16MatrixLoadZeroFilledV2 { .. }
        | O::F32MatrixAccumulatorZero { .. }
        | O::MatrixMultiplyAccumulate { .. } => {
            return unavailable("unsupported selected tensor profile");
        }
        _ => return Ok(None),
    };
    Ok(Some(selected))
}

pub(super) fn unique<T: Copy>(slot: &mut Option<T>, value: T, why: &'static str) -> Result<()> {
    if slot.replace(value).is_some() {
        return unavailable(why);
    }
    Ok(())
}
fn base_use(
    rows: &Occurrences<'_>,
    site: Site,
    operand: OperandRole,
    budget: &mut Budget<'_>,
) -> Result<SsaValueV1> {
    let mut value = None;
    for event in rows.events() {
        budget.charge_work(1)?;
        if event.site() == site && event.operand() == operand && event.role() == EventRole::BaseUse
        {
            record_base_use(
                &mut value,
                event.resolved(),
                event.is_reachable(),
                event.is_promoted(),
                budget,
            )?;
        }
    }
    require_base_use(value)
}
// Scalar predicate controls below cannot construct an occurrence owner. The
// only production caller above supplies fields borrowed from its real rows.
pub(super) fn record_base_use(
    value: &mut Option<SsaValueV1>,
    resolved: Option<SsaResolvedEventV1>,
    reachable: bool,
    promoted: bool,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(1)?;
    let Some(SsaResolvedEventV1::Use { value: actual, .. }) = resolved else {
        return unavailable("operand is not a promoted actual SSA use");
    };
    if !reachable || !promoted {
        return unavailable("unavailable source operand");
    }
    unique(value, actual, "duplicate source operand")
}
pub(super) fn require_base_use(value: Option<SsaValueV1>) -> Result<SsaValueV1> {
    value.ok_or(Error::Unavailable("missing actual source operand"))
}
fn whole_operand(operand: &SemanticOperandV1) -> bool {
    matches!(operand,SemanticOperandV1::Copy(place)|SemanticOperandV1::Move(place) if place.projections().is_empty())
}

// Only the existing whole-value move/copy and a shared whole-place reference
// are inspection aliases. Address formation, mutable/fake borrows and
// projected/retained memory are not admitted by this relation.
pub(super) fn nominal_alias_operand(value: &SemanticRvalueKindV1) -> Result<OperandRole> {
    match value {
        SemanticRvalueKindV1::Use(operand) if whole_operand(operand) => {
            Ok(OperandRole::RvalueOperand(0))
        }
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place,
        } if place.projections().is_empty() => Ok(OperandRole::RvaluePlace),
        _ => unavailable("computed/reconstructed nominal fragment"),
    }
}
pub(super) fn require_non_elided_borrow(actual_elision: bool) -> Result<()> {
    if actual_elision {
        return unavailable("elided borrow cannot establish nominal source SSA transport");
    }
    Ok(())
}
pub(super) fn require_nominal_borrow_source(role: Role) -> Result<()> {
    if !matches!(role, Role::Lane | Role::Context) {
        return unavailable("shared borrow is not a current lane or matrix context");
    }
    Ok(())
}

impl Recorder {
    pub(super) fn prepare(
        owner: &ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(1)?;
        let source = owner.source_semantic();
        let [function] = source.functions() else {
            return unavailable("one source function without helper bodies required");
        };
        if function.blocks().len() > SOURCE_BLOCKS {
            return unavailable("source block cap");
        }
        let root = SemanticFunctionIdV1::from_index(0);
        let rows = owner
            .occurrences_v1()
            .and_then(|r| r.function(root))
            .ok_or(Error::Unavailable("actual occurrence capture required"))?;
        reject_source_cycles(&rows, function.blocks().len(), budget)?;
        let mut recorder = Self::blank(root);
        for (index, block) in function.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let callable = source
                .callables()
                .get(call.callee().index() as usize)
                .ok_or(Error::Unavailable("callable coordinate"))?;
            let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable else {
                return unavailable("defined/foreign calls are outside the inspection subset");
            };
            let Some(role) = role(operation)? else {
                continue;
            };
            let block = SemanticBlockIdV1::from_index(index as u32);
            let destination = call
                .destination()
                .ok_or(Error::Unavailable("producer has no source destination"))?;
            if !destination.place().projections().is_empty()
                || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
            {
                return unavailable("projected producer or exceptional transport");
            }
            let local = destination.place().local().index();
            let mut value = None;
            for edge in rows.edge_definitions() {
                budget.charge_work(1)?;
                if edge.edge().source().get() == block.index() && edge.variable().get() == local {
                    if edge.edge().ordinal() != 0 || !edge.is_reachable() || !edge.is_promoted() {
                        return unavailable("producer return edge is unavailable");
                    }
                    unique(
                        &mut value,
                        edge.value()
                            .ok_or(Error::Unavailable("missing return definition"))?,
                        "duplicate call result",
                    )?;
                }
            }
            // No invented terminator definition if the real adapter uses an
            // edge definition. Conversely tolerate the exact real destination
            // event representation only when it is the unique captured one.
            for event in rows.events() {
                budget.charge_work(1)?;
                if event.site()
                    == (Site::Terminator {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
                    })
                    && event.role() == EventRole::DestinationDefine
                    && let Some(SsaResolvedEventV1::Define {
                        variable,
                        value: actual,
                    }) = event.resolved()
                    && variable.get() == local
                {
                    if !event.is_reachable() || !event.is_promoted() {
                        return unavailable("producer definition unavailable");
                    }
                    unique(&mut value, actual, "duplicate call result representation")?;
                }
            }
            let value = value.ok_or(Error::Unavailable(
                "producer result is retained memory, not SSA",
            ))?;
            let mut arguments = [None; 4];
            let indexes: &[usize] = match role {
                Role::Context | Role::Lane => &[],
                Role::Lhs | Role::Rhs => &[1],
                Role::Zero => &[0],
                Role::Result => &[0, 1, 2, 3],
            };
            let expected = match role {
                Role::Context | Role::Lane => 0,
                Role::Lhs | Role::Rhs => 4,
                Role::Zero => 1,
                Role::Result => 4,
            };
            if call.arguments().len() != expected {
                return unavailable("nominal producer arity");
            }
            for &index in indexes {
                budget.charge_work(1)?;
                if !whole_operand(&call.arguments()[index]) {
                    return unavailable("nominal input has projection or constant");
                }
                arguments[index] = Some(base_use(
                    &rows,
                    Site::Terminator {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
                    },
                    OperandRole::CallArgument(index as u32),
                    budget,
                )?);
            }
            unique(
                &mut recorder.producers[role.index()],
                Producer {
                    block,
                    value,
                    arguments,
                },
                "duplicate nominal producer",
            )?;
        }
        for role in Role::ALL {
            let producer = recorder.producer(role)?;
            recorder.resolve(owner, &rows, producer.value, 0, budget)?;
        }
        for role in [Role::Lhs, Role::Rhs, Role::Zero] {
            let producer = recorder.producer(role)?;
            let index = if role == Role::Zero { 0 } else { 1 };
            let alias = recorder.resolve(
                owner,
                &rows,
                producer.arguments[index].ok_or(Error::Unavailable("lane input"))?,
                0,
                budget,
            )?;
            if recorder.alias(alias)?.role != Role::Lane {
                return unavailable("different nominal current-lane producer");
            }
        }
        for (index, expected) in [Role::Context, Role::Lhs, Role::Rhs, Role::Zero]
            .into_iter()
            .enumerate()
        {
            let value = recorder.producer(Role::Result)?.arguments[index]
                .ok_or(Error::Unavailable("matrix input"))?;
            let alias = recorder.resolve(owner, &rows, value, 0, budget)?;
            if recorder.alias(alias)?.role != expected {
                return unavailable("matrix nominal role/source producer mismatch");
            }
            recorder.mfma_arguments[index] = alias;
        }
        Ok(recorder)
    }

    fn resolve(
        &mut self,
        owner: &ProductionSemanticSsaOwnerV1,
        rows: &Occurrences<'_>,
        value: SsaValueV1,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> Result<usize> {
        budget.charge_work(self.alias_count + 1)?;
        if depth >= ALIASES {
            return unavailable("cyclic/over-limit source alias chain");
        }
        if let Some(index) = self.aliases[..self.alias_count]
            .iter()
            .position(|a| a.is_some_and(|a| a.value == value))
        {
            return Ok(index);
        }
        let source = owner.source_semantic();
        let function = &source.functions()[self.root.index() as usize];
        let mut producer = None;
        for role in Role::ALL {
            budget.charge_work(1)?;
            if self.producer(role)?.value == value {
                unique(
                    &mut producer,
                    role,
                    "source definition belongs to two nominal producers",
                )?;
            }
        }
        let (role, kind) = if let Some(role) = producer {
            (role, AliasKind::Producer)
        } else {
            match value {
                SsaValueV1::BlockArgument { block, variable } => {
                    let plan = owner
                        .plan_for_function(self.root)
                        .ok_or(Error::Unavailable("actual SSA plan"))?;
                    let mut incoming = None;
                    for successor in rows.successors() {
                        budget.charge_work(1)?;
                        if successor.edge().target().index() != block.get() {
                            continue;
                        }
                        let Some(arguments) = plan.plan().edge_arguments(successor.id()) else {
                            continue;
                        };
                        let mut actual = None;
                        for argument in arguments {
                            budget.charge_work(1)?;
                            if argument.variable() == variable {
                                unique(
                                    &mut actual,
                                    argument.value(),
                                    "duplicate SSA edge variable",
                                )?;
                            }
                        }
                        unique(
                            &mut incoming,
                            (
                                successor.id(),
                                actual.ok_or(Error::Unavailable("missing edge argument"))?,
                            ),
                            "fragment phi/multiple predecessors unavailable",
                        )?;
                    }
                    let (edge, input) =
                        incoming.ok_or(Error::Unavailable("missing SSA predecessor"))?;
                    let from = self.resolve(owner, rows, input, depth + 1, budget)?;
                    (self.alias(from)?.role, AliasKind::Edge { from, edge })
                }
                SsaValueV1::Definition(_) => {
                    let mut site = None;
                    for event in rows.events() {
                        budget.charge_work(1)?;
                        if let Some(SsaResolvedEventV1::Define { value: actual, .. }) =
                            event.resolved()
                            && actual == value
                            && event.is_reachable()
                            && event.is_promoted()
                        {
                            unique(&mut site, event.site(), "duplicate source alias definition")?;
                        }
                    }
                    let Some(site @ Site::Statement { block, statement }) = site else {
                        return unavailable("fragment origin is not an admitted call or move");
                    };
                    let statement = function
                        .blocks()
                        .get(block.get() as usize)
                        .and_then(|b| b.statements().get(statement as usize))
                        .ok_or(Error::Unavailable("alias statement"))?;
                    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                        return unavailable("non-assignment nominal alias");
                    };
                    if !assignment.destination().projections().is_empty() {
                        return unavailable("projected nominal alias");
                    }
                    let operand = nominal_alias_operand(assignment.value().kind())?;
                    if operand == OperandRole::RvaluePlace {
                        budget.charge_work(rows.elisions().len())?;
                        require_non_elided_borrow(rows.elisions().contains(&site))?;
                    }
                    // A transparent borrow retains actual BaseUse + Define
                    // occurrences. Grid-leader elision is different: it omits
                    // the base use, so that absence cannot authenticate this
                    // relation. A nontransparent whole borrow makes its source
                    // storage-retained in the original adapter and fails here.
                    let input = base_use(rows, site, operand, budget)?;
                    let from = self.resolve(owner, rows, input, depth + 1, budget)?;
                    let role = self.alias(from)?.role;
                    if operand == OperandRole::RvaluePlace {
                        require_nominal_borrow_source(role)?;
                    }
                    // The genuine emitter still must find this SSA binding
                    // and prove identical components to the actual source.
                    (role, AliasKind::Copy { from })
                }
            }
        };
        if self.alias_count == ALIASES {
            return unavailable("fragment alias cap");
        }
        let index = self.alias_count;
        self.aliases[index] = Some(Alias {
            value,
            role,
            kind,
            components: [ValueId(0); 4],
        });
        self.alias_count += 1;
        if matches!(kind, AliasKind::Producer) {
            self.producer_aliases[role.index()] = index;
        }
        Ok(index)
    }
}

fn reject_source_cycles(
    rows: &Occurrences<'_>,
    count: usize,
    budget: &mut Budget<'_>,
) -> Result<()> {
    // successors() is an immutable borrowed slice; get(index), id() and edge()
    // are constant-time. No per-edge linear lookup or copied graph is hidden.
    let successors = rows.successors();
    reject_source_cycle_edges(
        count,
        successors.len(),
        |index| {
            let successor = successors.get(index)?;
            Some((
                successor.id().source().get() as usize,
                successor.edge().target().index() as usize,
            ))
        },
        budget,
    )
}

// This private indexed adapter makes missing/out-of-range row refusal testable
// without constructing a source owner. Only the wrapper above borrows actual
// source occurrences; caller tuples or test rows never create that custody.
pub(super) fn reject_source_cycle_edges(
    count: usize,
    edge_count: usize,
    mut edge_at: impl FnMut(usize) -> Option<(usize, usize)>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    if count > SOURCE_BLOCKS {
        return unavailable("source block cap");
    }
    let work = count
        .checked_mul(count)
        .and_then(|v| v.checked_add(edge_count))
        .and_then(|v| v.checked_add(count))
        .and_then(|v| v.checked_add(SOURCE_BLOCKS))
        .ok_or(Resource::Arithmetic)?;
    // Prepay full fixed initialization, every actual edge access, transitive
    // closure pair and diagonal read on the ORIGINAL supplied phase ledger.
    budget.charge_work(work)?;
    let scratch = size_of::<[u32; SOURCE_BLOCKS]>();
    budget.reserve_storage(scratch)?;
    let outcome = (|| {
        let mut reach = [0u32; SOURCE_BLOCKS];
        for index in 0..edge_count {
            let (source, target) =
                edge_at(index).ok_or(Error::Unavailable("missing source successor row"))?;
            if source >= count || target >= count {
                return unavailable("source successor outside fixed roster");
            }
            reach[source] |= 1u32 << target;
        }
        for via in 0..count {
            for from in 0..count {
                if reach[from] & (1u32 << via) != 0 {
                    reach[from] |= reach[via];
                }
            }
        }
        let cyclic = (0..count).any(|index| reach[index] & (1u32 << index) != 0);
        if cyclic {
            return unavailable("looping source unavailable for first inspection profile");
        }
        Ok(())
    })();
    // Scratch lifetime has ended on both ordinary success and refusal. Refund
    // only its exact reservation, never the caller's floor or consumed work.
    budget.release_storage(scratch)?;
    outcome
}
