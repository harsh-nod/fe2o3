// Original source, actual checked history, and fresh final-F policy observations.
use fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1 as EdgePlacement;
use fe2o3_lower_mir_kernel::ProductionCanonicalScalarOperationOriginV1 as OperationOrigin;

fn hcheck(ok: bool, reason: &'static str) -> Result<(), HError> {
    if ok {
        Ok(())
    } else {
        Err(HError::Invalid(reason))
    }
}
fn hpush<T>(rows: &mut Vec<T>, row: T, budget: &mut Budget<'_>) -> Result<(), HError> {
    push(rows, row, budget).map_err(Into::into)
}
fn operation_coordinate(op: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1) -> [u32; 3] {
    [op.block.function.0, op.block.block, op.operation]
}
fn full_use_coordinate(value: UseCoordinate) -> [u32; 5] {
    match value {
        UseCoordinate::OperationOperand { operation, operand } => [
            0,
            operation.block.function.0,
            operation.block.block,
            operation.operation,
            operand,
        ],
        UseCoordinate::TerminatorOperand { block, operand } => {
            [1, block.function.0, block.block, 0, operand]
        }
    }
}
fn argument_coordinate(value: fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1) -> [u32; 4] {
    [
        value.edge.source.function.0,
        value.edge.source.block,
        value.edge.successor,
        value.argument,
    ]
}
fn placement(value: EdgePlacement) -> Placement {
    match value {
        EdgePlacement::Retained(edge) => Placement::Retained(edge_coordinate(edge)),
        EdgePlacement::InternalConnector(p) => {
            Placement::Internal([p.output.function.0, p.output.block, p.segment])
        }
        EdgePlacement::Omitted => Placement::Omitted,
    }
}
fn history_sizes(view: &HView<'_, '_, '_>, budget: &mut Budget<'_>) -> Result<Sizes, HError> {
    let metadata = view.original_metadata(budget)?;
    let original = metadata.inventory(budget)?;
    let output = view.final_inventory(budget)?;
    let lineage = view.lineage(budget)?;
    let policies = view.policies(budget)?;
    let mut descendants = 0usize;
    for ordinal in 0..lineage.original_definition_count(budget)? {
        descendants = descendants
            .checked_add(lineage.definition_descendant_count(ordinal, budget)?)
            .ok_or(HError::Invalid("diagnostic descendant extent overflow"))?;
    }
    let mut segments = 0usize;
    for ordinal in 0..output.blocks().len() {
        segments = segments
            .checked_add(lineage.block_segments(ordinal, budget)?.len())
            .ok_or(HError::Invalid("diagnostic segment extent overflow"))?;
    }
    let mut incoming = 0usize;
    for ordinal in 0..policies.pair_count(budget)? {
        incoming = incoming
            .checked_add(policies.pair(ordinal, budget)?.incoming_edges().len())
            .ok_or(HError::Invalid("diagnostic incoming extent overflow"))?;
    }
    Ok(Sizes {
        assertions: view.assertion_count(budget)?,
        spans: metadata.spans(budget)?.len(),
        associations: metadata.function_count(budget)?,
        functions: output.functions().len(),
        operations: output.operations().len(),
        descendants,
        segments,
        blocks: original.blocks().len(),
        controls: original.edges().len(),
        uses: output.uses().len(),
        edges: output.edges().len(),
        arguments: output.edge_arguments().len(),
        pairs: policies.pair_count(budget)?,
        incoming,
        definitions: policies.definition_count(budget)?,
    })
}
fn history_rows(
    s: Sizes,
    graph: &GraphRow,
    budget: &mut Budget<'_>,
) -> Result<HistoryRows, String> {
    Ok(HistoryRows {
        original: paid_vec(s.assertions, budget)?,
        assertions: paid_vec(s.assertions, budget)?,
        spans: paid_vec(s.spans, budget)?,
        functions: paid_vec(s.functions, budget)?,
        operations: paid_vec(s.operations, budget)?,
        descendants: paid_vec(s.descendants, budget)?,
        segments: paid_vec(s.segments, budget)?,
        blocks: paid_vec(s.blocks, budget)?,
        controls: paid_vec(s.controls, budget)?,
        uses: paid_vec(s.uses, budget)?,
        edges: paid_vec(s.edges, budget)?,
        arguments: paid_vec(s.arguments, budget)?,
        pairs: paid_vec(s.pairs, budget)?,
        incoming: paid_vec(s.incoming, budget)?,
        definitions: paid_vec(s.definitions, budget)?,
        final_edges: paid_vec(graph.edges.len(), budget)?,
        final_payloads: paid_vec(graph.payloads.len(), budget)?,
    })
}
fn paid_text(text: &str, budget: &mut Budget<'_>) -> Result<String, String> {
    budget
        .reserve_storage(text.len())
        .map_err(|e| e.to_string())?;
    let mut result = String::new();
    result
        .try_reserve_exact(text.len())
        .map_err(|e| e.to_string())?;
    budget
        .reserve_storage(
            result
                .capacity()
                .checked_sub(text.len())
                .ok_or("text capacity")?,
        )
        .map_err(|e| e.to_string())?;
    budget.charge_work(text.len()).map_err(|e| e.to_string())?;
    result.push_str(text);
    Ok(result)
}
fn history_graph_row(owner: &OriginalOwner, budget: &mut Budget<'_>) -> Result<GraphRow, String> {
    let counts = graph_counts(owner, budget)?;
    let mut edges = paid_vec(counts.edges, budget)?;
    let mut payloads = paid_vec(counts.payloads, budget)?;
    let mut definitions = paid_vec(counts.definitions, budget)?;
    original_graph_rows(owner, &mut edges, &mut payloads, &mut definitions, budget)?;
    let mut symbols = paid_vec(owner.module().functions.len(), budget)?;
    let mut kernels = paid_vec(owner.module().kernels.len(), budget)?;
    let mut checked_adds = 0usize;
    for f in &owner.module().functions {
        let text = paid_text(f.id.as_str(), budget)?;
        push(&mut symbols, text, budget).map_err(|e| e.to_string())?;
        if let Some(body) = &f.body {
            for block in &body.blocks {
                budget
                    .charge_work(block.operations.len())
                    .map_err(|e| e.to_string())?;
                for op in &block.operations {
                    if matches!(
                        op.kind,
                        OperationKind::Binary {
                            op: BinaryOp::Checked(fe2o3_kernel_ir::CheckedBinaryOperator::Add),
                            ..
                        }
                    ) {
                        checked_adds = add(checked_adds, 1)?;
                    }
                }
            }
        }
    }
    for root in &owner.module().kernels {
        let text = paid_text(root.entry.as_str(), budget)?;
        push(&mut kernels, text, budget).map_err(|e| e.to_string())?;
    }
    Ok(GraphRow {
        subject: subject(owner),
        edges,
        payloads,
        definitions,
        symbols,
        kernels,
        counts: [
            counts.switches,
            counts.masks,
            counts.shifts,
            checked_adds,
            counts.blocks,
            counts.definitions,
        ],
        memory: counts.memory,
    })
}
fn history_fill_source(
    owner: &HOwner,
    view: &HView<'_, '_, '_>,
    rows: &mut HistoryRows,
    fixture: &Origin,
    budget: &mut Budget<'_>,
) -> Result<(), HError> {
    let metadata = view.original_metadata(budget)?;
    let inventory = metadata.inventory(budget)?;
    hcheck(
        std::ptr::eq(inventory.owner(), owner.original_source().executable()),
        "actual original N owner",
    )?;
    let semantic = owner.original_source().semantic_ssa().source_semantic();
    let spans = metadata.spans(budget)?;
    for span in spans {
        budget.charge_work(1)?;
        let (kind, source) = match span.site() {
            Site::Statement { span, source } => {
                hcheck(
                    std::ptr::eq(
                        source,
                        &semantic.functions()[span.semantic_function().index() as usize].blocks()
                            [span.semantic_block().index() as usize]
                            .statements()[span.statement_ordinal() as usize],
                    ),
                    "original statement identity",
                )?;
                (
                    0,
                    [
                        span.correspondence_owner().index(),
                        span.semantic_function().index(),
                        span.semantic_block().index(),
                        span.statement_ordinal(),
                    ],
                )
            }
            Site::Terminator { span, source } => {
                hcheck(
                    std::ptr::eq(
                        source,
                        semantic.functions()[span.semantic_function().index() as usize].blocks()
                            [span.semantic_block().index() as usize]
                            .terminator(),
                    ),
                    "original terminator identity",
                )?;
                (
                    1,
                    [
                        span.correspondence_owner().index(),
                        span.semantic_function().index(),
                        span.semantic_block().index(),
                        0,
                    ],
                )
            }
            Site::Synthetic(span) => {
                use fe2o3_lower_mir_kernel::SemanticKirSyntheticOperationRuleV1 as R;
                let rule = match span.rule() {
                    R::RetainedLocalStorage => 0,
                    R::EnumPayloadStorage => 1,
                    R::RuntimeAssertFailureTrap => 2,
                };
                (
                    2,
                    [
                        span.correspondence_owner().index(),
                        span.semantic_function().index(),
                        0,
                        rule,
                    ],
                )
            }
        };
        let range = span.operations();
        hpush(
            &mut rows.spans,
            SpanRow {
                association: span.association(),
                block: block_coordinate(span.block()),
                kind,
                source,
                operations: [range.start, range.end],
            },
            budget,
        )?;
    }
    let originals = metadata.assertions(budget)?;
    hcheck(
        originals.len() == view.assertion_count(budget)?
            && originals.len() == owner.original_source().assert_origins().source_site_count(),
        "complete source assertion alias roster",
    )?;
    for (ordinal, original) in originals.iter().enumerate() {
        budget.charge_work(8)?;
        let span = &spans[original.span()];
        let Site::Terminator {
            span: source_span,
            source,
        } = span.site()
        else {
            return Err(HError::Invalid("assertion is not a source terminator"));
        };
        let STerminator::Assert {
            expected,
            message,
            target,
            ..
        } = source.kind()
        else {
            return Err(HError::Invalid("source assertion kind"));
        };
        let binding = original.binding();
        let sealed = owner
            .original_source()
            .assert_origins()
            .assert_condition(
                source_span.correspondence_owner(),
                source_span.semantic_function(),
                source_span.semantic_block(),
                budget,
            )
            .map_err(|_| HError::Invalid("sealed original binding"))?;
        hcheck(
            binding == sealed
                && binding.expected() == *expected
                && binding.semantic_success() == target.target(),
            "exact original assertion binding",
        )?;
        use fe2o3_mir_model::semantic_mir_v1::{
            SemanticConstantValueV1 as ConstantValue, SemanticOperandV1 as Operand,
            SemanticScalarTypeV1 as Scalar, SemanticTypeShapeV1 as Shape,
        };
        let signed_literal = matches!(message,
            AssertMessage::Overflow { operation: SBinary::ShiftLeft, right: Operand::Constant(c), .. }
                if matches!(c.value(), ConstantValue::Scalar(v) if v.bits() == 3 && v.size_bytes() == 4)
                && matches!(semantic.types()[c.ty().index() as usize].shape(),
                    Shape::Scalar(Scalar::Integer { signed: true, bits: 32 })));
        let message = match message {
            AssertMessage::Overflow {
                operation: SBinary::Add,
                ..
            } => MessageRow::Add,
            AssertMessage::Overflow {
                operation: SBinary::Multiply,
                ..
            } => MessageRow::Multiply,
            AssertMessage::Overflow {
                operation: SBinary::ShiftLeft | SBinary::ShiftRight,
                ..
            } => MessageRow::Shift,
            _ => return Err(HError::Invalid("unexpected scalar fixture assertion")),
        };
        let provenance = source.source();
        let origin = source_origin(
            provenance
                .call_site()
                .ok_or(HError::Invalid("source provenance"))?,
        );
        let expansion = source_origin(
            provenance
                .expansion()
                .ok_or(HError::Invalid("source expansion"))?,
        );
        hcheck(
            origin_valid(&origin, fixture) && origin_valid(&expansion, fixture),
            "actual rustc fixture source",
        )?;
        let (condition, definition, success, failure) = match binding.outcome() {
            Outcome::Emitted {
                condition_use,
                definition,
                success_edge,
                failure_edge,
            } => (
                Some(use_coordinate(condition_use)?),
                Some(definition_coordinate(definition)),
                edge_coordinate(success_edge),
                Some(edge_coordinate(failure_edge)),
            ),
            Outcome::ElidedByExistingRule { success_edge } => {
                (None, None, edge_coordinate(success_edge), None)
            }
        };
        let association = metadata.function(span.association(), budget)?;
        let row = AssertionRow {
            span: original.span(),
            association: span.association(),
            source: [
                source_span.correspondence_owner().index(),
                source_span.semantic_function().index(),
                source_span.semantic_block().index(),
            ],
            helper: association.source().role() == SemanticKirFunctionRoleV1::InternalHelper,
            origin,
            expansion,
            expected: *expected,
            message,
            signed_literal,
            semantic_success: target.target().index(),
            condition,
            definition,
            success,
            failure,
            proof: ProofRow::Other,
        };
        let proved = view.assertion(ordinal, budget)?;
        hcheck(
            proved.span() == original.span() && proved.original_binding() == binding,
            "fresh history proof exact N binding",
        )?;
        let current = proved.condition();
        let step = |s: fe2o3_lower_mir_kernel::ProductionCanonicalScalarAssertionStepV1| {
            [s.round(), u16::from(s.is_integer())]
        };
        hpush(&mut rows.original, row, budget)?;
        hpush(
            &mut rows.assertions,
            HistoryAssertion {
                original: AssertionRow {
                    proof: proof_row(proved.proof_kind()),
                    ..row
                },
                disposition: match proved.disposition() {
                    HDisposition::Retained => 0,
                    HDisposition::SourceElided => 1,
                    HDisposition::HistoryElided => 2,
                    HDisposition::UnreachableRemoved => 3,
                },
                condition: current.map(|c| use_coordinate(c.0)).transpose()?,
                definition: current.map(|c| definition_coordinate(c.1)),
                failure: proved.failure_edge().map(edge_coordinate),
                success: placement(proved.success_placement()),
                selection: proved.selection().map(step),
                removal: proved.removal().map(step),
            },
            budget,
        )?;
    }
    let mut count = 0usize;
    for ordinal in 0..metadata.function_count(budget)? {
        let function = metadata.function(ordinal, budget)?.source();
        let semantic_function =
            &semantic.functions()[function.semantic_function().index() as usize];
        for (block, body) in semantic_function.blocks().iter().enumerate() {
            budget.charge_work(
                rows.original
                    .len()
                    .checked_add(1)
                    .ok_or(HError::Invalid("source scan extent"))?,
            )?;
            if matches!(body.terminator().kind(), STerminator::Assert { .. }) {
                hcheck(
                    rows.original
                        .iter()
                        .filter(|row| {
                            row.source
                                == [
                                    function.correspondence_owner().index(),
                                    function.semantic_function().index(),
                                    u32::try_from(block).unwrap(),
                                ]
                        })
                        .count()
                        == 1,
                    "bidirectional complete original assertion aliases",
                )?;
                count = count
                    .checked_add(1)
                    .ok_or(HError::Invalid("source assertion count"))?;
            }
        }
    }
    hcheck(count == rows.original.len(), "no extra assertion aliases")
}
fn history_fill_lineage(
    owner: &HOwner,
    view: &HView<'_, '_, '_>,
    rows: &mut HistoryRows,
    budget: &mut Budget<'_>,
) -> Result<(), HError> {
    let original = view.original_metadata(budget)?.inventory(budget)?;
    let output = view.final_inventory(budget)?;
    let lineage = view.lineage(budget)?;
    hcheck(
        std::ptr::eq(output.owner(), owner.output())
            && !std::ptr::eq(output.owner(), original.owner()),
        "real final graph owner",
    )?;
    for (ordinal, function) in output.functions().iter().enumerate() {
        hpush(
            &mut rows.functions,
            [
                function.coordinate.0,
                lineage.function_origin(ordinal, budget)?.0,
            ],
            budget,
        )?;
    }
    for (ordinal, op) in output.operations().iter().enumerate() {
        let (original, synthesis) = match lineage.operation_origin(ordinal, budget)? {
            OperationOrigin::Original(op) => (Some(operation_coordinate(op)), None),
            OperationOrigin::SynthesizedConstant {
                round,
                integer,
                definition,
            } => (
                None,
                Some((round, integer, definition_coordinate(definition))),
            ),
        };
        hpush(
            &mut rows.operations,
            OperationRow {
                output: operation_coordinate(op.coordinate),
                original,
                synthesis,
            },
            budget,
        )?;
    }
    for ordinal in 0..lineage.original_definition_count(budget)? {
        for descendant in 0..lineage.definition_descendant_count(ordinal, budget)? {
            let d = lineage.definition_descendant(ordinal, descendant, budget)?;
            hpush(
                &mut rows.descendants,
                DescendantRow {
                    original: definition_coordinate(d.original),
                    output: definition_coordinate(d.output),
                    retained: d.kind
                        == fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Retained,
                },
                budget,
            )?;
        }
    }
    for (ordinal, block) in output.blocks().iter().enumerate() {
        let segments = lineage.block_segments(ordinal, budget)?;
        for segment in segments {
            hpush(
                &mut rows.segments,
                SegmentRow {
                    output: block_coordinate(block.coordinate),
                    original: block_coordinate(segment.original),
                    connector: segment.connector.map(edge_coordinate),
                },
                budget,
            )?;
        }
    }
    for ordinal in 0..original.blocks().len() {
        let block = lineage.original_block_control(ordinal, budget)?;
        hpush(
            &mut rows.blocks,
            BlockRow {
                original: block_coordinate(block.original),
                placement: block
                    .placement
                    .map(|p| [p.output.function.0, p.output.block, p.segment]),
                reachable: block.reachable,
            },
            budget,
        )?;
    }
    for ordinal in 0..original.edges().len() {
        let edge = lineage.original_edge_control(ordinal, budget)?;
        hpush(
            &mut rows.controls,
            ControlRow {
                original: edge_coordinate(edge.original),
                placement: placement(edge.placement),
                executable: edge.executable,
            },
            budget,
        )?;
    }
    for (ordinal, used) in output.uses().iter().enumerate() {
        hpush(
            &mut rows.uses,
            [
                full_use_coordinate(used.coordinate),
                full_use_coordinate(lineage.use_origin(ordinal, budget)?),
                definition_coordinate(output.definitions()[used.definition].coordinate),
            ],
            budget,
        )?;
    }
    for (ordinal, edge) in output.edges().iter().enumerate() {
        hpush(
            &mut rows.edges,
            [
                edge_coordinate(edge.coordinate),
                edge_coordinate(lineage.edge_origin(ordinal, budget)?),
            ],
            budget,
        )?;
        let start = rows.final_payloads.len();
        for argument in edge.arguments {
            hpush(&mut rows.final_payloads, argument.0, budget)?;
        }
        hpush(
            &mut rows.final_edges,
            EdgeRow {
                coordinate: edge_coordinate(edge.coordinate),
                target: block_coordinate(edge.target),
                target_id: edge.target_id.0,
                payload: [start, rows.final_payloads.len()],
            },
            budget,
        )?;
    }
    for (ordinal, argument) in output.edge_arguments().iter().enumerate() {
        hpush(
            &mut rows.arguments,
            [
                argument_coordinate(argument.coordinate),
                argument_coordinate(lineage.edge_argument_origin(ordinal, budget)?),
            ],
            budget,
        )?;
    }
    Ok(())
}
#[derive(Clone, Copy)]
struct HistorySummary {
    policy_subject: Subject,
    policy_resources: [usize; 3],
    source_resources: [usize; 3],
    pending: usize,
    no_denial: bool,
}
fn history_fill(
    owner: &HOwner,
    view: &HView<'_, '_, '_>,
    rows: &mut HistoryRows,
    sizes: Sizes,
    fixture: &Origin,
    budget: &mut Budget<'_>,
) -> Result<HistorySummary, HError> {
    hcheck(
        history_sizes(view, budget)? == sizes,
        "fresh second invocation exact roster",
    )?;
    history_fill_source(owner, view, rows, fixture, budget)?;
    history_fill_lineage(owner, view, rows, budget)?;
    let policies = view.policies(budget)?;
    hcheck(
        std::ptr::eq(policies.owner(budget)?, owner.output()),
        "fresh policies belong to actual F",
    )?;
    for ordinal in 0..policies.pair_count(budget)? {
        let pair = policies.pair(ordinal, budget)?;
        let range = pair.incoming_edges();
        hpush(
            &mut rows.pairs,
            PairRow {
                call: operation_coordinate(pair.call()),
                declaration: pair.declaration().0,
                incoming: [range.start, range.end],
            },
            budget,
        )?;
        for ordinal in range {
            let incoming = policies.incoming_edge(ordinal, budget)?;
            hcheck(
                incoming.definition().ty == &fe2o3_kernel_ir::Type::BOOL,
                "actual final Bool selector",
            )?;
            hpush(
                &mut rows.incoming,
                IncomingRow {
                    condition: use_coordinate(incoming.condition().coordinate)?,
                    definition: definition_coordinate(incoming.definition().coordinate),
                    success: edge_coordinate(incoming.success().coordinate),
                    failure: edge_coordinate(incoming.failure().coordinate),
                    expected: incoming.success_when(),
                },
                budget,
            )?;
            for physical in [incoming.success(), incoming.failure()] {
                budget.charge_work(
                    rows.final_edges
                        .len()
                        .checked_add(physical.arguments.len())
                        .ok_or(HError::Invalid("incoming extent"))?,
                )?;
                let row = rows
                    .final_edges
                    .iter()
                    .find(|e| e.coordinate == edge_coordinate(physical.coordinate))
                    .ok_or(HError::Invalid("missing final physical edge"))?;
                hcheck(
                    row.target == block_coordinate(physical.target)
                        && row.target_id == physical.target_id.0
                        && rows.final_payloads[row.payload[0]..row.payload[1]]
                            .iter()
                            .copied()
                            .eq(physical.arguments.iter().map(|v| v.0)),
                    "full ordered final successor payload",
                )?;
            }
        }
    }
    for ordinal in 0..policies.definition_count(budget)? {
        let coordinate = policies.definition_coordinate(ordinal, budget)?;
        let report = policies.report(ordinal, budget)?;
        let history = policies.history(ordinal, budget)?;
        let floor = history.floor();
        let invocation = history.invocation();
        hpush(
            &mut rows.definitions,
            DefinitionRow {
                ordinal,
                function: coordinate.0,
                history_function: history.function(),
                stages: report.reports().pass_order().map(stage_row),
                paired: report.paired_stage_count(),
                clean: report.reports().is_clean(),
                floor: resources(floor),
                invocation: resources(invocation),
                denied: floor.first_denial().is_some() || invocation.first_denial().is_some(),
                panicked: floor.caught_panic() || invocation.caught_panic(),
            },
            budget,
        )?;
    }
    hcheck(
        !view.ranked_verification_is_complete()
            && !view.grants_artifact_or_launch_authority()
            && !policies.ranked_verification_is_complete()
            && !policies.grants_artifact_or_launch_authority(),
        "history has no completion or authority",
    )?;
    let policy = policies.observation(budget)?;
    let source = view.source_resources(budget)?;
    Ok(HistorySummary {
        policy_subject: subject(policies.owner(budget)?),
        policy_resources: resources(policy),
        source_resources: [source.work, source.storage, source.peak],
        pending: policies.pending_obligations().iter().count(),
        no_denial: policy.first_denial().is_none()
            && !policy.caught_panic()
            && source.failed_storage.is_none(),
    })
}
fn history_pass(pass: fe2o3_pliron::PlironOptimizationPassV1) -> u8 {
    use fe2o3_pliron::PlironOptimizationPassV1 as P;
    match pass {
        P::IntegerNeutralCanonicalization => 0,
        P::DeadCodeElimination => 1,
        P::SparseConditionalConstantPropagation => 2,
        P::SimplifyControlFlow => 3,
        P::SelectSameValueCanonicalization => 4,
        P::LocalPureCommonSubexpressionElimination => 5,
        P::DominancePureCommonSubexpressionElimination => 6,
    }
}
fn occurrence_counts(rows: &fe2o3_pliron::KirNeutralOccurrenceRowsV1) -> [usize; 9] {
    let c = rows.candidate();
    [
        c.functions.len(),
        c.blocks.len(),
        c.segments.len(),
        c.operations.len(),
        c.definitions.len(),
        c.definition_outputs.len(),
        c.uses.len(),
        c.edges.len(),
        c.edge_arguments.len(),
    ]
}
fn snapshot(owner: &OriginalOwner, budget: &mut Budget<'_>) -> Result<Vec<u8>, String> {
    let bytes = owner.canonical().canonical_bytes();
    let mut copy = paid_vec(bytes.len(), budget)?;
    budget.charge_work(bytes.len()).map_err(|e| e.to_string())?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}
