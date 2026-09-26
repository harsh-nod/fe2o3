// Borrow the original source and graph; no optimized or target-bound surrogate.
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirBlockCoordinateV1 as BlockCoordinate,
    CanonicalKirDefinitionCoordinateV1 as DefinitionCoordinate,
    CanonicalKirEdgeCoordinateV1 as EdgeCoordinate, CanonicalKirUseCoordinateV1 as UseCoordinate,
    OperationKind, Terminator, VerifiedCanonicalKernelIrModuleV12 as OriginalOwner,
};
use fe2o3_lower_mir_kernel::{
    ProductionCanonicalAssertionCallKindV1, ProductionCanonicalAssertionFailureV1 as Af,
    ProductionCanonicalAssertionSourcePoliciesV1 as BView,
    ProductionCanonicalRankedSourceSiteV1 as Site, ProductionHelperSourcePolicyV1,
    ProductionPreRankedKirOwnerV1, SemanticKirAssertConditionOutcomeV1 as Outcome,
    SemanticKirFunctionRoleV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssertMessageV1 as AssertMessage, SemanticBinaryOpV1 as SBinary,
    SemanticRvalueKindV1 as SRvalue, SemanticSourceOriginV1, SemanticStatementKindV1 as SStatement,
    SemanticTerminatorKindV1 as STerminator,
};

