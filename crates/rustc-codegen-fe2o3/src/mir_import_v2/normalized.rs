use std::error::Error;
use std::fmt;

pub(crate) const NORMALIZED_MIR_SCHEMA_V2: u16 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CaptureLimitsV2 {
    pub max_locals: usize,
    pub max_blocks: usize,
    pub max_statements_per_block: usize,
    pub max_total_statements: usize,
    pub max_operands: usize,
    pub max_projection_depth: usize,
    pub max_successors: usize,
    pub max_text_bytes: usize,
}

impl Default for CaptureLimitsV2 {
    fn default() -> Self {
        Self {
            max_locals: 65_536,
            max_blocks: 65_536,
            max_statements_per_block: 65_536,
            max_total_statements: 1_048_576,
            max_operands: 4_096,
            max_projection_depth: 64,
            max_successors: 65_536,
            max_text_bytes: 16_384,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaptureAuthorityV2 {
    CompilerObservationOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapturedBodyV2 {
    pub schema_version: u16,
    pub authority: CaptureAuthorityV2,
    pub function: FunctionIdentityV2,
    pub source: SourceSpanV2,
    pub arg_count: usize,
    pub locals: Vec<LocalDeclV2>,
    pub blocks: Vec<BasicBlockV2>,
}

impl CapturedBodyV2 {
    pub(crate) fn is_authorized_for_lowering(&self) -> bool {
        false
    }

    pub(crate) fn validate(&self, limits: CaptureLimitsV2) -> Result<(), ValidationErrorV2> {
        if self.schema_version != NORMALIZED_MIR_SCHEMA_V2 {
            return Err(ValidationErrorV2::new(
                "schema_version",
                format!(
                    "unknown normalized MIR schema {}; expected {}",
                    self.schema_version, NORMALIZED_MIR_SCHEMA_V2
                ),
            ));
        }
        if self.authority != CaptureAuthorityV2::CompilerObservationOnly {
            return Err(ValidationErrorV2::new(
                "authority",
                "normalized MIR V2 is an observation and cannot grant lowering authority",
            ));
        }
        validate_function_identity(&self.function, limits)?;
        validate_span("source", &self.source, limits)?;
        bounded("locals", self.locals.len(), limits.max_locals)?;
        bounded("blocks", self.blocks.len(), limits.max_blocks)?;
        if self.blocks.is_empty() {
            return Err(ValidationErrorV2::new(
                "blocks",
                "a captured MIR body must contain an entry block",
            ));
        }
        if self.arg_count >= self.locals.len() {
            return Err(ValidationErrorV2::new(
                "arg_count",
                "arguments and the return local do not fit in the local table",
            ));
        }

        for (expected, local) in self.locals.iter().enumerate() {
            if local.index != expected {
                return Err(ValidationErrorV2::new(
                    format!("locals[{expected}].index"),
                    format!("local index {} is not canonical", local.index),
                ));
            }
            let expected_role = if expected == 0 {
                LocalRoleV2::Return
            } else if expected <= self.arg_count {
                LocalRoleV2::Argument
            } else {
                LocalRoleV2::Temporary
            };
            if local.role != expected_role {
                return Err(ValidationErrorV2::new(
                    format!("locals[{expected}].role"),
                    format!("expected {expected_role:?}, found {:?}", local.role),
                ));
            }
            validate_type(&format!("locals[{expected}].type"), &local.ty, limits)?;
            validate_span(&format!("locals[{expected}].source"), &local.source, limits)?;
            validate_text(
                &format!("locals[{expected}].rustc_debug"),
                &local.rustc_debug,
                limits,
            )?;
        }

        let mut total_statements = 0usize;
        for (expected, block) in self.blocks.iter().enumerate() {
            if block.index != expected {
                return Err(ValidationErrorV2::new(
                    format!("blocks[{expected}].index"),
                    format!("block index {} is not canonical", block.index),
                ));
            }
            bounded(
                format!("blocks[{expected}].statements"),
                block.statements.len(),
                limits.max_statements_per_block,
            )?;
            total_statements = total_statements
                .checked_add(block.statements.len())
                .ok_or_else(|| {
                    ValidationErrorV2::new("statements", "statement count overflowed")
                })?;
            bounded(
                format!("blocks[{expected}].successors"),
                block.terminator.successors.len(),
                limits.max_successors,
            )?;
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if statement.index != statement_index {
                    return Err(ValidationErrorV2::new(
                        format!("blocks[{expected}].statements[{statement_index}].index"),
                        format!("statement index {} is not canonical", statement.index),
                    ));
                }
                validate_statement(
                    &format!("blocks[{expected}].statements[{statement_index}]"),
                    statement,
                    self.locals.len(),
                    limits,
                )?;
            }
            validate_terminator(
                &format!("blocks[{expected}].terminator"),
                &block.terminator,
                self.locals.len(),
                self.blocks.len(),
                limits,
            )?;
        }
        bounded("statements", total_statements, limits.max_total_statements)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionIdentityV2 {
    pub definition: DefinitionIdentityV2,
    pub instance: InstanceIdentityV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DefinitionIdentityV2 {
    pub crate_name: String,
    pub def_path: String,
    pub def_path_hash: [u8; 16],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IntrinsicIdentityV2 {
    pub definition: DefinitionIdentityV2,
    pub name: String,
    pub must_be_overridden: bool,
    pub const_stable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstanceIdentityV2 {
    pub kind: InstanceKindV2,
    pub generic_args: String,
    pub rustc_debug: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InstanceKindV2 {
    Item,
    GeneratedCallable { rustc_kind: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceSpanV2 {
    pub file: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub source_scope: usize,
    pub rustc_debug: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeIdentityV2 {
    pub rust: String,
    pub rustc_kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocalDeclV2 {
    pub index: usize,
    pub role: LocalRoleV2,
    pub ty: TypeIdentityV2,
    pub mutable: bool,
    pub source: SourceSpanV2,
    pub rustc_debug: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocalRoleV2 {
    Return,
    Argument,
    Temporary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BasicBlockV2 {
    pub index: usize,
    pub cleanup: bool,
    pub statements: Vec<StatementV2>,
    pub terminator: TerminatorV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatementV2 {
    pub index: usize,
    pub source: SourceSpanV2,
    pub rustc_debug: String,
    pub kind: StatementKindV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StatementKindV2 {
    Assign {
        destination: PlaceV2,
        value: RvalueV2,
    },
    StorageLive {
        local: usize,
    },
    StorageDead {
        local: usize,
    },
    SetDiscriminant {
        place: PlaceV2,
        variant: usize,
    },
    Intrinsic(IntrinsicStatementV2),
    Deinit {
        place: PlaceV2,
    },
    Retag {
        place: PlaceV2,
        rustc_kind: String,
    },
    PlaceMention {
        place: PlaceV2,
    },
    Coverage {
        rustc_kind: String,
    },
    Nop,
    CompilerOpaque {
        rustc_kind: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum IntrinsicStatementV2 {
    CopyNonOverlapping {
        source: OperandV2,
        destination: OperandV2,
        count: OperandV2,
    },
    Assume {
        condition: OperandV2,
    },
    CompilerOpaque {
        rustc_kind: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlaceV2 {
    pub local: usize,
    pub projection: Vec<ProjectionV2>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProjectionV2 {
    Deref,
    Field {
        index: usize,
        ty: TypeIdentityV2,
    },
    Index {
        local: usize,
    },
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },
    Downcast {
        variant: usize,
        name: Option<String>,
    },
    OpaqueCast {
        ty: TypeIdentityV2,
    },
    Subtype {
        ty: TypeIdentityV2,
    },
    UnwrapUnsafeBinder {
        ty: TypeIdentityV2,
    },
    CompilerOpaque {
        rustc_kind: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OperandV2 {
    Copy(PlaceV2),
    Move(PlaceV2),
    Constant {
        ty: TypeIdentityV2,
        literal: String,
        source: SourceSpanV2,
    },
    RuntimeChecks {
        kind: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RvalueV2 {
    Use(OperandV2),
    Repeat {
        operand: OperandV2,
        count: String,
    },
    Reference {
        borrow_kind: String,
        place: PlaceV2,
    },
    RawPointer {
        mutability: String,
        place: PlaceV2,
    },
    Len(PlaceV2),
    Cast {
        kind: String,
        operand: OperandV2,
        target: TypeIdentityV2,
    },
    Binary {
        operation: String,
        lhs: OperandV2,
        rhs: OperandV2,
    },
    Unary {
        operation: String,
        operand: OperandV2,
    },
    Discriminant {
        place: PlaceV2,
    },
    Aggregate {
        kind: AggregateKindV2,
        operands: Vec<OperandV2>,
    },
    CopyForDeref(PlaceV2),
    Nullary {
        operation: String,
        ty: TypeIdentityV2,
    },
    ThreadLocalRef {
        definition: DefinitionIdentityV2,
    },
    WrapUnsafeBinder {
        operand: OperandV2,
        target: TypeIdentityV2,
    },
    CompilerOpaque {
        rustc_kind: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AggregateKindV2 {
    pub class: AggregateClassV2,
    pub definition: Option<DefinitionIdentityV2>,
    pub variant: Option<usize>,
    pub rustc_kind: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AggregateClassV2 {
    Array,
    Tuple,
    Adt,
    Closure,
    CoroutineClosure,
    Coroutine,
    RawPointer,
    CompilerOpaque,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminatorV2 {
    pub source: SourceSpanV2,
    pub rustc_debug: String,
    pub kind: TerminatorKindV2,
    pub successors: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminatorKindV2 {
    Return,
    Unreachable,
    Goto {
        target: usize,
    },
    SwitchInt {
        discriminant: OperandV2,
        targets: Vec<SwitchTargetV2>,
        otherwise: usize,
    },
    Call {
        function: OperandV2,
        declared: Option<DefinitionIdentityV2>,
        resolved: Option<FunctionIdentityV2>,
        intrinsic: Option<IntrinsicIdentityV2>,
        arguments: Vec<CallArgumentV2>,
        destination: PlaceV2,
        target: Option<usize>,
        unwind: UnwindActionV2,
        call_source: String,
        function_span: SourceSpanV2,
    },
    TailCall {
        function: OperandV2,
        declared: Option<DefinitionIdentityV2>,
        resolved: Option<FunctionIdentityV2>,
        intrinsic: Option<IntrinsicIdentityV2>,
        arguments: Vec<CallArgumentV2>,
        function_span: SourceSpanV2,
    },
    Drop {
        place: PlaceV2,
        target: usize,
        unwind: UnwindActionV2,
        replace: bool,
        async_drop: Option<usize>,
        async_future_local: Option<usize>,
    },
    Assert {
        condition: OperandV2,
        expected: bool,
        target: usize,
        message: String,
        unwind: UnwindActionV2,
    },
    InlineAsm {
        rustc_kind: String,
    },
    CompilerOpaque {
        rustc_kind: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallArgumentV2 {
    pub operand: OperandV2,
    pub source: SourceSpanV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SwitchTargetV2 {
    pub value: u128,
    pub target: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UnwindActionV2 {
    Continue,
    Unreachable,
    Terminate { reason: String },
    Cleanup { target: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ValidationErrorV2 {
    pub path: String,
    pub reason: String,
}

impl ValidationErrorV2 {
    pub(crate) fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ValidationErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "normalized MIR V2 rejected at {}: {}",
            self.path, self.reason
        )
    }
}

impl Error for ValidationErrorV2 {}

fn validate_function_identity(
    identity: &FunctionIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_definition("function.definition", &identity.definition, limits)?;
    validate_text(
        "function.instance.generic_args",
        &identity.instance.generic_args,
        limits,
    )?;
    validate_text(
        "function.instance.rustc_debug",
        &identity.instance.rustc_debug,
        limits,
    )?;
    if let InstanceKindV2::GeneratedCallable { rustc_kind } = &identity.instance.kind {
        validate_text("function.instance.kind", rustc_kind, limits)?;
    }
    Ok(())
}

fn validate_definition(
    path: &str,
    definition: &DefinitionIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_text(
        &format!("{path}.crate_name"),
        &definition.crate_name,
        limits,
    )?;
    validate_text(&format!("{path}.def_path"), &definition.def_path, limits)
}

fn validate_span(
    path: &str,
    span: &SourceSpanV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_text(&format!("{path}.file"), &span.file, limits)?;
    validate_text(&format!("{path}.rustc_debug"), &span.rustc_debug, limits)?;
    if span.start_line == 0
        || span.start_column == 0
        || span.end_line == 0
        || span.end_column == 0
        || (span.start_line, span.start_column) > (span.end_line, span.end_column)
    {
        return Err(ValidationErrorV2::new(
            path,
            "source span is empty or reversed",
        ));
    }
    Ok(())
}

fn validate_type(
    path: &str,
    ty: &TypeIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_text(&format!("{path}.rust"), &ty.rust, limits)?;
    validate_text(&format!("{path}.rustc_kind"), &ty.rustc_kind, limits)
}

fn validate_statement(
    path: &str,
    statement: &StatementV2,
    local_count: usize,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_span(&format!("{path}.source"), &statement.source, limits)?;
    validate_text(
        &format!("{path}.rustc_debug"),
        &statement.rustc_debug,
        limits,
    )?;
    match &statement.kind {
        StatementKindV2::Assign { destination, value } => {
            validate_place(
                &format!("{path}.destination"),
                destination,
                local_count,
                limits,
            )?;
            validate_rvalue(&format!("{path}.value"), value, local_count, limits)
        }
        StatementKindV2::StorageLive { local } | StatementKindV2::StorageDead { local } => {
            validate_local(&format!("{path}.local"), *local, local_count)
        }
        StatementKindV2::SetDiscriminant { place, .. }
        | StatementKindV2::Deinit { place }
        | StatementKindV2::PlaceMention { place } => {
            validate_place(&format!("{path}.place"), place, local_count, limits)
        }
        StatementKindV2::Intrinsic(intrinsic) => match intrinsic {
            IntrinsicStatementV2::CopyNonOverlapping {
                source,
                destination,
                count,
            } => {
                validate_operand(
                    &format!("{path}.source_operand"),
                    source,
                    local_count,
                    limits,
                )?;
                validate_operand(
                    &format!("{path}.destination_operand"),
                    destination,
                    local_count,
                    limits,
                )?;
                validate_operand(&format!("{path}.count"), count, local_count, limits)
            }
            IntrinsicStatementV2::Assume { condition } => {
                validate_operand(&format!("{path}.condition"), condition, local_count, limits)
            }
            IntrinsicStatementV2::CompilerOpaque { rustc_kind } => {
                validate_text(&format!("{path}.intrinsic"), rustc_kind, limits)
            }
        },
        StatementKindV2::Retag { place, rustc_kind } => {
            validate_place(&format!("{path}.place"), place, local_count, limits)?;
            validate_text(&format!("{path}.retag"), rustc_kind, limits)
        }
        StatementKindV2::Coverage { rustc_kind }
        | StatementKindV2::CompilerOpaque { rustc_kind } => {
            validate_text(&format!("{path}.rustc_kind"), rustc_kind, limits)
        }
        StatementKindV2::Nop => Ok(()),
    }
}

fn validate_rvalue(
    path: &str,
    value: &RvalueV2,
    local_count: usize,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    match value {
        RvalueV2::Use(operand) | RvalueV2::Unary { operand, .. } => {
            validate_operand(&format!("{path}.operand"), operand, local_count, limits)
        }
        RvalueV2::Repeat { operand, count } => {
            validate_operand(&format!("{path}.operand"), operand, local_count, limits)?;
            validate_text(&format!("{path}.count"), count, limits)
        }
        RvalueV2::Reference { borrow_kind, place } => {
            validate_text(&format!("{path}.borrow_kind"), borrow_kind, limits)?;
            validate_place(&format!("{path}.place"), place, local_count, limits)
        }
        RvalueV2::RawPointer { mutability, place } => {
            validate_text(&format!("{path}.mutability"), mutability, limits)?;
            validate_place(&format!("{path}.place"), place, local_count, limits)
        }
        RvalueV2::Len(place) | RvalueV2::Discriminant { place } | RvalueV2::CopyForDeref(place) => {
            validate_place(&format!("{path}.place"), place, local_count, limits)
        }
        RvalueV2::Cast {
            kind,
            operand,
            target,
        } => {
            validate_text(&format!("{path}.kind"), kind, limits)?;
            validate_operand(&format!("{path}.operand"), operand, local_count, limits)?;
            validate_type(&format!("{path}.target"), target, limits)
        }
        RvalueV2::Binary {
            operation,
            lhs,
            rhs,
        } => {
            validate_text(&format!("{path}.operation"), operation, limits)?;
            validate_operand(&format!("{path}.lhs"), lhs, local_count, limits)?;
            validate_operand(&format!("{path}.rhs"), rhs, local_count, limits)
        }
        RvalueV2::Aggregate { kind, operands } => {
            bounded(
                &format!("{path}.operands"),
                operands.len(),
                limits.max_operands,
            )?;
            validate_text(&format!("{path}.aggregate_kind"), &kind.rustc_kind, limits)?;
            if let Some(definition) = &kind.definition {
                validate_definition(&format!("{path}.definition"), definition, limits)?;
            }
            for (index, operand) in operands.iter().enumerate() {
                validate_operand(
                    &format!("{path}.operands[{index}]"),
                    operand,
                    local_count,
                    limits,
                )?;
            }
            Ok(())
        }
        RvalueV2::Nullary { operation, ty } => {
            validate_text(&format!("{path}.operation"), operation, limits)?;
            validate_type(&format!("{path}.type"), ty, limits)
        }
        RvalueV2::ThreadLocalRef { definition } => {
            validate_definition(&format!("{path}.definition"), definition, limits)
        }
        RvalueV2::WrapUnsafeBinder { operand, target } => {
            validate_operand(&format!("{path}.operand"), operand, local_count, limits)?;
            validate_type(&format!("{path}.target"), target, limits)
        }
        RvalueV2::CompilerOpaque { rustc_kind } => {
            validate_text(&format!("{path}.rustc_kind"), rustc_kind, limits)
        }
    }
}

fn validate_terminator(
    path: &str,
    terminator: &TerminatorV2,
    local_count: usize,
    block_count: usize,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_span(&format!("{path}.source"), &terminator.source, limits)?;
    validate_text(
        &format!("{path}.rustc_debug"),
        &terminator.rustc_debug,
        limits,
    )?;
    for (index, successor) in terminator.successors.iter().copied().enumerate() {
        if successor >= block_count {
            return Err(ValidationErrorV2::new(
                format!("{path}.successors[{index}]"),
                format!("block {successor} is outside 0..{block_count}"),
            ));
        }
    }
    match &terminator.kind {
        TerminatorKindV2::Return | TerminatorKindV2::Unreachable => {
            validate_exact_successors(path, &terminator.successors, &[])
        }
        TerminatorKindV2::Goto { target } => {
            validate_block(path, *target, block_count)?;
            validate_exact_successors(path, &terminator.successors, &[*target])
        }
        TerminatorKindV2::SwitchInt {
            discriminant,
            targets,
            otherwise,
        } => {
            bounded(
                &format!("{path}.targets"),
                targets.len(),
                limits.max_successors,
            )?;
            validate_operand(
                &format!("{path}.discriminant"),
                discriminant,
                local_count,
                limits,
            )?;
            let mut expected = Vec::with_capacity(targets.len() + 1);
            for (index, target) in targets.iter().enumerate() {
                validate_block(
                    &format!("{path}.targets[{index}]"),
                    target.target,
                    block_count,
                )?;
                expected.push(target.target);
            }
            validate_block(&format!("{path}.otherwise"), *otherwise, block_count)?;
            expected.push(*otherwise);
            validate_exact_successors(path, &terminator.successors, &expected)
        }
        TerminatorKindV2::Call {
            function,
            declared,
            resolved,
            intrinsic,
            arguments,
            destination,
            target,
            unwind,
            call_source,
            function_span,
        } => {
            validate_operand(&format!("{path}.function"), function, local_count, limits)?;
            if let Some(declared) = declared {
                validate_definition(&format!("{path}.declared"), declared, limits)?;
            }
            if let Some(resolved) = resolved {
                validate_function_identity(resolved, limits)?;
            }
            if let Some(intrinsic) = intrinsic {
                validate_definition(
                    &format!("{path}.intrinsic.definition"),
                    &intrinsic.definition,
                    limits,
                )?;
                validate_text(&format!("{path}.intrinsic.name"), &intrinsic.name, limits)?;
            }
            bounded(
                &format!("{path}.arguments"),
                arguments.len(),
                limits.max_operands,
            )?;
            for (index, argument) in arguments.iter().enumerate() {
                validate_operand(
                    &format!("{path}.arguments[{index}]"),
                    &argument.operand,
                    local_count,
                    limits,
                )?;
                validate_span(
                    &format!("{path}.arguments[{index}].source"),
                    &argument.source,
                    limits,
                )?;
            }
            validate_place(
                &format!("{path}.destination"),
                destination,
                local_count,
                limits,
            )?;
            if let Some(target) = target {
                validate_block(&format!("{path}.target"), *target, block_count)?;
            }
            validate_unwind(&format!("{path}.unwind"), unwind, block_count, limits)?;
            validate_text(&format!("{path}.call_source"), call_source, limits)?;
            validate_span(&format!("{path}.function_span"), function_span, limits)?;
            let expected = normal_and_unwind_successors(*target, unwind);
            validate_exact_successors(path, &terminator.successors, &expected)
        }
        TerminatorKindV2::TailCall {
            function,
            declared,
            resolved,
            intrinsic,
            arguments,
            function_span,
        } => {
            validate_operand(&format!("{path}.function"), function, local_count, limits)?;
            if let Some(declared) = declared {
                validate_definition(&format!("{path}.declared"), declared, limits)?;
            }
            if let Some(resolved) = resolved {
                validate_function_identity(resolved, limits)?;
            }
            if let Some(intrinsic) = intrinsic {
                validate_definition(
                    &format!("{path}.intrinsic.definition"),
                    &intrinsic.definition,
                    limits,
                )?;
                validate_text(&format!("{path}.intrinsic.name"), &intrinsic.name, limits)?;
            }
            bounded(
                &format!("{path}.arguments"),
                arguments.len(),
                limits.max_operands,
            )?;
            for (index, argument) in arguments.iter().enumerate() {
                validate_operand(
                    &format!("{path}.arguments[{index}]"),
                    &argument.operand,
                    local_count,
                    limits,
                )?;
                validate_span(
                    &format!("{path}.arguments[{index}].source"),
                    &argument.source,
                    limits,
                )?;
            }
            validate_span(&format!("{path}.function_span"), function_span, limits)?;
            validate_exact_successors(path, &terminator.successors, &[])
        }
        TerminatorKindV2::Drop {
            place,
            target,
            unwind,
            async_drop,
            async_future_local,
            ..
        } => {
            validate_place(&format!("{path}.place"), place, local_count, limits)?;
            validate_block(&format!("{path}.target"), *target, block_count)?;
            validate_unwind(&format!("{path}.unwind"), unwind, block_count, limits)?;
            if let Some(async_drop) = async_drop {
                validate_block(&format!("{path}.async_drop"), *async_drop, block_count)?;
            }
            if let Some(local) = async_future_local {
                validate_local(&format!("{path}.async_future_local"), *local, local_count)?;
            }
            let mut expected = normal_and_unwind_successors(Some(*target), unwind);
            expected.extend(async_drop);
            validate_exact_successors(path, &terminator.successors, &expected)
        }
        TerminatorKindV2::Assert {
            condition,
            target,
            message,
            unwind,
            ..
        } => {
            validate_operand(&format!("{path}.condition"), condition, local_count, limits)?;
            validate_block(&format!("{path}.target"), *target, block_count)?;
            validate_text(&format!("{path}.message"), message, limits)?;
            validate_unwind(&format!("{path}.unwind"), unwind, block_count, limits)?;
            let expected = normal_and_unwind_successors(Some(*target), unwind);
            validate_exact_successors(path, &terminator.successors, &expected)
        }
        TerminatorKindV2::InlineAsm { rustc_kind }
        | TerminatorKindV2::CompilerOpaque { rustc_kind } => {
            validate_text(&format!("{path}.rustc_kind"), rustc_kind, limits)
        }
    }
}

fn validate_unwind(
    path: &str,
    unwind: &UnwindActionV2,
    block_count: usize,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    match unwind {
        UnwindActionV2::Continue | UnwindActionV2::Unreachable => Ok(()),
        UnwindActionV2::Terminate { reason } => validate_text(path, reason, limits),
        UnwindActionV2::Cleanup { target } => validate_block(path, *target, block_count),
    }
}

fn normal_and_unwind_successors(normal: Option<usize>, unwind: &UnwindActionV2) -> Vec<usize> {
    normal
        .into_iter()
        .chain(match unwind {
            UnwindActionV2::Cleanup { target } => Some(*target),
            UnwindActionV2::Continue
            | UnwindActionV2::Unreachable
            | UnwindActionV2::Terminate { .. } => None,
        })
        .collect()
}

fn validate_exact_successors(
    path: &str,
    actual: &[usize],
    expected: &[usize],
) -> Result<(), ValidationErrorV2> {
    if actual != expected {
        return Err(ValidationErrorV2::new(
            format!("{path}.successors"),
            format!("successor list {actual:?} disagrees with terminator targets {expected:?}"),
        ));
    }
    Ok(())
}

fn validate_operand(
    path: &str,
    operand: &OperandV2,
    local_count: usize,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    match operand {
        OperandV2::Copy(place) | OperandV2::Move(place) => {
            validate_place(path, place, local_count, limits)
        }
        OperandV2::Constant {
            ty,
            literal,
            source,
        } => {
            validate_type(&format!("{path}.type"), ty, limits)?;
            validate_text(&format!("{path}.literal"), literal, limits)?;
            validate_span(&format!("{path}.source"), source, limits)
        }
        OperandV2::RuntimeChecks { kind } => {
            validate_text(&format!("{path}.runtime_checks"), kind, limits)
        }
    }
}

fn validate_place(
    path: &str,
    place: &PlaceV2,
    local_count: usize,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_local(&format!("{path}.local"), place.local, local_count)?;
    bounded(
        &format!("{path}.projection"),
        place.projection.len(),
        limits.max_projection_depth,
    )?;
    for (index, projection) in place.projection.iter().enumerate() {
        let projection_path = format!("{path}.projection[{index}]");
        match projection {
            ProjectionV2::Index { local } => {
                validate_local(&format!("{projection_path}.local"), *local, local_count)?;
            }
            ProjectionV2::Field { ty, .. }
            | ProjectionV2::OpaqueCast { ty }
            | ProjectionV2::Subtype { ty }
            | ProjectionV2::UnwrapUnsafeBinder { ty } => {
                validate_type(&format!("{projection_path}.type"), ty, limits)?;
            }
            ProjectionV2::Downcast {
                name: Some(name), ..
            } => {
                validate_text(&format!("{projection_path}.name"), name, limits)?;
            }
            ProjectionV2::CompilerOpaque { rustc_kind } => {
                validate_text(&format!("{projection_path}.rustc_kind"), rustc_kind, limits)?;
            }
            ProjectionV2::Deref
            | ProjectionV2::ConstantIndex { .. }
            | ProjectionV2::Subslice { .. }
            | ProjectionV2::Downcast { name: None, .. } => {}
        }
    }
    Ok(())
}

fn validate_local(path: &str, local: usize, local_count: usize) -> Result<(), ValidationErrorV2> {
    if local >= local_count {
        return Err(ValidationErrorV2::new(
            path,
            format!("local {local} is outside 0..{local_count}"),
        ));
    }
    Ok(())
}

fn validate_block(path: &str, block: usize, block_count: usize) -> Result<(), ValidationErrorV2> {
    if block >= block_count {
        return Err(ValidationErrorV2::new(
            path,
            format!("block {block} is outside 0..{block_count}"),
        ));
    }
    Ok(())
}

fn validate_text(path: &str, text: &str, limits: CaptureLimitsV2) -> Result<(), ValidationErrorV2> {
    if text.is_empty() {
        return Err(ValidationErrorV2::new(path, "text must not be empty"));
    }
    bounded(path, text.len(), limits.max_text_bytes)?;
    if text.contains('\0') {
        return Err(ValidationErrorV2::new(path, "text contains a NUL byte"));
    }
    Ok(())
}

fn bounded(path: impl Into<String>, actual: usize, limit: usize) -> Result<(), ValidationErrorV2> {
    if actual > limit {
        return Err(ValidationErrorV2::new(
            path,
            format!("bound exceeded: {actual} > {limit}"),
        ));
    }
    Ok(())
}
