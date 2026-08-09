use std::error::Error;
use std::fmt;

use super::accounting::recompute_capture_accounting_v2;
use sha2::{Digest, Sha256};

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
    pub max_type_depth: usize,
    pub max_type_nodes: usize,
    pub max_type_arity: usize,
    pub max_generic_args: usize,
    pub max_source_scopes: usize,
    pub max_macro_expansion_depth: usize,
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
            max_type_depth: 64,
            max_type_nodes: 16_384,
            max_type_arity: 4_096,
            max_generic_args: 1_024,
            max_source_scopes: 65_536,
            max_macro_expansion_depth: 64,
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
    pub source_scopes: Vec<SourceScopeIdentityV2>,
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
        let accounting = recompute_capture_accounting_v2(self, limits)?;
        if self.capture_work_items != accounting.work_items {
            return Err(ValidationErrorV2::new(
                "capture_work_items",
                format!(
                    "reported work count {} does not equal recomputed count {}",
                    self.capture_work_items, accounting.work_items
                ),
            ));
        }
        if self.capture_text_bytes != accounting.text_bytes {
            return Err(ValidationErrorV2::new(
                "capture_text_bytes",
                format!(
                    "reported text count {} does not equal recomputed count {}",
                    self.capture_text_bytes, accounting.text_bytes
                ),
            ));
        }
        validate_source_scopes(&self.source_scopes, limits)?;
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
                &self.locals,
                self.blocks.len(),
                limits,
            )?;
        }
        bounded("statements", total_statements, limits.max_total_statements)?;
        validate_body_scope_bindings(self)
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
    pub binding_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionSignatureIdentityV2 {
    pub stable_hash: [u8; 16],
    pub origin: FunctionSignatureOriginV2,
    pub inputs: Vec<TypeIdentityV2>,
    pub output: Box<TypeIdentityV2>,
    pub safety: FunctionSafetyV2,
    pub abi: FunctionAbiIdentityV2,
    pub c_variadic: bool,
    pub binding_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FunctionSignatureOriginV2 {
    CompilerFnSig,
    GeneratedMir,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FunctionSafetyV2 {
    Safe,
    Unsafe,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionAbiIdentityV2 {
    pub stable_hash: [u8; 16],
    pub canonical_name: String,
    pub unwind_allowed: bool,
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
    pub original_span_hash: [u8; 16],
    pub span_hash: [u8; 16],
    pub expansion: MacroExpansionIdentityV2,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub source_scope: usize,
    pub source_scope_hash: [u8; 16],
    pub source_scope_parent: Option<usize>,
    pub inlined_instance_hash: Option<[u8; 16]>,
    pub source_scope_record_hash: [u8; 32],
    pub diagnostic_debug: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceScopeIdentityV2 {
    pub index: usize,
    pub compiler_hash: [u8; 16],
    pub parent: Option<usize>,
    pub inlined_parent: Option<usize>,
    pub inlined: Option<FunctionIdentityV2>,
    pub scope_span: StructuralSpanIdentityV2,
    pub inlined_callsite: Option<StructuralSpanIdentityV2>,
    pub record_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StructuralSpanIdentityV2 {
    pub original_span_hash: [u8; 16],
    pub callsite_span_hash: [u8; 16],
    pub expansion: MacroExpansionIdentityV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MacroExpansionIdentityV2 {
    pub syntax_context_hash: [u8; 16],
    pub frames: Vec<MacroExpansionFrameV2>,
    pub chain_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MacroExpansionFrameV2 {
    pub expansion_hash: [u8; 16],
    pub callsite_span_hash: [u8; 16],
    pub definition_site_hash: [u8; 16],
    pub macro_definition: Option<StableDefinitionKeyV2>,
    pub parent_module: Option<StableDefinitionKeyV2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StableDefinitionKeyV2 {
    pub def_path_hash: [u8; 16],
    pub stable_crate_id: [u8; 16],
    pub local_def_path_hash: [u8; 8],
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
        generic_arg_count: usize,
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
    pub type_hash: [u8; 16],
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
        contract_hash: [u8; 32],
    },
    TailCall {
        function: OperandV2,
        callee: CalleeIdentityV2,
        arguments: Vec<CallArgumentV2>,
        function_span: SourceSpanV2,
        contract_hash: [u8; 32],
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
        declared_generic_arg_count: usize,
        declared_signature: FunctionSignatureIdentityV2,
        resolved: Box<FunctionIdentityV2>,
        resolved_signature: Box<FunctionSignatureIdentityV2>,
        intrinsic: Option<Box<IntrinsicIdentityV2>>,
        resolution_binding_hash: [u8; 32],
    },
    Indirect {
        callable_type: TypeIdentityV2,
        signature: Box<FunctionSignatureIdentityV2>,
        callable_binding_hash: [u8; 32],
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

pub(super) fn intrinsic_binding_hash_v2(
    intrinsic: &IntrinsicIdentityV2,
) -> Result<[u8; 32], ValidationErrorV2> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3.mir-v2.intrinsic-binding.v1\0");
    hash_definition_binding(&mut hasher, &intrinsic.definition);
    hash_usize_binding(&mut hasher, intrinsic.name.len(), "intrinsic.name")?;
    hasher.update(intrinsic.name.as_bytes());
    hasher.update([u8::from(intrinsic.must_be_overridden)]);
    hasher.update([u8::from(intrinsic.const_stable)]);
    Ok(hasher.finalize().into())
}

pub(super) fn expansion_chain_hash_v2(
    expansion: &MacroExpansionIdentityV2,
) -> Result<[u8; 32], ValidationErrorV2> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3.mir-v2.macro-expansion.v1\0");
    hasher.update(expansion.syntax_context_hash);
    hash_usize_binding(
        &mut hasher,
        expansion.frames.len(),
        "macro expansion frames",
    )?;
    for frame in &expansion.frames {
        hasher.update(frame.expansion_hash);
        hasher.update(frame.callsite_span_hash);
        hasher.update(frame.definition_site_hash);
        hash_optional_definition_key(&mut hasher, frame.macro_definition.as_ref());
        hash_optional_definition_key(&mut hasher, frame.parent_module.as_ref());
    }
    Ok(hasher.finalize().into())
}

pub(super) fn source_scope_record_hash_v2(
    scope: &SourceScopeIdentityV2,
) -> Result<[u8; 32], ValidationErrorV2> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3.mir-v2.source-scope.v1\0");
    hash_usize_binding(&mut hasher, scope.index, "source scope index")?;
    hasher.update(scope.compiler_hash);
    hash_optional_usize_binding(&mut hasher, scope.parent, "source scope parent")?;
    hash_optional_usize_binding(
        &mut hasher,
        scope.inlined_parent,
        "source scope inlined parent",
    )?;
    match &scope.inlined {
        Some(inlined) => {
            hasher.update([1]);
            hash_definition_binding(&mut hasher, &inlined.definition);
            hasher.update(inlined.instance.generic_args_hash);
            hash_usize_binding(
                &mut hasher,
                inlined.instance.generic_arg_count,
                "source scope inlined generic arguments",
            )?;
            hasher.update(inlined.instance.instance_hash);
            hash_instance_kind_binding(&mut hasher, &inlined.instance.kind)?;
        }
        None => hasher.update([0]),
    }
    hash_structural_span_binding(&mut hasher, &scope.scope_span);
    match &scope.inlined_callsite {
        Some(span) => {
            hasher.update([1]);
            hash_structural_span_binding(&mut hasher, span);
        }
        None => hasher.update([0]),
    }
    Ok(hasher.finalize().into())
}

fn hash_structural_span_binding(hasher: &mut Sha256, span: &StructuralSpanIdentityV2) {
    hasher.update(span.original_span_hash);
    hasher.update(span.callsite_span_hash);
    hasher.update(span.expansion.chain_hash);
}

fn hash_optional_definition_key(hasher: &mut Sha256, definition: Option<&StableDefinitionKeyV2>) {
    match definition {
        Some(definition) => {
            hasher.update([1]);
            hasher.update(definition.def_path_hash);
            hasher.update(definition.stable_crate_id);
            hasher.update(definition.local_def_path_hash);
        }
        None => hasher.update([0]),
    }
}

fn hash_optional_usize_binding(
    hasher: &mut Sha256,
    value: Option<usize>,
    path: &str,
) -> Result<(), ValidationErrorV2> {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_usize_binding(hasher, value, path)
        }
        None => {
            hasher.update([0]);
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolution_binding_hash_v2(
    callable_type: &TypeIdentityV2,
    declared: &DefinitionIdentityV2,
    declared_generic_args_hash: &[u8; 16],
    declared_generic_arg_count: usize,
    declared_signature: &FunctionSignatureIdentityV2,
    resolved: &FunctionIdentityV2,
    resolved_signature: &FunctionSignatureIdentityV2,
    intrinsic: Option<&IntrinsicIdentityV2>,
) -> Result<[u8; 32], ValidationErrorV2> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3.mir-v2.direct-call-binding.v1\0");
    hasher.update(callable_type.stable_hash);
    hash_definition_binding(&mut hasher, declared);
    hasher.update(declared_generic_args_hash);
    hash_usize_binding(
        &mut hasher,
        declared_generic_arg_count,
        "callee.declared_generic_arg_count",
    )?;
    hash_signature_binding(&mut hasher, declared_signature)?;
    hash_definition_binding(&mut hasher, &resolved.definition);
    hasher.update(resolved.instance.generic_args_hash);
    hash_usize_binding(
        &mut hasher,
        resolved.instance.generic_arg_count,
        "callee.resolved.generic_arg_count",
    )?;
    hasher.update(resolved.instance.instance_hash);
    hash_instance_kind_binding(&mut hasher, &resolved.instance.kind)?;
    hash_signature_binding(&mut hasher, resolved_signature)?;
    match intrinsic {
        Some(intrinsic) => {
            hasher.update([1]);
            hasher.update(intrinsic.binding_hash);
        }
        None => hasher.update([0]),
    }
    Ok(hasher.finalize().into())
}

fn hash_definition_binding(hasher: &mut Sha256, definition: &DefinitionIdentityV2) {
    hasher.update(definition.def_path_hash);
    hasher.update(definition.stable_crate_id);
    hasher.update(definition.local_def_path_hash);
}

fn hash_signature_binding(
    hasher: &mut Sha256,
    signature: &FunctionSignatureIdentityV2,
) -> Result<(), ValidationErrorV2> {
    hasher.update(signature.stable_hash);
    hasher.update(signature.binding_hash);
    Ok(())
}

pub(super) fn function_signature_binding_hash_v2(
    signature: &FunctionSignatureIdentityV2,
) -> Result<[u8; 32], ValidationErrorV2> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3.mir-v2.function-signature.v1\0");
    hasher.update(signature.stable_hash);
    hasher.update([match signature.origin {
        FunctionSignatureOriginV2::CompilerFnSig => 0,
        FunctionSignatureOriginV2::GeneratedMir => 1,
    }]);
    hash_usize_binding(&mut hasher, signature.inputs.len(), "signature.inputs")?;
    for input in &signature.inputs {
        hasher.update(input.stable_hash);
    }
    hasher.update(signature.output.stable_hash);
    hasher.update([match signature.safety {
        FunctionSafetyV2::Safe => 0,
        FunctionSafetyV2::Unsafe => 1,
    }]);
    hasher.update(signature.abi.stable_hash);
    hash_text_binding(
        &mut hasher,
        &signature.abi.canonical_name,
        "signature.abi.canonical_name",
    )?;
    hasher.update([
        u8::from(signature.abi.unwind_allowed),
        u8::from(signature.c_variadic),
    ]);
    Ok(hasher.finalize().into())
}

pub(super) fn indirect_callable_binding_hash_v2(
    callable_type: &TypeIdentityV2,
    signature: &FunctionSignatureIdentityV2,
) -> Result<[u8; 32], ValidationErrorV2> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3.mir-v2.indirect-callable.v1\0");
    hasher.update(callable_type.stable_hash);
    hash_signature_binding(&mut hasher, signature)?;
    Ok(hasher.finalize().into())
}

pub(super) fn call_contract_hash_v2(
    function: &OperandV2,
    callee: &CalleeIdentityV2,
    arguments: &[CallArgumentV2],
    destination: Option<&PlaceV2>,
    target: Option<usize>,
    unwind: Option<&UnwindActionV2>,
) -> Result<[u8; 32], ValidationErrorV2> {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3.mir-v2.call-contract.v1\0");
    hash_operand_binding(&mut hasher, function)?;
    match callee {
        CalleeIdentityV2::Direct {
            resolution_binding_hash,
            ..
        } => {
            hasher.update([0]);
            hasher.update(resolution_binding_hash);
        }
        CalleeIdentityV2::Indirect {
            callable_binding_hash,
            ..
        } => {
            hasher.update([1]);
            hasher.update(callable_binding_hash);
        }
    }
    hash_usize_binding(&mut hasher, arguments.len(), "call.arguments")?;
    for argument in arguments {
        hash_operand_binding(&mut hasher, &argument.operand)?;
    }
    match destination {
        Some(destination) => {
            hasher.update([1]);
            hash_place_binding(&mut hasher, destination)?;
        }
        None => hasher.update([0]),
    }
    hash_optional_usize_binding(&mut hasher, target, "call.target")?;
    match unwind {
        Some(unwind) => {
            hasher.update([1]);
            hash_unwind_binding(&mut hasher, unwind)?;
        }
        None => hasher.update([0]),
    }
    Ok(hasher.finalize().into())
}

fn hash_operand_binding(hasher: &mut Sha256, operand: &OperandV2) -> Result<(), ValidationErrorV2> {
    match operand {
        OperandV2::Copy(place) => {
            hasher.update([0]);
            hash_place_binding(hasher, place)
        }
        OperandV2::Move(place) => {
            hasher.update([1]);
            hash_place_binding(hasher, place)
        }
        OperandV2::Constant { ty, value, .. } => {
            hasher.update([2]);
            hasher.update(ty.stable_hash);
            hasher.update(value.stable_hash);
            Ok(())
        }
        OperandV2::RuntimeChecks { kind } => {
            hasher.update([3]);
            hasher.update(kind.stable_hash);
            Ok(())
        }
    }
}

fn hash_place_binding(hasher: &mut Sha256, place: &PlaceV2) -> Result<(), ValidationErrorV2> {
    hash_usize_binding(hasher, place.local, "call.place.local")?;
    hasher.update(place.type_hash);
    hash_usize_binding(hasher, place.projection.len(), "call.place.projection")?;
    for projection in &place.projection {
        match projection {
            ProjectionV2::Deref => hasher.update([0]),
            ProjectionV2::Field { index, ty } => {
                hasher.update([1]);
                hash_usize_binding(hasher, *index, "call.place.field")?;
                hasher.update(ty.stable_hash);
            }
            ProjectionV2::Index { local } => {
                hasher.update([2]);
                hash_usize_binding(hasher, *local, "call.place.index")?;
            }
            ProjectionV2::ConstantIndex {
                offset,
                min_length,
                from_end,
            } => {
                hasher.update([3]);
                hasher.update(offset.to_le_bytes());
                hasher.update(min_length.to_le_bytes());
                hasher.update([u8::from(*from_end)]);
            }
            ProjectionV2::Subslice { from, to, from_end } => {
                hasher.update([4]);
                hasher.update(from.to_le_bytes());
                hasher.update(to.to_le_bytes());
                hasher.update([u8::from(*from_end)]);
            }
            ProjectionV2::Downcast { variant, .. } => {
                hasher.update([5]);
                hash_usize_binding(hasher, *variant, "call.place.downcast")?;
            }
            ProjectionV2::OpaqueCast { ty } => {
                hasher.update([6]);
                hasher.update(ty.stable_hash);
            }
            ProjectionV2::UnwrapUnsafeBinder { ty } => {
                hasher.update([7]);
                hasher.update(ty.stable_hash);
            }
        }
    }
    Ok(())
}

fn hash_unwind_binding(
    hasher: &mut Sha256,
    unwind: &UnwindActionV2,
) -> Result<(), ValidationErrorV2> {
    match unwind {
        UnwindActionV2::Continue => hasher.update([0]),
        UnwindActionV2::Unreachable => hasher.update([1]),
        UnwindActionV2::Terminate { reason } => {
            hasher.update([2]);
            hasher.update(reason.stable_hash);
        }
        UnwindActionV2::Cleanup { target } => {
            hasher.update([3]);
            hash_usize_binding(hasher, *target, "call.unwind.cleanup")?;
        }
    }
    Ok(())
}

fn hash_text_binding(hasher: &mut Sha256, text: &str, path: &str) -> Result<(), ValidationErrorV2> {
    hash_usize_binding(hasher, text.len(), path)?;
    hasher.update(text.as_bytes());
    Ok(())
}

fn hash_instance_kind_binding(
    hasher: &mut Sha256,
    kind: &InstanceKindV2,
) -> Result<(), ValidationErrorV2> {
    match kind {
        InstanceKindV2::Item => hasher.update([0]),
        InstanceKindV2::Intrinsic => hasher.update([1]),
        InstanceKindV2::VTableShim => hasher.update([2]),
        InstanceKindV2::ReifyShim { reason } => {
            hasher.update([3]);
            hasher.update([match reason {
                None => 0,
                Some(ReifyReasonV2::FunctionPointer) => 1,
                Some(ReifyReasonV2::Vtable) => 2,
            }]);
        }
        InstanceKindV2::FnPtrShim { fn_pointer } => {
            hasher.update([4]);
            hasher.update(fn_pointer.stable_hash);
        }
        InstanceKindV2::Virtual { vtable_index } => {
            hasher.update([5]);
            hash_usize_binding(hasher, *vtable_index, "callee.resolved.vtable_index")?;
        }
        InstanceKindV2::ClosureOnceShim { track_caller } => {
            hasher.update([6]);
            hasher.update([u8::from(*track_caller)]);
        }
        InstanceKindV2::ConstructCoroutineInClosureShim {
            coroutine_closure,
            receiver_by_ref,
        } => {
            hasher.update([7]);
            hash_definition_binding(hasher, coroutine_closure);
            hasher.update([u8::from(*receiver_by_ref)]);
        }
        InstanceKindV2::ThreadLocalShim => hasher.update([8]),
        InstanceKindV2::FutureDropPollShim {
            proxy_coroutine,
            implementation_coroutine,
        } => {
            hasher.update([9]);
            hasher.update(proxy_coroutine.stable_hash);
            hasher.update(implementation_coroutine.stable_hash);
        }
        InstanceKindV2::DropGlue { ty } => {
            hasher.update([10]);
            match ty {
                Some(ty) => {
                    hasher.update([1]);
                    hasher.update(ty.stable_hash);
                }
                None => hasher.update([0]),
            }
        }
        InstanceKindV2::CloneShim { ty } => {
            hasher.update([11]);
            hasher.update(ty.stable_hash);
        }
        InstanceKindV2::FnPtrAddrShim { ty } => {
            hasher.update([12]);
            hasher.update(ty.stable_hash);
        }
        InstanceKindV2::AsyncDropGlueCtorShim { ty } => {
            hasher.update([13]);
            hasher.update(ty.stable_hash);
        }
        InstanceKindV2::AsyncDropGlue { ty } => {
            hasher.update([14]);
            hasher.update(ty.stable_hash);
        }
    }
    Ok(())
}

fn hash_usize_binding(
    hasher: &mut Sha256,
    value: usize,
    path: &str,
) -> Result<(), ValidationErrorV2> {
    let value = u64::try_from(value).map_err(|_| {
        ValidationErrorV2::new(path, "value cannot be represented canonically as u64")
    })?;
    hasher.update(value.to_le_bytes());
    Ok(())
}

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
    bounded(
        "function.instance.generic_arg_count",
        identity.instance.generic_arg_count,
        limits.max_generic_args,
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

fn validate_source_scopes(
    scopes: &[SourceScopeIdentityV2],
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    bounded("source_scopes", scopes.len(), limits.max_source_scopes)?;
    if scopes.is_empty() {
        return Err(ValidationErrorV2::new(
            "source_scopes",
            "a captured MIR body must have a root source scope",
        ));
    }
    for (index, scope) in scopes.iter().enumerate() {
        let path = format!("source_scopes[{index}]");
        if scope.index != index {
            return Err(ValidationErrorV2::new(
                format!("{path}.index"),
                "source scope index is not canonical",
            ));
        }
        validate_hash(&format!("{path}.compiler_hash"), &scope.compiler_hash)?;
        if index == 0 && scope.parent.is_some() {
            return Err(ValidationErrorV2::new(
                format!("{path}.parent"),
                "the root source scope cannot have a parent",
            ));
        }
        for (label, parent) in [
            ("parent", scope.parent),
            ("inlined_parent", scope.inlined_parent),
        ] {
            if parent.is_some_and(|parent| parent >= index) {
                return Err(ValidationErrorV2::new(
                    format!("{path}.{label}"),
                    "source-scope links must target an earlier canonical scope",
                ));
            }
        }
        if scope.inlined.is_some() != scope.inlined_callsite.is_some() {
            return Err(ValidationErrorV2::new(
                format!("{path}.inlined"),
                "inlined instance and callsite identities must be present together",
            ));
        }
        if let Some(parent) = scope.inlined_parent
            && scopes[parent].inlined.is_none()
        {
            return Err(ValidationErrorV2::new(
                format!("{path}.inlined_parent"),
                "inlined parent must identify an earlier inlined scope",
            ));
        }
        if let Some(inlined) = &scope.inlined {
            validate_function_identity(inlined, limits)?;
        }
        validate_structural_span(&format!("{path}.scope_span"), &scope.scope_span, limits)?;
        if let Some(callsite) = &scope.inlined_callsite {
            validate_structural_span(&format!("{path}.inlined_callsite"), callsite, limits)?;
        }
        validate_hash32(&format!("{path}.record_hash"), &scope.record_hash)?;
        if scope.record_hash != source_scope_record_hash_v2(scope)? {
            return Err(ValidationErrorV2::new(
                format!("{path}.record_hash"),
                "source-scope record binding does not match its captured fields",
            ));
        }
    }
    Ok(())
}

fn validate_structural_span(
    path: &str,
    span: &StructuralSpanIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_hash(
        &format!("{path}.original_span_hash"),
        &span.original_span_hash,
    )?;
    validate_hash(
        &format!("{path}.callsite_span_hash"),
        &span.callsite_span_hash,
    )?;
    validate_expansion(&format!("{path}.expansion"), &span.expansion, limits)
}

fn validate_expansion(
    path: &str,
    expansion: &MacroExpansionIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_hash(
        &format!("{path}.syntax_context_hash"),
        &expansion.syntax_context_hash,
    )?;
    bounded(
        format!("{path}.frames"),
        expansion.frames.len(),
        limits.max_macro_expansion_depth,
    )?;
    for (index, frame) in expansion.frames.iter().enumerate() {
        let frame_path = format!("{path}.frames[{index}]");
        validate_hash(
            &format!("{frame_path}.expansion_hash"),
            &frame.expansion_hash,
        )?;
        validate_hash(
            &format!("{frame_path}.callsite_span_hash"),
            &frame.callsite_span_hash,
        )?;
        validate_hash(
            &format!("{frame_path}.definition_site_hash"),
            &frame.definition_site_hash,
        )?;
        if let Some(definition) = &frame.macro_definition {
            validate_definition_key(&format!("{frame_path}.macro_definition"), definition)?;
        }
        if let Some(definition) = &frame.parent_module {
            validate_definition_key(&format!("{frame_path}.parent_module"), definition)?;
        }
    }
    validate_hash32(&format!("{path}.chain_hash"), &expansion.chain_hash)?;
    if expansion.chain_hash != expansion_chain_hash_v2(expansion)? {
        return Err(ValidationErrorV2::new(
            format!("{path}.chain_hash"),
            "macro expansion binding does not match its captured frames",
        ));
    }
    Ok(())
}

fn validate_definition_key(
    path: &str,
    definition: &StableDefinitionKeyV2,
) -> Result<(), ValidationErrorV2> {
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
    validate_hash(
        &format!("{path}.original_span_hash"),
        &span.original_span_hash,
    )?;
    validate_hash(&format!("{path}.span_hash"), &span.span_hash)?;
    validate_expansion(&format!("{path}.expansion"), &span.expansion, limits)?;
    validate_hash(
        &format!("{path}.source_scope_hash"),
        &span.source_scope_hash,
    )?;
    validate_hash32(
        &format!("{path}.source_scope_record_hash"),
        &span.source_scope_record_hash,
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

fn validate_body_scope_bindings(body: &CapturedBodyV2) -> Result<(), ValidationErrorV2> {
    validate_span_scope_binding("source", &body.source, &body.source_scopes)?;
    for (index, local) in body.locals.iter().enumerate() {
        validate_span_scope_binding(
            &format!("locals[{index}].source"),
            &local.source,
            &body.source_scopes,
        )?;
    }
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (statement_index, statement) in block.statements.iter().enumerate() {
            let path = format!("blocks[{block_index}].statements[{statement_index}]");
            validate_span_scope_binding(
                &format!("{path}.source"),
                &statement.source,
                &body.source_scopes,
            )?;
            validate_statement_scope_bindings(&path, &statement.kind, &body.source_scopes)?;
        }
        let path = format!("blocks[{block_index}].terminator");
        validate_span_scope_binding(
            &format!("{path}.source"),
            &block.terminator.source,
            &body.source_scopes,
        )?;
        validate_terminator_scope_bindings(&path, &block.terminator.kind, &body.source_scopes)?;
    }
    Ok(())
}

fn validate_span_scope_binding(
    path: &str,
    span: &SourceSpanV2,
    scopes: &[SourceScopeIdentityV2],
) -> Result<(), ValidationErrorV2> {
    let scope = scopes.get(span.source_scope).ok_or_else(|| {
        ValidationErrorV2::new(
            format!("{path}.source_scope"),
            "source scope index is outside the canonical scope table",
        )
    })?;
    let inlined_hash = scope
        .inlined
        .as_ref()
        .map(|inlined| inlined.instance.instance_hash);
    if span.source_scope_hash != scope.compiler_hash
        || span.source_scope_parent != scope.parent
        || span.inlined_instance_hash != inlined_hash
        || span.source_scope_record_hash != scope.record_hash
    {
        return Err(ValidationErrorV2::new(
            format!("{path}.source_scope"),
            "span source-scope identity does not exactly match its canonical table record",
        ));
    }
    Ok(())
}

fn validate_statement_scope_bindings(
    path: &str,
    statement: &StatementKindV2,
    scopes: &[SourceScopeIdentityV2],
) -> Result<(), ValidationErrorV2> {
    match statement {
        StatementKindV2::Assign { value, .. } => {
            validate_rvalue_scope_bindings(&format!("{path}.value"), value, scopes)
        }
        StatementKindV2::Intrinsic(intrinsic) => match intrinsic.as_ref() {
            IntrinsicStatementV2::CopyNonOverlapping {
                source,
                destination,
                count,
            } => {
                validate_operand_scope_bindings(&format!("{path}.source_operand"), source, scopes)?;
                validate_operand_scope_bindings(
                    &format!("{path}.destination_operand"),
                    destination,
                    scopes,
                )?;
                validate_operand_scope_bindings(&format!("{path}.count"), count, scopes)
            }
            IntrinsicStatementV2::Assume { condition } => {
                validate_operand_scope_bindings(&format!("{path}.condition"), condition, scopes)
            }
        },
        StatementKindV2::StorageLive { .. }
        | StatementKindV2::StorageDead { .. }
        | StatementKindV2::SetDiscriminant { .. }
        | StatementKindV2::Retag { .. }
        | StatementKindV2::PlaceMention { .. }
        | StatementKindV2::Coverage { .. }
        | StatementKindV2::Nop
        | StatementKindV2::Unsupported(_) => Ok(()),
    }
}

fn validate_rvalue_scope_bindings(
    path: &str,
    value: &RvalueV2,
    scopes: &[SourceScopeIdentityV2],
) -> Result<(), ValidationErrorV2> {
    match value {
        RvalueV2::Use(operand)
        | RvalueV2::Repeat { operand, .. }
        | RvalueV2::Cast { operand, .. }
        | RvalueV2::Unary { operand, .. }
        | RvalueV2::WrapUnsafeBinder { operand, .. } => {
            validate_operand_scope_bindings(&format!("{path}.operand"), operand, scopes)
        }
        RvalueV2::Binary { lhs, rhs, .. } => {
            validate_operand_scope_bindings(&format!("{path}.lhs"), lhs, scopes)?;
            validate_operand_scope_bindings(&format!("{path}.rhs"), rhs, scopes)
        }
        RvalueV2::Aggregate { operands, .. } => {
            for (index, operand) in operands.iter().enumerate() {
                validate_operand_scope_bindings(
                    &format!("{path}.operands[{index}]"),
                    operand,
                    scopes,
                )?;
            }
            Ok(())
        }
        RvalueV2::Reference { .. }
        | RvalueV2::RawPointer { .. }
        | RvalueV2::Discriminant { .. }
        | RvalueV2::CopyForDeref(_)
        | RvalueV2::ThreadLocalRef { .. } => Ok(()),
    }
}

fn validate_operand_scope_bindings(
    path: &str,
    operand: &OperandV2,
    scopes: &[SourceScopeIdentityV2],
) -> Result<(), ValidationErrorV2> {
    match operand {
        OperandV2::Constant { source, .. } => {
            validate_span_scope_binding(&format!("{path}.source"), source, scopes)
        }
        OperandV2::Copy(_) | OperandV2::Move(_) | OperandV2::RuntimeChecks { .. } => Ok(()),
    }
}

fn validate_terminator_scope_bindings(
    path: &str,
    terminator: &TerminatorKindV2,
    scopes: &[SourceScopeIdentityV2],
) -> Result<(), ValidationErrorV2> {
    match terminator {
        TerminatorKindV2::SwitchInt { discriminant, .. } => {
            validate_operand_scope_bindings(&format!("{path}.discriminant"), discriminant, scopes)
        }
        TerminatorKindV2::Call {
            function,
            arguments,
            function_span,
            ..
        }
        | TerminatorKindV2::TailCall {
            function,
            arguments,
            function_span,
            ..
        } => {
            validate_operand_scope_bindings(&format!("{path}.function"), function, scopes)?;
            for (index, argument) in arguments.iter().enumerate() {
                validate_operand_scope_bindings(
                    &format!("{path}.arguments[{index}].operand"),
                    &argument.operand,
                    scopes,
                )?;
                validate_span_scope_binding(
                    &format!("{path}.arguments[{index}].source"),
                    &argument.source,
                    scopes,
                )?;
            }
            validate_span_scope_binding(&format!("{path}.function_span"), function_span, scopes)
        }
        TerminatorKindV2::Assert { condition, .. } => {
            validate_operand_scope_bindings(&format!("{path}.condition"), condition, scopes)
        }
        TerminatorKindV2::Yield { value, .. } => {
            validate_operand_scope_bindings(&format!("{path}.value"), value, scopes)
        }
        TerminatorKindV2::Return
        | TerminatorKindV2::Unreachable
        | TerminatorKindV2::Goto { .. }
        | TerminatorKindV2::Drop { .. }
        | TerminatorKindV2::UnwindResume
        | TerminatorKindV2::UnwindTerminate { .. }
        | TerminatorKindV2::CoroutineDrop
        | TerminatorKindV2::FalseEdge { .. }
        | TerminatorKindV2::FalseUnwind { .. }
        | TerminatorKindV2::Unsupported(_) => Ok(()),
    }
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
        } => {
            validate_definition(&format!("{path}.definition"), definition, limits)?;
            validate_hash(&format!("{path}.generic_args_hash"), generic_args_hash)
        }
        TypeClassV2::FunctionDefinition {
            definition,
            generic_args_hash,
            generic_arg_count,
        } => {
            validate_definition(&format!("{path}.definition"), definition, limits)?;
            validate_hash(&format!("{path}.generic_args_hash"), generic_args_hash)?;
            bounded(
                format!("{path}.generic_arg_count"),
                *generic_arg_count,
                limits.max_generic_args,
            )
        }
        TypeClassV2::Closure {
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
        | TypeClassV2::Tuple { arity: 0 } => Ok(()),
        TypeClassV2::Tuple { arity } => {
            bounded(format!("{path}.tuple_arity"), *arity, limits.max_type_arity)
        }
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
    locals: &[LocalDeclV2],
    block_count: usize,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    let local_count = locals.len();
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
            contract_hash,
        } => {
            validate_operand(&format!("{path}.function"), function, local_count, limits)?;
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
            validate_callee(
                &format!("{path}.callee"),
                function,
                callee,
                CallValidationContextV2 {
                    arguments,
                    destination: Some(destination),
                    target: *target,
                    unwind: Some(unwind),
                    locals,
                },
                limits,
            )?;
            validate_hash32(&format!("{path}.contract_hash"), contract_hash)?;
            let expected_contract = call_contract_hash_v2(
                function,
                callee,
                arguments,
                Some(destination),
                *target,
                Some(unwind),
            )?;
            if *contract_hash != expected_contract {
                return Err(ValidationErrorV2::new(
                    format!("{path}.contract_hash"),
                    "call contract does not match its ordered operands, destination, target, and unwind",
                ));
            }
            let expected = normal_and_unwind_successors(*target, unwind);
            validate_exact_successors(path, &terminator.successors, &expected)
        }
        TerminatorKindV2::TailCall {
            function,
            callee,
            arguments,
            function_span,
            contract_hash,
        } => {
            validate_operand(&format!("{path}.function"), function, local_count, limits)?;
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
            validate_callee(
                &format!("{path}.callee"),
                function,
                callee,
                CallValidationContextV2 {
                    arguments,
                    destination: None,
                    target: None,
                    unwind: None,
                    locals,
                },
                limits,
            )?;
            validate_hash32(&format!("{path}.contract_hash"), contract_hash)?;
            let expected_contract =
                call_contract_hash_v2(function, callee, arguments, None, None, None)?;
            if *contract_hash != expected_contract {
                return Err(ValidationErrorV2::new(
                    format!("{path}.contract_hash"),
                    "tail-call contract does not match its ordered operands",
                ));
            }
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

#[derive(Clone, Copy)]
struct CallValidationContextV2<'a> {
    arguments: &'a [CallArgumentV2],
    destination: Option<&'a PlaceV2>,
    target: Option<usize>,
    unwind: Option<&'a UnwindActionV2>,
    locals: &'a [LocalDeclV2],
}

fn validate_callee(
    path: &str,
    function: &OperandV2,
    callee: &CalleeIdentityV2,
    call: CallValidationContextV2<'_>,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    let signature = match callee {
        CalleeIdentityV2::Direct {
            declared,
            declared_generic_args_hash,
            declared_generic_arg_count,
            declared_signature,
            resolved,
            resolved_signature,
            intrinsic,
            resolution_binding_hash,
        } => {
            validate_definition(&format!("{path}.declared"), declared, limits)?;
            validate_hash(
                &format!("{path}.declared_generic_args_hash"),
                declared_generic_args_hash,
            )?;
            bounded(
                format!("{path}.declared_generic_arg_count"),
                *declared_generic_arg_count,
                limits.max_generic_args,
            )?;
            validate_signature(
                &format!("{path}.declared_signature"),
                declared_signature,
                limits,
            )?;
            validate_function_identity(resolved, limits)?;
            validate_signature(
                &format!("{path}.resolved_signature"),
                resolved_signature,
                limits,
            )?;
            let OperandV2::Constant { ty, .. } = function else {
                return Err(ValidationErrorV2::new(
                    path,
                    "a direct callee requires a constant function-definition operand",
                ));
            };
            let TypeClassV2::FunctionDefinition {
                definition,
                generic_args_hash,
                generic_arg_count,
            } = &ty.class
            else {
                return Err(ValidationErrorV2::new(
                    path,
                    "direct callee operand does not have a function-definition type",
                ));
            };
            if definition != declared
                || generic_args_hash != declared_generic_args_hash
                || generic_arg_count != declared_generic_arg_count
            {
                return Err(ValidationErrorV2::new(
                    path,
                    "declared callee identity disagrees with its operand type",
                ));
            }
            let resolved_is_intrinsic = matches!(resolved.instance.kind, InstanceKindV2::Intrinsic);
            if intrinsic.is_some() != resolved_is_intrinsic {
                return Err(ValidationErrorV2::new(
                    format!("{path}.intrinsic"),
                    "intrinsic metadata must be present if and only if the resolved instance is intrinsic",
                ));
            }
            if let Some(intrinsic) = intrinsic {
                validate_intrinsic(&format!("{path}.intrinsic"), intrinsic, limits)?;
                if intrinsic.definition != resolved.definition {
                    return Err(ValidationErrorV2::new(
                        path,
                        "intrinsic definition disagrees with the resolved instance",
                    ));
                }
            }
            validate_hash32(
                &format!("{path}.resolution_binding_hash"),
                resolution_binding_hash,
            )?;
            let expected_binding = resolution_binding_hash_v2(
                ty,
                declared,
                declared_generic_args_hash,
                *declared_generic_arg_count,
                declared_signature,
                resolved,
                resolved_signature,
                intrinsic.as_deref(),
            )?;
            if *resolution_binding_hash != expected_binding {
                return Err(ValidationErrorV2::new(
                    format!("{path}.resolution_binding_hash"),
                    "direct-call identity binding does not match its captured fields",
                ));
            }
            validate_signature_compatibility(
                &format!("{path}.resolved_signature"),
                declared_signature,
                resolved_signature,
            )?;
            declared_signature
        }
        CalleeIdentityV2::Indirect {
            callable_type,
            signature,
            callable_binding_hash,
        } => {
            validate_type(&format!("{path}.callable_type"), callable_type, limits)?;
            validate_signature(&format!("{path}.signature"), signature, limits)?;
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
            if operand_type_hash(function)? != callable_type.stable_hash {
                return Err(ValidationErrorV2::new(
                    path,
                    "indirect callable operand type does not exactly match its captured function-pointer type",
                ));
            }
            validate_hash32(
                &format!("{path}.callable_binding_hash"),
                callable_binding_hash,
            )?;
            let expected = indirect_callable_binding_hash_v2(callable_type, signature)?;
            if *callable_binding_hash != expected {
                return Err(ValidationErrorV2::new(
                    format!("{path}.callable_binding_hash"),
                    "indirect callable type and signature binding does not match",
                ));
            }
            signature
        }
    };
    validate_call_signature(
        path,
        signature,
        call.arguments,
        call.destination,
        call.target,
        call.unwind,
        call.locals,
    )
}

fn validate_signature_compatibility(
    path: &str,
    declared: &FunctionSignatureIdentityV2,
    resolved: &FunctionSignatureIdentityV2,
) -> Result<(), ValidationErrorV2> {
    let inputs_match = declared.inputs.len() == resolved.inputs.len()
        && declared
            .inputs
            .iter()
            .zip(&resolved.inputs)
            .all(|(left, right)| left.stable_hash == right.stable_hash);
    if !inputs_match
        || declared.output.stable_hash != resolved.output.stable_hash
        || declared.safety != resolved.safety
        || declared.abi != resolved.abi
        || declared.c_variadic != resolved.c_variadic
    {
        return Err(ValidationErrorV2::new(
            path,
            "resolved callable signature is not structurally compatible with the declared signature",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_call_signature(
    path: &str,
    signature: &FunctionSignatureIdentityV2,
    arguments: &[CallArgumentV2],
    destination: Option<&PlaceV2>,
    target: Option<usize>,
    unwind: Option<&UnwindActionV2>,
    locals: &[LocalDeclV2],
) -> Result<(), ValidationErrorV2> {
    if arguments.len() != signature.inputs.len() {
        return Err(ValidationErrorV2::new(
            format!("{path}.arguments"),
            format!(
                "call argument count {} does not exactly match signature input count {}",
                arguments.len(),
                signature.inputs.len()
            ),
        ));
    }
    for (index, (argument, input)) in arguments.iter().zip(&signature.inputs).enumerate() {
        if operand_type_hash(&argument.operand)? != input.stable_hash {
            return Err(ValidationErrorV2::new(
                format!("{path}.arguments[{index}]"),
                "call operand type does not exactly match the ordered signature input",
            ));
        }
    }

    match destination {
        Some(destination) => {
            if destination.type_hash != signature.output.stable_hash {
                return Err(ValidationErrorV2::new(
                    format!("{path}.destination"),
                    "call destination place type does not exactly match the signature output",
                ));
            }
            let diverges = matches!(signature.output.class, TypeClassV2::Never);
            if diverges != target.is_none() {
                return Err(ValidationErrorV2::new(
                    format!("{path}.target"),
                    "normal target presence is incompatible with the signature output type",
                ));
            }
            let _returns_unit = matches!(signature.output.class, TypeClassV2::Tuple { arity: 0 });
        }
        None => {
            let caller_output = locals.first().ok_or_else(|| {
                ValidationErrorV2::new(path, "tail call has no caller return local")
            })?;
            if signature.output.stable_hash != caller_output.ty.stable_hash {
                return Err(ValidationErrorV2::new(
                    format!("{path}.output"),
                    "tail-call output does not exactly match the caller return type",
                ));
            }
            if target.is_some() || unwind.is_some() {
                return Err(ValidationErrorV2::new(
                    path,
                    "tail call cannot carry a normal target or unwind edge",
                ));
            }
        }
    }

    if !signature.abi.unwind_allowed
        && matches!(
            unwind,
            Some(UnwindActionV2::Continue | UnwindActionV2::Cleanup { .. })
        )
    {
        return Err(ValidationErrorV2::new(
            format!("{path}.unwind"),
            "call unwind action is incompatible with a non-unwinding ABI",
        ));
    }
    Ok(())
}

fn operand_type_hash(operand: &OperandV2) -> Result<[u8; 16], ValidationErrorV2> {
    match operand {
        OperandV2::Copy(place) | OperandV2::Move(place) => Ok(place.type_hash),
        OperandV2::Constant { ty, .. } => Ok(ty.stable_hash),
        OperandV2::RuntimeChecks { .. } => Err(ValidationErrorV2::new(
            "call.operand",
            "runtime-check operands do not have an exact callable argument type",
        )),
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
    validate_hash(&format!("{path}.type_hash"), &place.type_hash)?;
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

fn validate_signature(
    path: &str,
    signature: &FunctionSignatureIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_hash(&format!("{path}.stable_hash"), &signature.stable_hash)?;
    bounded(
        format!("{path}.inputs"),
        signature.inputs.len(),
        limits.max_type_arity,
    )?;
    for (index, input) in signature.inputs.iter().enumerate() {
        validate_type(&format!("{path}.inputs[{index}]"), input, limits)?;
    }
    validate_type(&format!("{path}.output"), &signature.output, limits)?;
    validate_hash(
        &format!("{path}.abi.stable_hash"),
        &signature.abi.stable_hash,
    )?;
    validate_text(
        &format!("{path}.abi.canonical_name"),
        &signature.abi.canonical_name,
        limits,
    )?;
    let expected_unwind = signature.abi.canonical_name.ends_with("-unwind")
        || matches!(
            signature.abi.canonical_name.as_str(),
            "Rust" | "rust-call" | "rust-cold" | "rust-preserve-none"
        );
    if signature.abi.unwind_allowed != expected_unwind {
        return Err(ValidationErrorV2::new(
            format!("{path}.abi.unwind_allowed"),
            "ABI unwind capability disagrees with its canonical ABI identity",
        ));
    }
    validate_hash32(&format!("{path}.binding_hash"), &signature.binding_hash)?;
    let expected = function_signature_binding_hash_v2(signature)?;
    if signature.binding_hash != expected {
        return Err(ValidationErrorV2::new(
            format!("{path}.binding_hash"),
            "function signature structural binding does not match its components",
        ));
    }
    Ok(())
}

fn validate_intrinsic(
    path: &str,
    intrinsic: &IntrinsicIdentityV2,
    limits: CaptureLimitsV2,
) -> Result<(), ValidationErrorV2> {
    validate_definition(&format!("{path}.definition"), &intrinsic.definition, limits)?;
    validate_text(&format!("{path}.name"), &intrinsic.name, limits)?;
    validate_hash32(&format!("{path}.binding_hash"), &intrinsic.binding_hash)?;
    if intrinsic.binding_hash != intrinsic_binding_hash_v2(intrinsic)? {
        return Err(ValidationErrorV2::new(
            format!("{path}.binding_hash"),
            "intrinsic identity binding does not match its captured fields",
        ));
    }
    Ok(())
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

fn validate_hash32(path: &str, hash: &[u8; 32]) -> Result<(), ValidationErrorV2> {
    if hash.iter().all(|byte| *byte == 0) {
        return Err(ValidationErrorV2::new(
            path,
            "structural identity hash must not be the reserved zero value",
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
