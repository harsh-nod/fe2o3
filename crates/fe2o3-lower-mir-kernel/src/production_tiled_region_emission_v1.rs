use super::*;

#[cfg(test)]
#[path = "production_tiled_region_result_uses_v1_tests.rs"]
mod result_uses_tests;

pub(super) fn components(binding: &SemanticValueBindingV1, role: Role) -> Result<[ValueId; 4]> {
    let mut result = [ValueId(0); 4];
    let values = match (role, binding) {
        (Role::Context, SemanticValueBindingV1::MatrixContext) => return Ok(result),
        (Role::Lane, SemanticValueBindingV1::WaveLane { value, wave }) if wave.width == 64 => {
            result[0] = *value;
            return Ok(result);
        }
        (
            Role::Lhs | Role::Rhs,
            SemanticValueBindingV1::MatrixFragment {
                values,
                contract,
                storage_layout,
                wave,
            },
        ) if *storage_layout == SemanticMfmaStorageLayoutV1::RowMajor
            && wave.width == 64
            && contract.wave_width == 64
            && contract.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
            && contract.register_distribution == SemanticMfmaRegisterDistributionV1::Tile16x16
            && contract.role
                == if role == Role::Lhs {
                    SemanticMfmaOperandRoleV1::A
                } else {
                    SemanticMfmaOperandRoleV1::B
                } =>
        {
            values
        }
        (
            Role::Zero | Role::Result,
            SemanticValueBindingV1::AccumulatorFragment {
                values,
                contract,
                wave,
            },
        ) if wave.width == 64
            && contract.wave_width == 64
            && contract.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
            && contract.distribution == SemanticMfmaAccumulatorDistributionV1::RowMajor =>
        {
            values
        }
        _ => return unavailable("live nominal binding kind/role/layout/wave differs"),
    };
    let expected = if matches!(role, Role::Lhs | Role::Rhs) {
        ScalarType::Bf16
    } else {
        ScalarType::F32
    };
    if values.len() != 4 {
        return unavailable("live nominal component arity");
    }
    for (index, (id, ty)) in values.iter().enumerate() {
        if *ty != Type::Scalar(expected) || result[..index].contains(id) {
            return unavailable("live component type/order/identity");
        }
        result[index] = *id;
    }
    Ok(result)
}
fn block<'a>(
    blocks: &'a [BasicBlock],
    id: BlockId,
    budget: &mut Budget<'_>,
) -> Result<&'a BasicBlock> {
    budget.charge_work(blocks.len())?;
    blocks
        .iter()
        .find(|block| block.id == id)
        .ok_or(Error::Unavailable("canonical block"))
}
pub(super) fn bounded_blocks(blocks: &[BasicBlock], budget: &mut Budget<'_>) -> Result<()> {
    // Canonical guarded expansions may add blocks; use operation cap as an
    // independent finite block cap rather than assume raw/source coordinates.
    if blocks.len() > OPERATIONS {
        return unavailable("canonical block cap");
    }
    let mut total = 0usize;
    for block in blocks {
        budget.charge_work(1)?;
        total = total
            .checked_add(block.operations.len())
            .ok_or(Resource::Arithmetic)?;
        if total > OPERATIONS {
            return unavailable("canonical operation scan cap");
        }
    }
    Ok(())
}

