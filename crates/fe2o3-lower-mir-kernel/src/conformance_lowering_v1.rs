#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConformanceSemanticOperationV1 {
    ordinal: u32,
    kind: MirSemanticOperationKind,
    identity: [u64; 4],
    provenance: MirSemanticSpanProvenance,
}

/// Pointer-independent function recipe for the V1 MIR lowering conformance facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirKernelLoweringConformanceFunctionV1 {
    identity: String,
    argument_type_ids: Vec<MirTypeId>,
    block_count: usize,
    semantic_statements: Vec<ConformanceSemanticOperationV1>,
    semantic_return: Option<ConformanceSemanticOperationV1>,
}

impl MirKernelLoweringConformanceFunctionV1 {
    /// Creates a function recipe with one canonical return block.
    pub fn new(identity: impl Into<String>, argument_type_ids: Vec<MirTypeId>) -> Self {
        Self {
            identity: identity.into(),
            argument_type_ids,
            block_count: 1,
            semantic_statements: Vec::new(),
            semantic_return: None,
        }
    }

    /// Sets the total number of canonical return blocks in this function.
    ///
    /// Zero and values outside the MIR hard bounds fail closed when the recipe
    /// is run.
    pub const fn with_block_count(mut self, block_count: usize) -> Self {
        self.block_count = block_count;
        self
    }

    /// Appends one typed rustc statement observation to the entry block.
    pub fn with_semantic_statement(
        mut self,
        ordinal: u32,
        kind: MirSemanticOperationKind,
        identity: [u64; 4],
        provenance: MirSemanticSpanProvenance,
    ) -> Self {
        self.semantic_statements
            .push(ConformanceSemanticOperationV1 {
                ordinal,
                kind,
                identity,
                provenance,
            });
        self
    }

    /// Replaces the entry block's canonical return with a typed rustc return.
    pub const fn with_semantic_return(
        mut self,
        ordinal: u32,
        identity: [u64; 4],
        provenance: MirSemanticSpanProvenance,
    ) -> Self {
        self.semantic_return = Some(ConformanceSemanticOperationV1 {
            ordinal,
            kind: MirSemanticOperationKind::TerminatorReturn,
            identity,
            provenance,
        });
        self
    }
}

/// Pointer-independent module recipe for the V1 MIR lowering conformance facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirKernelLoweringConformanceInputV1 {
    module_identity: String,
    functions: Vec<MirKernelLoweringConformanceFunctionV1>,
}

impl MirKernelLoweringConformanceInputV1 {
    /// Creates a bounded module recipe.
    ///
    /// Invalid identities, duplicate functions, and resource overflow are
    /// rejected by [`MirKernelLoweringConformanceV1::run`].
    pub fn new(
        module_identity: impl Into<String>,
        functions: Vec<MirKernelLoweringConformanceFunctionV1>,
    ) -> Self {
        Self {
            module_identity: module_identity.into(),
            functions,
        }
    }
}

/// Versioned pointer-independent facade for MIR lowering conformance tests.
///
/// The facade owns and destroys its Pliron context on every invocation. It
/// exposes only deterministic configuration and lowering records; it never
/// exposes contextless arena pointers or registration hooks. This is not a
/// production compilation or authority-granting API.
///
/// The retired raw service and registration hook are intentionally absent:
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::MirKernelLoweringPass;
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::register_pass;
/// ```
///
/// Lowering observations cannot be converted back into arena capabilities:
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::LoweringResult;
/// fn contextless_pointer(result: &LoweringResult) {
///     let _ = result.source_root();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::LoweringResult;
/// fn contextless_outputs(result: &LoweringResult) {
///     let _ = result.operations();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::LoweringResult;
/// fn context_injection(result: &LoweringResult, context: &pliron::context::Context) {
///     let _ = result.validate(context);
/// }
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MirKernelLoweringConformanceV1;

impl MirKernelLoweringConformanceV1 {
    /// Runs a recipe in a fresh private owner context and returns its stable observation.
    pub fn run(
        self,
        input: &MirKernelLoweringConformanceInputV1,
        config: LoweringConfig,
    ) -> Result<LoweringResult, LoweringError> {
        let mut context = Context::new();
        register_pass(&mut context).map_err(|_| LoweringError::RegistrationCorrupt)?;
        let source = materialize_conformance_input_v1(&mut context, input)?;
        let mut service = MirKernelLoweringPass::new(config);
        let result = service.run_checked(source, &mut context)?;
        Ok(result.observation.clone())
    }
}

/// Bounded target-neutral detached MIR-to-kernel lowering service.
///
#[derive(Clone, Debug)]
struct MirKernelLoweringPass {
    config: LoweringConfig,
    last_result: Option<MaterializedLoweringResult>,
}

impl MirKernelLoweringPass {
    const fn new(config: LoweringConfig) -> Self {
        Self {
            config,
            last_result: None,
        }
    }

    #[cfg(test)]
    fn take_result(&mut self) -> Option<MaterializedLoweringResult> {
        self.last_result.take()
    }

    fn run_checked(
        &mut self,
        source: Ptr<Operation>,
        context: &mut Context,
    ) -> Result<&MaterializedLoweringResult, LoweringError> {
        self.last_result = None;
        let context_identity = require_registration(context)?;
        let source_evidence = inspect_source(context, source, &self.config)?;
        let steps = build_steps(&source_evidence, &self.config)?;
        let operations = materialize_steps(context, &steps)?;
        let result = MaterializedLoweringResult {
            source_root: source,
            observation: LoweringResult {
                config: self.config.clone(),
                record: LoweringRecord {
                    source: source_evidence,
                    steps,
                },
            },
            operations,
            context_identity,
        };
        result
            .validate(context)
            .map_err(LoweringError::Postcondition)?;
        Ok(self.last_result.insert(result))
    }
}

fn materialize_conformance_input_v1(
    context: &mut Context,
    input: &MirKernelLoweringConformanceInputV1,
) -> Result<Ptr<Operation>, LoweringError> {
    let module = MirModuleOp::try_new(
        context,
        input.module_identity.clone(),
        dialect_mir::pliron::MirDialectLimits::default(),
    )
    .map_err(|_| LoweringError::SourceVerificationFailed)?;
    for recipe in &input.functions {
        if recipe.block_count == 0 {
            return Err(LoweringError::SourceVerificationFailed);
        }
        let function = module
            .append_function(context, recipe.identity.clone(), &recipe.argument_type_ids)
            .map_err(|_| LoweringError::SourceVerificationFailed)?;
        let entry = function
            .entry_block(context)
            .map_err(|_| LoweringError::SourceVerificationFailed)?;
        for statement in &recipe.semantic_statements {
            entry
                .append_semantic_statement(
                    context,
                    statement.ordinal,
                    statement.kind,
                    statement.identity,
                    statement.provenance,
                )
                .map_err(|_| LoweringError::SourceVerificationFailed)?;
        }
        if let Some(terminator) = recipe.semantic_return {
            entry
                .replace_with_semantic_terminator(
                    context,
                    terminator.ordinal,
                    terminator.kind,
                    terminator.identity,
                    terminator.provenance,
                    &[],
                )
                .map_err(|_| LoweringError::SourceVerificationFailed)?;
        }
        for _ in 1..recipe.block_count {
            function
                .append_block(context)
                .map_err(|_| LoweringError::SourceVerificationFailed)?;
        }
    }
    Ok(module.get_operation())
}
