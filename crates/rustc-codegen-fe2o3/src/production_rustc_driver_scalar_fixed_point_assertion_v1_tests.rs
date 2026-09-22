//! Exact source assertion identity and independent graph-route oracle.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssertMessageV1 as AssertMessage, SemanticBinaryOpV1 as SemanticBinary,
    SemanticConstantValueV1 as ConstantValue, SemanticOperandV1 as Operand,
    SemanticSourceOriginV1 as SourceOrigin, SemanticSourceProvenanceV1 as Provenance,
    SemanticTerminatorKindV1 as SemanticTerminator,
};
use rustc_span::{FileName, SourceFile, Span};
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OriginRow {
    file: [u8; 32],
    bytes: [u64; 2],
    start: [u32; 2],
    end: [u32; 2],
}
impl OriginRow {
    fn new(source: fe2o3_mir_model::semantic_mir_v1::SemanticSourceOriginV1) -> Self {
        let (a, b) = source.byte_range();
        let (line, column) = source.start_coordinate();
        let (end_line, end_column) = source.end_coordinate();
        Self {
            file: *source.file().as_bytes(),
            bytes: [a, b],
            start: [line, column],
            end: [end_line, end_column],
        }
    }
    fn valid(&self) -> bool {
        self.file != [0; 32]
            && self.bytes[0] < self.bytes[1]
            && self.start[0] > 0
            && self.start <= self.end
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssertionRow {
    pub(super) input: Subject,
    pub(super) root: String,
    pub(super) entry: String,
    source_function: [u8; 32],
    source_body: [u8; 32],
    semantic_block: u32,
    expected: bool,
    condition_local: Option<u32>,
    expansion: Option<OriginRow>,
    call_site: Option<OriginRow>,
    fixture_file: OriginRow,
    pub(super) success_edge: [u32; 3],
    pub(super) failure_edge: [u32; 3],
}
impl AssertionRow {
    pub(super) fn valid(&self) -> bool {
        self.input.digest != [0; 32]
            && self.input.bytes > 0
            && self.root == "scalar_effect_then_overflow"
            && !self.entry.is_empty()
            && self.source_function != [0; 32]
            && self.source_body != [0; 32]
            && !self.expected
            && self.expansion.as_ref().is_some_and(OriginRow::valid)
            && self.call_site.as_ref().is_some_and(OriginRow::valid)
            && self.fixture_file.valid()
            && [&self.expansion, &self.call_site]
                .iter()
                .all(|origin| origin.as_ref().is_some_and(|origin| self.contains(origin)))
            && self.success_edge[..2] == self.failure_edge[..2]
            && self.success_edge[2] != self.failure_edge[2]
    }
    fn contains(&self, origin: &OriginRow) -> bool {
        origin.file == self.fixture_file.file
            && self.fixture_file.bytes[0] == 0
            && origin.bytes[0] < origin.bytes[1]
            && origin.bytes[1] <= self.fixture_file.bytes[1]
            && origin.start >= self.fixture_file.start
            && origin.end <= self.fixture_file.end
    }
    pub(super) fn check_file(&self, file: &FileStamp) -> ResultV1<()> {
        require(
            self.valid(),
            Phase::Ranked,
            "independent active fixture identity/range",
        )?;
        let bytes = read_fixture(file)?;
        require(
            bytes.len() <= INPUT_CAP
                && digest(&bytes) == file.sha256
                && self.fixture_file.bytes == [0, bytes.len() as u64],
            Phase::Ranked,
            "assert source file changed",
        )?;
        for origin in [&self.expansion, &self.call_site] {
            let origin = origin
                .as_ref()
                .ok_or_else(|| failure(Phase::Ranked, "missing assert source origin"))?;
            let a = usize::try_from(origin.bytes[0]).map_err(|e| failure(Phase::Ranked, e))?;
            let b = usize::try_from(origin.bytes[1]).map_err(|e| failure(Phase::Ranked, e))?;
            require(
                bytes.get(a..b) == Some(b"value + 1".as_slice()),
                Phase::Ranked,
                "assert is not the unchanged nonneutral source expression",
            )?;
            let prefix = bytes
                .get(..a)
                .ok_or_else(|| failure(Phase::Ranked, "assert source offset"))?;
            let line = prefix.iter().filter(|byte| **byte == b'\n').count() + 1;
            let column = prefix
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(a + 1, |position| a - position);
            require(
                origin.start[0] as usize == line
                    && origin.end[0] as usize == line
                    && origin.start[1] as usize == column
                    && origin.end[1] as usize == column + b - a,
                Phase::Ranked,
                "assert source line coordinates",
            )?;
        }
        Ok(())
    }
}
pub(super) struct SelectedAssertion {
    root: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    source_function: [u8; 32],
    source_body: [u8; 32],
    block: usize,
    expected: bool,
    condition_local: Option<u32>,
    source: Provenance,
    success: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    failure: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
}
impl SelectedAssertion {
    pub(super) fn row(
        &self,
        owner: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
        fixture_file: SourceOrigin,
    ) -> AssertionRow {
        let input = owner.executable();
        let root = &owner.semantic_ssa().source_semantic().functions()[self.root.index() as usize];
        let edge = |edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1| {
            [edge.source.function.0, edge.source.block, edge.successor]
        };
        AssertionRow {
            input: subject(input),
            root: std::str::from_utf8(root.kernel_entry().unwrap().export_symbol().as_bytes())
                .unwrap()
                .into(),
            entry: input.module().functions[self.failure.source.function.0 as usize]
                .id
                .as_str()
                .into(),
            source_function: self.source_function,
            source_body: self.source_body,
            semantic_block: self.block as u32,
            expected: self.expected,
            condition_local: self.condition_local,
            expansion: self.source.expansion().map(OriginRow::new),
            call_site: self.source.call_site().map(OriginRow::new),
            fixture_file: OriginRow::new(fixture_file),
            success_edge: edge(self.success),
            failure_edge: edge(self.failure),
        }
    }
    pub(super) fn matches_refusal(
        &self,
        result: &Result<
            crate::production_pipeline::RankedVerifiedProductionCompilation,
            crate::production_pipeline::ProductionPipelineError,
        >,
    ) -> bool {
        matches!(result, Err(crate::production_pipeline::ProductionPipelineError::RankedProjection(
            crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::UnprovenAssert {
                block, kind: "arithmetic-overflow", expected, condition_local, source,
            })) if *block == self.block && *expected == self.expected
                && *condition_local == self.condition_local && source.as_ref() == &self.source)
    }
}
pub(super) fn select_assertion(
    owner: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    budget: &mut Budget<'_>,
) -> ResultV1<SelectedAssertion> {
    let storage = std::mem::size_of::<fe2o3_lower_mir_kernel::SemanticKirAssertOriginsV1<'_>>()
        + std::mem::size_of::<fe2o3_lower_mir_kernel::SemanticKirAssertConditionBindingV1>();
    budget
        .reserve_storage(storage)
        .map_err(|e| failure(Phase::Ranked, e))?;
    let result = select_assertion_inner(owner, budget);
    budget
        .release_storage(storage)
        .map_err(|e| failure(Phase::Ranked, e))?;
    result
}
fn unique_file<'a, T>(
    active: &Path,
    candidates: impl IntoIterator<Item = (Option<&'a Path>, T)>,
) -> ResultV1<T> {
    let mut selected = None;
    for (path, file) in candidates {
        let Some(path) = path.and_then(|path| path.canonicalize().ok()) else {
            continue;
        };
        if path == active {
            require(
                selected.replace(file).is_none(),
                Phase::Ranked,
                "ambiguous actual fixture file",
            )?;
        }
    }
    selected.ok_or_else(|| failure(Phase::Ranked, "fixture absent from actual SourceMap"))
}
fn read_fixture(stamp: &FileStamp) -> ResultV1<Vec<u8>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let input = options
        .open(&stamp.path)
        .map_err(|e| failure(Phase::Ranked, e))?;
    let metadata = input.metadata().map_err(|e| failure(Phase::Ranked, e))?;
    require(
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= INPUT_CAP as u64,
        Phase::Ranked,
        "exact regular fixture length",
    )?;
    let mut original = Vec::new();
    input
        .take(metadata.len() + 1)
        .read_to_end(&mut original)
        .map_err(|e| failure(Phase::Ranked, e))?;
    require(
        original.len() as u64 == metadata.len() && digest(&original) == stamp.sha256,
        Phase::Ranked,
        "exact bounded original fixture bytes",
    )?;
    Ok(original)
}
fn check_original(file: &SourceFile, stamp: &FileStamp) -> ResultV1<()> {
    let length = file.unnormalized_source_len as usize;
    require(
        length > 0 && length <= INPUT_CAP,
        Phase::Ranked,
        "bounded compiled fixture bytes",
    )?;
    let bytes = read_fixture(stamp)?;
    let original = std::str::from_utf8(&bytes).map_err(|e| failure(Phase::Ranked, e))?;
    require(
        original.len() == length
            && digest(original.as_bytes()) == stamp.sha256
            && file.src_hash.matches(original)
            && file
                .src
                .as_ref()
                .is_some_and(|normalized| normalized.as_bytes() == original.as_bytes()),
        Phase::Ranked,
        "actual rustc/stamped fixture bytes; normalized offsets required",
    )
}
pub(super) fn resolve_fixture(tcx: TyCtxt<'_>, stamp: &FileStamp) -> ResultV1<SourceOrigin> {
    let active = stamp
        .path
        .canonicalize()
        .map_err(|e| failure(Phase::Ranked, e))?;
    require(
        active == stamp.path,
        Phase::Ranked,
        "canonical fixture path",
    )?;
    // Drop the files guard before the canonical adapter performs a SourceMap lookup.
    let file = {
        let files = tcx.sess.source_map().files();
        unique_file(
            &active,
            files.iter().map(|file| {
                let path = match &file.name {
                    FileName::Real(name) => name.local_path(),
                    _ => None,
                };
                (path, file)
            }),
        )?
        .clone()
    };
    check_original(&file, stamp)?;
    let span = Span::with_root_ctxt(file.start_pos, file.end_position());
    let source = crate::rustc_semantic_adapter_v1::canonical_source_provenance_v1(tcx, span, 0)
        .map_err(|e| failure(Phase::Ranked, e))?
        .provenance();
    let whole = source
        .call_site()
        .ok_or_else(|| failure(Phase::Ranked, "fixture origin absent"))?;
    require(
        source.expansion() == Some(whole)
            && whole.byte_range() == (0, u64::from((file.end_position() - file.start_pos).0)),
        Phase::Ranked,
        "exact whole fixture provenance",
    )?;
    Ok(whole)
}
fn select_assertion_inner(
    owner: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    budget: &mut Budget<'_>,
) -> ResultV1<SelectedAssertion> {
    use fe2o3_lower_mir_kernel::SemanticKirAssertConditionOutcomeV1 as Outcome;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBlockIdV1, SemanticScalarTypeV1 as Scalar, SemanticTypeShapeV1 as Shape,
    };
    let semantic = owner.semantic_ssa().source_semantic();
    let mut selected = None;
    for root in semantic.roots() {
        budget
            .charge_work(4)
            .map_err(|e| failure(Phase::Ranked, e))?;
        let root_function = &semantic.functions()[root.index() as usize];
        let body = semantic
            .select_kernel_body_for_root_v1(*root)
            .ok_or_else(|| failure(Phase::Ranked, "actual assert source body absent"))?
            .body();
        let function = &semantic.functions()[body.index() as usize];
        for (block_index, block) in function.blocks().iter().enumerate() {
            budget
                .charge_work(8)
                .map_err(|e| failure(Phase::Ranked, e))?;
            let SemanticTerminator::Assert {
                condition,
                expected,
                message:
                    AssertMessage::Overflow {
                        operation: SemanticBinary::Add,
                        left,
                        right,
                    },
                target,
                ..
            } = block.terminator().kind()
            else {
                continue;
            };
            let Operand::Constant(constant) = right else {
                continue;
            };
            if !matches!(constant.value(), ConstantValue::Scalar(value) if value.bits() == 1 && value.size_bytes() == 4)
                || matches!(left, Operand::Constant(_))
            {
                continue;
            }
            require(
                !*expected
                    && left.ty() == right.ty()
                    && matches!(
                        semantic.types()[left.ty().index() as usize].shape(),
                        Shape::Scalar(Scalar::Integer {
                            signed: false,
                            bits: 32
                        })
                    ),
                Phase::Ranked,
                "selected actual source Add is not exact checked u32 + 1",
            )?;
            let binding = owner
                .assert_origins()
                .assert_condition(
                    *root,
                    body,
                    SemanticBlockIdV1::from_index(block_index as u32),
                    budget,
                )
                .map_err(|e| failure(Phase::Ranked, e))?;
            require(
                binding.expected() == *expected && binding.semantic_success() == target.target(),
                Phase::Ranked,
                "source/physical assert polarity or success changed",
            )?;
            let Outcome::Emitted {
                success_edge,
                failure_edge,
                ..
            } = binding.outcome()
            else {
                return Err(failure(
                    Phase::Ranked,
                    "nonneutral actual source assertion was elided",
                ));
            };
            require(
                selected.is_none(),
                Phase::Ranked,
                "multiple source nonneutral Add assertions",
            )?;
            selected = Some(SelectedAssertion {
                root: *root,
                source_function: *root_function.identity().as_bytes(),
                source_body: *function.identity().as_bytes(),
                block: block_index,
                expected: *expected,
                condition_local: match condition {
                    Operand::Copy(place) | Operand::Move(place)
                        if place.projections().is_empty() =>
                    {
                        Some(place.local().index())
                    }
                    _ => None,
                },
                source: block.terminator().source(),
                success: success_edge,
                failure: failure_edge,
            });
        }
    }
    selected.ok_or_else(|| {
        failure(
            Phase::Ranked,
            "missing actual source nonneutral Add assertion",
        )
    })
}
fn scalar_bits(value: &Constant) -> Option<u128> {
    match value {
        Constant::Bool(v) => Some(u128::from(*v)),
        Constant::U8(v) => Some((*v).into()),
        Constant::U16(v) => Some((*v).into()),
        Constant::U32(v) => Some((*v).into()),
        Constant::U64(v) | Constant::Index(v) => Some((*v).into()),
        _ => None,
    }
}
fn flag_values(
    function: &Function,
    overflow: ValueId,
    truth: bool,
) -> ResultV1<std::collections::BTreeMap<ValueId, u128>> {
    use fe2o3_kernel_ir::{CastKind, ComparePredicate as Compare, UnaryOp};
    let count = operations(function).count();
    require(
        count <= OBSERVATION_CAP,
        Phase::Sim,
        "bounded assertion expression",
    )?;
    let mut values = std::collections::BTreeMap::from([(overflow, u128::from(truth))]);
    for _ in 0..=count {
        let mut changed = false;
        for op in operations(function) {
            let [result] = op.results.as_slice() else {
                continue;
            };
            let binary =
                |left: &ValueId, right: &ValueId| Some((*values.get(left)?, *values.get(right)?));
            let value = match &op.kind {
                Kind::Constant(value) => scalar_bits(value),
                Kind::Unary {
                    op: UnaryOp::Not,
                    operand,
                } if result.ty == Type::Scalar(ScalarType::Bool) => {
                    values.get(operand).filter(|v| **v <= 1).map(|v| 1 - *v)
                }
                Kind::Cast {
                    kind: CastKind::ZeroExtend,
                    value,
                    ..
                } => values.get(value).copied(),
                Kind::Compare {
                    predicate,
                    lhs,
                    rhs,
                } => binary(lhs, rhs).map(|(a, b)| {
                    u128::from(match predicate {
                        Compare::Equal => a == b,
                        Compare::NotEqual => a != b,
                        Compare::LessThan => a < b,
                        Compare::LessThanOrEqual => a <= b,
                        Compare::GreaterThan => a > b,
                        Compare::GreaterThanOrEqual => a >= b,
                    })
                }),
                Kind::Binary { op, lhs, rhs } if result.ty == Type::Scalar(ScalarType::Bool) => {
                    binary(lhs, rhs).and_then(|(a, b)| match op {
                        BinaryOp::BitAnd => Some(a & b),
                        BinaryOp::BitOr => Some(a | b),
                        BinaryOp::BitXor => Some(a ^ b),
                        _ => None,
                    })
                }
                Kind::Select {
                    condition,
                    true_value,
                    false_value,
                } => values
                    .get(condition)
                    .and_then(|c| match c {
                        0 => values.get(false_value),
                        1 => values.get(true_value),
                        _ => None,
                    })
                    .copied(),
                _ => None,
            };
            if let Some(value) = value {
                if let Some(previous) = values.insert(result.id, value) {
                    require(
                        previous == value,
                        Phase::Sim,
                        "assertion expression changed an SSA definition",
                    )?;
                } else {
                    changed = true;
                }
            }
        }
        if !changed {
            return Ok(values);
        }
    }
    Err(failure(
        Phase::Sim,
        "assertion expression did not reach bounded closure",
    ))
}
fn selected_edge(
    term: &Terminator,
    values: &std::collections::BTreeMap<ValueId, u128>,
) -> Option<usize> {
    match term {
        Terminator::ConditionalBranch { condition, .. } => match *values.get(condition)? {
            1 => Some(0),
            0 => Some(1),
            _ => None,
        },
        Terminator::Switch {
            selector, cases, ..
        } => {
            let value = *values.get(selector)?;
            Some(
                cases
                    .iter()
                    .position(|case| u128::from(case.value) == value)
                    .unwrap_or(cases.len()),
            )
        }
        Terminator::IntegerSwitch {
            selector, cases, ..
        } => {
            let value = *values.get(selector)?;
            let mut selected = cases.len();
            for (index, case) in cases.iter().enumerate() {
                if scalar_bits(&case.value)? == value {
                    selected = index;
                }
            }
            Some(selected)
        }
        _ => None,
    }
}

