// Source SSA identities and actual N recipes share one per-association cursor.
#[derive(Clone, Copy)]
struct UnitLocalBlockIndexV1 {
    id: BlockId,
    ordinal: usize,
    operation_base: usize,
    terminator_span: Option<usize>,
    synthetic_span: Option<usize>,
}

#[derive(Clone, Copy)]
struct UnitLocalDefinitionV1 {
    value: ValueId,
    block: usize,
    operation: Option<usize>,
    result: usize,
}

#[derive(Clone, Copy)]
struct UnitLocalSourceIndexV1 {
    key: [u64; 7],
    index: usize,
}

fn unit_local_source_key_v1(
    site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    subrole: Option<fe2o3_pliron::ProductionSemanticSsaEventRoleV1>,
) -> [u64; 7] {
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as E, ProductionSemanticSsaOccurrenceSiteV1 as S,
        ProductionSemanticSsaOperandRoleV1 as R,
    };
    let (site_tag, block, statement) = match site {
        S::Statement { block, statement } => (0, u64::from(block.get()), u64::from(statement)),
        S::Terminator { block } => (1, u64::from(block.get()), 0),
    };
    let (role_tag, operand) = match role {
        R::RvalueOperand(n) => (0, n),
        R::RvaluePlace => (1, 0),
        R::Destination => (2, 0),
        R::StoreValue => (3, 0),
        R::StoreDestination => (4, 0),
        R::AtomicAddress => (5, 0),
        R::AtomicValue => (6, 0),
        R::AtomicExpected => (7, 0),
        R::AtomicReplacement => (8, 0),
        R::AtomicDestination => (9, 0),
        R::StatementPlace => (10, 0),
        R::Assume => (11, 0),
        R::StorageLive => (12, 0),
        R::StorageDead => (13, 0),
        R::CallArgument(n) => (14, n),
        R::CallDestinationAddress => (15, 0),
        R::TailCallArgument(n) => (16, n),
        R::SwitchDiscriminant => (17, 0),
        R::DropPlace => (18, 0),
        R::AssertCondition => (19, 0),
        R::AssertMessage(n) => (20, n),
        R::ReturnValue => (21, 0),
        R::ElidedBorrowDestination => (22, 0),
    };
    let (event_tag, projection) = match subrole {
        None => (0, 0),
        Some(E::BaseUse) => (1, 0),
        Some(E::ProjectionIndexUse(n)) => (2, n),
        Some(E::MoveKill) => (3, 0),
        Some(E::DestinationDefine) => (4, 0),
        Some(E::StorageKill) => (5, 0),
    };
    [
        site_tag,
        block,
        statement,
        role_tag,
        u64::from(operand),
        event_tag,
        u64::from(projection),
    ]
}

#[derive(Clone, Copy, Default)]
struct UnitLocalLocalStateV1 {
    value: Option<usize>,
    allocation: Option<usize>,
}

struct SourceLocalCursorV1<'a, 'r> {
    input: &'a UnitLocalAssociationInputV1<'a>,
    rows: &'r mut SealedUnitLocalSourceV1,
    blocks: Vec<UnitLocalBlockIndexV1>,
    definitions: Vec<UnitLocalDefinitionV1>,
    event_index: Vec<UnitLocalSourceIndexV1>,
    constant_index: Vec<UnitLocalSourceIndexV1>,
    statement_index: Vec<UnitLocalSourceIndexV1>,
    locals: Vec<UnitLocalLocalStateV1>,
    operation_claims: Vec<bool>,
    event_claims: Vec<bool>,
    constant_claims: Vec<bool>,
    statement_claims: Vec<bool>,
    visited: Vec<bool>,
    memory: UnitLocalMemoryStateV1,
    core_control: usize,
    core_edge: usize,
    native_span: Option<(usize, usize, usize)>,
    staged: Option<(usize, usize)>,
}

impl<'a, 'r> SourceLocalCursorV1<'a, 'r> {
    fn value(&self, row: usize) -> Option<&UnitLocalValueRowV1> {
        self.rows.values.get(row)
    }

