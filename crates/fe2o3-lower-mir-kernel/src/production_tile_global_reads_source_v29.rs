#[derive(Clone, Copy)]
struct ValueRow {
    function: FunctionCoordinate,
    source: ValueId,
    output: ValueId,
    definition: Definition,
}
#[derive(Clone, Copy)]
struct FragmentRow {
    origin: usize,
    component: Option<u32>,
    operation: Coordinate,
}

fn operation<'a>(
    module: &'a Module,
    coordinate: Coordinate,
    budget: &mut Budget<'_>,
) -> R<&'a Operation> {
    budget.charge_work(5)?;
    module
        .functions
        .get(coordinate.block.function.0 as usize)
        .and_then(|f| f.body.as_ref())
        .and_then(|b| b.blocks.get(coordinate.block.block as usize))
        .and_then(|b| b.operations.get(coordinate.operation as usize))
        .ok_or_else(|| binding(None, "operation coordinate"))
}
fn original_value<'a>(
    module: &'a Module,
    source: SourceCoordinate,
    budget: &mut Budget<'_>,
) -> R<Option<(FunctionCoordinate, ValueId, &'a Type)>> {
    budget.charge_work(6)?;
    Ok(match source {
        SourceCoordinate::FunctionParameter {
            function,
            parameter,
        } => {
            let function_id =
                FunctionCoordinate(u32::try_from(function).map_err(|_| Resource::Arithmetic)?);
            let f = module
                .functions
                .get(function)
                .ok_or_else(|| binding(None, "source function parameter"))?;
            match f.body.as_ref() {
                Some(body) => Some((
                    function_id,
                    *body
                        .parameters
                        .get(parameter)
                        .ok_or_else(|| binding(None, "source parameter value"))?,
                    f.signature
                        .parameters
                        .get(parameter)
                        .ok_or_else(|| binding(None, "source parameter type"))?,
                )),
                None => None,
            }
        }
        SourceCoordinate::BlockParameter {
            function,
            block,
            parameter,
        } => {
            let f = module
                .functions
                .get(function)
                .and_then(|f| f.body.as_ref())
                .ok_or_else(|| binding(None, "source block function"))?;
            let p = f
                .blocks
                .get(block)
                .and_then(|b| b.parameters.get(parameter))
                .ok_or_else(|| binding(None, "source block parameter"))?;
            Some((
                FunctionCoordinate(u32::try_from(function).map_err(|_| Resource::Arithmetic)?),
                p.id,
                &p.ty,
            ))
        }
        SourceCoordinate::Result {
            operation: coordinate,
            result,
        } => {
            let row = operation(module, coordinate, budget)?
                .results
                .get(result)
                .ok_or_else(|| binding(None, "source result"))?;
            Some((coordinate.block.function, row.id, &row.ty))
        }
        _ => None,
    })
}
fn target_definition(target: TargetCoordinate) -> R<Option<Definition>> {
    Ok(match target {
        TargetCoordinate::FunctionParameter {
            function,
            parameter,
        } => Some(Definition::FunctionArgument {
            function: FunctionCoordinate(
                u32::try_from(function).map_err(|_| Resource::Arithmetic)?,
            ),
            argument: u32::try_from(parameter).map_err(|_| Resource::Arithmetic)?,
        }),
        TargetCoordinate::BlockParameter { block, parameter } => Some(Definition::BlockArgument {
            block,
            argument: u32::try_from(parameter).map_err(|_| Resource::Arithmetic)?,
        }),
        TargetCoordinate::Result { operation, result } => Some(Definition::Result {
            operation,
            result: u32::try_from(result).map_err(|_| Resource::Arithmetic)?,
        }),
        _ => None,
    })
}
fn definition_value(
    module: &Module,
    definition: Definition,
    budget: &mut Budget<'_>,
) -> R<ValueId> {
    budget.charge_work(5)?;
    match definition {
        Definition::FunctionArgument { function, argument } => module
            .functions
            .get(function.0 as usize)
            .and_then(|f| f.body.as_ref())
            .and_then(|b| b.parameters.get(argument as usize))
            .copied(),
        Definition::BlockArgument { block, argument } => module
            .functions
            .get(block.function.0 as usize)
            .and_then(|f| f.body.as_ref())
            .and_then(|b| b.blocks.get(block.block as usize))
            .and_then(|b| b.parameters.get(argument as usize))
            .map(|p| p.id),
        Definition::Result {
            operation: coordinate,
            result,
        } => operation(module, coordinate, budget)?
            .results
            .get(result as usize)
            .map(|p| p.id),
    }
    .ok_or_else(|| binding(None, "current definition value"))
}
fn value_map(
    transport: &Transport<'_>,
    inventory: &Inventory<'_>,
    budget: &mut Budget<'_>,
) -> R<(Vec<ValueRow>, Vec<FragmentRow>)> {
    let original = transport.pending_ancestor(budget)?.pending_module();
    let current = inventory.owner().module();
    let mut values = Vec::new();
    let mut fragments = Vec::new();
    for ordinal in 0..transport.origin_count(budget)? {
        let origin = transport.origin(ordinal, budget)?;
        let source = original_value(original, origin.source(), budget)?;
        for piece_ordinal in origin.pieces() {
            let piece = transport.piece(piece_ordinal, budget)?;
            budget.charge_work(2)?;
            if piece.origin() != ordinal {
                return Err(binding(None, "piece origin inverse"));
            }
            if let Some((function, source, _)) = source {
                if let Some(definition) = target_definition(piece.target())? {
                    let output = definition_value(current, definition, budget)?;
                    push(
                        &mut values,
                        ValueRow {
                            function,
                            source,
                            output,
                            definition,
                        },
                        budget,
                    )?;
                }
            }
            if let TargetCoordinate::Operation(operation) = piece.target() {
                if matches!(piece.stage(), Stage::Prelude | Stage::Predicate) {
                    push(
                        &mut fragments,
                        FragmentRow {
                            origin: ordinal,
                            component: piece.component(),
                            operation,
                        },
                        budget,
                    )?;
                }
            }
        }
    }
    sort(&mut values, 3, budget, |a, b| {
        (a.function, a.source, a.output).cmp(&(b.function, b.source, b.output))
    })?;
    sort(&mut fragments, 5, budget, |a, b| {
        (a.origin, a.component, a.operation).cmp(&(b.origin, b.component, b.operation))
    })?;
    Ok((values, fragments))
}
fn mapped(
    values: &[ValueRow],
    function: FunctionCoordinate,
    source: ValueId,
    budget: &mut Budget<'_>,
) -> R<ValueRow> {
    let index = lower_bound(values, 2, budget, |row| {
        (row.function, row.source).cmp(&(function, source))
    })?;
    budget.charge_work(5)?;
    let row = values
        .get(index)
        .filter(|row| (row.function, row.source) == (function, source))
        .ok_or_else(|| binding(None, "missing current source input/base definition"))?;
    if values
        .get(index + 1)
        .is_some_and(|next| (next.function, next.source) == (function, source))
    {
        return Err(binding(
            None,
            "ambiguous current source input/base definition",
        ));
    }
    Ok(*row)
}
fn actual_definition_operation<'g>(
    inventory: &Inventory<'g>,
    function: FunctionCoordinate,
    value: ValueId,
    budget: &mut Budget<'_>,
) -> R<(Coordinate, &'g Operation)> {
    let definition = inventory
        .definition_for_value(function, value, budget)
        .map_err(ProductionTileScalarTransportErrorV29::Inventory)?
        .ok_or_else(|| binding(None, "actual operand definition missing"))?;
    let Definition::Result {
        operation: coordinate,
        ..
    } = definition.coordinate
    else {
        return Err(binding(None, "actual operand is not an operation result"));
    };
    Ok((
        coordinate,
        operation(inventory.owner().module(), coordinate, budget)?,
    ))
}

