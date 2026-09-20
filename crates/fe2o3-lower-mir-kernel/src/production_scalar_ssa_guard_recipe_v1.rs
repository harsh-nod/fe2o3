use super::*;

fn statement_site(site: fe2o3_mir_model::SemanticU32InductionStatementSiteV1) -> Site {
    Site::Statement {
        block: SsaBlockIdV1::new(site.block().block().index()),
        statement: site.statement(),
    }
}

fn matches_place(
    operand: &SemanticOperandV1,
    binding: fe2o3_mir_model::SemanticU32InductionPlaceBindingV1,
) -> bool {
    let place = match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
        _ => return false,
    };
    place.projections().is_empty() && place.local() == binding.local() && place.ty() == binding.ty()
}

pub(super) fn check<'s, R: GuardRequest>(
    owner: &'s ProductionScalarSsaEmissionOwnerV1,
    request: R,
    fact: GuardRecurrence<'s>,
    inventory: &Inventory<'s>,
    indices: &Indices,
    incoming: &Incoming,
    budget: &mut Budget<'_>,
) -> Result<ProductionU32GuardConsistencyV1<'s>> {
    budget.charge_work(5)?;
    let original = owner.original();
    let source = original.semantic_ssa().source_semantic();
    let certificate = request
        .certificate()
        .ok_or(Error::Mismatch("guard report certificate ordinal"))?;
    if !std::ptr::eq(fact.source(), original)
        || !inventory.belongs_to(original.executable())
        || fact.root() != request.root()
        || fact.certificate() != certificate
        || fact.authorizes_compiler_transform()
    {
        return Err(Error::Mismatch("guard genuine recurrence/source custody"));
    }
    let sealed = &owner.emission;
    charge_lookup(sealed.aliases.len(), budget)?;
    let alias = sealed
        .aliases
        .binary_search_by_key(
            &(request.root().index(), certificate.function().index()),
            |row| row.key(),
        )
        .map_err(|_| Error::Mismatch("guard C alias"))?;
    let function = &sealed.capture.functions[sealed.aliases[alias].emitted];
    let declaration = &source.functions()[certificate.function().index() as usize];
    let guard = declaration
        .blocks()
        .get(certificate.guard().block().block().index() as usize)
        .and_then(|block| {
            block
                .statements()
                .get(certificate.guard().statement() as usize)
        })
        .ok_or(Error::Mismatch("guard source statement"))?;
    let SemanticStatementKindV1::Assign(assignment) = guard.kind() else {
        return Err(Error::Mismatch("guard source assignment"));
    };
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::LessThan,
        left,
        right,
    } = assignment.value().kind()
    else {
        return Err(Error::Mismatch("guard exact source LessThan"));
    };
    budget.charge_work(12)?;
    if certificate.guard().block() != certificate.header()
        || !assignment.destination().projections().is_empty()
        || assignment.destination().local() != certificate.predicate().local()
        || assignment.destination().ty() != certificate.predicate().ty()
        || assignment.value().result_type() != certificate.predicate().ty()
        || !matches_place(left, certificate.guard_induction())
        || !matches_place(right, certificate.guard_bound())
        || !declaration.locals()[certificate.bound().local().index() as usize]
            .role()
            .is_entry_argument()
        || fixed_scalar(source.types(), certificate.induction().ty()) != Some(ScalarType::U32)
        || fixed_scalar(source.types(), certificate.guard_induction().ty()) != Some(ScalarType::U32)
        || fixed_scalar(source.types(), certificate.bound().ty()) != Some(ScalarType::U32)
        || !matches!(
            source.types()[certificate.predicate().ty().index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
        )
    {
        return Err(Error::Mismatch("guard source ordered operands/destination"));
    }
    let predicate = SsaVariableIdV1::new(certificate.predicate().local().index());
    let induction = SsaVariableIdV1::new(certificate.induction().local().index());
    let bound = SsaVariableIdV1::new(certificate.bound().local().index());
    let guard_bound = SsaVariableIdV1::new(certificate.guard_bound().local().index());
    let guard_value = indices.operand(
        owner,
        certificate.function(),
        statement_site(certificate.guard()),
        0,
        SsaVariableIdV1::new(certificate.guard_induction().local().index()),
        budget,
    )?;
    let bound_value = indices.operand(
        owner,
        certificate.function(),
        statement_site(certificate.guard()),
        1,
        guard_bound,
        budget,
    )?;
    let predicate_definition = indices.operand(
        owner,
        certificate.function(),
        statement_site(certificate.guard()),
        3,
        predicate,
        budget,
    )?;
    let predicate_use = indices.operand(
        owner,
        certificate.function(),
        Site::Terminator {
            block: SsaBlockIdV1::new(certificate.header().block().index()),
        },
        2,
        predicate,
        budget,
    )?;
    if predicate_use != predicate_definition {
        return Err(Error::Mismatch(
            "guard source switch consumes actual predicate",
        ));
    }
    let header_value = SsaValueV1::BlockArgument {
        block: SsaBlockIdV1::new(certificate.header().block().index()),
        variable: induction,
    };
    if let Some(snapshot) = certificate.guard_induction_snapshot() {
        budget.charge_work(8)?;
        let statement = declaration
            .blocks()
            .get(snapshot.block().block().index() as usize)
            .and_then(|block| block.statements().get(snapshot.statement() as usize))
            .ok_or(Error::Mismatch("guard source snapshot statement"))?;
        let SemanticStatementKindV1::Assign(snapshot_assignment) = statement.kind() else {
            return Err(Error::Mismatch("guard source snapshot assignment"));
        };
        let SemanticRvalueKindV1::Use(snapshot_operand) = snapshot_assignment.value().kind() else {
            return Err(Error::Mismatch("guard source snapshot recipe"));
        };
        if snapshot.block() != certificate.header()
            || snapshot.statement() >= certificate.guard().statement()
            || !snapshot_assignment.destination().projections().is_empty()
            || snapshot_assignment.destination().local() != certificate.guard_induction().local()
            || snapshot_assignment.destination().ty() != certificate.induction().ty()
            || snapshot_assignment.value().result_type() != certificate.induction().ty()
            || !matches_place(snapshot_operand, certificate.induction())
            || indices.operand(
                owner,
                certificate.function(),
                statement_site(snapshot),
                0,
                induction,
                budget,
            )? != header_value
            || indices.operand(
                owner,
                certificate.function(),
                statement_site(snapshot),
                3,
                SsaVariableIdV1::new(certificate.guard_induction().local().index()),
                budget,
            )? != guard_value
        {
            return Err(Error::Mismatch("guard exact source snapshot SSA relation"));
        }
    } else if certificate.guard_induction() != certificate.induction()
        || guard_value != header_value
    {
        return Err(Error::Mismatch("guard exact source header SSA value"));
    }
    let left = sealed.definition(function, guard_value, budget)?;
    let right = sealed.definition(function, bound_value, budget)?;
    let entry = if let Some(snapshot) = certificate.bound_snapshot() {
        let Some(entry) = check_rhs_snapshot(
            owner,
            certificate,
            snapshot,
            function,
            right,
            bound_value,
            indices,
            budget,
        )?
        else {
            return Ok(ProductionU32GuardConsistencyV1::Unavailable(
                ProductionU32GuardUnavailableV1::GuardRecipe,
            ));
        };
        entry
    } else {
        right
    };
    budget.charge_work(5)?;
    let right_expected = &sealed.capture.expected[entry.expected];
    if left.definitions[0] != Some(fact.recurrence().parameter())
        || right_expected.site != SourceSite::Entry
        || right_expected.variable != bound
        || right_expected.shape != Shape::Scalar(ScalarType::U32)
    {
        return Err(Error::Mismatch(
            "guard actual parameter/entry bound definitions",
        ));
    }
    let Some(bound_definition @ Definition::FunctionArgument { .. }) = entry.definitions[0] else {
        return Err(Error::Mismatch("guard actual N entry bound"));
    };
    let header = indices.block(
        owner,
        inventory,
        function,
        certificate.header().block(),
        budget,
    )?;
    let body = indices.block(
        owner,
        inventory,
        function,
        certificate.body_entry().block(),
        budget,
    )?;
    let exit = indices.block(
        owner,
        inventory,
        function,
        certificate.exit().block(),
        budget,
    )?;
    let span = sealed.statement(
        original,
        function,
        certificate.guard().block().block().index(),
        certificate.guard().statement(),
        budget,
    )?;
    budget.charge_work(3)?;
    if span.kernel_ir_block() != header.block.id {
        return Err(Error::Mismatch("guard actual N header span"));
    }
    if span.operation_count() != 1 {
        return Ok(ProductionU32GuardConsistencyV1::Unavailable(
            ProductionU32GuardUnavailableV1::GuardRecipe,
        ));
    }
    let ordinal =
        usize::try_from(span.first_operation_ordinal()).map_err(|_| Resource::Arithmetic)?;
    let index = header
        .operations
        .start
        .checked_add(ordinal)
        .ok_or(Resource::Arithmetic)?;
    let operation = inventory
        .operations()
        .get(index)
        .filter(|_| index < header.operations.end)
        .ok_or(Error::Mismatch("guard actual comparison span bounds"))?;
    let (condition, selector) = check_comparison(
        operation,
        header.coordinate,
        left.values[0],
        right.values[0],
        budget,
    )?;
    let source_header = &declaration.blocks()[certificate.header().block().index() as usize];
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = source_header.terminator().kind()
    else {
        return Err(Error::Mismatch("guard exact source switch"));
    };
    budget.charge_work(7)?;
    if !matches_place(discriminant, certificate.predicate())
        || targets.values().len() != 1
        || targets.values()[0].value() != 0
        || targets.values()[0].edge().target() != certificate.exit().block()
        || targets.values()[0].edge().role() != SemanticEdgeRoleV1::SwitchValue
        || targets.otherwise().target() != certificate.body_entry().block()
        || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
    {
        return Err(Error::Mismatch(
            "guard exact source false/otherwise occurrences",
        ));
    }
    let (then_edge, else_edge) =
        check_branch(header, body, exit, selector, inventory, incoming, budget)?;
    let Definition::BlockArgument {
        block: recurrence_header,
        ..
    } = fact.recurrence().parameter()
    else {
        return Err(Error::Mismatch("guard recurrence header parameter"));
    };
    if recurrence_header != header.coordinate {
        return Err(Error::Mismatch("guard actual recurrence/header join"));
    }
    Ok(ProductionU32GuardConsistencyV1::Joined(
        ProductionU32GuardConsistencyFactV1 {
            source: original,
            root: request.root(),
            function: certificate.function(),
            ordinal: request.ordinal(),
            recurrence: fact.recurrence(),
            bound: bound_definition,
            condition,
            body: body.coordinate,
            exit: exit.coordinate,
            then_edge,
            else_edge,
        },
    ))
}

