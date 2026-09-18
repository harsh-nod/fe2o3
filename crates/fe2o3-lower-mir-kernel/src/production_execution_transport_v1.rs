// Ordinary-owner preflight, not a mixed-V29 classifier or issuance authority.
// Checking every declaration once also covers roles hidden by empty arrays,
// inactive enum variants, or unused type rows. The existing V29 gate remains.
fn require_execution_free_types_v29(
    types: &[SemanticTypeDeclV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(types.len())?;
    for declaration in types {
        require_ordinary_execution_representation_v29(declaration)?;
    }
    Ok(())
}

// These hooks preserve already-bound nominal values. They cannot issue a role
// or establish source custody, reaching definitions, or scope lifetime proofs.
impl SemanticFunctionLoweringV1<'_> {
    fn execution_transport_error_v29(
        &self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        detail: &'static str,
    ) -> ProductionSemanticKirErrorV1 {
        unsupported(
            self.semantic_function.index(),
            Some(block.index()),
            statement,
            detail,
        )
    }

    fn try_lower_execution_borrow_v29(
        &self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let (source, kind) = match value {
            SemanticRvalueKindV1::Borrow { place, kind } => (place, Some(*kind)),
            SemanticRvalueKindV1::AddressOf { place, .. } => (place, None),
            _ => return Ok(None),
        };
        let Some(binding) = self
            .locals
            .get(source.local().index() as usize)
            .and_then(Option::as_ref)
        else {
            return Ok(None);
        };
        if !semantic_binding_contains_execution_v29(binding) {
            return Ok(None);
        }
        let error = |detail| self.execution_transport_error_v29(block, statement, detail);
        let kind =
            kind.ok_or_else(|| error("execution roles cannot acquire a physical address"))?;
        let instance = self
            .execution_instance
            .ok_or_else(|| error("execution borrow lacks a source call instance"))?;
        let ordinal =
            statement.ok_or_else(|| error("execution borrow lacks a source statement"))? as usize;
        let source_statement = self
            .function
            .blocks()
            .get(block.index() as usize)
            .and_then(|block| block.statements().get(ordinal))
            .ok_or_else(|| error("execution borrow source statement is unavailable"))?;
        let SemanticStatementKindV1::Assign(assignment) = source_statement.kind() else {
            return Err(error(
                "execution borrow is not its retained source assignment",
            ));
        };
        if !std::ptr::eq(assignment.value().kind(), value)
            || assignment.destination().ty() != result_type
        {
            return Err(error(
                "execution borrow differs from its retained source assignment",
            ));
        }
        let source_borrow = SemanticExecutionBorrowSourceV29 {
            instance,
            block,
            statement: ordinal,
            destination: assignment.destination(),
            kind,
            source,
        };
        let borrowed = match binding {
            SemanticValueBindingV1::Execution(binding) => {
                SemanticExecutionBorrowBindingV29::from_source(self.types, source_borrow, binding)
            }
            SemanticValueBindingV1::ExecutionBorrow(binding) => binding.reborrow(
                self.types,
                self.function.locals()[source.local().index() as usize].ty(),
                source_borrow,
            ),
            _ => Err("execution projected borrows require checked source transport"),
        }
        .map_err(error)?;
        Ok(Some(SemanticValueBindingV1::ExecutionBorrow(borrowed)))
    }

    fn try_lower_execution_operand_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        operand: &SemanticOperandV1,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let (place, moved) = match operand {
            SemanticOperandV1::Move(place) => (place, true),
            SemanticOperandV1::Copy(place) => (place, false),
            SemanticOperandV1::Constant(_) => return Ok(None),
        };
        let local = place.local().index() as usize;
        if !self
            .locals
            .get(local)
            .and_then(Option::as_ref)
            .is_some_and(semantic_binding_contains_execution_v29)
        {
            return Ok(None);
        }
        let function = self.semantic_function.index();
        let error = |detail| unsupported(function, Some(block.index()), statement, detail);
        if place.projections().len() > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(error(
                "execution operand projection exceeds its structural bound",
            ));
        }
        let mut selected = self.locals[local]
            .as_mut()
            .expect("live binding checked above");
        let mut ty = self.function.locals()[local].ty();
        for projection in place.projections() {
            let (SemanticValueBindingV1::Aggregate(fields), SemanticProjectionKindV1::Field(field)) =
                (selected, projection.kind())
            else {
                return Err(error(
                    "execution operand requires exact logical field transport",
                ));
            };
            let declaration = self
                .types
                .get(ty.index() as usize)
                .ok_or_else(|| error("execution operand aggregate type is unavailable"))?;
            let declared = match declaration.shape() {
                SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                    fields.fields()
                }
                _ => return Err(error("execution operand is not a logical aggregate")),
            };
            if declared.get(field as usize) != Some(&projection.result_type()) {
                return Err(error(
                    "execution operand projection changes the declared field type",
                ));
            }
            selected = fields
                .get_mut(field as usize)
                .ok_or_else(|| error("execution operand field is unavailable"))?;
            ty = projection.result_type();
        }
        if ty != place.ty() {
            return Err(error(
                "execution operand type differs from its source place",
            ));
        }
        match selected {
            SemanticValueBindingV1::MovedExecution => {
                return Err(error("execution operand has already been moved"));
            }
            SemanticValueBindingV1::Execution(binding) => {
                binding.check_type(self.types, place.ty()).map_err(error)?;
                if !moved {
                    return Err(error("owned execution roles cannot be copied"));
                }
            }
            SemanticValueBindingV1::ExecutionBorrow(binding) => {
                binding.check_type(self.types, place.ty()).map_err(error)?;
                if !moved && binding.kind() != SemanticBorrowKindV1::Shared {
                    return Err(error("mutable execution borrows cannot be copied"));
                }
            }
            SemanticValueBindingV1::ExecutionReferent(_) => {
                return Err(error(
                    "borrowed execution referents cannot become owned values",
                ));
            }
            SemanticValueBindingV1::Aggregate(_) if moved => {
                require_complete_execution_aggregate_v29(selected).map_err(error)?;
            }
            SemanticValueBindingV1::Aggregate(_) => {
                return Err(error("execution-bearing aggregates cannot be copied"));
            }
            SemanticValueBindingV1::Enum { .. } => {
                return Err(error("execution roles cannot be transported through enums"));
            }
            _ => return Ok(None),
        }
        let value = if moved {
            std::mem::replace(selected, SemanticValueBindingV1::MovedExecution)
        } else {
            selected.clone()
        };
        if moved && place.projections().is_empty() {
            self.locals[local] = None;
        }
        Ok(Some(value))
    }
}

fn require_complete_execution_aggregate_v29(
    binding: &SemanticValueBindingV1,
) -> Result<(), &'static str> {
    match binding {
        SemanticValueBindingV1::Unmaterialized | SemanticValueBindingV1::MovedExecution => {
            Err("execution aggregate contains a moved value")
        }
        SemanticValueBindingV1::ExecutionReferent(_) => {
            Err("borrowed execution referents cannot become owned values")
        }
        SemanticValueBindingV1::Enum { .. } => {
            Err("execution roles cannot be transported through enums")
        }
        SemanticValueBindingV1::Aggregate(fields) => {
            for field in fields {
                require_complete_execution_aggregate_v29(field)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