fn source_representation(
    transport: &Transport<'_>,
    subject: &crate::ProductionTilePendingGlobalReadV29,
    budget: &mut Budget<'_>,
) -> R<()> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
        SemanticExecutionOperationV29, SemanticPointerMetadataV1, SemanticScalarTypeV1,
        SemanticTerminatorKindV1, SemanticTypeShapeV1,
    };
    let semantic = transport.source_semantic(budget)?;
    let alias = transport.source_alias(subject.source_alias(), budget)?;
    let crate::ProductionTileSourceSpanV29::Terminator(span) = alias.source() else {
        return Err(binding(
            None,
            "Load source is not its actual call terminator",
        ));
    };
    budget.charge_work(12)?;
    let function = semantic
        .functions()
        .get(span.semantic_function().index() as usize)
        .ok_or_else(|| binding(None, "Load semantic function"))?;
    let terminator = function
        .blocks()
        .get(span.semantic_block().index() as usize)
        .ok_or_else(|| binding(None, "Load semantic block"))?
        .terminator();
    let SemanticTerminatorKindV1::Call(call) = terminator.kind() else {
        return Err(binding(None, "Load source call"));
    };
    let callable = semantic
        .callables()
        .get(call.callee().index() as usize)
        .ok_or_else(|| binding(None, "Load source callable"))?;
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding: callable_binding,
        operation:
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::MaskedTileLoadU32 { workgroup, .. },
            ),
        ..
    } = callable
    else {
        return Err(binding(None, "Load source intrinsic identity"));
    };
    let [workgroup_argument, input, base] = call.arguments() else {
        return Err(binding(None, "Load source argument roster"));
    };
    budget.charge_work(30)?;
    if *workgroup != subject.workgroup().semantic_type()
        || callable_binding.abi().source_input_types()
            != [workgroup_argument.ty(), input.ty(), base.ty()]
    {
        return Err(binding(None, "Load source exact callable ABI"));
    }
    let source_type = |ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1| {
        semantic
            .types()
            .get(ty.index() as usize)
            .ok_or_else(|| binding(None, "Load source type roster"))
    };
    let input_type = source_type(input.ty())?;
    let SemanticTypeShapeV1::Pointer(pointer) = input_type.shape() else {
        return Err(binding(None, "Load source fat slice pointer"));
    };
    let SemanticTypeShapeV1::Slice { element } = source_type(pointer.pointee())?.shape() else {
        return Err(binding(None, "Load source exact slice extent"));
    };
    let element = source_type(*element)?;
    let base = source_type(base.ty())?;
    if pointer.metadata() != SemanticPointerMetadataV1::SliceLength
        || pointer.pointer_width_bits() != 64
        || input_type.layout().size_bytes() != Some(16)
        || element.shape()
            != &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            })
        || element.layout().size_bytes() != Some(4)
        || base.shape()
            != &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            })
        || base.layout().size_bytes() != Some(8)
    {
        return Err(binding(
            None,
            "Load retained source slice/base representation",
        ));
    }
    // The checked transport owns the exact source-to-original operand/instance
    // replay. This schema check does not reinterpret source places or infer a
    // live allocation, pointer alignment, root-parameter origin or launch truth.
    Ok(())
}
fn join(
    transport: &Transport<'_>,
    inventory: &Inventory<'_>,
    graph: &Graph<'_, '_>,
    budget: &mut Budget<'_>,
) -> R<Joined> {
    budget.reserve_storage(
        4_usize
            .checked_mul(size_of::<Vec<()>>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let (values, fragments) = value_map(transport, inventory, budget)?;
    let semantic = transport.source_semantic(budget)?;
    let original = transport.pending_ancestor(budget)?.pending_module();
    let current = inventory.owner().module();
    // Every root and instance remains part of the subject, including non-tile
    // roots and helper instances that merely carry source aliases.
    for ordinal in 0..transport.root_count(budget)? {
        let root = transport.root(ordinal, budget)?;
        budget.charge_work(8)?;
        let function = semantic
            .functions()
            .get(root.launch().selected_root().index() as usize)
            .ok_or_else(|| binding(None, "source root roster"))?;
        if function.identity() != root.launch().semantic_root_identity()
            || root.function().0 as usize >= current.functions.len()
        {
            return Err(binding(None, "source root identity"));
        }
        for instance_index in root.instances() {
            let instance = transport.instance(instance_index, budget)?;
            budget.charge_work(5)?;
            let source = semantic
                .functions()
                .get(instance.function().index() as usize)
                .ok_or_else(|| binding(None, "source helper roster"))?;
            if instance.root() != ordinal || instance.identity() != source.identity() {
                return Err(binding(None, "source helper identity"));
            }
        }
    }
    let mut reads = Vec::new();
    for ordinal in 0..transport.pending_obligation_count(budget)? {
        let pending = transport.pending_obligation(ordinal, budget)?;
        budget.charge_work(2)?;
        match (pending.kind(), pending.global_read()) {
            (PendingKind::GlobalRead, Some(subject)) => {
                source_representation(transport, subject, budget)?;
                let root = transport.root(subject.root(), budget)?;
                let origin = transport.origin(subject.origin(), budget)?;
                let piece = transport.piece(subject.piece(), budget)?;
                let primary = transport.source_alias(subject.source_alias(), budget)?;
                budget.charge_work(24)?;
                if origin.source() != SourceCoordinate::Operation(subject.source())
                    || !origin.pieces().contains(&subject.piece())
                    || piece.target() != TargetCoordinate::Operation(subject.output())
                    || piece.origin() != subject.origin()
                    || piece.stage() != Stage::Read
                    || piece.component() != Some(subject.component())
                    || subject.component() >= u32::from(subject.elements())
                    || primary.root() != subject.root()
                    || primary.instance() != subject.instance()
                    || root.function() != subject.output().block.function
                {
                    return Err(binding(
                        Some(ordinal),
                        "exact generated source/read/component subject",
                    ));
                }
                let OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 {
                    workgroup,
                    input,
                    base,
                    lanes,
                    elements,
                }) = operation(original, subject.source(), budget)?.kind
                else {
                    return Err(binding(Some(ordinal), "actual original source Load"));
                };
                if (workgroup, input, base, lanes, elements)
                    != (
                        subject.workgroup().value(),
                        subject.input(),
                        subject.base(),
                        subject.lanes(),
                        subject.elements(),
                    )
                {
                    return Err(binding(
                        Some(ordinal),
                        "source Load operand/geometry identity",
                    ));
                }
                let source_type = semantic
                    .types()
                    .get(subject.workgroup().semantic_type().index() as usize)
                    .ok_or_else(|| binding(Some(ordinal), "source workgroup type"))?;
                if source_type.identity() != subject.workgroup().type_identity()
                    || root.launch().source_launch().exact_workgroup()
                        != Some([u32::from(lanes), 1, 1])
                    || root.launch().layout().workgroup_extents() != [u64::from(lanes), 1, 1]
                {
                    return Err(binding(
                        Some(ordinal),
                        "selected root/source workgroup agreement",
                    ));
                }
                let input = mapped(&values, subject.source().block.function, input, budget)?;
                let base = mapped(&values, subject.source().block.function, base, budget)?;
                let ReadOutcome::ProvedLocalConditions(fact) =
                    graph.read_at(subject.output(), budget)?
                else {
                    return Err(Failure::Unproved {
                        operation: subject.output(),
                        reason: "actual slice bound/provenance/alignment",
                    });
                };
                if !std::ptr::eq(fact.owner(), inventory.owner()) {
                    return Err(binding(Some(ordinal), "foreign local fact"));
                }
                let domain = *fact.domain();
                let checked_operations = join_fragment(
                    transport, inventory, graph, subject, &fragments, input, base, domain, budget,
                )?;
                push(
                    &mut reads,
                    ProductionTileGlobalReadAdmissionV29 {
                        obligation: ordinal,
                        operation: subject.output(),
                        root: subject.root(),
                        input: input.output,
                        base: base.output,
                        domain,
                        checked_operations,
                        aliases: 0..0,
                    },
                    budget,
                )?;
            }
            (PendingKind::GlobalRead, None) | (_, Some(_)) => {
                return Err(binding(Some(ordinal), "pending GlobalRead kind/subject"));
            }
            _ => {}
        }
    }
    sort(&mut reads, 3, budget, |a, b| a.operation.cmp(&b.operation))?;
    budget.charge_work(reads.len())?;
    if reads
        .windows(2)
        .any(|pair| pair[0].operation == pair[1].operation)
    {
        return Err(binding(None, "duplicate physical generated read"));
    }
    // Independent inverse over every actual operation. Original reads remain
    // pending, but no generated physical Global Load may lack its source row.
    for (ordinal, actual) in inventory.operations().iter().enumerate() {
        let occurrence = transport.operation(ordinal, budget)?;
        budget.charge_work(4)?;
        if occurrence.coordinate() != Occurrence::Operation(actual.coordinate) {
            return Err(binding(None, "operation inventory covariance"));
        }
        if let OperationKind::Load { access, .. } = actual.operation.kind {
            if access.address_space != AddressSpace::Global {
                continue;
            }
            let crate::ProductionTileProvenanceV29::Piece(piece) = occurrence.provenance() else {
                return Err(binding(None, "Global operation without structural origin"));
            };
            if transport.piece(piece, budget)?.stage() != Stage::Preserved {
                let index = lower_bound(&reads, 3, budget, |row| {
                    row.operation.cmp(&actual.coordinate)
                })?;
                budget.charge_work(2)?;
                if reads
                    .get(index)
                    .is_none_or(|row| row.operation != actual.coordinate)
                {
                    return Err(binding(
                        None,
                        "generated Global read omitted by pending roster",
                    ));
                }
            }
        }
    }
    let mut aliases = Vec::new();
    for alias_index in 0..transport.source_alias_count(budget)? {
        let alias = transport.source_alias(alias_index, budget)?;
        for attachment_index in alias.attachments() {
            let attachment = transport.attachment(attachment_index, budget)?;
            budget.charge_work(4)?;
            if attachment.family() != AttachmentFamily::InstanceSpans
                || attachment.root() != alias.root()
            {
                return Err(binding(None, "source alias attachment covariance"));
            }
            let AttachmentTarget::Pieces(pieces) = attachment.target() else {
                continue;
            };
            for piece_index in pieces.clone() {
                let piece = transport.piece(piece_index, budget)?;
                let TargetCoordinate::Operation(coordinate) = piece.target() else {
                    continue;
                };
                let read = lower_bound(&reads, 3, budget, |row| row.operation.cmp(&coordinate))?;
                budget.charge_work(3)?;
                if reads
                    .get(read)
                    .is_some_and(|row| row.operation == coordinate)
                {
                    if reads[read].root != alias.root() {
                        return Err(binding(None, "cross-root read alias"));
                    }
                    push(
                        &mut aliases,
                        ProductionTileGlobalReadAliasV29 {
                            read,
                            source_alias: alias_index,
                            attachment: attachment_index,
                            piece: piece_index,
                        },
                        budget,
                    )?;
                }
            }
        }
    }
    sort(&mut aliases, 4, budget, |a, b| {
        (a.read, a.source_alias, a.attachment, a.piece).cmp(&(
            b.read,
            b.source_alias,
            b.attachment,
            b.piece,
        ))
    })?;
    for (index, alias) in aliases.iter().enumerate() {
        budget.charge_work(3)?;
        let row = &mut reads[alias.read];
        if row.aliases.is_empty() {
            row.aliases.start = index;
        }
        row.aliases.end = index.checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    validate_alias_coverage(transport, &reads, &aliases, budget)?;
    // All other attachment families and pending obligations are retained by the
    // borrowed transport; none is converted into a discharged state here.
    for ordinal in 0..transport.attachment_count(budget)? {
        transport.attachment(ordinal, budget)?;
    }
    Ok(Joined { reads, aliases })
}

fn join_fragment(
    _transport: &Transport<'_>,
    inventory: &Inventory<'_>,
    graph: &Graph<'_, '_>,
    subject: &crate::ProductionTilePendingGlobalReadV29,
    fragments: &[FragmentRow],
    input: ValueRow,
    base: ValueRow,
    domain: FormalRuntimeSliceReadDomainV1,
    budget: &mut Budget<'_>,
) -> R<usize> {
    let current = inventory.owner().module();
    let function = subject.output().block.function;
    let (_, gep) = actual_definition_operation(inventory, function, domain.pointer(), budget)?;
    let OperationKind::GetElementPointer { base: data, offset } = gep.kind else {
        return Err(binding(None, "actual read GEP"));
    };
    let (_, data) = actual_definition_operation(inventory, function, data, budget)?;
    let OperationKind::SliceData { slice } = data.kind else {
        return Err(binding(None, "actual read SliceData"));
    };
    let (_, length) = actual_definition_operation(inventory, function, domain.length(), budget)?;
    budget.charge_work(16)?;
    if slice != input.output
        || offset != domain.index()
        || !matches!(length.kind, OperationKind::SliceLength { slice: actual } if actual == input.output)
        || domain.element_bytes() != 4
    {
        return Err(binding(None, "same source input/extent/current address"));
    }
    let input_definition = inventory
        .definition_for_value(function, input.output, budget)
        .map_err(ProductionTileScalarTransportErrorV29::Inventory)?
        .ok_or_else(|| binding(None, "source input missing"))?;
    let base_definition = inventory
        .definition_for_value(function, base.output, budget)
        .map_err(ProductionTileScalarTransportErrorV29::Inventory)?
        .ok_or_else(|| binding(None, "source base missing"))?;
    if input_definition.coordinate != input.definition
        || base_definition.coordinate != base.definition
        || base_definition.ty != &Type::INDEX
        || !matches!(input_definition.ty, Type::Slice(slice) if slice.address_space == AddressSpace::Global && slice.element.as_ref() == &Type::Scalar(ScalarType::U32))
    {
        return Err(binding(
            None,
            "exact input/base current type and definition",
        ));
    }
    let mut checked = 0_usize;
    let mut base_add = false;
    let mut lane_guard = false;
    for component in [None, Some(subject.component())] {
        let key = (subject.origin(), component);
        let start = lower_bound(fragments, 2, budget, |row| {
            (row.origin, row.component).cmp(&key)
        })?;
        for row in &fragments[start..] {
            budget.charge_work(2)?;
            if (row.origin, row.component) != key {
                break;
            }
            let operation = operation(current, row.operation, budget)?;
            match operation.kind {
                OperationKind::Binary {
                    op: BinaryOp::Checked(_),
                    lhs,
                    ..
                } => {
                    let fact = graph
                        .no_wrap_at(subject.output(), row.operation, budget)?
                        .ok_or(Failure::Unproved {
                            operation: row.operation,
                            reason: "actual own overflow false",
                        })?;
                    budget.charge_work(4)?;
                    if !std::ptr::eq(fact.operation(), operation)
                        || fact.operation_coordinate() != row.operation
                    {
                        return Err(binding(None, "exact checked arithmetic subject"));
                    }
                    if fact.value() == domain.index() {
                        if lhs != base.output
                            || !matches!(
                                operation.kind,
                                OperationKind::Binary {
                                    op: BinaryOp::Checked(
                                        fe2o3_kernel_ir::CheckedBinaryOperator::Add
                                    ),
                                    ..
                                }
                            )
                        {
                            return Err(binding(
                                None,
                                "source base is not exact final checked Add operand",
                            ));
                        }
                        base_add = true;
                    }
                    checked = checked.checked_add(1).ok_or(Resource::Arithmetic)?;
                }
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs,
                    rhs,
                } if component.is_none() => {
                    let (_, lane) = actual_definition_operation(inventory, function, lhs, budget)?;
                    let (_, width) = actual_definition_operation(inventory, function, rhs, budget)?;
                    budget.charge_work(8)?;
                    if matches!(&lane.kind, OperationKind::Intrinsic(intrinsic) if intrinsic.kind == IntrinsicKind::InvocationIndex { kind: IndexKind::Local, axis: Axis::X })
                        && matches!(width.kind, OperationKind::Constant(Constant::Index(actual)) if actual == u64::from(subject.lanes()))
                    {
                        let [predicate] = operation.results.as_slice() else {
                            return Err(binding(None, "lane predicate result"));
                        };
                        graph
                            .true_at(subject.output(), predicate.id, budget)?
                            .ok_or(Failure::Unproved {
                                operation: row.operation,
                                reason: "actual lane predicate true",
                            })?;
                        lane_guard = true;
                    }
                }
                _ => {}
            }
        }
    }
    if checked == 0 || !base_add || !lane_guard {
        return Err(binding(
            None,
            "complete generated no-wrap/base/lane obligations",
        ));
    }
    Ok(checked)
}

fn validate_alias_coverage(
    transport: &Transport<'_>,
    reads: &[ProductionTileGlobalReadAdmissionV29],
    aliases: &[ProductionTileGlobalReadAliasV29],
    budget: &mut Budget<'_>,
) -> R<()> {
    for pair in aliases.windows(2) {
        budget.charge_work(4)?;
        let key = |row: &ProductionTileGlobalReadAliasV29| {
            (row.read, row.source_alias, row.attachment, row.piece)
        };
        if key(&pair[0]) >= key(&pair[1]) {
            return Err(binding(
                None,
                "duplicate or unordered exact source association",
            ));
        }
    }
    for (read_index, read) in reads.iter().enumerate() {
        let subject = transport
            .pending_obligation(read.obligation, budget)?
            .global_read()
            .ok_or_else(|| binding(Some(read.obligation), "read source subject"))?;
        budget.charge_work(3)?;
        if subject.output() != read.operation || subject.root() != read.root {
            return Err(binding(
                Some(read.obligation),
                "source association current operation/root",
            ));
        }
        let range = aliases
            .get(read.aliases.clone())
            .ok_or_else(|| binding(Some(read.obligation), "source alias range"))?;
        let mut primary = false;
        for alias in range {
            budget.charge_work(4)?;
            if alias.read != read_index {
                return Err(binding(Some(read.obligation), "source alias read inverse"));
            }
            primary |=
                alias.source_alias == subject.source_alias() && alias.piece == subject.piece();
        }
        if !primary {
            return Err(binding(
                Some(read.obligation),
                "missing primary source Load alias",
            ));
        }
    }
    Ok(())
}

// Reader-only hostile coverage. These mutated rows are unauthenticated scratch;
// this helper cannot construct a transport owner, source fact or successful view.
#[cfg(test)]
pub(crate) fn test_alias_fault_v29(
    transport: &Transport<'_>,
    budget: &mut Budget<'_>,
    fault: u8,
) -> R<()> {
    with_checked_tile_global_reads_v29(transport, budget, |checked, budget| {
        assert!(
            checked.read_count(budget)? >= 2,
            "genuinely accepted full consumer baseline"
        );
        source_scope(budget, |budget| {
            let inventory = transport.current_inventory(budget)?;
            let mut scratch = join(transport, inventory, checked.graph(budget)?, budget)?;
            validate_alias_coverage(transport, &scratch.reads, &scratch.aliases, budget)?;
            assert!(scratch.aliases.len() >= 2);
            match fault {
                0 => {
                    scratch.aliases.remove(0);
                }
                1 => {
                    let first = &scratch.aliases[0];
                    scratch.aliases[1] = ProductionTileGlobalReadAliasV29 {
                        read: first.read,
                        source_alias: first.source_alias,
                        attachment: first.attachment,
                        piece: first.piece,
                    };
                }
                2 => {
                    scratch.reads[0].root = usize::MAX;
                }
                3 => {
                    scratch.reads[0].operation.operation = u32::MAX;
                }
                _ => panic!("closed reader fault roster"),
            }
            let result =
                validate_alias_coverage(transport, &scratch.reads, &scratch.aliases, budget);
            drop(scratch);
            result
        })
    })
}
