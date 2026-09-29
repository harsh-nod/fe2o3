//! Literal live-outs of an already authenticated source induction.
use super::*;
use fe2o3_mir_model::SsaBlockIdV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1;

pub(in crate::production_ranked_projection_v1) enum Resolution {
    Origin(Origin),
    LiveOut(u64),
}

// The CFG itself remains borrowed from the existing source proof. Only numeric
// offsets, scratch, and one avoidance result per queried exit are retained here.
pub(super) struct State {
    successor_offsets: Vec<usize>,
    exit_avoidance: Vec<Option<Vec<bool>>>,
    pending: Vec<usize>,
}

impl Index<'_> {
    fn live_out_reserve(
        &self,
        facts: &mut dyn ProjectedAssertionFactsV1,
        amount: usize,
    ) -> Result<()> {
        self.check(facts)?;
        let next = sum(self.extra_owned.get(), amount)?;
        facts.reserve_scalar_private_storage_v1(amount)?;
        self.extra_owned.set(next);
        Ok(())
    }

    fn live_out_vec<T>(
        &self,
        facts: &mut dyn ProjectedAssertionFactsV1,
        count: usize,
    ) -> Result<Vec<T>> {
        self.live_out_reserve(facts, bytes::<T>(count)?)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| resource(Resource::Allocation))?;
        self.live_out_reserve(
            facts,
            bytes::<T>(
                rows.capacity()
                    .checked_sub(count)
                    .ok_or_else(|| resource(Resource::Accounting))?,
            )?,
        )?;
        Ok(rows)
    }

    fn live_out_state(&self, facts: &mut dyn ProjectedAssertionFactsV1) -> Result<State> {
        let blocks = self.function.blocks().len();
        self.charge(facts, sum(blocks, 1)?)?;
        self.live_out_reserve(
            facts,
            [
                size_of::<Result<State>>(),
                size_of::<Result<Resolution>>(),
                size_of::<Option<Vec<bool>>>(),
                size_of::<std::cell::RefMut<'_, Option<State>>>(),
            ]
            .into_iter()
            .try_fold(0, sum)?,
        )?;
        let mut successor_offsets = self.live_out_vec(facts, sum(blocks, 1)?)?;
        let mut exit_avoidance = self.live_out_vec(facts, blocks)?;
        let pending = self.live_out_vec(facts, blocks)?;
        let successors = self.occurrences.successors();
        let mut cursor = 0;
        for block in 0..blocks {
            self.charge(facts, 2)?;
            successor_offsets.push(cursor);
            exit_avoidance.push(None);
            while let Some(edge) = successors.get(cursor) {
                self.charge(facts, 2)?;
                let source = edge.id().source().get() as usize;
                if source < block || edge.edge().target().index() as usize >= blocks {
                    return self
                        .fail("a pipeline live-out has inconsistent original successor rows");
                }
                if source != block {
                    break;
                }
                cursor += 1;
            }
        }
        successor_offsets.push(cursor);
        if cursor != successors.len() {
            return self.fail("a pipeline live-out has unindexed original successor rows");
        }
        Ok(State {
            successor_offsets,
            exit_avoidance,
            pending,
        })
    }

    pub(in crate::production_ranked_projection_v1) fn resolve_projected(
        &self,
        function: &SemanticFunctionDeclV1,
        local: usize,
        use_site: ScalarAssignmentSiteV1,
        inductions: &[ProjectedUniformInductionV1],
        proofs: &SemanticAssertProofsV1<'_>,
        scalar_work: &mut usize,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<Resolution> {
        let result = self.resolve_projected_inner(
            function,
            local,
            use_site,
            inductions,
            proofs,
            scalar_work,
            facts,
        );
        if let Err(error) = &result {
            self.record(error);
        }
        result
    }

    fn resolve_projected_inner(
        &self,
        function: &SemanticFunctionDeclV1,
        local: usize,
        use_site: ScalarAssignmentSiteV1,
        inductions: &[ProjectedUniformInductionV1],
        proofs: &SemanticAssertProofsV1<'_>,
        scalar_work: &mut usize,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<Resolution> {
        let value = self.resolved_value(function, local, use_site, facts)?;
        let SsaValueV1::BlockArgument { block, variable } = value else {
            return self.origin(local, value).map(Resolution::Origin);
        };
        if variable.get() as usize != local || !std::ptr::eq(proofs.function, function) {
            return self.fail("a pipeline live-out substituted its original source proof");
        }
        let mut selected = None;
        for induction in inductions {
            self.charge(facts, 2)?;
            if induction.header == block.get() as usize
                && induction.source_progress.induction.index() as usize == local
            {
                if selected.replace(induction).is_some() {
                    return self.fail("a pipeline live-out has multiple source induction proofs");
                }
            }
        }
        let Some(induction) = selected else {
            return self.fail("a pipeline scalar requires an unsupported SSA block argument");
        };
        self.charge(
            facts,
            (usize::BITS - induction.loop_blocks.len().leading_zeros()) as usize,
        )?;
        if induction.contains_block(use_site.block) || induction.header == use_site.block {
            return self.fail("a pipeline live-out use is still inside its source induction");
        }
        let mut state = self
            .live_out
            .try_borrow_mut()
            .map_err(|_| resource(Resource::Accounting))?;
        if state.is_none() {
            *state = Some(self.live_out_state(facts)?);
        }
        let state = state.as_mut().expect("initialized live-out state");
        if !self.exit_dominates(state, &proofs.graph, induction.exit, use_site.block, facts)? {
            return self.fail("a pipeline live-out does not follow its unique normal source exit");
        }
        let result = self.literal_live_out(state, induction, value, proofs, scalar_work, facts)?;
        Ok(Resolution::LiveOut(result))
    }

    fn exit_dominates(
        &self,
        state: &mut State,
        graph: &ProjectedLoopCfgV1,
        exit: usize,
        use_block: usize,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<bool> {
        self.charge(facts, 4)?;
        let count = self.function.blocks().len();
        if graph.successors.len() != count
            || graph.predecessors.len() != count
            || graph.reachable.len() != count
            || graph.entry != self.function.entry().index() as usize
            || exit >= count
            || use_block >= count
            || !graph.reachable[exit]
            || !graph.reachable[use_block]
        {
            return self.fail("a pipeline live-out has no exact reachable source CFG");
        }
        if state.exit_avoidance[exit].is_none() {
            self.charge(facts, count)?;
            let mut reached = self.live_out_vec(facts, count)?;
            reached.resize(count, false);
            state.pending.clear();
            if graph.entry != exit {
                reached[graph.entry] = true;
                state.pending.push(graph.entry);
            }
            while let Some(block) = state.pending.pop() {
                self.charge(facts, 2)?;
                for &target in &graph.successors[block] {
                    self.charge(facts, 3)?;
                    if target >= count {
                        return self.fail("a pipeline live-out has an invalid source edge");
                    }
                    if target != exit && !reached[target] {
                        reached[target] = true;
                        // Each source block is queued at most once.
                        state.pending.push(target);
                    }
                }
            }
            state.exit_avoidance[exit] = Some(reached);
        }
        Ok(!state.exit_avoidance[exit].as_ref().unwrap()[use_block])
    }

    fn literal_live_out(
        &self,
        state: &State,
        induction: &ProjectedUniformInductionV1,
        phi: SsaValueV1,
        proofs: &SemanticAssertProofsV1<'_>,
        scalar_work: &mut usize,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<u64> {
        self.charge(facts, 12)?;
        let candidate = &induction.source_progress;
        let ty = candidate.induction_type;
        let local = candidate.induction.index() as usize;
        let types = self.source.owner.source_semantic().types();
        let bits = unsigned_index_bits_v1(types, ty)
            .filter(|bits| *bits > 0)
            .ok_or_else(|| {
                reject("a pipeline live-out requires an original unsigned index type")
            })?;
        if self
            .function
            .locals()
            .get(local)
            .is_none_or(|decl| decl.ty() != ty)
            || proofs.address_escaped.get(local).copied() != Some(false)
        {
            return self
                .fail("a pipeline live-out changed its original scalar type or address semantics");
        }
        let header_site = ScalarAssignmentSiteV1 {
            block: induction.header,
            statement: candidate.header_statement,
        };
        let comparison = self.assignment(header_site)?;
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right,
        } = comparison.value().kind()
        else {
            return self
                .fail("a pipeline live-out requires an exact strict unsigned less-than header");
        };
        if left.ty() != ty
            || right.ty() != ty
            || right != &candidate.bound_operand
            || !self.is_phi_operand(left, header_site, phi, scalar_work, facts)?
        {
            return self
                .fail("a pipeline live-out changed the orientation or operands of its comparison");
        }
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = self.function.blocks()[induction.header].terminator().kind()
        else {
            return self.fail("a pipeline live-out lost its original comparison branch");
        };
        if raw_operand_place(discriminant) != Some(comparison.destination())
            || targets.values().len() != 1
            || targets.values()[0].value() != 0
            || targets.values()[0].edge().target().index() as usize != induction.exit
            || targets.otherwise().target().index() as usize != induction.body_entry
        {
            return self.fail("a pipeline live-out changed its true body or false normal exit");
        }
        let bound = literal_unsigned(right, ty, bits)
            .ok_or_else(|| reject("a pipeline live-out requires a literal unsigned bound"))?;
        let step = literal_unsigned(&candidate.step_operand, ty, bits)
            .filter(|step| *step > 0)
            .ok_or_else(|| {
                reject("a pipeline live-out requires a positive literal unsigned step")
            })?;
        if candidate.step_value != step {
            return self.fail("a pipeline live-out changed its source recurrence step");
        }
        let latch_site = ScalarAssignmentSiteV1 {
            block: induction.latch,
            statement: candidate.latch_statement,
        };
        let latch = self.assignment(latch_site)?;
        if latch.destination().local() != candidate.induction
            || !latch.destination().projections().is_empty()
            || latch.destination().ty() != ty
            || latch.value().result_type() != ty
        {
            return self.fail("a pipeline live-out changed its original latch definition");
        }
        let (left, right, update_site) = match (&candidate.update, latch.value().kind()) {
            (
                ProjectedSourceInductionUpdateV1::Ordinary,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left,
                    right,
                },
            ) => (left, right, latch_site),
            (
                ProjectedSourceInductionUpdateV1::Unchecked,
                SemanticRvalueKindV1::UncheckedBinary(value),
            ) if value.operation() == SemanticUncheckedBinaryOpV1::Add => {
                (value.left(), value.right(), latch_site)
            }
            (
                ProjectedSourceInductionUpdateV1::Checked {
                    producer_block,
                    producer_statement,
                    result_local,
                },
                SemanticRvalueKindV1::Use(value),
            ) if tuple_field_operand_local_v1(value, 0) == Some(*result_local) => {
                let site = ScalarAssignmentSiteV1 {
                    block: *producer_block,
                    statement: *producer_statement,
                };
                self.charge(facts, 4)?;
                let result_index = result_local.index() as usize;
                if proofs.definition_counts.get(result_index).copied() != Some(1)
                    || proofs.assignments.get(result_index).copied().flatten() != Some(site)
                    || proofs.address_escaped.get(result_index).copied() != Some(false)
                {
                    return self.fail(
                        "a pipeline live-out checked tuple no longer has its exact unique producer",
                    );
                }
                let producer = self.assignment(site)?;
                if producer.destination().local() != *result_local
                    || !producer.destination().projections().is_empty()
                {
                    return self
                        .fail("a pipeline live-out changed its authenticated checked producer");
                }
                let SemanticRvalueKindV1::CheckedBinary(value) = producer.value().kind() else {
                    return self.fail("a pipeline live-out lost its authenticated checked Add");
                };
                if value.operation() != SemanticCheckedBinaryOpV1::Add {
                    return self.fail("a pipeline live-out changed its checked recurrence");
                }
                (value.left(), value.right(), site)
            }
            _ => return self.fail("a pipeline live-out changed its authenticated Add recurrence"),
        };
        if left.ty() != ty
            || right.ty() != ty
            || !((literal_unsigned(left, ty, bits) == Some(step)
                && self.is_phi_operand(right, update_site, phi, scalar_work, facts)?)
                || (literal_unsigned(right, ty, bits) == Some(step)
                    && self.is_phi_operand(left, update_site, phi, scalar_work, facts)?))
        {
            return self
                .fail("a pipeline live-out recurrence is not its exact phi plus literal step");
        }
        let plan = self
            .source
            .owner
            .plan_for_function(self.source.function)
            .ok_or_else(|| reject("a pipeline live-out lost its original SSA plan"))?
            .plan();
        let mut incoming = 0;
        let mut seed = None;
        let mut backedge = false;
        for &predecessor in &proofs.graph.predecessors[induction.header] {
            self.charge(facts, 2)?;
            let block = SsaBlockIdV1::new(
                u32::try_from(predecessor).map_err(|_| resource(Resource::Arithmetic))?,
            );
            if !plan.is_reachable(block) {
                continue;
            }
            for edge in &self.occurrences.successors()
                [state.successor_offsets[predecessor]..state.successor_offsets[predecessor + 1]]
            {
                self.charge(facts, 3)?;
                if edge.edge().target().index() as usize != induction.header {
                    continue;
                }
                incoming += 1;
                let arguments = plan.edge_arguments(edge.id()).ok_or_else(|| {
                    reject("a pipeline live-out has no exact incoming SSA arguments")
                })?;
                let mut selected = None;
                for argument in arguments {
                    self.charge(facts, 1)?;
                    if argument.variable().get() as usize == local
                        && selected.replace(argument.value()).is_some()
                    {
                        return self
                            .fail("a pipeline live-out has duplicate incoming phi arguments");
                    }
                }
                let Some(SsaValueV1::Definition(id)) = selected else {
                    return self.fail(
                        "a pipeline live-out incoming value is not an exact original definition",
                    );
                };
                let Some(Origin::Assignment {
                    local: actual,
                    site,
                }) = self.definitions.get(id.get() as usize).copied().flatten()
                else {
                    return self.fail(
                        "a pipeline live-out incoming definition has no original assignment",
                    );
                };
                if actual != local {
                    return self
                        .fail("a pipeline live-out incoming definition changed source local");
                }
                if predecessor == induction.latch {
                    if site != latch_site || backedge {
                        return self.fail(
                            "a pipeline live-out changed its unique SSA backedge definition",
                        );
                    }
                    backedge = true;
                } else {
                    let initial = self.assignment(site)?;
                    let SemanticRvalueKindV1::Use(value) = initial.value().kind() else {
                        return self
                            .fail("a pipeline live-out requires a literal original initializer");
                    };
                    if initial.destination().ty() != ty
                        || initial.value().result_type() != ty
                        || seed
                            .replace(literal_unsigned(value, ty, bits).ok_or_else(|| {
                                reject(
                                    "a pipeline live-out requires a literal original initializer",
                                )
                            })?)
                            .is_some()
                    {
                        return self
                            .fail("a pipeline live-out has multiple or changed initializers");
                    }
                }
            }
        }
        if incoming != 2 || !backedge {
            return self.fail("a pipeline live-out requires one seed and one backedge");
        }
        let seed =
            seed.ok_or_else(|| reject("a pipeline live-out has no original literal seed"))?;
        self.charge(facts, 8)?;
        literal_exit(seed, bound, step, bits)
            .ok_or_else(|| reject("a pipeline live-out may wrap its original unsigned type"))
    }

    fn assignment(&self, site: ScalarAssignmentSiteV1) -> Result<&SemanticAssignmentV1> {
        match self
            .function
            .blocks()
            .get(site.block)
            .and_then(|block| block.statements().get(site.statement))
            .map(|statement| statement.kind())
        {
            Some(SemanticStatementKindV1::Assign(assignment)) => Ok(assignment),
            _ => self.fail("a pipeline live-out lost an original assignment"),
        }
    }

    fn is_phi_operand<'a>(
        &'a self,
        mut operand: &'a SemanticOperandV1,
        mut site: ScalarAssignmentSiteV1,
        phi: SsaValueV1,
        scalar_work: &mut usize,
        facts: &mut dyn ProjectedAssertionFactsV1,
    ) -> Result<bool> {
        while *scalar_work < MAX_PIPELINE_SCALAR_NODES_V1 {
            *scalar_work += 1;
            self.charge(facts, 3)?;
            let Some(place) =
                raw_operand_place(operand).filter(|place| place.projections().is_empty())
            else {
                return Ok(false);
            };
            let local = place.local().index() as usize;
            let value = self.resolved_value(self.function, local, site, facts)?;
            if value == phi {
                return Ok(true);
            }
            let SsaValueV1::Definition(id) = value else {
                return Ok(false);
            };
            let Some(Origin::Assignment {
                local: actual,
                site: producer,
            }) = self.definitions.get(id.get() as usize).copied().flatten()
            else {
                return Ok(false);
            };
            if actual != local {
                return Ok(false);
            }
            let assignment = self.assignment(producer)?;
            let SemanticRvalueKindV1::Use(next) = assignment.value().kind() else {
                return Ok(false);
            };
            if next.ty() != operand.ty() || assignment.destination().ty() != operand.ty() {
                return Ok(false);
            }
            operand = next;
            site = producer;
        }
        self.fail("a pipeline live-out alias proof exceeded the existing scalar node cap")
    }
}

fn literal_unsigned(operand: &SemanticOperandV1, ty: SemanticTypeIdV1, bits: u16) -> Option<u64> {
    if operand.ty() != ty || bits == 0 || bits > 64 {
        return None;
    }
    let SemanticOperandV1::Constant(constant) = operand else {
        return None;
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return None;
    };
    let value = u64::try_from(value.bits()).ok()?;
    (u128::from(value) < (1_u128 << bits)).then_some(value)
}

fn literal_exit(seed: u64, bound: u64, step: u64, bits: u16) -> Option<u64> {
    if step == 0 || bits == 0 || bits > 64 {
        return None;
    }
    let maximum = (1_u128 << bits) - 1;
    let (seed, bound, step) = (u128::from(seed), u128::from(bound), u128::from(step));
    if seed > maximum || bound > maximum || step > maximum {
        return None;
    }
    let result = if seed >= bound {
        seed
    } else {
        seed.checked_add((bound - seed).div_ceil(step).checked_mul(step)?)?
    };
    (result <= maximum).then_some(result as u64)
}