#[allow(
    clippy::too_many_arguments,
    reason = "Closed borrowed source/SSA subjects remain explicit"
)]
fn check_rhs_snapshot<'a>(
    owner: &'a ProductionScalarSsaEmissionOwnerV1,
    certificate: GuardCertificate,
    snapshot: fe2o3_mir_model::SemanticU32InductionStatementSiteV1,
    function: &EmittedFunction,
    right: &EmittedDefinition,
    bound_value: SsaValueV1,
    indices: &Indices,
    budget: &mut Budget<'_>,
) -> Result<Option<&'a EmittedDefinition>> {
    budget.charge_work(14)?;
    let original = owner.original();
    let sealed = &owner.emission;
    let source = original.semantic_ssa().source_semantic();
    let declaration = &source.functions()[certificate.function().index() as usize];
    let statement = declaration
        .blocks()
        .get(snapshot.block().block().index() as usize)
        .and_then(|block| block.statements().get(snapshot.statement() as usize))
        .ok_or(Error::Mismatch("guard RHS snapshot statement"))?;
    let SemanticStatementKindV1::Assign(copy) = statement.kind() else {
        return Err(Error::Mismatch("guard RHS snapshot assignment"));
    };
    let SemanticRvalueKindV1::Use(operand @ SemanticOperandV1::Copy(_)) = copy.value().kind()
    else {
        return Err(Error::Mismatch("guard RHS exact Copy recipe"));
    };
    let rhs_variable = SsaVariableIdV1::new(certificate.guard_bound().local().index());
    let entry_variable = SsaVariableIdV1::new(certificate.bound().local().index());
    let expected = &sealed.capture.expected[right.expected];
    if snapshot.block() != certificate.header()
        || snapshot.statement() >= certificate.guard().statement()
        || copy.destination().local() != certificate.guard_bound().local()
        || copy.destination().ty() != certificate.bound().ty()
        || !copy.destination().projections().is_empty()
        || copy.value().result_type() != certificate.bound().ty()
        || !matches_place(operand, certificate.bound())
        || expected.site != SourceSite::Event(statement_site(snapshot))
        || expected.variable != rhs_variable
        || expected.value != bound_value
        || expected.shape != Shape::Scalar(ScalarType::U32)
        || indices.operand(
            owner,
            certificate.function(),
            statement_site(snapshot),
            3,
            rhs_variable,
            budget,
        )? != bound_value
    {
        return Err(Error::Mismatch("guard RHS actual Statement definition"));
    }
    let entry_value = indices.operand(
        owner,
        certificate.function(),
        statement_site(snapshot),
        0,
        entry_variable,
        budget,
    )?;
    let entry = sealed.definition(function, entry_value, budget)?;
    let entry_expected = &sealed.capture.expected[entry.expected];
    budget.charge_work(6)?;
    if entry_expected.site != SourceSite::Entry
        || entry_expected.variable != entry_variable
        || entry_expected.shape != Shape::Scalar(ScalarType::U32)
        || !matches!(
            entry.definitions[0],
            Some(Definition::FunctionArgument { .. })
        )
        || right.definitions[0] != entry.definitions[0]
        || right.values[0] != entry.values[0]
    {
        return Err(Error::Mismatch(
            "guard RHS Copy input is actual Entry/N value",
        ));
    }
    let span = sealed.statement(
        original,
        function,
        snapshot.block().block().index(),
        snapshot.statement(),
        budget,
    )?;
    budget.charge_work(1)?;
    if span.operation_count() != 0 {
        return Ok(None);
    }
    Ok(Some(entry))
}