#[test]
fn scalar_assertion_oracle_requires_exact_polarity_and_transparent_failure_path() {
    use fe2o3_kernel_ir::{BasicBlock, BlockId, ComparePredicate, Signature, ValueDef};
    let boolean = Type::Scalar(ScalarType::Bool);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), boolean.clone()),
        Kind::Constant(Constant::Bool(false)),
    ));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), boolean.clone()),
        Kind::Compare {
            predicate: ComparePredicate::Equal,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
    ));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut success = BasicBlock::new(BlockId(1));
    success.terminator = Some(Terminator::Return { values: vec![] });
    let mut trap = BasicBlock::new(BlockId(2));
    trap.operations.push(Operation::new(
        vec![],
        Kind::Call {
            callee: AmdGpuDiagnosticOperation::Trap.intrinsic_function_id(),
            arguments: vec![],
        },
    ));
    trap.terminator = Some(Terminator::Unreachable);
    let function = Function::kernel_entry(
        "test_assertion",
        Signature::new(vec![boolean], vec![]),
        vec![ValueId(0)],
        vec![entry, success, trap],
    );
    for (truth, expected) in [(false, 0), (true, 1)] {
        let values = flag_values(&function, ValueId(0), truth).unwrap();
        assert_eq!(
            selected_edge(
                function.body.as_ref().unwrap().blocks[0]
                    .terminator
                    .as_ref()
                    .unwrap(),
                &values
            ),
            Some(expected)
        );
    }
    let selected = failure_trap(&function, BlockId(2)).unwrap();
    let term = function.body.as_ref().unwrap().blocks[0]
        .terminator
        .as_ref()
        .unwrap();
    assert_eq!(edge_targets(term, 0, 1).unwrap(), (1, 2));
    assert!(edge_targets(term, 0, 0).is_err());
    assert!(edge_targets(term, 0, 2).is_err());
    let mut ambiguous = term.clone();
    if let Terminator::ConditionalBranch { else_target, .. } = &mut ambiguous {
        *else_target = BlockId(1);
    }
    assert!(edge_targets(&ambiguous, 0, 1).is_err());
    assert_eq!((selected.block, selected.operation), (BlockId(2), Some(0)));
    assert!(failure_trap(&function, BlockId(1)).is_err());
    for mutation in 0..3 {
        let mut changed = function.clone();
        let block = &mut changed.body.as_mut().unwrap().blocks[2];
        match mutation {
            0 => block.terminator = Some(Terminator::Return { values: vec![] }),
            1 => block.operations.push(block.operations[0].clone()),
            _ => {
                block.operations[0].kind = Kind::Call {
                    callee: AmdGpuDiagnosticOperation::DebugTrap.intrinsic_function_id(),
                    arguments: vec![],
                }
            }
        }
        assert!(failure_trap(&changed, BlockId(2)).is_err());
    }
}
fn failure_trap(
    function: &Function,
    mut block: fe2o3_kernel_ir::BlockId,
) -> ResultV1<sim::SimulationSiteV1> {
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| failure(Phase::Sim, "assert function body missing"))?;
    require(
        body.blocks.len() <= OBSERVATION_CAP,
        Phase::Sim,
        "bounded failure continuation",
    )?;
    let mut seen = BTreeSet::new();
    loop {
        require(
            seen.insert(block),
            Phase::Sim,
            "cyclic assertion failure continuation",
        )?;
        let current = body
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .ok_or_else(|| failure(Phase::Sim, "missing assertion failure target"))?;
        let mut trap = None;
        for (ordinal, op) in current.operations.iter().enumerate() {
            if is_trap(&op.kind) {
                require(
                    trap.is_none() && ordinal + 1 == current.operations.len(),
                    Phase::Sim,
                    "multiple or nonfinal trap",
                )?;
                trap = Some(ordinal as u32);
            } else {
                require(
                    matches!(op.kind, Kind::Constant(_)),
                    Phase::Sim,
                    "nontransparent source assert failure continuation",
                )?;
            }
        }
        match (&current.terminator, trap) {
            (Some(Terminator::Unreachable), Some(operation)) => {
                return Ok(sim::SimulationSiteV1 {
                    function: function.id.clone(),
                    block,
                    operation: Some(operation),
                });
            }
            (Some(Terminator::Branch { target, .. }), None) => block = *target,
            _ => {
                return Err(failure(
                    Phase::Sim,
                    "assertion failure does not end at one exact trap",
                ));
            }
        }
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssertionRoute {
    pub(super) subject: Subject,
    pub(super) function: String,
    pub(super) failure_edge: [u32; 3],
    pub(super) success_successor: u32,
    pub(super) block: u32,
    pub(super) success_target: u32,
    pub(super) failure_target: u32,
    pub(super) trap: component_sim::TrapSiteRow,
}
impl AssertionRoute {
    pub(super) fn valid(&self) -> bool {
        self.subject.digest != [0; 32]
            && self.subject.bytes > 0
            && !self.function.is_empty()
            && self.trap.function == self.function
            && self.success_successor != self.failure_edge[2]
            && self.success_target != self.failure_target
    }
}
fn edge_targets(term: &Terminator, success: usize, failed: usize) -> ResultV1<(u32, u32)> {
    let targets = term.successors();
    require(
        targets.len() <= OBSERVATION_CAP
            && success != failed
            && targets.iter().collect::<BTreeSet<_>>().len() == targets.len(),
        Phase::Sim,
        "ambiguous ordered assertion successors",
    )?;
    let a = targets
        .get(success)
        .ok_or_else(|| failure(Phase::Sim, "success successor absent"))?;
    let b = targets
        .get(failed)
        .ok_or_else(|| failure(Phase::Sim, "failure successor absent"))?;
    Ok((a.0, b.0))
}
pub(super) fn assertion_route(
    owner: &Owner,
    assertion: &AssertionRow,
    original: bool,
) -> ResultV1<AssertionRoute> {
    require(
        assertion.valid() && (!original || assertion.input == subject(owner)),
        Phase::Sim,
        "source assert/input identity",
    )?;
    let function_index = owner
        .module()
        .functions
        .iter()
        .position(|f| f.id.as_str() == assertion.entry)
        .ok_or_else(|| failure(Phase::Sim, "selected source assertion function absent"))?;
    let function = &owner.module().functions[function_index];
    let mut additions = operations(function).filter(|op| {
        matches!(&op.kind,
        Kind::Binary { op: BinaryOp::Checked(Checked::Add), lhs, rhs }
        if literal(function, *rhs) == Some(1) && literal(function, *lhs).is_none())
    });
    let add = additions
        .next()
        .ok_or_else(|| failure(Phase::Sim, "selected nonneutral checked Add disappeared"))?;
    require(
        additions.next().is_none(),
        Phase::Sim,
        "ambiguous checked Add source continuation",
    )?;
    let [value, overflow] = add.results.as_slice() else {
        return Err(failure(Phase::Sim, "actual checked pair missing"));
    };
    require(
        value.ty == Type::Scalar(ScalarType::U32) && overflow.ty == Type::Scalar(ScalarType::Bool),
        Phase::Sim,
        "selected checked Add exact types",
    )?;
    let (normal, failed) = (
        flag_values(function, overflow.id, false)?,
        flag_values(function, overflow.id, true)?,
    );
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| failure(Phase::Sim, "selected source body absent"))?;
    let mut selected = None;
    for (block_index, block) in body.blocks.iter().enumerate() {
        let Some(term) = &block.terminator else {
            continue;
        };
        let (Some(success), Some(failed_edge)) =
            (selected_edge(term, &normal), selected_edge(term, &failed))
        else {
            continue;
        };
        if success == failed_edge {
            continue;
        }
        require(
            selected.is_none(),
            Phase::Sim,
            "ambiguous overflow-dependent assertion branch",
        )?;
        if original {
            require(
                assertion.success_edge
                    == [function_index as u32, block_index as u32, success as u32]
                    && assertion.failure_edge
                        == [
                            function_index as u32,
                            block_index as u32,
                            failed_edge as u32,
                        ],
                Phase::Sim,
                "independent flag projection differs from sealed source assertion edges",
            )?;
        }
        let (success_target, failure_target) = edge_targets(term, success, failed_edge)?;
        selected = Some(AssertionRoute {
            subject: subject(owner),
            function: function.id.as_str().into(),
            failure_edge: [
                function_index as u32,
                block_index as u32,
                failed_edge as u32,
            ],
            success_successor: success as u32,
            block: block.id.0,
            success_target,
            failure_target,
            trap: component_sim::TrapSiteRow::from_site(&failure_trap(
                function,
                fe2o3_kernel_ir::BlockId(failure_target),
            )?)?,
        });
    }
    selected.ok_or_else(|| failure(Phase::Sim, "no exact overflow flag assertion continuation"))
}
fn assertion_trap(
    owner: &Owner,
    assertion: &AssertionRow,
    original: bool,
) -> ResultV1<sim::SimulationSiteV1> {
    let route = assertion_route(owner, assertion, original)?;
    Ok(sim::SimulationSiteV1 {
        function: route.function.into(),
        block: fe2o3_kernel_ir::BlockId(route.trap.block),
        operation: Some(route.trap.operation),
    })
}
#[test]
fn scalar_source_required_proof_refusal_matches_exact_typed_source_fields() {
    use crate::{
        production_pipeline::ProductionPipelineError as Pipeline,
        production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as Ranked,
    };
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirEdgeCoordinateV1 as Edge,
        CanonicalKirFunctionCoordinateV1 as Function,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticFunctionIdV1, SemanticSourceFileIdentityV1, SemanticSourceOriginV1,
    };
    let source = Provenance::new(
        Some(
            SemanticSourceOriginV1::new(
                SemanticSourceFileIdentityV1::from_sha256([7; 32]),
                10,
                19,
                2,
                1,
                2,
                10,
            )
            .unwrap(),
        ),
        None,
    );
    let block = Block {
        function: Function(0),
        block: 3,
    };
    let selected = SelectedAssertion {
        root: SemanticFunctionIdV1::from_index(0),
        source_function: [1; 32],
        source_body: [2; 32],
        block: 3,
        expected: false,
        condition_local: None,
        source,
        success: Edge {
            source: block,
            successor: 0,
        },
        failure: Edge {
            source: block,
            successor: 1,
        },
    };
    for mutation in 0..6 {
        let error = Ranked::UnprovenAssert {
            block: if mutation == 1 { 4 } else { 3 },
            kind: if mutation == 2 {
                "bounds-check"
            } else {
                "arithmetic-overflow"
            },
            expected: mutation == 3,
            condition_local: (mutation == 4).then_some(9),
            source: Box::new(if mutation == 5 {
                Provenance::unavailable()
            } else {
                source
            }),
        };
        assert_eq!(
            selected.matches_refusal(&Err(Pipeline::RankedProjection(error))),
            mutation == 0
        );
    }
    assert!(
        !selected.matches_refusal(&Err(Pipeline::RankedProjection(Ranked::Incomplete(
            "unrelated failure"
        ))))
    );
}