impl Recorder {
    /// Genuine emitter-only entry. The original live map owns its storage;
    /// this function borrows it and copies only the prepaid sparse component
    /// rows. A future paid-archive adapter must retain that archive's postflight
    /// and receipt instead of calling this with a reconstructed map.
    pub(in super::super) fn record_function(
        &mut self,
        plan: &LoweredFunctionPlanV1,
        lowering: &SemanticFunctionLoweringV1<'_>,
        blocks: &[BasicBlock],
        spans: &[SemanticKirTerminatorOperationSpanV1],
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(4)?;
        if self.captured
            || plan.semantic_function != self.root
            || plan.correspondence_owner != self.root
            || lowering.semantic_function != self.root
            || lowering.execution.is_some()
            || lowering.emission_placement != SemanticEmissionPlacementV1::default()
        {
            return unavailable("foreign/repeated/expanded function frame");
        }
        bounded_blocks(blocks, budget)?;
        for index in 0..self.alias_count {
            let row = *self.alias(index)?;
            budget.charge_work(
                2 + (usize::BITS - lowering.semantic_ssa_bindings.len().leading_zeros()) as usize,
            )?;
            let binding = lowering
                .semantic_ssa_bindings
                .get(&row.value)
                .ok_or(Error::Unavailable("actual selected SSA binding absent"))?;
            let actual = components(binding, row.role)?;
            match row.kind {
                AliasKind::Producer => {}
                AliasKind::Copy { from } => {
                    if actual != self.alias(from)?.components {
                        return unavailable(
                            "source move does not preserve actual component binding",
                        );
                    }
                }
                AliasKind::Edge { from, edge } => {
                    let SsaValueV1::BlockArgument { block: target, .. } = row.value else {
                        return unavailable("source edge target coordinate");
                    };
                    let target = block(blocks, BlockId(target.get()), budget)?;
                    let source = block(blocks, BlockId(edge.source().get()), budget)?;
                    let Some(Terminator::Branch {
                        target: actual_target,
                        arguments,
                    }) = &source.terminator
                    else {
                        return unavailable("non-single-edge fragment transport unavailable");
                    };
                    if edge.ordinal() != 0
                        || *actual_target != target.id
                        || arguments.len() != target.parameters.len()
                    {
                        return unavailable("actual fragment edge shape");
                    }
                    let before = self.alias(from)?.components;
                    for component in 0..row.role.width() {
                        budget.charge_work(target.parameters.len())?;
                        let mut slot = None;
                        for (index, parameter) in target.parameters.iter().enumerate() {
                            if parameter.id == actual[component] {
                                if slot.replace(index).is_some() {
                                    return unavailable("duplicate target component");
                                }
                            }
                        }
                        if arguments
                            .get(slot.ok_or(Error::Unavailable("actual fragment block parameter"))?)
                            != Some(&before[component])
                        {
                            return unavailable("actual fragment edge argument differs");
                        }
                    }
                }
            }
            self.aliases[index]
                .as_mut()
                .expect("selected alias")
                .components = actual;
        }
        for role in Role::ALL {
            let producer = self.producer(role)?;
            let mut selected = None;
            for span in spans {
                budget.charge_work(1)?;
                if span.correspondence_owner() == self.root
                    && span.semantic_function() == self.root
                    && span.semantic_block() == producer.block
                {
                    if selected.replace(*span).is_some() {
                        return unavailable("duplicate producer span");
                    }
                }
            }
            self.spans[role.index()] =
                Some(selected.ok_or(Error::Unavailable("missing actual producer span"))?);
        }
        self.captured = true;
        Ok(())
    }

