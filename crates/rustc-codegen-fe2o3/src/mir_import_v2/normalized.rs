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
    pub max_switch_targets: usize,
    pub max_text_bytes: usize,
    pub max_total_text_bytes: usize,
    pub max_total_work_items: usize,
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
            max_switch_targets: 65_535,
            max_text_bytes: 16_384,
            max_total_text_bytes: 16 * 1_048_576,
            max_total_work_items: 8 * 1_048_576,
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
    pub capture_work_items: usize,
    pub capture_text_bytes: usize,
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
        bounded(
            "capture_work_items",
            self.capture_work_items,
            limits.max_total_work_items,
        )?;
        bounded(
            "capture_text_bytes",
            self.capture_text_bytes,
            limits.max_total_text_bytes,
        )?;
        if self.capture_work_items == 0 || self.capture_text_bytes == 0 {
            return Err(ValidationErrorV2::new(
                "capture_budget",
                "captured work and text accounting must be nonzero",
            ));
        }
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
                &format!("locals[{expected}].diagnostic_debug"),
                &local.diagnostic_debug,
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
    pub diagnostic_crate_name: String,
    pub diagnostic_def_path: String,
    pub def_path_hash: [u8; 16],
    pub stable_crate_id: [u8; 16],
    pub local_def_path_hash: [u8; 8],
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
    pub generic_args_hash: [u8; 16],
    pub generic_arg_count: usize,
    pub instance_hash: [u8; 16],
    pub diagnostic_generic_args: String,
    pub diagnostic_debug: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InstanceKindV2 {
    Item,
    Intrinsic,
    VTableShim,
    ReifyShim {
        reason: Option<ReifyReasonV2>,
    },
    FnPtrShim {
        fn_pointer: Box<TypeIdentityV2>,
    },
    Virtual {
        vtable_index: usize,
    },
    ClosureOnceShim {
        track_caller: bool,
    },
    ConstructCoroutineInClosureShim {
        coroutine_closure: DefinitionIdentityV2,
        receiver_by_ref: bool,
    },
    ThreadLocalShim,
    FutureDropPollShim {
        proxy_coroutine: Box<TypeIdentityV2>,
        implementation_coroutine: Box<TypeIdentityV2>,
    },
    DropGlue {
        ty: Option<Box<TypeIdentityV2>>,
    },
    CloneShim {
        ty: Box<TypeIdentityV2>,
    },
    FnPtrAddrShim {
        ty: Box<TypeIdentityV2>,
    },
    AsyncDropGlueCtorShim {
        ty: Box<TypeIdentityV2>,
    },
    AsyncDropGlue {
        ty: Box<TypeIdentityV2>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReifyReasonV2 {
    FunctionPointer,
    Vtable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceSpanV2 {
    pub authority: SourceAuthorityV2,
    pub remapped_file: String,
    pub source_file_hash: [u8; 16],
    pub span_hash: [u8; 16],
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub source_scope: usize,
    pub source_scope_hash: [u8; 16],
    pub source_scope_parent: Option<usize>,
    pub inlined_instance_hash: Option<[u8; 16]>,
    pub diagnostic_debug: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceAuthorityV2 {
    CanonicalRemapped,
    Unauthoritative(SourceRejectionV2),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceRejectionV2 {
    DummySpan,
    CrossFileSpan,
    InvalidPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeIdentityV2 {
    pub stable_hash: [u8; 16],
    pub class: TypeClassV2,
    pub diagnostic_display: String,
    pub diagnostic_debug: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TypeClassV2 {
    Bool,
    Char,
    SignedInteger(IntegerWidthV2),
    UnsignedInteger(IntegerWidthV2),
    Float(FloatWidthV2),
    Adt {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
    },
    Foreign {
        definition: DefinitionIdentityV2,
    },
    StringSlice,
    Array,
    Pattern,
    Slice,
    RawPointer {
        mutable: bool,
    },
    Reference {
        mutable: bool,
    },
    FunctionDefinition {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
    },
    FunctionPointer,
    UnsafeBinder,
    Dynamic,
    Closure {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
    },
    CoroutineClosure {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
    },
    Coroutine {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
    },
    CoroutineWitness {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
    },
    Never,
    Tuple {
        arity: usize,
    },
    Unsupported(UnresolvedTypeClassV2),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegerWidthV2 {
    Pointer,
    Bits8,
    Bits16,
    Bits32,
    Bits64,
    Bits128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FloatWidthV2 {
    Bits16,
    Bits32,
    Bits64,
    Bits128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnresolvedTypeClassV2 {
    Alias,
    Parameter,
    Bound,
    Placeholder,
    Inference,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StableCompilerValueV2 {
    pub stable_hash: [u8; 16],
    pub diagnostic: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocalDeclV2 {
    pub index: usize,
    pub role: LocalRoleV2,
    pub ty: TypeIdentityV2,
    pub mutable: bool,
    pub source: SourceSpanV2,
    pub diagnostic_debug: String,
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
    pub diagnostic_debug: String,
    pub kind: StatementKindV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StatementKindV2 {
    Assign {
        destination: PlaceV2,
        value: Box<RvalueV2>,
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
    Intrinsic(Box<IntrinsicStatementV2>),
    Retag {
        place: PlaceV2,
        kind: StableCompilerValueV2,
    },
    PlaceMention {
        place: PlaceV2,
    },
    Coverage {
        kind: StableCompilerValueV2,
    },
    Nop,
    Unsupported(UnsupportedStatementV2),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UnsupportedStatementV2 {
    FakeRead {
        cause: StableCompilerValueV2,
        place: PlaceV2,
    },
    AscribeUserType {
        place: PlaceV2,
        projection: StableCompilerValueV2,
        variance: StableCompilerValueV2,
    },
    ConstEvalCounter,
    BackwardIncompatibleDropHint {
        place: PlaceV2,
        reason: StableCompilerValueV2,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum IntrinsicStatementV2 {
    CopyNonOverlapping {
        source: Box<OperandV2>,
        destination: Box<OperandV2>,
        count: Box<OperandV2>,
    },
    Assume {
        condition: Box<OperandV2>,
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
    UnwrapUnsafeBinder {
        ty: TypeIdentityV2,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OperandV2 {
    Copy(PlaceV2),
    Move(PlaceV2),
    Constant {
        ty: Box<TypeIdentityV2>,
        value: StableCompilerValueV2,
        source: Box<SourceSpanV2>,
    },
    RuntimeChecks {
        kind: StableCompilerValueV2,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RvalueV2 {
    Use(OperandV2),
    Repeat {
        operand: OperandV2,
        count: StableCompilerValueV2,
    },
    Reference {
        borrow_kind: StableCompilerValueV2,
        place: PlaceV2,
    },
    RawPointer {
        kind: StableCompilerValueV2,
        place: PlaceV2,
    },
    Cast {
        kind: StableCompilerValueV2,
        operand: OperandV2,
        target: Box<TypeIdentityV2>,
    },
    Binary {
        operation: StableCompilerValueV2,
        lhs: OperandV2,
        rhs: Box<OperandV2>,
    },
    Unary {
        operation: StableCompilerValueV2,
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
    ThreadLocalRef {
        definition: DefinitionIdentityV2,
    },
    WrapUnsafeBinder {
        operand: OperandV2,
        target: TypeIdentityV2,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AggregateKindV2 {
    Array {
        element: TypeIdentityV2,
    },
    Tuple,
    Adt {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
        variant: usize,
        user_type_annotation: Option<usize>,
        active_field: Option<usize>,
    },
    Closure {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
    },
    CoroutineClosure {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
    },
    Coroutine {
        definition: DefinitionIdentityV2,
        generic_args_hash: [u8; 16],
    },
    RawPointer {
        pointee: TypeIdentityV2,
        mutable: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminatorV2 {
    pub source: SourceSpanV2,
    pub diagnostic_debug: String,
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
        callee: CalleeIdentityV2,
        arguments: Vec<CallArgumentV2>,
        destination: PlaceV2,
        target: Option<usize>,
        unwind: UnwindActionV2,
        call_source: StableCompilerValueV2,
        function_span: SourceSpanV2,
    },
    TailCall {
        function: OperandV2,
        callee: CalleeIdentityV2,
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
        message: StableCompilerValueV2,
        unwind: UnwindActionV2,
    },
    UnwindResume,
    UnwindTerminate {
        reason: StableCompilerValueV2,
    },
    Yield {
        value: OperandV2,
        resume: usize,
        resume_argument: PlaceV2,
        drop: Option<usize>,
    },
    CoroutineDrop,
    FalseEdge {
        real_target: usize,
        imaginary_target: usize,
    },
    FalseUnwind {
        real_target: usize,
        unwind: UnwindActionV2,
    },
    Unsupported(UnsupportedTerminatorV2),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CalleeIdentityV2 {
    Direct {
        declared: DefinitionIdentityV2,
        declared_generic_args_hash: [u8; 16],
        resolved: Box<FunctionIdentityV2>,
        intrinsic: Option<IntrinsicIdentityV2>,
    },
    Indirect {
        callable_type: TypeIdentityV2,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnsupportedTerminatorV2 {
    InlineAssembly {
        template_pieces: usize,
        operands: usize,
        line_spans: usize,
        targets: usize,
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
    Terminate { reason: StableCompilerValueV2 },
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
    validate_hash(
        "function.instance.generic_args_hash",
        &identity.instance.generic_args_hash,
    )?;
    validate_hash(
        "function.instance.instance_hash",
        &identity.instance.instance_hash,
    )?;
    validate_text(
        "function.instance.diagnostic_generic_args",
        &identity.instance.diagnostic_generic_args,
        limits,
    )?;
    validate_text(
        "function.instance.diagnostic_debug",
        &identity.instance.diagnostic_debug,
        limits,
    )?;
    match &identity.instance.kind {
        InstanceKindV2::Item
        | InstanceKindV2::Intrinsic
        | InstanceKindV2::VTableShim
        | InstanceKindV2::ReifyShim { .. }
        | InstanceKindV2::Virtual { .. }
        | InstanceKindV2::ClosureOnceShim { .. }
        | InstanceKindV2::ThreadLocalShim => {}
        InstanceKindV2::ConstructCoroutineInClosureShim {
            coroutine_closure, ..
        } => {
            validate_definition(
                "function.instance.coroutine_closure",
                coroutine_closure,
                limits,
            )?;
            if coroutine_closure != &identity.definition {
                return Err(ValidationErrorV2::new(
                    "function.instance.coroutine_closure",
                    "generated coroutine-closure shim definition disagrees with its instance",
                ));
            }
        }
        InstanceKindV2::FnPtrShim { fn_pointer }
        | InstanceKindV2::CloneShim { ty: fn_pointer }
        | InstanceKindV2::FnPtrAddrShim { ty: fn_pointer }
        | InstanceKindV2::AsyncDropGlueCtorShim { ty: fn_pointer }
        | InstanceKindV2::AsyncDropGlue { ty: fn_pointer } => {
            validate_type("function.instance.type", fn_pointer, limits)?;
        }
        InstanceKindV2::FutureDropPollShim {
            proxy_coroutine,
            implementation_coroutine,
        } => {
            validate_type("function.instance.proxy_coroutine", proxy_coroutine, limits)?;
            validate_type(
                "function.instance.implementation_coroutine",
                implementation_coroutine,
                limits,
            )?;
        }
        InstanceKindV2::DropGlue { ty } => {
            if let Some(ty) = ty {
                validate_type("function.instance.drop_type", ty, limits)?;
            }
        }
    }
    Ok(())
}

fn validate_definition(
    path: &str,
    definition: &DefinitionIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_text(
        &format!("{path}.diagnostic_crate_name"),
        &definition.diagnostic_crate_name,
        limits,
    )?;
    validate_text(
        &format!("{path}.diagnostic_def_path"),
        &definition.diagnostic_def_path,
        limits,
    )?;
    validate_hash(&format!("{path}.def_path_hash"), &definition.def_path_hash)?;
    validate_hash(
        &format!("{path}.stable_crate_id"),
        &definition.stable_crate_id,
    )?;
    if definition.local_def_path_hash.iter().all(|byte| *byte == 0) {
        return Err(ValidationErrorV2::new(
            format!("{path}.local_def_path_hash"),
            "stable local DefPath hash must not be the reserved zero value",
        ));
    }
    Ok(())
}

fn validate_span(
    path: &str,
    span: &SourceSpanV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    if span.authority != SourceAuthorityV2::CanonicalRemapped {
        return Err(ValidationErrorV2::new(
            format!("{path}.authority"),
            "source identity is unauthoritative and cannot be an exact capture",
        ));
    }
    validate_text(
        &format!("{path}.remapped_file"),
        &span.remapped_file,
        limits,
    )?;
    validate_text(
        &format!("{path}.diagnostic_debug"),
        &span.diagnostic_debug,
        limits,
    )?;
    validate_hash(&format!("{path}.source_file_hash"), &span.source_file_hash)?;
    validate_hash(&format!("{path}.span_hash"), &span.span_hash)?;
    validate_hash(
        &format!("{path}.source_scope_hash"),
        &span.source_scope_hash,
    )?;
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
    if span.source_scope == 0 && span.source_scope_parent.is_some() {
        return Err(ValidationErrorV2::new(
            format!("{path}.source_scope_parent"),
            "the root source scope cannot have a parent",
        ));
    }
    if span
        .source_scope_parent
        .is_some_and(|parent| parent >= span.source_scope)
    {
        return Err(ValidationErrorV2::new(
            format!("{path}.source_scope_parent"),
            "source scopes must refer to an earlier canonical parent",
        ));
    }
    Ok(())
}

fn validate_type(
    path: &str,
    ty: &TypeIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_hash(&format!("{path}.stable_hash"), &ty.stable_hash)?;
    validate_text(
        &format!("{path}.diagnostic_display"),
        &ty.diagnostic_display,
        limits,
    )?;
    validate_text(
        &format!("{path}.diagnostic_debug"),
        &ty.diagnostic_debug,
        limits,
    )?;
    match &ty.class {
        TypeClassV2::Adt {
            definition,
            generic_args_hash,
        }
        | TypeClassV2::FunctionDefinition {
            definition,
            generic_args_hash,
        }
        | TypeClassV2::Closure {
            definition,
            generic_args_hash,
        }
        | TypeClassV2::CoroutineClosure {
            definition,
            generic_args_hash,
        }
        | TypeClassV2::Coroutine {
            definition,
            generic_args_hash,
        }
        | TypeClassV2::CoroutineWitness {
            definition,
            generic_args_hash,
        } => {
            validate_definition(&format!("{path}.definition"), definition, limits)?;
            validate_hash(&format!("{path}.generic_args_hash"), generic_args_hash)
        }
        TypeClassV2::Foreign { definition } => {
            validate_definition(&format!("{path}.definition"), definition, limits)
        }
        TypeClassV2::Unsupported(kind) => Err(ValidationErrorV2::new(
            format!("{path}.class"),
            format!("unresolved compiler type {kind:?} cannot be an exact capture"),
        )),
        TypeClassV2::Bool
        | TypeClassV2::Char
        | TypeClassV2::SignedInteger(_)
        | TypeClassV2::UnsignedInteger(_)
        | TypeClassV2::Float(_)
        | TypeClassV2::StringSlice
        | TypeClassV2::Array
        | TypeClassV2::Pattern
        | TypeClassV2::Slice
        | TypeClassV2::RawPointer { .. }
        | TypeClassV2::Reference { .. }
        | TypeClassV2::FunctionPointer
        | TypeClassV2::UnsafeBinder
        | TypeClassV2::Dynamic
        | TypeClassV2::Never
        | TypeClassV2::Tuple { .. } => Ok(()),
    }
}

fn validate_statement(
    path: &str,
    statement: &StatementV2,
    local_count: usize,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_span(&format!("{path}.source"), &statement.source, limits)?;
    validate_text(
        &format!("{path}.diagnostic_debug"),
        &statement.diagnostic_debug,
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
        | StatementKindV2::PlaceMention { place } => {
            validate_place(&format!("{path}.place"), place, local_count, limits)
        }
        StatementKindV2::Intrinsic(intrinsic) => match intrinsic.as_ref() {
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
        },
        StatementKindV2::Retag { place, kind } => {
            validate_place(&format!("{path}.place"), place, local_count, limits)?;
            validate_stable_value(&format!("{path}.retag"), kind, limits)
        }
        StatementKindV2::Coverage { kind } => {
            validate_stable_value(&format!("{path}.coverage"), kind, limits)
        }
        StatementKindV2::Nop => Ok(()),
        StatementKindV2::Unsupported(kind) => Err(ValidationErrorV2::new(
            format!("{path}.kind"),
            format!("unsupported statement {kind:?} cannot be an exact capture"),
        )),
    }
}

fn validate_rvalue(
    path: &str,
    value: &RvalueV2,
    local_count: usize,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    match value {
        RvalueV2::Use(operand) => {
            validate_operand(&format!("{path}.operand"), operand, local_count, limits)
        }
        RvalueV2::Unary { operation, operand } => {
            validate_stable_value(&format!("{path}.operation"), operation, limits)?;
            validate_operand(&format!("{path}.operand"), operand, local_count, limits)
        }
        RvalueV2::Repeat { operand, count } => {
            validate_operand(&format!("{path}.operand"), operand, local_count, limits)?;
            validate_stable_value(&format!("{path}.count"), count, limits)
        }
        RvalueV2::Reference { borrow_kind, place } => {
            validate_stable_value(&format!("{path}.borrow_kind"), borrow_kind, limits)?;
            validate_place(&format!("{path}.place"), place, local_count, limits)
        }
        RvalueV2::RawPointer { kind, place } => {
            validate_stable_value(&format!("{path}.raw_pointer_kind"), kind, limits)?;
            validate_place(&format!("{path}.place"), place, local_count, limits)
        }
        RvalueV2::Discriminant { place } | RvalueV2::CopyForDeref(place) => {
            validate_place(&format!("{path}.place"), place, local_count, limits)
        }
        RvalueV2::Cast {
            kind,
            operand,
            target,
        } => {
            validate_stable_value(&format!("{path}.kind"), kind, limits)?;
            validate_operand(&format!("{path}.operand"), operand, local_count, limits)?;
            validate_type(&format!("{path}.target"), target, limits)
        }
        RvalueV2::Binary {
            operation,
            lhs,
            rhs,
        } => {
            validate_stable_value(&format!("{path}.operation"), operation, limits)?;
            validate_operand(&format!("{path}.lhs"), lhs, local_count, limits)?;
            validate_operand(&format!("{path}.rhs"), rhs, local_count, limits)
        }
        RvalueV2::Aggregate { kind, operands } => {
            bounded(
                format!("{path}.operands"),
                operands.len(),
                limits.max_operands,
            )?;
            validate_aggregate_kind(&format!("{path}.kind"), kind, limits)?;
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
        RvalueV2::ThreadLocalRef { definition } => {
            validate_definition(&format!("{path}.definition"), definition, limits)
        }
        RvalueV2::WrapUnsafeBinder { operand, target } => {
            validate_operand(&format!("{path}.operand"), operand, local_count, limits)?;
            validate_type(&format!("{path}.target"), target, limits)
        }
    }
}

fn validate_aggregate_kind(
    path: &str,
    kind: &AggregateKindV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    match kind {
        AggregateKindV2::Array { element } => validate_type(path, element, limits),
        AggregateKindV2::Tuple => Ok(()),
        AggregateKindV2::Adt {
            definition,
            generic_args_hash,
            ..
        }
        | AggregateKindV2::Closure {
            definition,
            generic_args_hash,
        }
        | AggregateKindV2::CoroutineClosure {
            definition,
            generic_args_hash,
        }
        | AggregateKindV2::Coroutine {
            definition,
            generic_args_hash,
        } => {
            validate_definition(&format!("{path}.definition"), definition, limits)?;
            validate_hash(&format!("{path}.generic_args_hash"), generic_args_hash)
        }
        AggregateKindV2::RawPointer { pointee, .. } => validate_type(path, pointee, limits),
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
        &format!("{path}.diagnostic_debug"),
        &terminator.diagnostic_debug,
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
                format!("{path}.targets"),
                targets.len(),
                limits.max_switch_targets,
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
            callee,
            arguments,
            destination,
            target,
            unwind,
            call_source,
            function_span,
        } => {
            validate_operand(&format!("{path}.function"), function, local_count, limits)?;
            validate_callee(&format!("{path}.callee"), function, callee, limits)?;
            bounded(
                format!("{path}.arguments"),
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
            validate_stable_value(&format!("{path}.call_source"), call_source, limits)?;
            validate_span(&format!("{path}.function_span"), function_span, limits)?;
            let expected = normal_and_unwind_successors(*target, unwind);
            validate_exact_successors(path, &terminator.successors, &expected)
        }
        TerminatorKindV2::TailCall {
            function,
            callee,
            arguments,
            function_span,
        } => {
            validate_operand(&format!("{path}.function"), function, local_count, limits)?;
            validate_callee(&format!("{path}.callee"), function, callee, limits)?;
            bounded(
                format!("{path}.arguments"),
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
            validate_stable_value(&format!("{path}.message"), message, limits)?;
            validate_unwind(&format!("{path}.unwind"), unwind, block_count, limits)?;
            let expected = normal_and_unwind_successors(Some(*target), unwind);
            validate_exact_successors(path, &terminator.successors, &expected)
        }
        TerminatorKindV2::UnwindResume
        | TerminatorKindV2::UnwindTerminate { .. }
        | TerminatorKindV2::CoroutineDrop => {
            if let TerminatorKindV2::UnwindTerminate { reason } = &terminator.kind {
                validate_stable_value(&format!("{path}.reason"), reason, limits)?;
            }
            validate_exact_successors(path, &terminator.successors, &[])
        }
        TerminatorKindV2::Yield {
            value,
            resume,
            resume_argument,
            drop,
        } => {
            validate_operand(&format!("{path}.value"), value, local_count, limits)?;
            validate_block(&format!("{path}.resume"), *resume, block_count)?;
            validate_place(
                &format!("{path}.resume_argument"),
                resume_argument,
                local_count,
                limits,
            )?;
            let mut expected = vec![*resume];
            if let Some(drop) = drop {
                validate_block(&format!("{path}.drop"), *drop, block_count)?;
                expected.push(*drop);
            }
            validate_exact_successors(path, &terminator.successors, &expected)
        }
        TerminatorKindV2::FalseEdge {
            real_target,
            imaginary_target,
        } => {
            validate_block(&format!("{path}.real_target"), *real_target, block_count)?;
            validate_block(
                &format!("{path}.imaginary_target"),
                *imaginary_target,
                block_count,
            )?;
            validate_exact_successors(
                path,
                &terminator.successors,
                &[*real_target, *imaginary_target],
            )
        }
        TerminatorKindV2::FalseUnwind {
            real_target,
            unwind,
        } => {
            validate_block(&format!("{path}.real_target"), *real_target, block_count)?;
            validate_unwind(&format!("{path}.unwind"), unwind, block_count, limits)?;
            let expected = normal_and_unwind_successors(Some(*real_target), unwind);
            validate_exact_successors(path, &terminator.successors, &expected)
        }
        TerminatorKindV2::Unsupported(kind) => Err(ValidationErrorV2::new(
            format!("{path}.kind"),
            format!("unsupported terminator {kind:?} cannot be an exact capture"),
        )),
    }
}

fn validate_callee(
    path: &str,
    function: &OperandV2,
    callee: &CalleeIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    match callee {
        CalleeIdentityV2::Direct {
            declared,
            declared_generic_args_hash,
            resolved,
            intrinsic,
        } => {
            validate_definition(&format!("{path}.declared"), declared, limits)?;
            validate_hash(
                &format!("{path}.declared_generic_args_hash"),
                declared_generic_args_hash,
            )?;
            validate_function_identity(resolved, limits)?;
            let OperandV2::Constant { ty, .. } = function else {
                return Err(ValidationErrorV2::new(
                    path,
                    "a direct callee requires a constant function-definition operand",
                ));
            };
            let TypeClassV2::FunctionDefinition {
                definition,
                generic_args_hash,
            } = &ty.class
            else {
                return Err(ValidationErrorV2::new(
                    path,
                    "direct callee operand does not have a function-definition type",
                ));
            };
            if definition != declared || generic_args_hash != declared_generic_args_hash {
                return Err(ValidationErrorV2::new(
                    path,
                    "declared callee identity disagrees with its operand type",
                ));
            }
            if let Some(intrinsic) = intrinsic {
                validate_definition(
                    &format!("{path}.intrinsic.definition"),
                    &intrinsic.definition,
                    limits,
                )?;
                validate_text(&format!("{path}.intrinsic.name"), &intrinsic.name, limits)?;
                if intrinsic.definition != *declared || resolved.definition != *declared {
                    return Err(ValidationErrorV2::new(
                        path,
                        "intrinsic, declared, and resolved DefId identities disagree",
                    ));
                }
            }
            Ok(())
        }
        CalleeIdentityV2::Indirect { callable_type } => {
            validate_type(&format!("{path}.callable_type"), callable_type, limits)?;
            if !matches!(callable_type.class, TypeClassV2::FunctionPointer) {
                return Err(ValidationErrorV2::new(
                    path,
                    "only a structurally identified function pointer is legitimately indirect",
                ));
            }
            if matches!(
                function,
                OperandV2::Constant { ty, .. }
                    if matches!(ty.class, TypeClassV2::FunctionDefinition { .. })
            ) {
                return Err(ValidationErrorV2::new(
                    path,
                    "a function-definition constant cannot be marked indirect",
                ));
            }
            Ok(())
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
        UnwindActionV2::Terminate { reason } => validate_stable_value(path, reason, limits),
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
        OperandV2::Constant { ty, value, source } => {
            validate_type(&format!("{path}.type"), ty, limits)?;
            validate_stable_value(&format!("{path}.value"), value, limits)?;
            validate_span(&format!("{path}.source"), source, limits)
        }
        OperandV2::RuntimeChecks { kind } => {
            validate_stable_value(&format!("{path}.runtime_checks"), kind, limits)
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
        format!("{path}.projection"),
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
            | ProjectionV2::UnwrapUnsafeBinder { ty } => {
                validate_type(&format!("{projection_path}.type"), ty, limits)?;
            }
            ProjectionV2::Downcast {
                name: Some(name), ..
            } => {
                validate_text(&format!("{projection_path}.name"), name, limits)?;
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

fn validate_stable_value(
    path: &str,
    value: &StableCompilerValueV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_hash(&format!("{path}.stable_hash"), &value.stable_hash)?;
    validate_text(&format!("{path}.diagnostic"), &value.diagnostic, limits)
}

fn validate_hash(path: &str, hash: &[u8; 16]) -> Result<(), ValidationErrorV2> {
    if hash.iter().all(|byte| *byte == 0) {
        return Err(ValidationErrorV2::new(
            path,
            "stable identity hash must not be the reserved zero value",
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