fn subject(owner: &OriginalOwner) -> Subject {
    Subject {
        digest: *owner.canonical().identity().digest(),
        bytes: owner.canonical().canonical_bytes().len(),
    }
}
fn source_origin(origin: SemanticSourceOriginV1) -> Origin {
    let (start, end) = origin.byte_range();
    let (line, column) = origin.start_coordinate();
    let (last_line, last_column) = origin.end_coordinate();
    Origin {
        file: *origin.file().as_bytes(),
        bytes: [start, end],
        start: [line, column],
        end: [last_line, last_column],
    }
}
fn active_fixture(tcx: TyCtxt<'_>, stamp: &FileStamp) -> Result<Origin, String> {
    use rustc_span::{FileName, Span};
    require(
        stamp.path.canonicalize().map_err(|e| e.to_string())? == stamp.path,
        "canonical fixture path",
    )?;
    let file = {
        let files = tcx.sess.source_map().files();
        let mut matches = files.iter().filter(|file| match &file.name {
            FileName::Real(name) => {
                name.local_path()
                    .and_then(|path| path.canonicalize().ok())
                    .as_ref()
                    == Some(&stamp.path)
            }
            _ => false,
        });
        let file = matches
            .next()
            .ok_or("actual compiled fixture missing")?
            .clone();
        require(
            matches.next().is_none(),
            "duplicate actual compiled fixture",
        )?;
        file
    };
    let metadata = std::fs::metadata(&stamp.path).map_err(|e| e.to_string())?;
    require(
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= INPUT_CAP as u64,
        "bounded regular fixture",
    )?;
    let bytes = std::fs::read(&stamp.path).map_err(|e| e.to_string())?;
    let original = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
    require(
        bytes.len() as u64 == metadata.len()
            && digest(&bytes) == stamp.sha256
            && original.len() == file.unnormalized_source_len as usize
            && file.src_hash.matches(original)
            && file
                .src
                .as_ref()
                .is_some_and(|text| text.as_bytes() == original.as_bytes()),
        "exact rustc SourceMap original/normalized fixture bytes",
    )?;
    let span = Span::with_root_ctxt(file.start_pos, file.end_position());
    let source = crate::rustc_semantic_adapter_v1::canonical_source_provenance_v1(tcx, span, 0)
        .map_err(|e| e.to_string())?
        .provenance();
    let whole = source
        .call_site()
        .ok_or("actual fixture provenance missing")?;
    require(
        source.expansion() == Some(whole)
            && whole.byte_range() == (0, (file.end_position() - file.start_pos).0 as u64),
        "exact whole source file provenance",
    )?;
    Ok(source_origin(whole))
}
fn acheck(ok: bool, detail: &'static str) -> Result<(), Af> {
    if ok {
        Ok(())
    } else {
        Err(Af::Binding { span: None, detail })
    }
}
fn add(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b)
        .ok_or_else(|| "diagnostic arithmetic overflow".into())
}
fn mul(a: usize, b: usize) -> Result<usize, String> {
    a.checked_mul(b)
        .ok_or_else(|| "diagnostic arithmetic overflow".into())
}
fn paid_vec<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, String> {
    let bytes = mul(count, std::mem::size_of::<T>())?;
    budget.reserve_storage(bytes).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count).map_err(|e| e.to_string())?;
    budget
        .reserve_storage(mul(rows.capacity() - count, std::mem::size_of::<T>())?)
        .map_err(|e| e.to_string())?;
    Ok(rows)
}
fn push<T>(rows: &mut Vec<T>, value: T, budget: &mut Budget<'_>) -> Result<(), Af> {
    budget.charge_work(1)?;
    acheck(
        rows.len() < rows.capacity(),
        "prepaid diagnostic row capacity",
    )?;
    rows.push(value);
    Ok(())
}
fn edge_coordinate(edge: EdgeCoordinate) -> [u32; 3] {
    [edge.source.function.0, edge.source.block, edge.successor]
}
fn block_coordinate(block: BlockCoordinate) -> [u32; 2] {
    [block.function.0, block.block]
}
fn use_coordinate(value: UseCoordinate) -> Result<[u32; 3], Af> {
    match value {
        UseCoordinate::TerminatorOperand { block, operand } => {
            Ok([block.function.0, block.block, operand])
        }
        _ => Err(Af::Binding {
            span: None,
            detail: "actual assertion is not a terminator use",
        }),
    }
}
fn definition_coordinate(value: DefinitionCoordinate) -> [u32; 5] {
    match value {
        DefinitionCoordinate::FunctionArgument { function, argument } => {
            [0, function.0, 0, 0, argument]
        }
        DefinitionCoordinate::BlockArgument { block, argument } => {
            [1, block.function.0, block.block, 0, argument]
        }
        DefinitionCoordinate::Result { operation, result } => [
            2,
            operation.block.function.0,
            operation.block.block,
            operation.operation,
            result,
        ],
    }
}
fn graph_edges<'a, 'w>(
    terminator: &'a Terminator,
    budget: &mut Budget<'w>,
    mut emit: impl FnMut(
        usize,
        fe2o3_kernel_ir::BlockId,
        &'a [fe2o3_kernel_ir::ValueId],
        &mut Budget<'w>,
    ) -> Result<(), String>,
) -> Result<(), String> {
    budget.charge_work(1).map_err(|e| e.to_string())?;
    match terminator {
        Terminator::Branch { target, arguments } => {
            budget.charge_work(1).map_err(|e| e.to_string())?;
            emit(0, *target, arguments, budget)?;
        }
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => {
            budget.charge_work(1).map_err(|e| e.to_string())?;
            emit(0, *then_target, then_arguments, budget)?;
            budget.charge_work(1).map_err(|e| e.to_string())?;
            emit(1, *else_target, else_arguments, budget)?;
        }
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            for (n, case) in cases.iter().enumerate() {
                budget.charge_work(1).map_err(|e| e.to_string())?;
                emit(n, case.target, &case.arguments, budget)?;
            }
            budget.charge_work(1).map_err(|e| e.to_string())?;
            emit(cases.len(), *default_target, default_arguments, budget)?;
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            for (n, case) in cases.iter().enumerate() {
                budget.charge_work(1).map_err(|e| e.to_string())?;
                emit(n, case.target, &case.arguments, budget)?;
            }
            budget.charge_work(1).map_err(|e| e.to_string())?;
            emit(cases.len(), *default_target, default_arguments, budget)?;
        }
        Terminator::Return { .. } | Terminator::Unreachable => {}
    }
    Ok(())
}
#[derive(Default)]
struct Counts {
    edges: usize,
    payloads: usize,
    blocks: usize,
    definitions: usize,
    switches: usize,
    masks: usize,
    shifts: usize,
    memory: [usize; 3],
}
fn graph_counts(owner: &OriginalOwner, budget: &mut Budget<'_>) -> Result<Counts, String> {
    budget.charge_work(1).map_err(|e| e.to_string())?;
    let mut counts = Counts::default();
    for function in &owner.module().functions {
        budget.charge_work(1).map_err(|e| e.to_string())?;
        let Some(body) = &function.body else { continue };
        counts.definitions = add(counts.definitions, 1)?;
        counts.blocks = add(counts.blocks, body.blocks.len())?;
        for block in &body.blocks {
            budget.charge_work(1).map_err(|e| e.to_string())?;
            graph_edges(
                block.terminator.as_ref().expect("verified terminator"),
                budget,
                |_, _, arguments, _| {
                    counts.edges = add(counts.edges, 1)?;
                    counts.payloads = add(counts.payloads, arguments.len())?;
                    Ok(())
                },
            )?;
            counts.switches = add(
                counts.switches,
                usize::from(matches!(
                    block.terminator,
                    Some(Terminator::Switch { .. } | Terminator::IntegerSwitch { .. })
                )),
            )?;
            for operation in &block.operations {
                budget.charge_work(1).map_err(|e| e.to_string())?;
                match operation.kind {
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        ..
                    } => counts.masks = add(counts.masks, 1)?,
                    OperationKind::Binary {
                        op: BinaryOp::ShiftLeft | BinaryOp::ShiftRight,
                        ..
                    } => counts.shifts = add(counts.shifts, 1)?,
                    OperationKind::Alloca { .. } => counts.memory[0] = add(counts.memory[0], 1)?,
                    OperationKind::Load { .. } => counts.memory[1] = add(counts.memory[1], 1)?,
                    OperationKind::Store { .. } => counts.memory[2] = add(counts.memory[2], 1)?,
                    _ => {}
                }
            }
        }
    }
    Ok(counts)
}
fn original_graph_rows(
    owner: &OriginalOwner,
    edges: &mut Vec<EdgeRow>,
    payloads: &mut Vec<u32>,
    definitions: &mut Vec<u32>,
    budget: &mut Budget<'_>,
) -> Result<(), String> {
    budget.charge_work(1).map_err(|e| e.to_string())?;
    for (f, function) in owner.module().functions.iter().enumerate() {
        budget.charge_work(1).map_err(|e| e.to_string())?;
        let Some(body) = &function.body else { continue };
        push(
            definitions,
            u32::try_from(f).map_err(|e| e.to_string())?,
            budget,
        )
        .map_err(|e| e.to_string())?;
        for (b, block) in body.blocks.iter().enumerate() {
            budget.charge_work(1).map_err(|e| e.to_string())?;
            graph_edges(
                block.terminator.as_ref().expect("verified terminator"),
                budget,
                |n, target, arguments, budget| {
                    let start = payloads.len();
                    for value in arguments {
                        push(payloads, value.0, budget).map_err(|e| e.to_string())?;
                    }
                    budget
                        .charge_work(body.blocks.len())
                        .map_err(|e| e.to_string())?;
                    let target_index = body
                        .blocks
                        .iter()
                        .position(|block| block.id == target)
                        .unwrap();
                    push(
                        edges,
                        EdgeRow {
                            coordinate: [f as u32, b as u32, n as u32],
                            target: [f as u32, target_index as u32],
                            target_id: target.0,
                            payload: [start, payloads.len()],
                        },
                        budget,
                    )
                    .map_err(|e| e.to_string())
                },
            )?;
        }
    }
    Ok(())
}