    fn error(&self, detail: &'static str) -> ProductionSemanticKirErrorV1 {
        unsupported(self.input.key.function.index(), None, None, detail)
    }

    fn physical_block(
        &self,
        block: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, &'a BasicBlock), ProductionSemanticKirErrorV1> {
        let found = assert_origin_find_v1(&self.blocks, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.id.cmp(&block))
        })
        .map_err(call_index_error_v1)?
        .ok_or_else(unit_local_mismatch_v1)?;
        let ordinal = self.blocks[found].ordinal;
        let body = self
            .input
            .physical_function
            .body
            .as_ref()
            .ok_or_else(unit_local_mismatch_v1)?;
        Ok((ordinal, &body.blocks[ordinal]))
    }

    fn native_block(
        &self,
        source: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, &'a BasicBlock), ProductionSemanticKirErrorV1> {
        self.physical_block(BlockId(source.index()), budget)
    }

    fn terminator_span(
        &self,
        source: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a SemanticKirTerminatorOperationSpanV1, ProductionSemanticKirErrorV1> {
        let index = assert_origin_find_v1(&self.blocks, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.id.0.cmp(&source.index()))
        })
        .map_err(call_index_error_v1)?
        .ok_or_else(unit_local_mismatch_v1)?;
        budget.charge_work(1)?;
        let span = self.blocks[index]
            .terminator_span
            .ok_or_else(unit_local_mismatch_v1)?;
        self.input
            .subject
            .correspondence
            .terminator_operation_spans
            .get(span)
            .ok_or_else(unit_local_mismatch_v1)
    }

    fn synthetic_trap_span(
        &self,
        block: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, &'a SemanticKirSyntheticOperationSpanV1), ProductionSemanticKirErrorV1>
    {
        let index = assert_origin_find_v1(&self.blocks, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.id.cmp(&block))
        })
        .map_err(call_index_error_v1)?
        .ok_or_else(unit_local_mismatch_v1)?;
        budget.charge_work(1)?;
        let span = self.blocks[index]
            .synthetic_span
            .ok_or_else(unit_local_mismatch_v1)?;
        let row = self
            .input
            .subject
            .correspondence
            .synthetic_operation_spans
            .get(span)
            .ok_or_else(unit_local_mismatch_v1)?;
        if row.rule != SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap {
            return Err(unit_local_mismatch_v1());
        }
        Ok((span, row))
    }

    fn peek_core_control(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<(usize, RetainedLocalControlV1)>, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        if self.core_control == self.input.body.control.1 {
            return Ok(None);
        }
        let row = self
            .input
            .physical
            .control
            .get(self.core_control)
            .copied()
            .ok_or_else(unit_local_mismatch_v1)?;
        Ok(Some((self.core_control, row)))
    }

    fn claim_core_control(
        &mut self,
        index: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if index != self.core_control || index >= self.input.body.control.1 {
            return Err(unit_local_mismatch_v1());
        }
        self.core_control = index.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(())
    }

    fn core_edge_bindings(
        &mut self,
        source: BlockId,
        successor: u8,
        target: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, usize), ProductionSemanticKirErrorV1> {
        let start = self.core_edge;
        while self.core_edge < self.input.body.edge_bindings.1 {
            budget.charge_work(5)?;
            let row = self.input.physical.edge_bindings[self.core_edge];
            if row.source() != source {
                break;
            }
            if row.function_ordinal() != self.input.key.physical
                || row.successor() != successor
                || row.target() != target
                || row.ordinal() != self.core_edge - start
            {
                return Err(unit_local_mismatch_v1());
            }
            self.core_edge += 1;
        }
        Ok((start, self.core_edge))
    }

    fn claim_native_operation(
        &mut self,
        block: BlockId,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let index = assert_origin_find_v1(&self.blocks, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.id.cmp(&block))
        })
        .map_err(call_index_error_v1)?
        .ok_or_else(unit_local_mismatch_v1)?;
        budget.charge_work(3)?;
        let native = &self
            .input
            .physical_function
            .body
            .as_ref()
            .ok_or_else(unit_local_mismatch_v1)?
            .blocks[self.blocks[index].ordinal];
        if operation >= native.operations.len() {
            return Err(unit_local_mismatch_v1());
        }
        let at = self.blocks[index]
            .operation_base
            .checked_add(operation)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let claim = self
            .operation_claims
            .get_mut(at)
            .ok_or_else(unit_local_mismatch_v1)?;
        if *claim {
            return Err(unit_local_mismatch_v1());
        }
        *claim = true;
        Ok(())
    }

    fn peek_native_operation(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        Option<(
            &'a Operation,
            fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        )>,
        ProductionSemanticKirErrorV1,
    > {
        budget.charge_work(3)?;
        let (block, next, end) = self.native_span.ok_or_else(unit_local_mismatch_v1)?;
        if next == end {
            return Ok(None);
        }
        let body = self
            .input
            .physical_function
            .body
            .as_ref()
            .ok_or_else(unit_local_mismatch_v1)?;
        let operation = body
            .blocks
            .get(block)
            .and_then(|b| b.operations.get(next))
            .ok_or_else(unit_local_mismatch_v1)?;
        let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                    u32::try_from(self.input.key.physical)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ),
                block: u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            },
            operation: u32::try_from(next).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        };
        Ok(Some((operation, coordinate)))
    }

    fn take_native_operation(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        (
            &'a Operation,
            fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        ),
        ProductionSemanticKirErrorV1,
    > {
        let (operation, coordinate) = self
            .peek_native_operation(budget)?
            .ok_or_else(unit_local_mismatch_v1)?;
        let body = self
            .input
            .physical_function
            .body
            .as_ref()
            .ok_or_else(unit_local_mismatch_v1)?;
        self.claim_native_operation(
            body.blocks[coordinate.block.block as usize].id,
            coordinate.operation as usize,
            budget,
        )?;
        let (block, next, end) = self.native_span.ok_or_else(unit_local_mismatch_v1)?;
        self.native_span = Some((
            block,
            next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?,
            end,
        ));
        Ok((operation, coordinate))
    }

    fn append_control(
        &mut self,
        row: UnitLocalControlRowV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        unit_local_push_v1(&mut self.rows.control, row, budget)
    }

    fn source_scalar(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ScalarType, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let types = self.input.subject.semantic_ssa.source_semantic().types();
        let declaration = types
            .get(ty.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?;
        let scalar = match declaration.shape() {
            SemanticTypeShapeV1::Scalar(scalar) => *scalar,
            _ => return Err(self.error("local helper value is not an unsigned scalar")),
        };
        if !matches!(
            scalar,
            SemanticScalarTypeV1::Bool
                | SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 8 | 16 | 32 | 64
                }
        ) {
            return Err(self.error("local helper scalar kind is outside the closed grammar"));
        }
        let lowered = lower_scalar_kind(scalar)?
            .as_scalar()
            .ok_or_else(unit_local_mismatch_v1)?;
        if !matches!(
            lowered,
            ScalarType::Bool | ScalarType::U8 | ScalarType::U16 | ScalarType::U32 | ScalarType::U64
        ) {
            return Err(self.error("local helper scalar kind is outside the closed grammar"));
        }
        Ok(lowered)
    }

    fn claim_source_event(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        subrole: fe2o3_pliron::ProductionSemanticSsaEventRoleV1,
        expected: fe2o3_mir_model::SsaEventV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, Option<fe2o3_mir_model::SsaResolvedEventV1>), ProductionSemanticKirErrorV1>
    {
        budget.charge_work(4)?;
        let key = unit_local_source_key_v1(site, role, Some(subrole));
        let found = unit_local_source_find_v1(&self.event_index, key, budget)?;
        let index = self.event_index[found].index;
        let event = &self.input.occurrences.events()[index];
        if event.event() != expected
            || !event.is_reachable()
            || self.event_claims[index]
            || event.is_promoted() != event.resolved().is_some()
        {
            return Err(unit_local_mismatch_v1());
        }
        let resolved = event.resolved();
        self.event_claims[index] = true;
        Ok((index, resolved))
    }

    fn source_use(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        subrole: fe2o3_pliron::ProductionSemanticSsaEventRoleV1,
        local: SemanticLocalIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(UnitLocalValueOriginV1, Option<SsaValueV1>), ProductionSemanticKirErrorV1> {
        let variable = fe2o3_mir_model::SsaVariableIdV1::new(local.index());
        let (occurrence, resolved) = self.claim_source_event(
            site,
            role,
            subrole,
            fe2o3_mir_model::SsaEventV1::Use(variable),
            budget,
        )?;
        budget.charge_work(3)?;
        let source_ssa = match resolved {
            Some(fe2o3_mir_model::SsaResolvedEventV1::Use {
                variable: actual,
                value,
            }) if actual == variable => {
                let row = self
                    .locals
                    .get(local.index() as usize)
                    .and_then(|s| s.value)
                    .and_then(|i| self.value(i))
                    .ok_or_else(unit_local_mismatch_v1)?;
                if row.source_ssa != Some(value) {
                    return Err(unit_local_mismatch_v1());
                }
                Some(value)
            }
            None => None,
            _ => return Err(unit_local_mismatch_v1()),
        };
        Ok((UnitLocalValueOriginV1::Event { occurrence }, source_ssa))
    }

    fn check_native_use(
        &self,
        native: UnitLocalNativeValueV1,
        use_kind: UnitLocalOperandUseV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<UnitLocalNativeValueV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        match (use_kind, native) {
            (UnitLocalOperandUseV1::Diagnostic, _) => Ok(UnitLocalNativeValueV1::DiagnosticOnly),
            (
                UnitLocalOperandUseV1::Native(expected),
                UnitLocalNativeValueV1::Scalar { value, .. },
            ) if expected == value => Ok(native),
            (UnitLocalOperandUseV1::Next, _) => Ok(native),
            _ => Err(unit_local_mismatch_v1()),
        }
    }

    fn resolve_source_place(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        place: &SemanticPlaceV1,
        origin: UnitLocalValueOriginV1,
        source_ssa: Option<SsaValueV1>,
        use_kind: UnitLocalOperandUseV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let state = *self
            .locals
            .get(place.local().index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?;
        if state.allocation.is_some() {
            return self.read_memory_place(site, role, place, origin, source_ssa, use_kind, budget);
        }
        if !place.projections().is_empty() {
            return Err(self.error("local helper projected scalar is unsupported"));
        }
        let input = state.value.ok_or_else(unit_local_mismatch_v1)?;
        let row = *self.value(input).ok_or_else(unit_local_mismatch_v1)?;
        if row.source_type != place.ty() || row.source_ssa != source_ssa {
            return Err(unit_local_mismatch_v1());
        }
        let native = self.check_native_use(row.native, use_kind, budget)?;
        self.rows.append_value(
            UnitLocalValueRowV1 {
                key: self.input.key,
                origin,
                role: Some(role),
                source_type: place.ty(),
                source_ssa,
                native,
                recipe: UnitLocalValueRecipeV1::Copy { input },
                known_bits: row.known_bits,
            },
            budget,
        )
    }

    fn resolve_index_local(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        projection: u32,
        local: SemanticLocalIdV1,
        use_kind: UnitLocalOperandUseV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let (origin, source_ssa) = self.source_use(
            site,
            role,
            fe2o3_pliron::ProductionSemanticSsaEventRoleV1::ProjectionIndexUse(projection),
            local,
            budget,
        )?;
        budget.charge_work(2)?;
        let ty = self
            .input
            .source
            .locals()
            .get(local.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?
            .ty();
        let place =
            SemanticPlaceV1::new(local, Vec::new(), ty).map_err(|_| unit_local_mismatch_v1())?;
        self.resolve_source_place(site, role, &place, origin, source_ssa, use_kind, budget)
    }

    fn invalidate_source(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        local: SemanticLocalIdV1,
        reason: UnitLocalInvalidateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        use fe2o3_pliron::ProductionSemanticSsaEventRoleV1 as E;
        let variable = fe2o3_mir_model::SsaVariableIdV1::new(local.index());
        if !matches!(reason, UnitLocalInvalidateV1::Deinitialize) {
            let subrole = if matches!(reason, UnitLocalInvalidateV1::Move) {
                E::MoveKill
            } else {
                E::StorageKill
            };
            let (_, resolved) = self.claim_source_event(
                site,
                role,
                subrole,
                fe2o3_mir_model::SsaEventV1::Kill(variable),
                budget,
            )?;
            budget.charge_work(3)?;
            if let Some(fe2o3_mir_model::SsaResolvedEventV1::Kill {
                variable: actual,
                previous,
            }) = resolved
            {
                let current = self
                    .locals
                    .get(local.index() as usize)
                    .and_then(|s| s.value)
                    .and_then(|v| self.value(v))
                    .and_then(|v| v.source_ssa);
                if actual != variable || previous != current {
                    return Err(unit_local_mismatch_v1());
                }
            } else if resolved.is_some() {
                return Err(unit_local_mismatch_v1());
            }
        }
        self.invalidate_memory_local(site, Some(role), local, reason, budget)?;
        self.locals
            .get_mut(local.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?
            .value = None;
        Ok(())
    }

    fn resolve_operand(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        operand: &SemanticOperandV1,
        use_kind: UnitLocalOperandUseV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                let moved = matches!(operand, SemanticOperandV1::Move(_));
                if moved && !place.projections().is_empty() {
                    return Err(self.error("local helper projected move is unsupported"));
                }
                let (origin, source_ssa) = self.source_use(
                    site,
                    role,
                    fe2o3_pliron::ProductionSemanticSsaEventRoleV1::BaseUse,
                    place.local(),
                    budget,
                )?;
                let row = self.resolve_source_place(
                    site, role, place, origin, source_ssa, use_kind, budget,
                )?;
                if moved {
                    self.invalidate_source(
                        site,
                        role,
                        place.local(),
                        UnitLocalInvalidateV1::Move,
                        budget,
                    )?;
                }
                Ok(row)
            }
            SemanticOperandV1::Constant(constant) => {
                budget.charge_work(3)?;
                let key = unit_local_source_key_v1(site, role, None);
                let found = unit_local_source_find_v1(&self.constant_index, key, budget)?;
                let occurrence = self.constant_index[found].index;
                if self.input.occurrences.constants()[occurrence].ty() != constant.ty()
                    || self.constant_claims[occurrence]
                {
                    return Err(unit_local_mismatch_v1());
                }
                self.constant_claims[occurrence] = true;
                let (native, recipe, known_bits) =
                    if let SemanticConstantValueV1::Scalar(scalar) = constant.value() {
                        let kind = self.source_scalar(constant.ty(), budget)?;
                        let expected = lower_constant(Type::Scalar(kind), *scalar)?;
                        let bits =
                            u64::try_from(scalar.bits()).map_err(|_| unit_local_mismatch_v1())?;
                        let native = if matches!(use_kind, UnitLocalOperandUseV1::Diagnostic) {
                            UnitLocalNativeValueV1::DiagnosticOnly
                        } else {
                            let (op, at) = self.take_native_operation(budget)?;
                            budget.charge_work(5)?;
                            let OperationKind::Constant(actual) = &op.kind else {
                                return Err(unit_local_mismatch_v1());
                            };
                            let actual_kind =
                                if kind == ScalarType::U64 && *actual == Constant::Index(bits) {
                                    ScalarType::Index
                                } else {
                                    kind
                                };
                            if (*actual != expected && actual_kind != ScalarType::Index)
                                || op.results.len() != 1
                                || op.results[0].ty != Type::Scalar(actual_kind)
                            {
                                return Err(unit_local_mismatch_v1());
                            }
                            self.check_native_use(
                                UnitLocalNativeValueV1::Scalar {
                                    value: op.results[0].id,
                                    scalar: actual_kind,
                                    definition: Some(at),
                                },
                                use_kind,
                                budget,
                            )?
                        };
                        (native, UnitLocalValueRecipeV1::Literal { bits }, Some(bits))
                    } else if matches!(constant.value(), SemanticConstantValueV1::ZeroSized)
                        && constant.ty() == self.input.unit_type
                    {
                        (
                            self.check_native_use(
                                UnitLocalNativeValueV1::IgnoredUnit,
                                use_kind,
                                budget,
                            )?,
                            UnitLocalValueRecipeV1::Unit,
                            None,
                        )
                    } else {
                        return Err(self.error("local helper constant recipe is unsupported"));
                    };
                self.rows.append_value(
                    UnitLocalValueRowV1 {
                        key: self.input.key,
                        origin: UnitLocalValueOriginV1::Constant { occurrence },
                        role: Some(role),
                        source_type: constant.ty(),
                        source_ssa: None,
                        native,
                        recipe,
                        known_bits,
                    },
                    budget,
                )
            }
        }
    }

    fn native_value(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<UnitLocalNativeValueV1, ProductionSemanticKirErrorV1> {
        let index = assert_origin_find_v1(&self.definitions, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.value.cmp(&value))
        })
        .map_err(call_index_error_v1)?
        .ok_or_else(unit_local_mismatch_v1)?;
        budget.charge_work(5)?;
        let definition = self.definitions[index];
        let body = self
            .input
            .physical_function
            .body
            .as_ref()
            .ok_or_else(unit_local_mismatch_v1)?;
        let block = &body.blocks[definition.block];
        let (result, coordinate) = match definition.operation {
            Some(operation) => (
                &block.operations[operation].results[definition.result],
                Some(fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                            u32::try_from(self.input.key.physical)
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        block: u32::try_from(definition.block)
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    },
                    operation: u32::try_from(operation)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                }),
            ),
            None => (&block.parameters[definition.result], None),
        };
        let scalar = result.ty.as_scalar().ok_or_else(unit_local_mismatch_v1)?;
        Ok(UnitLocalNativeValueV1::Scalar {
            value,
            scalar,
            definition: coordinate,
        })
    }

    fn stage_edge_bindings(
        &mut self,
        edge: fe2o3_mir_model::SsaEdgeIdV1,
        bindings: (usize, usize),
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, usize), ProductionSemanticKirErrorV1> {
        budget.charge_work(6)?;
        if self.staged.is_some()
            || bindings.0 > bindings.1
            || bindings.1 > self.input.body.edge_bindings.1
        {
            return Err(unit_local_mismatch_v1());
        }
        let successor = assert_origin_find_v1(
            self.input.occurrences.successors(),
            budget,
            |row, budget| {
                budget.charge_work(2)?;
                Ok((row.id().source().get(), row.id().ordinal())
                    .cmp(&(edge.source().get(), edge.ordinal())))
            },
        )
        .map_err(call_index_error_v1)?
        .ok_or_else(unit_local_mismatch_v1)?;
        let target = self.input.occurrences.successors()[successor]
            .edge()
            .target();
        let target_ssa = fe2o3_mir_model::SsaBlockIdV1::new(target.index());
        let variables = self
            .input
            .plan
            .plan()
            .transport_variables(target_ssa)
            .ok_or_else(unit_local_mismatch_v1)?;
        let arguments = self
            .input
            .plan
            .plan()
            .edge_arguments(edge)
            .ok_or_else(unit_local_mismatch_v1)?;
        if variables.len() != arguments.len()
            || !self
                .input
                .plan
                .plan()
                .edge_definitions(edge)
                .ok_or_else(unit_local_mismatch_v1)?
                .is_empty()
        {
            return Err(unit_local_mismatch_v1());
        }
        let (_, destination) = self.native_block(target, budget)?;
        if destination.parameters.len() != bindings.1 - bindings.0 {
            return Err(unit_local_mismatch_v1());
        }
        let start = self.rows.values.len();
        let mut physical = bindings.0;
        for (variable, argument) in variables.iter().zip(arguments) {
            budget.charge_work(9)?;
            if argument.variable() != *variable {
                return Err(unit_local_mismatch_v1());
            }
            let local = variable.get() as usize;
            let input = self
                .locals
                .get(local)
                .and_then(|row| row.value)
                .ok_or_else(unit_local_mismatch_v1)?;
            let row = *self.value(input).ok_or_else(unit_local_mismatch_v1)?;
            if row.source_ssa != Some(argument.value()) {
                return Err(unit_local_mismatch_v1());
            }
            let (native, physical_binding) = match row.native {
                UnitLocalNativeValueV1::IgnoredUnit if row.source_type == self.input.unit_type => {
                    (row.native, None)
                }
                UnitLocalNativeValueV1::Scalar { value, scalar, .. } => {
                    let binding = self
                        .input
                        .physical
                        .edge_bindings
                        .get(physical)
                        .copied()
                        .filter(|_| physical < bindings.1)
                        .ok_or_else(unit_local_mismatch_v1)?;
                    let parameter = &destination.parameters[physical - bindings.0];
                    if binding.argument() != value
                        || binding.parameter() != parameter.id
                        || binding.ty() != scalar
                        || parameter.ty != Type::Scalar(scalar)
                        || binding
                            .known_unsigned()
                            .is_some_and(|bits| row.known_bits != Some(bits))
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                    let native = self.native_value(parameter.id, budget)?;
                    let index = physical;
                    physical = physical
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    (native, Some(index))
                }
                _ => return Err(unit_local_mismatch_v1()),
            };
            self.rows.append_value(
                UnitLocalValueRowV1 {
                    key: self.input.key,
                    origin: UnitLocalValueOriginV1::Edge {
                        edge,
                        variable: *variable,
                    },
                    role: None,
                    source_type: row.source_type,
                    source_ssa: Some(SsaValueV1::BlockArgument {
                        block: target_ssa,
                        variable: *variable,
                    }),
                    native,
                    recipe: UnitLocalValueRecipeV1::Edge {
                        input,
                        physical_binding,
                    },
                    known_bits: row.known_bits,
                },
                budget,
            )?;
        }
        if physical != bindings.1 {
            return Err(unit_local_mismatch_v1());
        }
        let range = (start, self.rows.values.len());
        self.staged = Some(range);
        Ok(range)
    }

    fn commit_edge_bindings(
        &mut self,
        target: SemanticBlockIdV1,
        range: (usize, usize),
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        if self.staged.take() != Some(range)
            || range.0 > range.1
            || range.1 != self.rows.values.len()
        {
            return Err(unit_local_mismatch_v1());
        }
        budget.charge_work(argument_product_v1(range.1 - range.0, 3)?)?;
        for index in range.0..range.1 {
            let row = self.rows.values[index];
            let Some(SsaValueV1::BlockArgument { block, variable }) = row.source_ssa else {
                return Err(unit_local_mismatch_v1());
            };
            if block.get() != target.index() {
                return Err(unit_local_mismatch_v1());
            }
            self.locals
                .get_mut(variable.get() as usize)
                .ok_or_else(unit_local_mismatch_v1)?
                .value = Some(index);
        }
        Ok(())
    }

    fn append_unit_return(
        &mut self,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let local = self.input.unit_return_local;
        let existing = self
            .locals
            .get(local.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?
            .value;
        let source_ssa = if let Some(index) = existing {
            let row = self.value(index).ok_or_else(unit_local_mismatch_v1)?;
            if row.source_type != self.input.unit_type
                || !matches!(row.native, UnitLocalNativeValueV1::IgnoredUnit)
            {
                return Err(unit_local_mismatch_v1());
            }
            row.source_ssa
        } else {
            None
        };
        self.rows.append_value(
            UnitLocalValueRowV1 {
                key: self.input.key,
                origin: UnitLocalValueOriginV1::UnitReturn { block, local },
                role: None,
                source_type: self.input.unit_type,
                source_ssa,
                native: UnitLocalNativeValueV1::IgnoredUnit,
                recipe: UnitLocalValueRecipeV1::Unit,
                known_bits: None,
            },
            budget,
        )
    }
}