pub(super) fn check_comparison(
    operation: &fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>,
    header: Block,
    left: ValueId,
    right: ValueId,
    budget: &mut Budget<'_>,
) -> Result<(Definition, ValueId)> {
    budget.charge_work(7)?;
    let [result] = operation.operation.results.as_slice() else {
        return Err(Error::Mismatch("guard single Bool compare result"));
    };
    let OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs,
        rhs,
    } = operation.operation.kind
    else {
        return Err(Error::Mismatch("guard actual N LessThan recipe"));
    };
    if result.ty != Type::BOOL
        || lhs != left
        || rhs != right
        || operation.coordinate.block != header
    {
        return Err(Error::Mismatch(
            "guard actual N ordered comparison operands",
        ));
    }
    Ok((
        Definition::Result {
            operation: operation.coordinate,
            result: 0,
        },
        result.id,
    ))
}

pub(super) fn check_branch(
    header: &fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>,
    body: &fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>,
    exit: &fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>,
    condition: ValueId,
    inventory: &Inventory<'_>,
    incoming: &Incoming,
    budget: &mut Budget<'_>,
) -> Result<(Edge, Edge)> {
    budget.charge_work(2)?;
    let Terminator::ConditionalBranch {
        condition: selector,
        then_target,
        else_target,
        ..
    } = header.terminator
    else {
        return Err(Error::Mismatch("guard actual N conditional branch"));
    };
    let [then_edge, else_edge] = inventory
        .edges()
        .get(header.edges.clone())
        .ok_or(Error::Mismatch("guard actual N branch edge range"))?
    else {
        return Err(Error::Mismatch("guard actual N two edge occurrences"));
    };
    budget.charge_work(10)?;
    if *selector != condition
        || body.coordinate == exit.coordinate
        || *then_target != body.block.id
        || *else_target != exit.block.id
        || then_edge.coordinate
            != (Edge {
                source: header.coordinate,
                successor: 0,
            })
        || else_edge.coordinate
            != (Edge {
                source: header.coordinate,
                successor: 1,
            })
        || then_edge.target != body.coordinate
        || else_edge.target != exit.coordinate
        || !incoming.unique(body.coordinate, then_edge.coordinate, budget)?
        || !incoming.unique(exit.coordinate, else_edge.coordinate, budget)?
    {
        return Err(Error::Mismatch(
            "guard actual N polarity/unique predecessor",
        ));
    }
    Ok((then_edge.coordinate, else_edge.coordinate))
}