    pub(super) fn operation<'a>(
        &self,
        owner: &'a ProductionPreRankedKirOwnerV1,
    ) -> Result<&'a Operation> {
        let (block, ordinal) = self.matrix.ok_or(Error::Unavailable("Matrix not sealed"))?;
        let function = owner
            .executable()
            .module()
            .functions
            .first()
            .ok_or(Error::Unavailable("canonical function"))?;
        function
            .body
            .as_ref()
            .and_then(|body| body.blocks.get(block))
            .and_then(|b| b.operations.get(ordinal as usize))
            .ok_or(Error::Unavailable("sealed Matrix coordinate"))
    }
    pub(super) fn seal(
        &mut self,
        owner: &ProductionPreRankedKirOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(4)?;
        if !self.captured {
            return unavailable("no genuine function capture");
        }
        let (function, trap_declaration) = declaration::select_root(
            owner.executable().module(),
            owner.correspondence.lowered_functions(),
            self.root,
            budget,
        )?;
        let trap_origins = declaration::Origins::from_owner(owner, self.root, budget)?;
        let body = function
            .body
            .as_ref()
            .ok_or(Error::Unavailable("source root has no body"))?;
        bounded_blocks(&body.blocks, budget)?;
        for role in Role::ALL {
            let span =
                self.spans[role.index()].ok_or(Error::Unavailable("missing capture span"))?;
            let mut matches = 0usize;
            for actual in owner.correspondence.terminator_operation_spans() {
                budget.charge_work(1)?;
                if *actual == span {
                    matches += 1;
                }
            }
            if matches != 1 {
                return unavailable("captured/current span identity mismatch");
            }
            let actual = block(&body.blocks, span.kernel_ir_block(), budget)?;
            let start = span.first_operation_ordinal() as usize;
            let end = start
                .checked_add(span.operation_count() as usize)
                .ok_or(Resource::Arithmetic)?;
            let operations = actual
                .operations
                .get(start..end)
                .ok_or(Error::Unavailable("producer span coverage"))?;
            let values = self.producer_alias(role)?.components;
            for component in 0..role.width() {
                let mut definition = None;
                for operation in operations {
                    budget.charge_work(operation.results.len() + 1)?;
                    for result in &operation.results {
                        if result.id == values[component] {
                            if definition.replace(operation).is_some() {
                                return unavailable("duplicate producer component definition");
                            }
                        }
                    }
                }
                let definition = definition.ok_or(Error::Unavailable(
                    "producer component outside actual source span",
                ))?;
                if role == Role::Zero
                    && !matches!(
                        definition.kind,
                        OperationKind::Constant(Constant::F32Bits(0))
                    )
                {
                    return unavailable("zero producer does not define actual f32 zero");
                }
                if role == Role::Lane
                    && !matches!(&definition.kind,OperationKind::Wave(w) if w.kind==WaveOperationKind::LaneId)
                {
                    return unavailable(
                        "current-lane producer does not define actual lane operation",
                    );
                }
            }
        }
        let matrix_span = self.spans[Role::Result.index()].expect("checked span");
        let mut trap_calls = 0usize;
        for (block_index, block) in body.blocks.iter().enumerate() {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                budget.charge_work(1)?;
                if matches!(operation.kind, OperationKind::Call { .. }) {
                    if !trap_declaration {
                        return unavailable("Trap call lacks the exact appended declaration");
                    }
                    trap_origins.validate_call(block, ordinal, budget)?;
                    trap_calls = trap_calls.checked_add(1).ok_or(Resource::Arithmetic)?;
                }
                if let OperationKind::Matrix(matrix) = &operation.kind {
                    if self.matrix.replace((block_index, ordinal as u32)).is_some() {
                        return unavailable("extra Matrix operation");
                    }
                    let end = matrix_span
                        .first_operation_ordinal()
                        .checked_add(matrix_span.operation_count())
                        .ok_or(Resource::Arithmetic)?;
                    if block.id != matrix_span.kernel_ir_block()
                        || ordinal < (matrix_span.first_operation_ordinal() as usize)
                        || ordinal >= end as usize
                    {
                        return unavailable("Matrix outside actual source call span");
                    }
                    let lhs = self.alias(self.mfma_arguments[1])?.components;
                    let rhs = self.alias(self.mfma_arguments[2])?.components;
                    let accumulator = self.alias(self.mfma_arguments[3])?.components;
                    let expected = MatrixOperation::multiply_accumulate(lhs, rhs, accumulator)
                        .with_declared_tensor_layout(
                            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                                .with_zero_filled_predicate_inputs(),
                        );
                    if *matrix != expected {
                        return unavailable("actual Matrix operands/profile differ");
                    }
                    let results = self.producer_alias(Role::Result)?.components;
                    matrix_results(operation, results)?;
                }
            }
        }
        declaration::require_declaration_use(trap_declaration, trap_calls, budget)?;
        if self.matrix.is_none() {
            return unavailable("Matrix missing");
        }
        let results = self.producer_alias(Role::Result)?.components;
        self.capture_result_uses(&body.blocks, results, budget)
    }

    // Called only after the genuine Matrix, all four actual definitions and
    // source correspondence have been sealed. Completeness means every actual
    // operand occurrence, not that the author must consume every result.
    // This bounded scanner is separately exercised by inert tests; they do not
    // construct an inspection view or establish source custody.
    pub(super) fn capture_result_uses(
        &mut self,
        blocks: &[BasicBlock],
        results: [ValueId; 4],
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        if self.use_count != 0 {
            return unavailable("result uses already captured");
        }
        bounded_blocks(blocks, budget)?;
        for block in blocks {
            budget.charge_work(1)?;
            for (ordinal, operation) in block.operations.iter().enumerate() {
                budget.charge_work(1)?;
                let mut operand = 0u32;
                operation.kind.try_visit_operands(|value| -> Result<()> {
                    budget.charge_work(1)?;
                    self.record_use(results, block.id, Some(ordinal as u32), operand, value)?;
                    operand = operand.checked_add(1).ok_or(Resource::Arithmetic)?;
                    Ok(())
                })?;
            }
            let mut operand = 0u32;
            visit_terminator(
                block
                    .terminator
                    .as_ref()
                    .ok_or(Error::Unavailable("canonical terminator"))?,
                |value| {
                    budget.charge_work(1)?;
                    self.record_use(results, block.id, None, operand, value)?;
                    operand = operand.checked_add(1).ok_or(Resource::Arithmetic)?;
                    Ok(())
                },
            )?;
        }
        // Zero uses for any component are truthful, including an entirely
        // unused result when the genuine selected Matrix remains in this owner.
        // No row is synthesized to make a physical definition appear live.
        Ok(())
    }
    pub(super) fn record_use(
        &mut self,
        results: [ValueId; 4],
        block: BlockId,
        operation: Option<u32>,
        operand: u32,
        value: ValueId,
    ) -> Result<()> {
        if let Some(component) = results.iter().position(|v| *v == value) {
            if self.use_count == USES {
                return unavailable("result use cap");
            }
            self.uses[self.use_count] = Some(ProductionBf16MfmaResultUseV1 {
                block,
                operation,
                operand,
                component: component as u8,
            });
            self.use_count += 1;
        }
        Ok(())
    }
}
pub(super) fn matrix_results(operation: &Operation, results: [ValueId; 4]) -> Result<()> {
    if operation.results.len() != 4
        || operation
            .results
            .iter()
            .zip(results)
            .any(|(actual, id)| actual.id != id || actual.ty != Type::Scalar(ScalarType::F32))
    {
        return unavailable("actual Matrix result coverage/order");
    }
    Ok(())
}

fn visit_terminator(
    terminator: &Terminator,
    mut visit: impl FnMut(ValueId) -> Result<()>,
) -> Result<()> {
    match terminator {
        Terminator::Branch { arguments, .. } | Terminator::Return { values: arguments } => {
            for &value in arguments {
                visit(value)?;
            }
        }
        Terminator::ConditionalBranch {
            condition,
            then_arguments,
            else_arguments,
            ..
        } => {
            visit(*condition)?;
            for &v in then_arguments.iter().chain(else_arguments) {
                visit(v)?;
            }
        }
        Terminator::Switch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            visit(*selector)?;
            for case in cases {
                for &v in &case.arguments {
                    visit(v)?;
                }
            }
            for &v in default_arguments {
                visit(v)?;
            }
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            visit(*selector)?;
            for case in cases {
                for &v in &case.arguments {
                    visit(v)?;
                }
            }
            for &v in default_arguments {
                visit(v)?;
            }
        }
        Terminator::Unreachable => {}
    }
    Ok(())
}