#[test]
fn scalar_trap_oracle_rejects_foreign_subject_source_edges_entry_and_trap_kind() {
    use fe2o3_kernel_ir::{BasicBlock, BlockId, Kernel, Signature, ValueDef};
    // Synthetic canonical graph for oracle hostility only; no source owner,
    // compiler callback, launch authority or source qualification is fabricated.
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
        Kind::Constant(Constant::U32(1)),
    ));
    entry.operations.push(Operation::checked_binary(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        ValueDef::new(ValueId(3), Type::Scalar(ScalarType::Bool)),
        Checked::Add,
        ValueId(0),
        ValueId(1),
    ));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(1),
        else_arguments: vec![],
    });
    let mut success = BasicBlock::new(BlockId(1));
    success.terminator = Some(Terminator::Return { values: vec![] });
    let mut trap = BasicBlock::new(BlockId(2));
    trap.operations.push(Operation::new(
        vec![],
        Kind::Call {
            callee: AmdGpuDiagnosticOperation::Trap.intrinsic_function_id(),
            arguments: vec![],
        },
    ));
    trap.terminator = Some(Terminator::Unreachable);
    let mut function = Function::kernel_entry(
        "test_assertion",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
        vec![entry, success, trap],
    );
    function.required_capabilities = AmdGpuDiagnosticOperation::Trap.required_capabilities();
    let mut module = Module::new("synthetic-source-trap-oracle");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions = vec![function, AmdGpuDiagnosticOperation::Trap.declaration()];
    let mut kernel = Kernel::new(
        "scalar_effect_then_overflow",
        "test_assertion",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = module.required_capabilities.clone();
    module.kernels.push(kernel);
    let admit = |module: &Module| {
        let mut work = Work::new(100_000_000);
        let mut budget = Budget::new(&mut work, 1024 * 1024 * 1024);
        let (owner, _) =
            Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
        owner
    };
    let owner = admit(&module);
    let origin = OriginRow {
        file: [7; 32],
        bytes: [1, 10],
        start: [1, 2],
        end: [1, 11],
    };
    let assertion = AssertionRow {
        input: subject(&owner),
        root: "scalar_effect_then_overflow".into(),
        entry: "test_assertion".into(),
        source_function: [1; 32],
        source_body: [2; 32],
        semantic_block: 3,
        expected: false,
        condition_local: None,
        expansion: Some(origin.clone()),
        call_site: Some(origin),
        fixture_file: OriginRow {
            file: [7; 32],
            bytes: [0, 100],
            start: [1, 1],
            end: [10, 1],
        },
        success_edge: [0, 0, 1],
        failure_edge: [0, 0, 0],
    };
    let before = assertion_trap(&owner, &assertion, true).unwrap();
    assert_eq!((before.block, before.operation), (BlockId(2), Some(0)));
    assert_eq!(before, assertion_trap(&owner, &assertion, false).unwrap());
    for mutation in 0..6 {
        let mut changed = assertion.clone();
        match mutation {
            0 => changed.input.digest[0] ^= 1,
            1 => changed.failure_edge[2] = 1,
            2 => changed.success_edge[1] = 1,
            3 => changed.entry = "foreign".into(),
            4 => changed.expected = true,
            _ => changed.root = "foreign".into(),
        }
        assert!(assertion_trap(&owner, &changed, true).is_err());
    }
    for mutation in 0..6 {
        let mut changed = assertion.clone();
        match mutation {
            0 => changed.expansion.as_mut().unwrap().file[0] ^= 1,
            1 => changed.call_site.as_mut().unwrap().file[0] ^= 1,
            2 => changed.fixture_file.file[0] ^= 1,
            3 => changed.expansion.as_mut().unwrap().bytes[1] = 101,
            4 => changed.call_site = None,
            _ => changed.fixture_file.bytes[0] = 1,
        }
        assert!(!changed.valid());
        assert!(assertion_route(&owner, &changed, true).is_err());
    }
    let mut changed = module;
    changed.functions[0].body.as_mut().unwrap().blocks[2].operations[0].kind = Kind::Call {
        callee: AmdGpuDiagnosticOperation::DebugTrap.intrinsic_function_id(),
        arguments: vec![],
    };
    changed
        .functions
        .push(AmdGpuDiagnosticOperation::DebugTrap.declaration());
    let changed = admit(&changed);
    assert!(assertion_trap(&changed, &assertion, false).is_err());
}

