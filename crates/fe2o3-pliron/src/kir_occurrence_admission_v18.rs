// V18-only conservative eight-pass envelope, also reused by the bounded
// two-pass integer continuation. Historical policies retain their limits.

#[derive(Clone, Copy, Debug)]
pub(crate) struct ObserverAdmissionV18 {
    source: StructuralCensus,
    operations: usize,
    results: usize,
    operands: usize,
    successors: usize,
    definition_arity: usize,
}

#[derive(Clone, Copy, Default)]
struct GrowthV18 {
    constants: usize,
    branches: usize,
    operands: usize,
}

fn admission_add_v18(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(E::Arithmetic)
}
fn admission_mul_v18(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or(E::Arithmetic)
}
fn admission_sum_v18(parts: &[usize]) -> Result<usize> {
    parts
        .iter()
        .try_fold(0, |sum, &part| admission_add_v18(sum, part))
}

impl ObserverAdmissionV18 {
    pub(crate) fn for_graph(
        ctx: &Context,
        root: Ptr<Operation>,
        source: &Module,
        budget: &mut Budget<'_>,
    ) -> Result<(Limits, Self)> {
        // Both canonical switch spellings import as native SwitchOpV3 and can
        // select a result-free branch just like CondBranchOp. Legacy policies
        // keep their original census through include_native_switches=false.
        let census = Limits::census_graph(ctx, root, source, budget, true)?;
        let limits = Limits::for_structure(census)?.for_policy3()?;
        Ok((limits, Self::for_structure(census)?))
    }

    fn for_structure(source: StructuralCensus) -> Result<Self> {
        Ok(Self {
            operations: admission_sum_v18(&[
                source.operations,
                source.values,
                source.conditional_branches,
            ])?,
            results: admission_add_v18(source.results, source.values)?,
            operands: admission_add_v18(source.operands, source.conditional_operands)?,
            successors: admission_add_v18(source.successors, source.conditional_branches)?,
            definition_arity: source.max_definition_arity.max(1),
            source,
        })
    }

    pub(crate) fn map_nodes(self) -> Result<usize> {
        // Map rows are operations and Values, not operand/edge occurrences.
        // SCCP adds at most one operation and one Value per original Value;
        // conditional selection adds a result-free branch operation.
        Ok(admission_sum_v18(&[
            self.source.operations,
            admission_mul_v18(3, self.source.values)?,
            self.source.conditional_branches,
        ])?
        .max(1))
    }

    pub(crate) fn definition_arity_bound(self) -> usize {
        self.definition_arity
    }

    fn validation(self) -> Result<usize> {
        // Per-event validation is cached by operation. Even if every use or
        // predecessor is visited, each operation's arity is scanned once.
        admission_sum_v18(&[
            admission_mul_v18(4, self.operands)?,
            admission_mul_v18(2, self.results)?,
            admission_mul_v18(
                admission_add_v18(self.source.max_successors.max(1), 4)?,
                self.successors,
            )?,
        ])
    }

    fn event_work(self) -> Result<usize> {
        let validation = self.validation()?;
        let value_replace = admission_sum_v18(&[
            32,
            admission_mul_v18(2, self.definition_arity)?,
            admission_mul_v18(3, self.operands)?,
            validation,
        ])?;
        // Phi removal includes the predecessor snapshot, operand position
        // search and both vector shifts. It is not a constant-time event.
        let argument_erase = admission_sum_v18(&[
            32,
            admission_mul_v18(2, self.definition_arity)?,
            admission_mul_v18(3, self.successors)?,
            validation,
            admission_mul_v18(
                3,
                admission_mul_v18(self.successors, self.source.max_operands)?,
            )?,
        ])?;
        let erase =
            admission_sum_v18(&[32, validation, self.operands, self.successors, self.results])?;
        let insert = admission_sum_v18(&[
            32,
            admission_mul_v18(8, self.source.max_operands)?,
            admission_mul_v18(8, self.definition_arity)?,
        ])?;
        let block = admission_sum_v18(&[
            32,
            admission_mul_v18(3, self.operations)?,
            validation,
            self.operands,
            self.successors,
            admission_mul_v18(2, self.definition_arity)?,
        ])?;
        Ok(value_replace
            .max(argument_erase)
            .max(erase)
            .max(insert)
            .max(block))
    }

    pub(crate) fn work(self, limits: Limits) -> Result<usize> {
        let census = admission_sum_v18(&[
            admission_mul_v18(4, limits.nodes)?,
            self.source.blocks,
            self.operations,
            admission_mul_v18(2, self.source.values)?,
            self.validation()?,
        ])?;
        // Initial registration, both coordinate rosters and row assembly are
        // linear. Twelve censuses cover initial, eight passes, final and margin.
        admission_sum_v18(&[
            admission_mul_v18(64, limits.nodes)?,
            admission_mul_v18(12, census)?,
            admission_mul_v18(limits.events, self.event_work()?)?,
        ])
    }

    pub(crate) fn additional_pass_work(self, limits: Limits, passes: usize) -> Result<usize> {
        // The census updates the same bounded registries in place. No copy of
        // a full graph is retained for each round; only new pass spans persist.
        admission_mul_v18(
            passes,
            admission_sum_v18(&[
                admission_mul_v18(4, limits.nodes)?,
                self.source.blocks,
                self.operations,
                admission_mul_v18(2, self.source.values)?,
                self.validation()?,
            ])?,
        )
    }