#[test]
fn diagnostic_edge_walk_exact_and_each_work_prefix() {
    use fe2o3_kernel_ir::{BlockId, ValueId};
    let terminator = Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(3),
        then_arguments: vec![ValueId(1), ValueId(2)],
        else_target: BlockId(3),
        else_arguments: vec![ValueId(2), ValueId(1)],
    };
    // One terminator plus two distinct successor occurrences, despite equal targets.
    for limit in 0..=3 {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 11);
        budget.reserve_storage(11).unwrap();
        let mut calls = 0;
        let result = graph_edges(
            &terminator,
            &mut budget,
            |ordinal, target, payload, budget| {
                assert_eq!(
                    (ordinal, target, budget.work()),
                    (calls, BlockId(3), 2 + calls)
                );
                let expected = if ordinal == 0 {
                    [ValueId(1), ValueId(2)]
                } else {
                    [ValueId(2), ValueId(1)]
                };
                assert_eq!(payload, &expected);
                calls += 1;
                Ok(())
            },
        );
        assert_eq!(result.is_ok(), limit == 3);
        assert_eq!(calls, limit.saturating_sub(1));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (limit, 11, 11)
        );
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.failed_work(), (limit < 3).then_some(limit + 1));
    }
}

#[test]
fn diagnostic_edge_walk_callback_failure_does_not_visit_later_occurrences() {
    use fe2o3_kernel_ir::{BlockId, ValueId};
    let terminator = Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(3),
        then_arguments: vec![ValueId(1)],
        else_target: BlockId(4),
        else_arguments: vec![ValueId(2)],
    };
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 11);
    budget.reserve_storage(11).unwrap();
    let mut calls = 0;
    let result = graph_edges(
        &terminator,
        &mut budget,
        |ordinal, target, payload, budget| {
            assert_eq!((ordinal, target, budget.work()), (0, BlockId(3), 2));
            assert_eq!(payload, &[ValueId(1)]);
            calls += 1;
            Err("first callback failure".into())
        },
    );
    assert_eq!(result, Err("first callback failure".into()));
    assert_eq!(calls, 1);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (2, 11, 11)
    );
    assert_eq!(budget.failed_storage(), None);
    drop(budget);
    assert_eq!(work.failed_work(), None);
}
fn observation_rows(
    owner: &ProductionPreRankedKirOwnerV1,
    counts: &Counts,
    associations: usize,
    budget: &mut Budget<'_>,
) -> Result<Rows, String> {
    let aliases = owner.assert_origins().source_site_count();
    Ok(Rows {
        original: paid_vec(aliases, budget)?,
        assertions: paid_vec(aliases, budget)?,
        edges: paid_vec(counts.edges, budget)?,
        payloads: paid_vec(counts.payloads, budget)?,
        calls: paid_vec(associations, budget)?,
        pairs: paid_vec(counts.blocks, budget)?,
        incoming: paid_vec(counts.edges, budget)?,
        definitions: paid_vec(counts.definitions, budget)?,
    })
}
fn proof_row(proof: fe2o3_mir_model::SemanticAssertionProofKindV1) -> ProofRow {
    use fe2o3_mir_model::SemanticAssertionProofKindV1 as P;
    match proof {
        P::ExactRange => ProofRow::ExactRange,
        P::CheckedArithmetic => ProofRow::CheckedArithmetic,
        P::LiteralShift => ProofRow::LiteralShift,
    }
}
fn stage_row(pass: fe2o3_kernel_analysis::KernelCheckPassKindV1) -> u8 {
    use fe2o3_kernel_analysis::KernelCheckPassKindV1 as P;
    match pass {
        P::TensorLayout => 0,
        P::MemoryBounds => 1,
        P::AtomicLegality => 2,
        P::RaceFreedom => 3,
        P::HierarchicalOwnership => 4,
        P::BarrierConvergence => 5,
        P::PipelineProtocol => 6,
        P::WorkgroupMemory => 7,
        P::SemanticRefinement => 8,
        _ => u8::MAX,
    }
}
fn resources(row: fe2o3_pliron::CanonicalRankedPolicyResourceObservationV1) -> [usize; 3] {
    [
        row.work_upper_bound(),
        row.retained_storage_units(),
        row.peak_storage_units(),
    ]
}
#[derive(Clone, Copy)]
struct Summary {
    policy_subject: Subject,
    policy_resources: [usize; 3],
    source_resources: [usize; 3],
    resource_denial: bool,
    associations: usize,
    call_transport: [usize; 4],
    spans: usize,
    zero_spans: usize,
    memory: [usize; 2],
    pending: usize,
}
fn fill_rows(
    owner: &ProductionPreRankedKirOwnerV1,
    view: &BView<'_, '_, '_>,
    rows: &mut Rows,
    fixture: &Origin,
    budget: &mut Budget<'_>,
) -> Result<Summary, Af> {
    let metadata = view.metadata(budget)?;
    let inventory = metadata.inventory(budget)?;
    let policies = view.policies(budget)?;
    acheck(
        std::ptr::eq(inventory.owner(), owner.executable())
            && std::ptr::eq(policies.owner(budget)?, owner.executable()),
        "same original graph owner",
    )?;
    for edge in inventory.edges() {
        let start = rows.payloads.len();
        for value in edge.arguments {
            push(&mut rows.payloads, value.0, budget)?;
        }
        push(
            &mut rows.edges,
            EdgeRow {
                coordinate: edge_coordinate(edge.coordinate),
                target: block_coordinate(edge.target),
                target_id: edge.target_id.0,
                payload: [start, rows.payloads.len()],
            },
            budget,
        )?;
    }
    let spans = metadata.spans(budget)?;
    budget.charge_work(spans.len())?;
    let span_count = spans.len();
    let zero_spans = spans
        .iter()
        .filter(|span| span.operations().is_empty())
        .count();
    let original_rows = metadata.assertions(budget)?;
    acheck(
        original_rows.len() == view.assertion_count(budget)?
            && original_rows.len() == owner.assert_origins().source_site_count(),
        "complete original source alias extent",
    )?;
    let semantic = owner.semantic_ssa().source_semantic();
    for (ordinal, original) in original_rows.iter().enumerate() {
        budget.charge_work(8)?;
        let span = &spans[original.span()];
        let Site::Terminator {
            span: source_span,
            source,
        } = span.site()
        else {
            return Err(Af::Binding {
                span: Some(original.span()),
                detail: "assertion source terminator",
            });
        };
        let STerminator::Assert {
            condition: _,
            expected,
            message,
            target,
            ..
        } = source.kind()
        else {
            return Err(Af::Binding {
                span: Some(original.span()),
                detail: "actual source Assert",
            });
        };
        let actual_function =
            &semantic.functions()[source_span.semantic_function().index() as usize];
        acheck(
            std::ptr::eq(
                source,
                actual_function.blocks()[source_span.semantic_block().index() as usize]
                    .terminator(),
            ),
            "borrowed original source terminator identity",
        )?;
        let binding = original.binding();
        let sealed = owner
            .assert_origins()
            .assert_condition(
                source_span.correspondence_owner(),
                source_span.semantic_function(),
                source_span.semantic_block(),
                budget,
            )
            .map_err(|_| Af::Binding {
                span: Some(original.span()),
                detail: "sealed original assertion binding",
            })?;
        acheck(
            binding == sealed
                && binding.expected() == *expected
                && binding.semantic_success() == target.target(),
            "original condition polarity/success",
        )?;
        use fe2o3_mir_model::semantic_mir_v1::{
            SemanticConstantValueV1 as ConstantValue, SemanticOperandV1 as Operand,
            SemanticScalarTypeV1 as Scalar, SemanticTypeShapeV1 as Shape,
        };
        let signed_literal = matches!(message,
            AssertMessage::Overflow { operation: SBinary::ShiftLeft, right: Operand::Constant(constant), .. }
                if matches!(constant.value(), ConstantValue::Scalar(value) if value.bits() == 3 && value.size_bytes() == 4)
                    && matches!(semantic.types()[constant.ty().index() as usize].shape(),
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
            AssertMessage::BoundsCheck { .. } => MessageRow::Bounds,
            _ => {
                return Err(Af::Binding {
                    span: Some(original.span()),
                    detail: "unexpected actual fixture assertion diagnostic",
                });
            }
        };
        let provenance = source.source();
        let origin = source_origin(provenance.call_site().ok_or(Af::Binding {
            span: Some(original.span()),
            detail: "actual assertion call-site provenance",
        })?);
        let expansion = source_origin(provenance.expansion().ok_or(Af::Binding {
            span: Some(original.span()),
            detail: "actual assertion expansion provenance",
        })?);
        acheck(
            origin_valid(&origin, fixture) && origin_valid(&expansion, fixture),
            "compiled fixture assertion provenance",
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
        acheck(
            proved.span() == original.span() && proved.binding() == binding,
            "fresh B exact original binding",
        )?;
        push(&mut rows.original, row, budget)?;
        push(
            &mut rows.assertions,
            AssertionRow {
                proof: proof_row(proved.proof_kind()),
                ..row
            },
            budget,
        )?;
    }
    let associations = metadata.function_count(budget)?;
    let mut source_count = 0usize;
    for association in 0..associations {
        let function = metadata.function(association, budget)?;
        let actual = function.source();
        let semantic_function = &semantic.functions()[actual.semantic_function().index() as usize];
        for (block, body) in semantic_function.blocks().iter().enumerate() {
            budget.charge_work(rows.original.len() + 1)?;
            if matches!(body.terminator().kind(), STerminator::Assert { .. }) {
                acheck(
                    rows.original
                        .iter()
                        .filter(|row| {
                            row.source
                                == [
                                    actual.correspondence_owner().index(),
                                    actual.semantic_function().index(),
                                    block as u32,
                                ]
                        })
                        .count()
                        == 1,
                    "all source assertions represented exactly once per association",
                )?;
                source_count += 1;
            }
        }
    }
    acheck(
        source_count == rows.original.len(),
        "no extra source assertion aliases",
    )?;
    for call in view.callables(budget)? {
        budget.charge_work(5)?;
        let function = metadata.function(call.association(), budget)?;
        let association = function.source();
        acheck(
            call.function() == function.canonical().coordinate
                && call.source_function() == association.semantic_function(),
            "exact source callable and canonical function",
        )?;
        let helper = association.role() == SemanticKirFunctionRoleV1::InternalHelper;
        budget.charge_work(inventory.calls().len())?;
        let physical_calls = inventory
            .calls()
            .iter()
            .filter(|edge| edge.target == Some(call.function()))
            .count();
        let semantic_function = &semantic.functions()[call.source_function().index() as usize];
        let abi = [
            semantic_function.abi().source_input_types().len(),
            function.canonical().function.signature.parameters.len(),
            function.canonical().function.signature.results.len(),
        ];
        let source_unit = matches!(
            semantic.types()[semantic_function.abi().source_output_type().index() as usize].shape(),
            fe2o3_mir_model::semantic_mir_v1::SemanticTypeShapeV1::Unit
        );
        let frame = if helper {
            if let Some(frame) = metadata.helper_frame(call.association(), budget)? {
                acheck(
                    std::ptr::eq(frame.source(), association)
                        && std::ptr::eq(frame.function(), function.canonical().function),
                    "actual source helper frame identity",
                )?;
                [
                    frame.allocations().len(),
                    frame.accesses().len(),
                    frame.control().len(),
                    frame.edge_bindings().len(),
                ]
            } else {
                [0; 4]
            }
        } else {
            [0; 4]
        };
        let kind = match call.kind() {
            ProductionCanonicalAssertionCallKindV1::Root => CallKind::Root,
            ProductionCanonicalAssertionCallKindV1::EmptyOnly => CallKind::EmptyOnly,
            ProductionCanonicalAssertionCallKindV1::DeterministicEmpty => {
                CallKind::DeterministicEmpty
            }
            ProductionCanonicalAssertionCallKindV1::PrivateFrame => CallKind::PrivateFrame,
        };
        use fe2o3_mir_model::SemanticCallableDecisionV1 as D;
        let decision = match call.source_decision() {
            D::Rejected => DecisionRow::Rejected,
            D::ExactEmptyOnly => DecisionRow::EmptyOnly,
            D::ExactEmptyDeterministicScalar => DecisionRow::DeterministicEmpty,
        };
        push(
            &mut rows.calls,
            CallRow {
                association: call.association(),
                source: [
                    association.correspondence_owner().index(),
                    association.semantic_function().index(),
                ],
                function: call.function().0,
                helper,
                kind,
                decision,
                frame,
                physical_calls,
                abi,
                source_unit,
            },
            budget,
        )?;
    }
    let mut call_transport = [metadata.call_transport_count(budget)?, 0, 0, 0];
    for ordinal in 0..call_transport[0] {
        let transport = metadata.call_transport(ordinal, budget)?;
        let (root, function, block) = transport.source_site();
        acheck(
            (root.index() as usize) < semantic.functions().len()
                && (function.index() as usize) < semantic.functions().len()
                && (block.index() as usize)
                    < semantic.functions()[function.index() as usize]
                        .blocks()
                        .len(),
            "actual call/return source coordinates",
        )?;
        if let Some((start, call, end)) = transport.call_operation_span() {
            acheck(
                start <= call && call < end && transport.destination().is_some(),
                "actual call/destination span",
            )?;
            call_transport[1] += 1;
        } else {
            call_transport[2] += 1;
        }
        budget.charge_work(transport.component_count())?;
        for component in 0..transport.component_count() {
            acheck(
                transport.component(component).is_some(),
                "complete actual ABI result components",
            )?;
            call_transport[3] += 1;
        }
    }
    for ordinal in 0..policies.pair_count(budget)? {
        let pair = policies.pair(ordinal, budget)?;
        let range = pair.incoming_edges();
        let call = pair.call();
        push(
            &mut rows.pairs,
            PairRow {
                call: [call.block.function.0, call.block.block, call.operation],
                declaration: pair.declaration().0,
                incoming: [range.start, range.end],
            },
            budget,
        )?;
        for ordinal in range {
            let incoming = policies.incoming_edge(ordinal, budget)?;
            push(
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
                budget.charge_work(rows.edges.len() + physical.arguments.len())?;
                let observed = rows
                    .edges
                    .iter()
                    .find(|edge| edge.coordinate == edge_coordinate(physical.coordinate))
                    .unwrap();
                acheck(
                    observed.target == block_coordinate(physical.target)
                        && observed.target_id == physical.target_id.0
                        && rows.payloads[observed.payload[0]..observed.payload[1]]
                            .iter()
                            .copied()
                            .eq(physical.arguments.iter().map(|value| value.0)),
                    "full ordered physical incoming successor payload",
                )?;
            }
        }
    }
    for ordinal in 0..policies.definition_count(budget)? {
        let function = policies.definition_coordinate(ordinal, budget)?;
        let report = policies.report(ordinal, budget)?;
        let history = policies.history(ordinal, budget)?;
        let floor = history.floor();
        let invocation = history.invocation();
        push(
            &mut rows.definitions,
            DefinitionRow {
                ordinal,
                function: function.0,
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
    acheck(
        !view.ranked_verification_is_complete()
            && !view.grants_artifact_or_launch_authority()
            && !policies.ranked_verification_is_complete()
            && !policies.grants_artifact_or_launch_authority(),
        "no completed compiler or publication authority",
    )?;
    let policy_subject = subject(policies.owner(budget)?);
    let memory = view.memory_census(budget)?;
    let policy_observation = policies.observation(budget)?;
    let source_observation = view.source_resources(budget)?;
    Ok(Summary {
        policy_subject,
        policy_resources: resources(policy_observation),
        source_resources: [
            source_observation.work,
            source_observation.storage,
            source_observation.peak,
        ],
        resource_denial: policy_observation.first_denial().is_some()
            || policy_observation.caught_panic()
            || source_observation.failed_storage.is_some(),
        associations,
        call_transport,
        spans: span_count,
        zero_spans,
        memory,
        pending: policies.pending_obligations().iter().count(),
    })
}
fn observe(tcx: TyCtxt<'_>, request: &AssertionRequest) -> Result<Observation, String> {
    let cpu = tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref());
    require(
        cpu == request.target.cpu(),
        "actual compiler collection target",
    )?;
    let fixture = active_fixture(tcx, &request.source[3])?;
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|e| e.to_string())?;
    let wire = transaction
        .consume_pre_ranked_for_test_v1(|owner, descriptors| {
            let semantic = owner.semantic_ssa().source_semantic();
            let source_roots = census::roots(semantic)?;
            require(
                descriptors.len() == request.case.roots().len()
                    && source_roots.len() == owner.executable().module().kernels.len()
                    && source_roots.len() == descriptors.len(),
                "actual complete descriptor/source roots",
            )?;
            let roots = source_roots
                .into_iter()
                .zip(&owner.executable().module().kernels)
                .map(|(root, kernel)| Root {
                    name: root.name,
                    function: root.function,
                    body: root.body,
                    entry: kernel.entry.as_str().into(),
                })
                .collect::<Vec<_>>();
            let original = subject(owner.executable());
            let source_floor = owner
                .unit_local_source_storage_floor_v1()
                .map_err(|e| e.to_string())?;
            let mut work = Work::new(
                usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                    .map_err(|e| e.to_string())?,
            );
            let mut budget = Budget::new(
                &mut work,
                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            );
            budget
                .reserve_storage(source_floor)
                .map_err(|e| e.to_string())?;
            // These diagnostic owners remain paid at the caller floor during every callback.
            let root_bytes = roots.iter().try_fold(
                mul(roots.capacity(), std::mem::size_of::<Root>())?,
                |bytes, root| add(add(bytes, root.name.capacity())?, root.entry.capacity()),
            )?;
            budget
                .reserve_storage(add(
                    std::mem::size_of::<Observation>()
                        + std::mem::size_of::<Summary>()
                        + std::mem::size_of::<Counts>(),
                    root_bytes,
                )?)
                .map_err(|e| e.to_string())?;
            let counts = graph_counts(owner.executable(), &mut budget)?;
            let preflight_floor = budget.storage();
            let associations = owner
                .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
                    let source = view.metadata(budget)?;
                    assert!(std::ptr::eq(
                        source.inventory(budget)?.owner(),
                        owner.executable()
                    ));
                    source.function_count(budget)
                })
                .map_err(|e| e.to_string())?;
            require(
                budget.storage() == preflight_floor,
                "actual association preflight restores its caller floor",
            )?;
            let mut original_edges = paid_vec(counts.edges, &mut budget)?;
            let mut original_payloads = paid_vec(counts.payloads, &mut budget)?;
            let mut physical_definitions = paid_vec(counts.definitions, &mut budget)?;
            original_graph_rows(
                owner.executable(),
                &mut original_edges,
                &mut original_payloads,
                &mut physical_definitions,
                &mut budget,
            )?;
            let mut rows = observation_rows(&owner, &counts, associations, &mut budget)?;
            budget
                .reserve_storage(std::mem::size_of::<Vec<u8>>())
                .map_err(|e| e.to_string())?;
            let mut original_bytes = paid_vec(
                owner.executable().canonical().canonical_bytes().len(),
                &mut budget,
            )?;
            original_bytes.extend_from_slice(owner.executable().canonical().canonical_bytes());
            budget
                .charge_work(original_bytes.len())
                .map_err(|e| e.to_string())?;
            let diagnostics_floor = budget.storage() - source_floor;
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let slot = std::ptr::from_ref(&budget) as usize;
            let summary = owner
                .with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
                    Ok(
                        source.with_assertion_policy_checks_v1(budget, |view, budget| {
                            fill_rows(&owner, view, &mut rows, &fixture, budget)
                        }),
                    )
                })
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            require(
                budget.storage() == floor
                    && budget.work_ledger_identity_v1() == ledger
                    && std::ptr::from_ref(&budget) as usize == slot,
                "original ledger/floor restored",
            )?;
            let mut semantic_switches = 0;
            let mut semantic_masks = 0;
            let mut semantic_shifts = 0;
            for function in semantic.functions() {
                for block in function.blocks() {
                    budget
                        .charge_work(block.statements().len() + 1)
                        .map_err(|e| e.to_string())?;
                    semantic_switches += usize::from(matches!(
                        block.terminator().kind(),
                        STerminator::SwitchInt { .. }
                    ));
                    for statement in block.statements() {
                        if let SStatement::Assign(assignment) = statement.kind() {
                            match assignment.value().kind() {
                                SRvalue::Binary {
                                    operation: SBinary::BitAnd,
                                    ..
                                } => semantic_masks += 1,
                                SRvalue::Binary {
                                    operation: SBinary::ShiftLeft | SBinary::ShiftRight,
                                    ..
                                } => semantic_shifts += 1,
                                _ => {}
                            }
                        }
                    }
                }
            }
            let source_assertions = owner.assert_origins().source_site_count();
            let physical_assertions = owner.assert_origins().binding_count();
            let private_policy =
                owner.helper_source_policy_v1() == ProductionHelperSourcePolicyV1::UnitLocal;
            let after = subject(owner.executable());
            require(
                original == after
                    && original_bytes == owner.executable().canonical().canonical_bytes(),
                "unchanged original N bytes",
            )?;
            let wire_limit = mul(diagnostics_floor, 64)?.min(REPORT_CAP);
            budget
                .reserve_storage(
                    std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<BoundedOutput<'_, '_>>(),
                )
                .map_err(|e| e.to_string())?;
            let mut wire = paid_vec(wire_limit, &mut budget)?;
            let protocol_storage = budget.storage() - floor;
            let final_work = budget.work();
            let observed = Observation {
                actual_target: request.target,
                original: original.clone(),
                after,
                policy_subject: summary.policy_subject,
                roots,
                fixture,
                original_edges,
                original_payloads,
                physical_definitions,
                rows,
                source_assertions,
                physical_assertions,
                associations: summary.associations,
                call_transport: summary.call_transport,
                spans: summary.spans,
                zero_spans: summary.zero_spans,
                semantic_switches,
                semantic_masks,
                semantic_shifts,
                graph_switches: counts.switches,
                graph_masks: counts.masks,
                graph_shifts: counts.shifts,
                graph_memory: counts.memory,
                memory: summary.memory,
                policy_resources: summary.policy_resources,
                source_resources: summary.source_resources,
                resource_denial: summary.resource_denial,
                private_policy,
                pending: summary.pending,
                complete: false,
                authority: false,
                source_floor,
                diagnostics_floor,
                restored_floor: floor,
                final_storage: 0,
                protocol_storage,
                work: final_work,
            };
            serde_json::to_writer(
                BoundedOutput {
                    bytes: &mut wire,
                    budget: &mut budget,
                    limit: wire_limit,
                },
                &observed,
            )
            .map_err(|e| e.to_string())?;
            drop(observed);
            drop(original_bytes);
            drop(owner);
            budget
                .release_storage(source_floor)
                .map_err(|e| e.to_string())?;
            budget
                .release_storage(diagnostics_floor)
                .map_err(|e| e.to_string())?;
            require(
                budget.storage() == protocol_storage,
                "source/structured floor restored; inert output remains live",
            )?;
            // The ledger ends with only explicitly identified protocol backing.
            // These bytes are detached diagnostics, not a production owner/receipt.
            drop(budget);
            Ok::<_, String>(wire)
        })
        .map_err(|e| e.to_string())??;
    require(wire.len() <= REPORT_CAP, "bounded inert observation")?;
    serde_json::from_slice(&wire).map_err(|e| e.to_string())
}

struct BoundedOutput<'b, 'w> {
    bytes: &'b mut Vec<u8>,
    budget: &'b mut Budget<'w>,
    limit: usize,
}
impl Write for BoundedOutput<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("protocol length overflow"))?;
        if end > self.limit || end > self.bytes.capacity() {
            return Err(std::io::Error::other("prepaid bounded protocol output"));
        }
        self.budget
            .charge_work(bytes.len())
            .map_err(std::io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
