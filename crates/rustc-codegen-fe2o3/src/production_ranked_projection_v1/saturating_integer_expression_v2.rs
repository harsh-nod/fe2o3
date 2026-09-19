use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCanonAbiV1, SemanticExternAbiV1, SemanticSaturatingIntegerOpV1,
};

impl<'a> GpuSemanticExpressionResolverV2<'a> {
    pub(super) fn with_scalar_callables_v1(
        mut self,
        callables: &'a [SemanticCallableDeclV1],
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        if callables.is_empty() {
            return Ok(self);
        }
        let admit_defined = self.helper_values.is_some();
        let is_saturation = |call: &SemanticDirectCallV1| {
            matches!(
                callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::SaturatingInteger(_),
                    ..
                })
            ) || (admit_defined
                && matches!(
                    callables.get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::Defined { .. })
                ))
        };
        let mut candidate = false;
        for body in self.function.blocks() {
            self.definitions.charge(1)?;
            if let SemanticTerminatorKindV1::Call(call) = body.terminator().kind()
                && is_saturation(call)
            {
                candidate = true;
                break;
            }
        }
        if !candidate {
            return Ok(self);
        }
        let locals = self.function.locals().len();
        if locals > MAX_RANKED_BOUNDS_OPERATIONS * 4 {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "GPU scalar call index exceeds the local limit",
            ));
        }
        self.definitions.charge(locals)?;
        self.scalar_calls.try_reserve_exact(locals).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "GPU scalar call index allocation failed",
            )
        })?;
        self.scalar_calls.resize(locals, None);
        self.scalar_callables = callables;
        for (block, body) in self.function.blocks().iter().enumerate() {
            self.definitions.charge(1)?;
            let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
                continue;
            };
            if !is_saturation(call) {
                continue;
            }
            let Some(destination) = call.destination() else {
                continue;
            };
            let Some(local) = local_definition_index(destination.place()) else {
                continue;
            };
            let slot = self.scalar_calls.get_mut(local).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "GPU scalar call destination is out of bounds",
                ),
            )?;
            // The complete existing source definition census includes all call
            // destinations and all direct/projected writes, not only this intrinsic.
            if destination.place().projections().is_empty()
                && self.definitions.definition_counts.get(local).copied() == Some(1)
            {
                *slot = Some((block, call));
            }
        }
        Ok(self)
    }

    pub(super) fn resolve_saturating_call_local_v2(
        &mut self,
        local: usize,
        use_site: ScalarAssignmentSiteV1,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        let (block, call) = self
            .scalar_calls
            .get(local)
            .copied()
            .flatten()
            .ok_or("GPU semantic local has no exact reaching assignment")?;
        if self.definitions.definition_counts.get(local).copied() != Some(1)
            || self.definitions.address_escaped.get(local).copied() != Some(false)
        {
            return Err("GPU scalar intrinsic result is ambiguous or escaped");
        }
        self.require_live_scalar_call_v1(local, block, use_site)?;
        let definition = ScalarAssignmentSiteV1 {
            block,
            statement: self.function.blocks()[block].statements().len(),
        };
        let key = (local as u32, definition.block, definition.statement);
        if !self.visiting.insert(key) {
            return Err("GPU semantic scalar local has a cyclic definition");
        }
        let previous = self.use_site.replace(definition);
        let result = if matches!(
            self.scalar_callables.get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::Defined { .. })
        ) {
            self.resolve_defined_call_v1(block, call, depth)
        } else {
            self.resolve_saturating_call_v2(call, depth)
        };
        self.use_site = previous;
        self.visiting.remove(&key);
        result
    }

    fn source_charge_v1(&mut self, amount: usize) -> Result<(), &'static str> {
        self.definitions
            .charge(amount)
            .map_err(|_| "GPU scalar call source analysis exceeds its work limit")
    }

    // Walk backwards from this exact use, stopping only after the defining
    // call. Every reachable incoming path must encounter it before a kill or
    // entry. The existing CFG coalesces parallel edges to the same block;
    // this is valid here because call results have no edge-specific arguments.
    // Each full block is scanned once.
    pub(super) fn require_live_scalar_call_v1(
        &mut self,
        local: usize,
        definition: usize,
        site: ScalarAssignmentSiteV1,
    ) -> Result<(), &'static str> {
        let count = self.function.blocks().len();
        if self.definitions.graph.reachable.get(site.block).copied() != Some(true) {
            return Err("GPU scalar intrinsic result use is unreachable");
        }
        self.source_charge_v1(
            count
                .checked_mul(2)
                .ok_or("GPU scalar call scratch overflow")?,
        )?;
        let mut visited = Vec::new();
        visited
            .try_reserve_exact(count)
            .map_err(|_| "GPU scalar call visited allocation failed")?;
        visited.resize(count, false);
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(count)
            .map_err(|_| "GPU scalar call worklist allocation failed")?;
        let mut current = Some((site.block, site.statement, false));
        while let Some((block, before, full)) = current.take().or_else(|| pending.pop()) {
            self.source_charge_v1(1)?;
            if full && block == definition {
                continue;
            }
            let body = &self.function.blocks()[block];
            if full && !self.scalar_call_terminator_preserves_v1(body.terminator().kind(), local)? {
                return Err("GPU scalar intrinsic result was killed before its use");
            }
            for statement in &body.statements()[..before] {
                if !self.scalar_call_statement_preserves_v1(statement.kind(), local)? {
                    return Err("GPU scalar intrinsic result was killed before its use");
                }
            }
            if block == self.definitions.graph.entry {
                return Err("GPU scalar intrinsic result is not defined on every path");
            }
            let predecessors = self.definitions.graph.predecessors[block].len();
            self.source_charge_v1(predecessors)?;
            for index in 0..predecessors {
                let predecessor = self.definitions.graph.predecessors[block][index];
                if self.definitions.graph.reachable[predecessor] && !visited[predecessor] {
                    visited[predecessor] = true;
                    pending.push((
                        predecessor,
                        self.function.blocks()[predecessor].statements().len(),
                        true,
                    ));
                }
            }
        }
        Ok(())
    }

    fn scalar_call_operand_preserves_v1(
        &mut self,
        operand: &SemanticOperandV1,
        local: usize,
    ) -> Result<bool, &'static str> {
        self.source_charge_v1(1)?;
        Ok(!matches!(operand, SemanticOperandV1::Move(place)
            if local_definition_index(place) == Some(local)))
    }

    fn scalar_call_statement_preserves_v1(
        &mut self,
        kind: &SemanticStatementKindV1,
        local: usize,
    ) -> Result<bool, &'static str> {
        self.source_charge_v1(1)?;
        let mut overwritten = false;
        visit_statement_definition_places(kind, &mut |place| {
            overwritten |= local_definition_index(place) == Some(local);
        });
        if overwritten {
            return Ok(false);
        }
        match kind {
            SemanticStatementKindV1::Assign(assignment) => {
                let mut preserved = true;
                assignment.value().kind().try_visit_operands(|operand| {
                    preserved &= self.scalar_call_operand_preserves_v1(operand, local)?;
                    Ok::<(), &'static str>(())
                })?;
                Ok(preserved)
            }
            SemanticStatementKindV1::Store(store) => {
                self.scalar_call_operand_preserves_v1(store.value(), local)
            }
            SemanticStatementKindV1::Assume(operand) => {
                self.scalar_call_operand_preserves_v1(operand, local)
            }
            SemanticStatementKindV1::StorageLive(id) | SemanticStatementKindV1::StorageDead(id) => {
                Ok(id.index() as usize != local)
            }
            SemanticStatementKindV1::Nop
            | SemanticStatementKindV1::Deinitialize(_)
            | SemanticStatementKindV1::SetDiscriminant { .. } => Ok(true),
            // No atomic operand is silently omitted from the new value recipe.
            SemanticStatementKindV1::AtomicRmw(_)
            | SemanticStatementKindV1::AtomicCompareExchange(_) => Ok(false),
        }
    }

    fn scalar_call_terminator_preserves_v1(
        &mut self,
        kind: &SemanticTerminatorKindV1,
        local: usize,
    ) -> Result<bool, &'static str> {
        self.source_charge_v1(1)?;
        match kind {
            SemanticTerminatorKindV1::Goto(_) | SemanticTerminatorKindV1::FalseEdge { .. } => {
                Ok(true)
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                self.scalar_call_operand_preserves_v1(discriminant, local)
            }
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    if !self.scalar_call_operand_preserves_v1(operand, local)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                if !self.scalar_call_operand_preserves_v1(condition, local)? {
                    return Ok(false);
                }
                let (first, second) = match message {
                    SemanticAssertMessageV1::BoundsCheck { length, index } => {
                        (Some(length), Some(index))
                    }
                    SemanticAssertMessageV1::Overflow { left, right, .. } => {
                        (Some(left), Some(right))
                    }
                    SemanticAssertMessageV1::MisalignedPointerDereference {
                        required_alignment,
                        found_alignment,
                    } => (Some(required_alignment), Some(found_alignment)),
                    SemanticAssertMessageV1::DivisionByZero(value)
                    | SemanticAssertMessageV1::RemainderByZero(value) => (Some(value), None),
                    SemanticAssertMessageV1::NullPointerDereference
                    | SemanticAssertMessageV1::ResumedAfterReturn
                    | SemanticAssertMessageV1::ResumedAfterPanic => (None, None),
                };
                for operand in first.into_iter().chain(second) {
                    if !self.scalar_call_operand_preserves_v1(operand, local)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn resolve_saturating_call_v2(
        &mut self,
        call: &'a SemanticDirectCallV1,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        Self::require_depth_v2(depth)?;
        self.charge_v2()?;
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::SaturatingInteger(operation),
            ..
        }) = self.scalar_callables.get(call.callee().index() as usize)
        else {
            return Err("GPU scalar call is not an authenticated saturation intrinsic");
        };
        let destination = call
            .destination()
            .ok_or("GPU scalar call has no destination")?;
        let ty = destination.place().ty();
        let abi = binding.abi();
        if call.arguments().len() != 2
            || call.arguments().iter().any(|operand| operand.ty() != ty)
            || !call.variadic_argument_abis().is_empty()
            || !destination.place().projections().is_empty()
            || self
                .function
                .locals()
                .get(destination.place().local().index() as usize)
                .is_none_or(|local| local.ty() != ty)
            || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
            || abi.canon_abi() != SemanticCanonAbiV1::Rust
            || abi.extern_abi() != SemanticExternAbiV1::Rust
            || abi.c_variadic()
            || abi.can_unwind()
            || abi.source_input_types() != [ty, ty]
            || abi.source_output_type() != ty
            || abi.return_type() != ty
        {
            return Err("GPU saturation requires exact non-unwinding (T, T) -> T");
        }
        let scalar = self.scalar_v2(ty)?;
        let ProductionSemanticScalarTypeV2::Integer { signed, bits } = scalar else {
            return Err("GPU saturation requires a fixed 8/16/32/64-bit integer");
        };
        if !matches!(bits, 8 | 16 | 32 | 64) {
            return Err("GPU saturation requires a fixed 8/16/32/64-bit integer");
        }
        let added = if signed {
            5
        } else if *operation == SemanticSaturatingIntegerOpV1::Add {
            3
        } else {
            2
        };
        let operand_depth = depth
            .checked_add(added)
            .ok_or("GPU saturation depth overflow")?;
        Self::require_depth_v2(operand_depth)?;
        let lhs = self.resolve_operand_v2(&call.arguments()[0], operand_depth)?;
        let rhs = self.resolve_operand_v2(&call.arguments()[1], operand_depth)?;
        self.totalize_saturating_integer_v2(*operation, scalar, lhs, rhs)
    }

    pub(super) fn totalize_saturating_integer_v2(
        &mut self,
        operation: SemanticSaturatingIntegerOpV1,
        scalar: ProductionSemanticScalarTypeV2,
        lhs: ProductionSemanticExpressionV2,
        rhs: ProductionSemanticExpressionV2,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        let ProductionSemanticScalarTypeV2::Integer { signed, bits } = scalar else {
            return Err("GPU saturation requires a fixed 8/16/32/64-bit integer");
        };
        if !matches!(bits, 8 | 16 | 32 | 64) {
            return Err("GPU saturation requires a fixed 8/16/32/64-bit integer");
        }
        if lhs.scalar() != scalar || rhs.scalar() != scalar {
            return Err("GPU saturation operand types disagree");
        }
        let left = lhs
            .validate()
            .map_err(|_| "GPU saturation left operand is invalid")?;
        let right = rhs
            .validate()
            .map_err(|_| "GPU saturation right operand is invalid")?;
        let (fixed, l, r, added): (usize, usize, usize, usize) = match (signed, operation) {
            (false, SemanticSaturatingIntegerOpV1::Subtract) => (4, 2, 2, 2),
            (false, SemanticSaturatingIntegerOpV1::Add) => (5, 3, 2, 3),
            (true, SemanticSaturatingIntegerOpV1::Subtract) => (13, 5, 3, 5),
            (true, SemanticSaturatingIntegerOpV1::Add) => (14, 5, 4, 5),
        };
        let nodes = left
            .nodes
            .checked_mul(l)
            .and_then(|n| right.nodes.checked_mul(r).and_then(|m| n.checked_add(m)))
            .and_then(|n| n.checked_add(fixed))
            .ok_or("GPU saturation node count overflow")?;
        Self::require_depth_v2(
            left.depth
                .max(right.depth)
                .checked_add(added)
                .ok_or("GPU saturation depth overflow")?,
        )?;
        self.work = self
            .work
            .checked_add(nodes)
            .ok_or("GPU semantic expression work overflowed")?;
        if self.work > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 {
            return Err("GPU semantic expression exceeds its bounded node budget");
        }
        // All copied subtrees and new boxes are paid before construction.
        let constant = |bits| ProductionSemanticExpressionV2::Constant { scalar, bits };
        let binary = |operation, lhs, rhs| ProductionSemanticExpressionV2::Binary {
            operation,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
        let less = |lhs, rhs| ProductionSemanticExpressionV2::Compare {
            operation: ProductionSemanticComparisonV2::LessThan,
            operand_scalar: scalar,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
        let select = |condition, when_true, when_false| ProductionSemanticExpressionV2::Select {
            scalar,
            condition: Box::new(condition),
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        };
        let raw = binary(
            match operation {
                SemanticSaturatingIntegerOpV1::Add => ProductionSemanticBinaryOpV2::Add,
                SemanticSaturatingIntegerOpV1::Subtract => ProductionSemanticBinaryOpV2::Subtract,
            },
            lhs.clone(),
            rhs.clone(),
        );
        let expression = if signed {
            let changed = binary(
                ProductionSemanticBinaryOpV2::BitXor,
                lhs.clone(),
                raw.clone(),
            );
            let other = match operation {
                SemanticSaturatingIntegerOpV1::Add => {
                    binary(ProductionSemanticBinaryOpV2::BitXor, rhs, raw.clone())
                }
                SemanticSaturatingIntegerOpV1::Subtract => {
                    binary(ProductionSemanticBinaryOpV2::BitXor, lhs.clone(), rhs)
                }
            };
            let overflow = less(
                binary(ProductionSemanticBinaryOpV2::BitAnd, changed, other),
                constant(0),
            );
            let sign = 1_u64 << (bits - 1);
            let clamp = select(less(lhs, constant(0)), constant(sign), constant(sign - 1));
            select(overflow, clamp, raw)
        } else {
            let (overflow, clamp) = match operation {
                SemanticSaturatingIntegerOpV1::Add => (
                    less(raw.clone(), lhs),
                    constant(((1_u128 << bits) - 1) as u64),
                ),
                SemanticSaturatingIntegerOpV1::Subtract => (less(lhs, rhs), constant(0)),
            };
            select(overflow, clamp, raw)
        };
        let actual = expression
            .validate()
            .map_err(|_| "GPU saturation expansion exceeds typed resource bounds")?;
        if actual.nodes != nodes {
            return Err("GPU saturation node accounting disagrees");
        }
        Ok(expression)
    }
}