    pub(crate) fn map_work(
        self,
        limits: crate::kir_optimization_map_v12::CaptureLimitsV12,
    ) -> Result<usize> {
        let log = (usize::BITS - limits.node_limit().max(1).leading_zeros()) as usize;
        // Hash-table operations use the existing logical lookup convention.
        // All actual result/argument scans and all eight dead-argument scans
        // (which call upstream linear try_find_index) are included explicitly.
        admission_mul_v18(
            16,
            admission_sum_v18(&[
                admission_mul_v18(
                    limits.event_limit(),
                    admission_add_v18(self.definition_arity, 1)?,
                )?,
                admission_mul_v18(
                    limits.node_limit(),
                    admission_add_v18(
                        admission_mul_v18(8, admission_sum_v18(&[self.definition_arity, log, 4])?)?,
                        4,
                    )?,
                )?,
            ])?,
        )
    }

    fn definition_arity(self, actual: usize) -> Result<()> {
        if actual > self.definition_arity {
            Err(E::Limit)
        } else {
            Ok(())
        }
    }
    fn operation_shape(self, ctx: &Context, raw: Ptr<Operation>) -> Result<()> {
        let op = raw.deref(ctx);
        self.definition_arity(op.get_num_results())?;
        if op.num_regions() != 0
            || op.get_num_operands() > self.source.max_operands
            || op.get_num_successors() > self.source.max_successors.max(1)
        {
            return Err(E::Limit);
        }
        Ok(())
    }
    fn value_shape(self, ctx: &Context, raw: Value) -> Result<()> {
        self.definition_arity(if let Some(op) = raw.defining_op() {
            op.deref(ctx).get_num_results()
        } else {
            raw.defining_block()
                .ok_or(E::Coverage)?
                .deref(ctx)
                .get_num_arguments()
        })
    }
    fn precheck(self, ctx: &Context, event: RewriteEvent) -> Result<()> {
        match event {
            RewriteEvent::OperationInserted(op)
            | RewriteEvent::OperationErased(op)
            | RewriteEvent::OperationUnlinked(op) => self.operation_shape(ctx, op),
            RewriteEvent::OperationReplaced { old, new } => {
                self.operation_shape(ctx, old)?;
                self.operation_shape(ctx, new)
            }
            RewriteEvent::ValueReplaced { old, new } => {
                self.value_shape(ctx, old)?;
                self.value_shape(ctx, new)
            }
            RewriteEvent::BlockErased(block) => {
                self.definition_arity(block.deref(ctx).get_num_arguments())
            }
            RewriteEvent::BlockInserted(_)
            | RewriteEvent::BlockUnlinked(_)
            | RewriteEvent::RegionErased(_)
            | RewriteEvent::ValueTypeChanged { .. } => Err(E::UnsupportedMutation),
        }
    }
    fn register_growth(
        self,
        ctx: &Context,
        raw: Ptr<Operation>,
        growth: &mut GrowthV18,
    ) -> Result<()> {
        let op = raw.deref(ctx);
        if is_constant(ctx, raw) {
            if op.get_num_results() != 1
                || op.get_num_operands() != 0
                || op.get_num_successors() != 0
            {
                return Err(E::UnsupportedMutation);
            }
            let next = admission_add_v18(growth.constants, 1)?;
            if next > self.source.values {
                return Err(E::Limit);
            }
            growth.constants = next;
        } else if Operation::is_op::<dialect_gpu::optimization_v1::BranchOp>(raw, ctx) {
            if op.get_num_results() != 0 || op.get_num_successors() != 1 {
                return Err(E::UnsupportedMutation);
            }
            let branches = admission_add_v18(growth.branches, 1)?;
            let operands = admission_add_v18(growth.operands, op.get_num_operands())?;
            if branches > self.source.conditional_branches
                || operands > self.source.conditional_operands
            {
                return Err(E::Limit);
            }
            growth.branches = branches;
            growth.operands = operands;
        } else {
            return Err(E::UnsupportedMutation);
        }
        Ok(())
    }
}

impl Capture {
    pub(crate) fn new_v18(
        ctx: &Context,
        root: Ptr<Operation>,
        source: &Module,
        roster: &LiveRosterV12,
        limits: Limits,
        roster_work: usize,
        admission: ObserverAdmissionV18,
    ) -> Result<Self> {
        Self::new_for_policy_v18(
            ctx,
            root,
            source,
            roster,
            limits,
            roster_work,
            admission,
            FixedPolicy::Checked3,
        )
    }

    pub(crate) fn new_for_policy_v18(
        ctx: &Context,
        root: Ptr<Operation>,
        source: &Module,
        roster: &LiveRosterV12,
        limits: Limits,
        roster_work: usize,
        admission: ObserverAdmissionV18,
        policy: FixedPolicy,
    ) -> Result<Self> {
        // Neutral arithmetic creates at most one false constant per checked
        // binary; the remaining integer/CSE/DCE steps only replace or erase.
        // The first round fits the existing eight-pass scalar/CFG envelope.
        // Policy11 admits each further round before extending its census work.
        if !matches!(
            policy,
            FixedPolicy::Checked3
                | FixedPolicy::Integer6
                | FixedPolicy::IntegerWorklist9
                | FixedPolicy::MixedPureCse10
                | FixedPolicy::MixedFixedpoint11
        ) {
            return Err(E::Passes);
        }
        Ok(Self(Arc::new(Shared {
            policy,
            state: Mutex::new(State::new_admitted(
                ctx,
                root,
                source,
                roster,
                limits,
                roster_work,
                Some(admission),
            )?),
            poisoned: std::sync::atomic::AtomicBool::new(false),
        })))
    }
}

#[cfg(test)]
#[path = "kir_occurrence_admission_v18_tests.rs"]
mod admission_v18_tests;