fn history_observe(
    tcx: TyCtxt<'_>,
    request: &HistoryRequest,
) -> Result<HistoryObservation, String> {
    let cpu = tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref());
    require(
        cpu == request.target.cpu(),
        "actual compiler history collection profile",
    )?;
    let fixture = active_fixture(tcx, &request.source[3])?;
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|e| e.to_string())?;
    let wire = transaction
        .consume_pre_ranked_for_test_v1(|source, descriptors| {
            let source_roots = census::roots(source.semantic_ssa().source_semantic())?;
            require(
                descriptors.len() == 1
                    && source_roots.len() == descriptors.len()
                    && source_roots.len() == source.executable().module().kernels.len(),
                "actual complete roots",
            )?;
            let mut work = Work::new(
                usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                    .map_err(|e| e.to_string())?,
            );
            let mut budget = Budget::new(
                &mut work,
                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            );
            let source_floor = source
                .unit_local_source_storage_floor_v1()
                .map_err(|e| e.to_string())?;
            budget
                .reserve_storage(source_floor)
                .map_err(|e| e.to_string())?;
            let headers = add(
                add(size_of::<HistoryObservation>(), size_of::<HistorySummary>())?,
                add(
                    mul(2, size_of::<Counts>())?,
                    add(size_of::<Vec<u8>>(), size_of::<Vec<Vec<u8>>>())?,
                )?,
            )?;
            budget.reserve_storage(headers).map_err(|e| e.to_string())?;
            // The census helper's inert temporary also stays paid until it is dropped.
            let root_census_bytes = source_roots.iter().try_fold(
                add(
                    std::mem::size_of_val(&source_roots),
                    mul(source_roots.capacity(), size_of_val_element(&source_roots))?,
                )?,
                |n, r| add(n, r.name.capacity()),
            )?;
            budget
                .reserve_storage(root_census_bytes)
                .map_err(|e| e.to_string())?;
            let mut roots = paid_vec(source_roots.len(), &mut budget)?;
            for (root, kernel) in source_roots
                .iter()
                .zip(&source.executable().module().kernels)
            {
                let name = paid_text(&root.name, &mut budget)?;
                let entry = paid_text(kernel.entry.as_str(), &mut budget)?;
                push(
                    &mut roots,
                    Root {
                        name,
                        function: root.function,
                        body: root.body,
                        entry,
                    },
                    &mut budget,
                )
                .map_err(|e| e.to_string())?;
            }
            drop(source_roots);
            budget
                .release_storage(root_census_bytes)
                .map_err(|e| e.to_string())?;
            let original = history_graph_row(source.executable(), &mut budget)?;
            let original_bytes = snapshot(source.executable(), &mut budget)?;
            let source_assertions = source.assert_origins().source_site_count();
            let mut semantic = [0usize; 3];
            for function in source.semantic_ssa().source_semantic().functions() {
                for block in function.blocks() {
                    budget
                        .charge_work(add(block.statements().len(), 1)?)
                        .map_err(|e| e.to_string())?;
                    semantic[0] = add(
                        semantic[0],
                        usize::from(matches!(
                            block.terminator().kind(),
                            STerminator::SwitchInt { .. }
                        )),
                    )?;
                    for statement in block.statements() {
                        if let SStatement::Assign(assignment) = statement.kind() {
                            match assignment.value().kind() {
                                SRvalue::Binary {
                                    operation: SBinary::BitAnd,
                                    ..
                                } => semantic[1] = add(semantic[1], 1)?,
                                SRvalue::Binary {
                                    operation: SBinary::ShiftLeft | SBinary::ShiftRight,
                                    ..
                                } => semantic[2] = add(semantic[2], 1)?,
                                _ => {}
                            }
                        }
                    }
                }
            }
            let before_factory = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let slot = std::ptr::from_ref(&budget) as usize;
            let (owner, receipt) = HOwner::try_prepare_with_assertions_v1(source, &mut budget)
                .map_err(|e| format!("assertion history factory: {e}"))?;
            require(
                budget.storage() == before_factory,
                "factory exact original/diagnostic floor",
            )?;
            let additional = receipt.retained_storage();
            budget
                .reserve_storage(additional)
                .map_err(|e| e.to_string())?;
            let owner_floor = add(source_floor, additional)?;
            require(
                owner.retained_storage_floor_v1() == owner_floor
                    && subject(owner.original_source().executable()) == original.subject,
                "actual owned original and prepaid history",
            )?;
            let round_count = owner.history().rounds().len();
            let mut rounds = paid_vec(round_count, &mut budget)?;
            let mut snapshots = paid_vec(mul(2, round_count)?, &mut budget)?;
            let mut input = original.subject;
            for round in owner.history().rounds() {
                let integer = round.integer();
                let scalar = round.scalar();
                budget.charge_work(10).map_err(|e| e.to_string())?;
                require(
                    integer.map().matches_execution(integer.report())
                        && scalar.map().matches_execution(scalar.report()),
                    "actual substage map/execution join",
                )?;
                let mut ip = [0u8; 2];
                let mut sp = [0u8; 8];
                require(
                    integer.report().passes().len() == ip.len()
                        && scalar.report().passes().len() == sp.len(),
                    "actual fixed pass roster",
                )?;
                for (dest, pass) in ip.iter_mut().zip(integer.report().passes()) {
                    *dest = history_pass(pass.pass());
                }
                for (dest, pass) in sp.iter_mut().zip(scalar.report().passes()) {
                    *dest = history_pass(pass.pass());
                }
                let output = subject(round.output());
                push(
                    &mut rounds,
                    HistoryRound {
                        ordinal: round.ordinal(),
                        input,
                        integer: subject(integer.owner()),
                        output,
                        changed: input != output,
                        integer_passes: ip,
                        scalar_passes: sp,
                        maps: [*integer.map().digest(), *scalar.map().digest()],
                        executions: [
                            digest(integer.execution().canonical_bytes()),
                            digest(scalar.execution().canonical_bytes()),
                        ],
                        occurrences: [
                            occurrence_counts(integer.occurrences()),
                            occurrence_counts(scalar.occurrences()),
                        ],
                    },
                    &mut budget,
                )
                .map_err(|e| e.to_string())?;
                for graph in [integer.owner(), scalar.owner()] {
                    let bytes = snapshot(graph, &mut budget)?;
                    push(&mut snapshots, bytes, &mut budget).map_err(|e| e.to_string())?;
                }
                input = output;
            }
            let final_graph = history_graph_row(owner.output(), &mut budget)?;
            let preflight_floor = budget.storage();
            // Counts only. No report, proof, owner or first-run facade is reused.
            let sizes = owner
                .with_assertion_policy_checks_v1(&mut budget, history_sizes)
                .map_err(|e| format!("history roster preflight: {e}"))?;
            require(
                budget.storage() == preflight_floor,
                "preflight restores exact paid owner floor",
            )?;
            let mut rows = history_rows(sizes, &final_graph, &mut budget)?;
            let floor = budget.storage();
            let diagnostics_floor = floor.checked_sub(owner_floor).ok_or("diagnostic floor")?;
            let summary = owner
                .with_assertion_policy_checks_v1(&mut budget, |view, budget| {
                    history_fill(&owner, view, &mut rows, sizes, &fixture, budget)
                })
                .map_err(|e| format!("fresh final assertion policies: {e}"))?;
            require(
                budget.storage() == floor
                    && budget.work_ledger_identity_v1() == ledger
                    && std::ptr::from_ref(&budget) as usize == slot,
                "same history ledger/slot/floor restored",
            )?;
            let after = subject(owner.original_source().executable());
            budget
                .charge_work(original_bytes.len())
                .map_err(|e| e.to_string())?;
            require(
                after == original.subject
                    && original_bytes
                        == owner
                            .original_source()
                            .executable()
                            .canonical()
                            .canonical_bytes(),
                "unchanged original canonical bytes",
            )?;
            for (ordinal, round) in owner.history().rounds().iter().enumerate() {
                let integer = round.integer();
                let scalar = round.scalar();
                budget
                    .charge_work(add(
                        64,
                        add(
                            integer.execution().canonical_bytes().len(),
                            scalar.execution().canonical_bytes().len(),
                        )?,
                    )?)
                    .map_err(|e| e.to_string())?;
                require(
                    rounds[ordinal].maps == [*integer.map().digest(), *scalar.map().digest()]
                        && rounds[ordinal].executions
                            == [
                                digest(integer.execution().canonical_bytes()),
                                digest(scalar.execution().canonical_bytes()),
                            ]
                        && rounds[ordinal].occurrences
                            == [
                                occurrence_counts(integer.occurrences()),
                                occurrence_counts(scalar.occurrences()),
                            ],
                    "unchanged actual maps, witnesses and occurrence rosters",
                )?;
                for (stage, graph) in [round.integer().owner(), round.scalar().owner()]
                    .into_iter()
                    .enumerate()
                {
                    let index = add(mul(ordinal, 2)?, stage)?;
                    budget
                        .charge_work(snapshots[index].len())
                        .map_err(|e| e.to_string())?;
                    require(
                        snapshots[index] == graph.canonical().canonical_bytes(),
                        "unchanged actual intermediate bytes",
                    )?;
                }
            }
            require(
                final_graph.subject == subject(owner.output()),
                "unchanged final F owner",
            )?;
            let mut observed = HistoryObservation {
                actual_target: request.target,
                fixture,
                roots,
                original,
                after,
                final_graph,
                policy_subject: summary.policy_subject,
                rounds,
                rows,
                sizes,
                semantic,
                source_assertions,
                owner_floors: [source_floor, additional, owner_floor],
                diagnostics_floor,
                restored_floor: floor,
                source_resources: summary.source_resources,
                policy_resources: summary.policy_resources,
                no_denial: summary.no_denial,
                pending: summary.pending,
                complete: false,
                authority: false,
                final_source_storage: 0,
                protocol_storage: 0,
                work: budget.work(),
            };
            let wire_limit = history_wire_bound(&observed, &mut budget)?;
            require(
                wire_limit <= REPORT_CAP,
                "history literal field-census exceeds report cap",
            )?;
            budget
                .reserve_storage(add(
                    size_of::<Vec<u8>>(),
                    size_of::<BoundedOutput<'_, '_>>(),
                )?)
                .map_err(|e| e.to_string())?;
            let mut wire = paid_vec(wire_limit, &mut budget)?;
            observed.protocol_storage = budget
                .storage()
                .checked_sub(floor)
                .and_then(|n| n.checked_sub(size_of::<BoundedOutput<'_, '_>>()))
                .ok_or("protocol storage")?;
            observed.work = budget.work();
            serde_json::to_writer(
                BoundedOutput {
                    bytes: &mut wire,
                    budget: &mut budget,
                    limit: wire_limit,
                },
                &observed,
            )
            .map_err(|e| e.to_string())?;
            budget
                .release_storage(size_of::<BoundedOutput<'_, '_>>())
                .map_err(|e| e.to_string())?;
            let protocol_storage = observed.protocol_storage;
            drop(observed);
            drop(snapshots);
            drop(original_bytes);
            drop(owner);
            budget
                .release_storage(add(owner_floor, diagnostics_floor)?)
                .map_err(|e| e.to_string())?;
            require(
                budget.storage() == protocol_storage,
                "only inert bounded protocol backing remains",
            )?;
            drop(budget);
            Ok::<_, String>(wire)
        })
        .map_err(|e| e.to_string())??;
    require(
        wire.len() <= REPORT_CAP,
        "bounded detached history diagnostics",
    )?;
    serde_json::from_slice(&wire).map_err(|e| e.to_string())
}
fn size_of_val_element<T>(_: &[T]) -> usize {
    size_of::<T>()
}