#[test]
fn scalar_fixture_file_selection_requires_unique_path_and_exact_compiled_bytes() {
    let scratch = crate::test_temp_dir::TestTempDir::create("scalar-source-file-identity");
    let path = scratch.path().join("fixture.rs");
    let original = "value + 1\n";
    std::fs::write(&path, original).unwrap();
    let stamp = FileStamp {
        path: path.canonicalize().unwrap(),
        sha256: digest(original.as_bytes()),
    };
    let file = |text: &str| {
        SourceFile::new(
            FileName::Custom("diagnostic-label.rs".into()),
            text.into(),
            rustc_span::SourceFileHashAlgorithm::Sha256,
            None,
        )
        .unwrap()
    };
    let compiled = file(original);
    check_original(&compiled, &stamp).unwrap();
    assert_eq!(
        unique_file(&stamp.path, [(None, 0), (Some(stamp.path.as_path()), 1)]).unwrap(),
        1
    );
    assert!(unique_file(&stamp.path, [(None, 0)]).is_err());
    assert!(
        unique_file(
            &stamp.path,
            [
                (Some(stamp.path.as_path()), 0),
                (Some(stamp.path.as_path()), 1)
            ]
        )
        .is_err()
    );
    assert!(unique_file(&stamp.path, [(Some(scratch.path()), 0)]).is_err());
    assert!(
        check_original(
            &compiled,
            &FileStamp {
                sha256: [0; 32],
                ..stamp.clone()
            }
        )
        .is_err()
    );
    assert!(
        check_original(
            &compiled,
            &FileStamp {
                path: scratch.path().to_owned(),
                ..stamp.clone()
            }
        )
        .is_err()
    );
    assert!(check_original(&file(&"x".repeat(INPUT_CAP + 1)), &stamp).is_err());
    std::fs::write(&stamp.path, "value + 2\n").unwrap();
    assert!(
        check_original(
            &compiled,
            &FileStamp {
                sha256: digest(b"value + 2\n"),
                ..stamp.clone()
            }
        )
        .is_err()
    );
    let normalized = "\u{feff}value + 1\r\n";
    std::fs::write(&stamp.path, normalized).unwrap();
    assert!(
        check_original(
            &file(normalized),
            &FileStamp {
                sha256: digest(normalized.as_bytes()),
                ..stamp
            }
        )
        .is_err()
    );
}
