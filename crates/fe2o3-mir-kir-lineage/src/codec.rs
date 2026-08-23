use std::fmt;

use crate::model::{
    BlockClassificationV4, BlockRecordV4, CanonicalKernelIrIdentityV4,
    CanonicalSemanticMirIdentityV4, CheckedArithmeticRefinementPolicyV4,
    CorrespondenceValidationPolicyV4, DiagnosticTrapDeclarationPolicyV4, DiagnosticTrapKindV4,
    F32IntrinsicDeclarationPolicyV4, F32IntrinsicV4, FunctionClassificationV4, FunctionRecordV4,
    KernelIrCanonicalWireVersionV4, KernelIrIdentitySchemeV4, KernelRecordV4, LineageModelV4,
    LineagePolicyModeV4, LineageResourceV4, LineageTotalsV4, LineageValidationErrorV4,
    LineageWorkStageV4, LineageWorkV4, LoweringConfigurationV4, LoweringResourceLimitsV4,
    LoweringTargetV4, RankedBoundsPolicyV4, SemanticMirCanonicalWireVersionV4,
    SemanticMirIdentitySchemeV4, SyntheticBlockRuleV4, WorkBudgetErrorV4, WorkBudgetV4,
};

/// Canonical V4 MIR-to-KIR lineage magic.
pub const MIR_TO_KIR_LINEAGE_MAGIC_V4: [u8; 8] = *b"FE2O3L4\0";
/// Canonical MIR-to-KIR lineage wire version.
pub const MIR_TO_KIR_LINEAGE_VERSION_V4: u64 = 4;
/// Absolute canonical V4 lineage byte limit, independent of caller policy.
pub const MAX_LINEAGE_BYTES_V4: u64 = 4 * 1024 * 1024;

const FLAGS_V4: u64 = 0;

/// Independent admission limits for canonical V4 decoding.
///
/// These limits protect the parser and are distinct from the lowering limits
/// retained as lineage content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineageDecodeLimitsV4 {
    max_input_bytes: u64,
    max_semantic_functions: u64,
    max_kir_functions: u64,
    max_kernels: u64,
    max_blocks: u64,
    max_statements: u64,
    max_operations: u64,
    max_work: u64,
}

impl LineageDecodeLimitsV4 {
    /// Constructs explicit parser admission limits.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        max_input_bytes: u64,
        max_semantic_functions: u64,
        max_kir_functions: u64,
        max_kernels: u64,
        max_blocks: u64,
        max_statements: u64,
        max_operations: u64,
        max_work: u64,
    ) -> Self {
        Self {
            max_input_bytes: if max_input_bytes < MAX_LINEAGE_BYTES_V4 {
                max_input_bytes
            } else {
                MAX_LINEAGE_BYTES_V4
            },
            max_semantic_functions,
            max_kir_functions,
            max_kernels,
            max_blocks,
            max_statements,
            max_operations,
            max_work,
        }
    }

    /// Returns the effective maximum input length after applying the hard cap.
    pub const fn max_input_bytes(self) -> u64 {
        self.max_input_bytes
    }

    /// Returns the maximum accepted validation work.
    pub const fn max_work(self) -> u64 {
        self.max_work
    }

    const fn count_limit(self, resource: LineageResourceV4) -> u64 {
        match resource {
            LineageResourceV4::SemanticFunctions => self.max_semantic_functions,
            LineageResourceV4::KirFunctions => self.max_kir_functions,
            LineageResourceV4::Kernels => self.max_kernels,
            LineageResourceV4::Blocks => self.max_blocks,
            LineageResourceV4::Statements => self.max_statements,
            LineageResourceV4::Operations => self.max_operations,
            LineageResourceV4::Work => self.max_work,
        }
    }

    fn admit_encode_totals(self, totals: LineageTotalsV4) -> Result<(), LineageEncodeErrorV4> {
        let blocks = totals
            .total_blocks()
            .ok_or(LineageEncodeErrorV4::CountOverflow {
                resource: LineageResourceV4::Blocks,
            })?;
        for (resource, actual) in [
            (
                LineageResourceV4::SemanticFunctions,
                totals.semantic_functions(),
            ),
            (LineageResourceV4::KirFunctions, totals.kir_functions()),
            (LineageResourceV4::Kernels, totals.kernels()),
            (LineageResourceV4::Blocks, blocks),
            (LineageResourceV4::Statements, totals.statements()),
            (LineageResourceV4::Operations, totals.operations()),
        ] {
            let max = self.count_limit(resource);
            if actual > max {
                return Err(LineageEncodeErrorV4::CountLimitExceeded {
                    resource,
                    actual,
                    max,
                });
            }
        }
        Ok(())
    }
}

impl Default for LineageDecodeLimitsV4 {
    fn default() -> Self {
        Self::new(
            4 * 1024 * 1024,
            1_024,
            2_048,
            1_024,
            16_384,
            1_048_576,
            4_194_304,
            16_777_216,
        )
    }
}

/// Exact canonical, bounded, inert V4 MIR-to-KIR lineage bytes and model.
///
/// Anyone can construct, copy, or decode this value. It deliberately grants no
/// compiler, verifier, proof, artifact, publication, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertCanonicalMirToKirLineageV4 {
    canonical_bytes: Vec<u8>,
    model: LineageModelV4,
    admission_work: LineageWorkV4,
}

impl InertCanonicalMirToKirLineageV4 {
    /// Canonicalizes a structurally valid inert model under parser limits.
    pub fn from_model(
        model: LineageModelV4,
        limits: LineageDecodeLimitsV4,
    ) -> Result<Self, LineageEncodeErrorV4> {
        limits.admit_encode_totals(model.totals())?;
        let mut budget = WorkBudgetV4::new(limits.max_work);
        model
            .revalidate_with_budget(&mut budget)
            .map_err(map_validation_encode_error)?;
        let bytes = encode_model(&model, limits.max_input_bytes, &mut budget)?;
        Ok(Self {
            canonical_bytes: bytes,
            model,
            admission_work: budget.work(),
        })
    }

    /// Decodes one complete, shortest-form, bounded canonical V4 encoding.
    ///
    /// Counts and validation work are charged before record loops. Parsed
    /// allocations grow fallibly one admitted record at a time. The accepted
    /// model is re-encoded and must reproduce the input byte-for-byte.
    pub fn decode_canonical(
        bytes: &[u8],
        limits: LineageDecodeLimitsV4,
    ) -> Result<Self, LineageDecodeErrorV4> {
        let actual =
            u64::try_from(bytes.len()).map_err(|_| LineageDecodeErrorV4::InputLengthOverflow)?;
        if actual > MAX_LINEAGE_BYTES_V4 {
            return Err(LineageDecodeErrorV4::InputLimitExceeded {
                actual,
                max: MAX_LINEAGE_BYTES_V4,
            });
        }
        if actual > limits.max_input_bytes {
            return Err(LineageDecodeErrorV4::InputLimitExceeded {
                actual,
                max: limits.max_input_bytes,
            });
        }

        let mut budget = WorkBudgetV4::new(limits.max_work);
        let mut reader = ReaderV4::new(bytes, limits, &mut budget);
        if reader.raw(MIR_TO_KIR_LINEAGE_MAGIC_V4.len())? != MIR_TO_KIR_LINEAGE_MAGIC_V4 {
            return Err(LineageDecodeErrorV4::InvalidMagic);
        }
        let version = reader.varint()?;
        if version != MIR_TO_KIR_LINEAGE_VERSION_V4 {
            return Err(LineageDecodeErrorV4::UnsupportedVersion(version));
        }
        let flags = reader.varint()?;
        if flags != FLAGS_V4 {
            return Err(LineageDecodeErrorV4::UnsupportedFlags(flags));
        }

        let semantic_scheme = match reader.tag("semantic MIR identity scheme")? {
            0 => SemanticMirIdentitySchemeV4::RawCanonicalSha256,
            value => return Err(reader.invalid_tag("semantic MIR identity scheme", value)),
        };
        let semantic_version = match reader.tag("semantic MIR canonical wire version")? {
            2 => SemanticMirCanonicalWireVersionV4::V2,
            3 => SemanticMirCanonicalWireVersionV4::V3,
            value => {
                return Err(reader.invalid_tag("semantic MIR canonical wire version", value));
            }
        };
        let semantic_mir = CanonicalSemanticMirIdentityV4::new(
            semantic_version,
            reader.array::<32>()?,
            reader.varint()?,
        )?;
        debug_assert_eq!(semantic_mir.scheme(), semantic_scheme);

        let kernel_ir_scheme = match reader.tag("Kernel IR identity scheme")? {
            0 => KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV5Sha256PolicyV1,
            1 => KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV6Sha256PolicyV1,
            value => return Err(reader.invalid_tag("Kernel IR identity scheme", value)),
        };
        let kernel_ir_version = match reader.tag("Kernel IR canonical wire version")? {
            5 => KernelIrCanonicalWireVersionV4::V5,
            6 => KernelIrCanonicalWireVersionV4::V6,
            value => return Err(reader.invalid_tag("Kernel IR canonical wire version", value)),
        };
        let kernel_ir = CanonicalKernelIrIdentityV4::from_wire(
            kernel_ir_scheme,
            kernel_ir_version,
            reader.array::<32>()?,
            reader.varint()?,
        )?;
        let configuration = decode_configuration(&mut reader)?;
        let declared_totals = decode_totals(&mut reader)?;
        reader.admit_declared_totals(declared_totals)?;

        let function_count =
            reader.count_to_usize("Kernel IR functions", declared_totals.kir_functions())?;
        reader.require_minimum_remaining("Kernel IR functions", function_count)?;
        let mut functions = Vec::new();
        for _ in 0..function_count {
            let value = decode_function(&mut reader, declared_totals)?;
            push_fallible(&mut functions, value, "Kernel IR functions")?;
        }

        let kernel_count = reader.count_to_usize("kernels", declared_totals.kernels())?;
        reader.require_minimum_remaining("kernels", kernel_count)?;
        let mut kernels = Vec::new();
        for _ in 0..kernel_count {
            let value = KernelRecordV4::new(reader.varint()?, reader.varint()?, reader.varint()?);
            push_fallible(&mut kernels, value, "kernels")?;
        }
        reader.finish()?;
        reader.verify_observed_totals(declared_totals)?;

        let model = LineageModelV4::from_wire_with_budget(
            semantic_mir,
            kernel_ir,
            configuration,
            declared_totals,
            functions,
            kernels,
            &mut budget,
        )
        .map_err(map_validation_decode_error)?;

        let reencoded = encode_model(&model, limits.max_input_bytes, &mut budget)
            .map_err(map_reencode_error)?;
        if reencoded != bytes {
            return Err(LineageDecodeErrorV4::NonCanonicalReencoding);
        }
        Ok(Self {
            canonical_bytes: reencoded,
            model,
            admission_work: budget.work(),
        })
    }

    /// Returns the exact retained canonical encoding.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the decoded inert data model.
    pub const fn model(&self) -> &LineageModelV4 {
        &self.model
    }

    /// Returns exact work charged by this construction or decode operation.
    pub const fn admission_work(&self) -> LineageWorkV4 {
        self.admission_work
    }

    /// Repeats strict decoding and exact model comparison.
    pub fn revalidate(&self, limits: LineageDecodeLimitsV4) -> Result<(), LineageDecodeErrorV4> {
        let decoded = Self::decode_canonical(&self.canonical_bytes, limits)?;
        if decoded.model != self.model {
            return Err(LineageDecodeErrorV4::NonCanonicalReencoding);
        }
        Ok(())
    }
}

fn decode_configuration(
    reader: &mut ReaderV4<'_, '_>,
) -> Result<LoweringConfigurationV4, LineageDecodeErrorV4> {
    let policy_version = reader.varint()?;
    let mode = match reader.tag("lineage policy mode")? {
        0 => LineagePolicyModeV4::ProductionSemanticMirV3ToKernelIrV6,
        1 => LineagePolicyModeV4::LegacyInertSemanticMirV2ToKernelIrV5,
        value => return Err(reader.invalid_tag("lineage policy mode", value)),
    };
    let target = match reader.tag("lowering target")? {
        0 => LoweringTargetV4::AmdGpuGfx942,
        value => return Err(reader.invalid_tag("lowering target", value)),
    };
    let ranked_bounds = match reader.tag("ranked-bounds policy")? {
        0 => RankedBoundsPolicyV4::RetainGenericChecks,
        1 => RankedBoundsPolicyV4::DischargeWithValidatedRankedInput,
        value => return Err(reader.invalid_tag("ranked-bounds policy", value)),
    };
    let f32_intrinsics = match reader.tag("f32 declaration policy")? {
        0 => F32IntrinsicDeclarationPolicyV4::DeclareReferencedIntrinsics,
        value => return Err(reader.invalid_tag("f32 declaration policy", value)),
    };
    let diagnostic_traps = match reader.tag("diagnostic declaration policy")? {
        0 => DiagnosticTrapDeclarationPolicyV4::DeclareReferencedTraps,
        value => return Err(reader.invalid_tag("diagnostic declaration policy", value)),
    };
    let correspondence = match reader.tag("correspondence policy")? {
        0 => CorrespondenceValidationPolicyV4::ExhaustiveTypedTraversal,
        value => return Err(reader.invalid_tag("correspondence policy", value)),
    };
    let checked_arithmetic = match reader.tag("checked-arithmetic refinement policy")? {
        0 => CheckedArithmeticRefinementPolicyV4::SemanticMirV3ToKernelIrV6CheckedV1,
        1 => CheckedArithmeticRefinementPolicyV4::LegacyInertNoRefinementAuthority,
        value => {
            return Err(reader.invalid_tag("checked-arithmetic refinement policy", value));
        }
    };
    let limits = LoweringResourceLimitsV4::from_wire_unvalidated(
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
    );
    LoweringConfigurationV4::from_wire(
        policy_version,
        mode,
        target,
        ranked_bounds,
        f32_intrinsics,
        diagnostic_traps,
        correspondence,
        checked_arithmetic,
        limits,
    )
    .map_err(Into::into)
}

fn decode_totals(reader: &mut ReaderV4<'_, '_>) -> Result<LineageTotalsV4, LineageDecodeErrorV4> {
    Ok(LineageTotalsV4::from_wire([
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
        reader.varint()?,
    ]))
}

fn decode_function(
    reader: &mut ReaderV4<'_, '_>,
    declared_totals: LineageTotalsV4,
) -> Result<FunctionRecordV4, LineageDecodeErrorV4> {
    let kir_function_ordinal = reader.varint()?;
    match reader.tag("function classification")? {
        0 => {
            let semantic_function_ordinal = reader.varint()?;
            let semantic_block_count = reader.varint()?;
            let block_count = reader.varint()?;
            reader.charge_semantic_function(semantic_block_count, declared_totals)?;
            reader.charge_blocks(block_count, declared_totals)?;
            let block_count = reader.count_to_usize("function blocks", block_count)?;
            reader.require_minimum_remaining("function blocks", block_count)?;
            let mut blocks = Vec::new();
            for _ in 0..block_count {
                let value = decode_block(reader, declared_totals)?;
                push_fallible(&mut blocks, value, "function blocks")?;
            }
            Ok(FunctionRecordV4::semantic_body(
                kir_function_ordinal,
                semantic_function_ordinal,
                semantic_block_count,
                blocks,
            ))
        }
        1 => Ok(FunctionRecordV4::f32_intrinsic_declaration(
            kir_function_ordinal,
            decode_f32_intrinsic(reader)?,
        )),
        2 => Ok(FunctionRecordV4::diagnostic_trap_declaration(
            kir_function_ordinal,
            decode_diagnostic_trap(reader)?,
        )),
        value => Err(reader.invalid_tag("function classification", value)),
    }
}

fn decode_block(
    reader: &mut ReaderV4<'_, '_>,
    declared_totals: LineageTotalsV4,
) -> Result<BlockRecordV4, LineageDecodeErrorV4> {
    let kir_block_ordinal = reader.varint()?;
    let operation_count = reader.varint()?;
    reader.charge_operations(operation_count, declared_totals)?;
    match reader.tag("block classification")? {
        0 => {
            reader.charge_semantic_block()?;
            let semantic_block_ordinal = reader.varint()?;
            let statement_count = reader.varint()?;
            reader.charge_statements(statement_count, declared_totals)?;
            let statement_count = reader.count_to_usize("statement spans", statement_count)?;
            reader.require_minimum_remaining("statement spans", statement_count)?;
            let mut statements = Vec::new();
            for _ in 0..statement_count {
                let value = reader.varint()?;
                push_fallible(&mut statements, value, "statement spans")?;
            }
            let terminator_operation_count = reader.varint()?;
            Ok(BlockRecordV4::semantic_from_wire_unchecked(
                kir_block_ordinal,
                operation_count,
                semantic_block_ordinal,
                statements,
                terminator_operation_count,
            ))
        }
        1 => {
            reader.charge_synthetic_block()?;
            let rule = match reader.tag("synthetic block rule")? {
                0 => SyntheticBlockRuleV4::RuntimeAssertFailureTrap,
                value => return Err(reader.invalid_tag("synthetic block rule", value)),
            };
            Ok(BlockRecordV4::synthetic_from_wire_unchecked(
                kir_block_ordinal,
                operation_count,
                rule,
            ))
        }
        value => Err(reader.invalid_tag("block classification", value)),
    }
}

fn decode_f32_intrinsic(
    reader: &mut ReaderV4<'_, '_>,
) -> Result<F32IntrinsicV4, LineageDecodeErrorV4> {
    Ok(match reader.tag("f32 intrinsic")? {
        0 => F32IntrinsicV4::Sqrt,
        1 => F32IntrinsicV4::FusedMultiplyAdd,
        2 => F32IntrinsicV4::Floor,
        3 => F32IntrinsicV4::Ceil,
        4 => F32IntrinsicV4::Truncate,
        5 => F32IntrinsicV4::RoundTiesEven,
        6 => F32IntrinsicV4::Sin,
        7 => F32IntrinsicV4::Cos,
        8 => F32IntrinsicV4::Exp,
        9 => F32IntrinsicV4::Exp2,
        10 => F32IntrinsicV4::Ln,
        11 => F32IntrinsicV4::Log2,
        12 => F32IntrinsicV4::Log10,
        value => return Err(reader.invalid_tag("f32 intrinsic", value)),
    })
}

fn decode_diagnostic_trap(
    reader: &mut ReaderV4<'_, '_>,
) -> Result<DiagnosticTrapKindV4, LineageDecodeErrorV4> {
    match reader.tag("diagnostic trap")? {
        0 => Ok(DiagnosticTrapKindV4::RuntimeAssertFailure),
        value => Err(reader.invalid_tag("diagnostic trap", value)),
    }
}

fn encode_model(
    model: &LineageModelV4,
    max_input_bytes: u64,
    budget: &mut WorkBudgetV4,
) -> Result<Vec<u8>, LineageEncodeErrorV4> {
    let mut writer = WriterV4::new(max_input_bytes, budget);
    writer.raw(&MIR_TO_KIR_LINEAGE_MAGIC_V4)?;
    writer.varint(MIR_TO_KIR_LINEAGE_VERSION_V4)?;
    writer.varint(FLAGS_V4)?;
    encode_semantic_identity(&mut writer, model.semantic_mir())?;
    encode_kernel_ir_identity(&mut writer, model.kernel_ir())?;
    encode_configuration(&mut writer, model.configuration())?;
    encode_totals(&mut writer, model.totals())?;
    for function in model.functions() {
        encode_function(&mut writer, function)?;
    }
    for kernel in model.kernels() {
        writer.varint(kernel.kernel_ordinal())?;
        writer.varint(kernel.semantic_function_ordinal())?;
        writer.varint(kernel.kir_function_ordinal())?;
    }
    Ok(writer.finish())
}

fn encode_semantic_identity(
    writer: &mut WriterV4<'_>,
    identity: CanonicalSemanticMirIdentityV4,
) -> Result<(), LineageEncodeErrorV4> {
    writer.varint(match identity.scheme() {
        SemanticMirIdentitySchemeV4::RawCanonicalSha256 => 0,
    })?;
    writer.varint(identity.wire_version() as u64)?;
    writer.raw(&identity.raw_canonical_sha256())?;
    writer.varint(identity.canonical_length())
}

fn encode_kernel_ir_identity(
    writer: &mut WriterV4<'_>,
    identity: CanonicalKernelIrIdentityV4,
) -> Result<(), LineageEncodeErrorV4> {
    writer.varint(match identity.scheme() {
        KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV5Sha256PolicyV1 => 0,
        KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV6Sha256PolicyV1 => 1,
    })?;
    writer.varint(identity.wire_version() as u64)?;
    writer.raw(&identity.scheme_digest())?;
    writer.varint(identity.canonical_length())
}

fn encode_configuration(
    writer: &mut WriterV4<'_>,
    configuration: LoweringConfigurationV4,
) -> Result<(), LineageEncodeErrorV4> {
    writer.varint(configuration.policy_version())?;
    writer.varint(match configuration.mode() {
        LineagePolicyModeV4::ProductionSemanticMirV3ToKernelIrV6 => 0,
        LineagePolicyModeV4::LegacyInertSemanticMirV2ToKernelIrV5 => 1,
    })?;
    writer.varint(match configuration.target() {
        LoweringTargetV4::AmdGpuGfx942 => 0,
    })?;
    writer.varint(match configuration.ranked_bounds() {
        RankedBoundsPolicyV4::RetainGenericChecks => 0,
        RankedBoundsPolicyV4::DischargeWithValidatedRankedInput => 1,
    })?;
    writer.varint(match configuration.f32_intrinsics() {
        F32IntrinsicDeclarationPolicyV4::DeclareReferencedIntrinsics => 0,
    })?;
    writer.varint(match configuration.diagnostic_traps() {
        DiagnosticTrapDeclarationPolicyV4::DeclareReferencedTraps => 0,
    })?;
    writer.varint(match configuration.correspondence() {
        CorrespondenceValidationPolicyV4::ExhaustiveTypedTraversal => 0,
    })?;
    writer.varint(match configuration.checked_arithmetic() {
        CheckedArithmeticRefinementPolicyV4::SemanticMirV3ToKernelIrV6CheckedV1 => 0,
        CheckedArithmeticRefinementPolicyV4::LegacyInertNoRefinementAuthority => 1,
    })?;
    for resource in [
        LineageResourceV4::SemanticFunctions,
        LineageResourceV4::KirFunctions,
        LineageResourceV4::Kernels,
        LineageResourceV4::Blocks,
        LineageResourceV4::Statements,
        LineageResourceV4::Operations,
        LineageResourceV4::Work,
    ] {
        writer.varint(configuration.limits().limit(resource))?;
    }
    Ok(())
}

fn encode_totals(
    writer: &mut WriterV4<'_>,
    totals: LineageTotalsV4,
) -> Result<(), LineageEncodeErrorV4> {
    for value in [
        totals.semantic_functions(),
        totals.kir_functions(),
        totals.kernels(),
        totals.semantic_blocks(),
        totals.synthetic_blocks(),
        totals.statements(),
        totals.terminators(),
        totals.operations(),
    ] {
        writer.varint(value)?;
    }
    Ok(())
}

fn encode_function(
    writer: &mut WriterV4<'_>,
    function: &FunctionRecordV4,
) -> Result<(), LineageEncodeErrorV4> {
    writer.varint(function.kir_function_ordinal())?;
    match function.classification() {
        FunctionClassificationV4::SemanticBody {
            semantic_function_ordinal,
            semantic_block_count,
        } => {
            writer.varint(0)?;
            writer.varint(semantic_function_ordinal)?;
            writer.varint(semantic_block_count)?;
            writer.varint(length_u64("function blocks", function.blocks().len())?)?;
            for block in function.blocks() {
                encode_block(writer, block)?;
            }
        }
        FunctionClassificationV4::F32IntrinsicDeclaration(intrinsic) => {
            writer.varint(1)?;
            writer.varint(intrinsic as u64)?;
        }
        FunctionClassificationV4::DiagnosticTrapDeclaration(trap) => {
            writer.varint(2)?;
            writer.varint(match trap {
                DiagnosticTrapKindV4::RuntimeAssertFailure => 0,
            })?;
        }
    }
    Ok(())
}

fn encode_block(
    writer: &mut WriterV4<'_>,
    block: &BlockRecordV4,
) -> Result<(), LineageEncodeErrorV4> {
    writer.varint(block.kir_block_ordinal())?;
    writer.varint(block.operation_count())?;
    match block.classification() {
        BlockClassificationV4::SemanticBlock {
            semantic_block_ordinal,
            statement_operation_counts,
            terminator_operation_count,
        } => {
            writer.varint(0)?;
            writer.varint(*semantic_block_ordinal)?;
            writer.varint(length_u64(
                "statement spans",
                statement_operation_counts.len(),
            )?)?;
            for operation_count in statement_operation_counts {
                writer.varint(*operation_count)?;
            }
            writer.varint(*terminator_operation_count)?;
        }
        BlockClassificationV4::SyntheticBlock { rule } => {
            writer.varint(1)?;
            writer.varint(match rule {
                SyntheticBlockRuleV4::RuntimeAssertFailureTrap => 0,
            })?;
        }
    }
    Ok(())
}

fn length_u64(context: &'static str, length: usize) -> Result<u64, LineageEncodeErrorV4> {
    u64::try_from(length).map_err(|_| LineageEncodeErrorV4::LengthOverflow { context })
}

fn map_reencode_error(error: LineageEncodeErrorV4) -> LineageDecodeErrorV4 {
    match error {
        LineageEncodeErrorV4::Validation(error) => LineageDecodeErrorV4::Validation(error),
        LineageEncodeErrorV4::CountOverflow { resource } => {
            LineageDecodeErrorV4::CountOverflow { resource }
        }
        LineageEncodeErrorV4::CountLimitExceeded {
            resource,
            actual,
            max,
        } => LineageDecodeErrorV4::CountLimitExceeded {
            resource,
            actual,
            max,
        },
        LineageEncodeErrorV4::LengthOverflow { context } => {
            LineageDecodeErrorV4::LengthOverflow { context }
        }
        LineageEncodeErrorV4::OutputLimitExceeded { actual, max } => {
            LineageDecodeErrorV4::InputLimitExceeded { actual, max }
        }
        LineageEncodeErrorV4::AllocationFailed => LineageDecodeErrorV4::AllocationFailed {
            context: "canonical re-encoding",
        },
        LineageEncodeErrorV4::WorkOverflow { stage } => {
            LineageDecodeErrorV4::WorkOverflow { stage }
        }
        LineageEncodeErrorV4::WorkLimitExceeded { stage, actual, max } => {
            LineageDecodeErrorV4::WorkLimitExceeded { stage, actual, max }
        }
    }
}

fn map_validation_encode_error(error: LineageValidationErrorV4) -> LineageEncodeErrorV4 {
    match error {
        LineageValidationErrorV4::WorkOverflow { stage } => {
            LineageEncodeErrorV4::WorkOverflow { stage }
        }
        LineageValidationErrorV4::WorkLimitExceeded { stage, actual, max } => {
            LineageEncodeErrorV4::WorkLimitExceeded { stage, actual, max }
        }
        error => LineageEncodeErrorV4::Validation(error),
    }
}

fn map_validation_decode_error(error: LineageValidationErrorV4) -> LineageDecodeErrorV4 {
    match error {
        LineageValidationErrorV4::WorkOverflow { stage } => {
            LineageDecodeErrorV4::WorkOverflow { stage }
        }
        LineageValidationErrorV4::WorkLimitExceeded { stage, actual, max } => {
            LineageDecodeErrorV4::WorkLimitExceeded { stage, actual, max }
        }
        error => LineageDecodeErrorV4::Validation(error),
    }
}

fn push_fallible<T>(
    values: &mut Vec<T>,
    value: T,
    context: &'static str,
) -> Result<(), LineageDecodeErrorV4> {
    values
        .try_reserve(1)
        .map_err(|_| LineageDecodeErrorV4::AllocationFailed { context })?;
    values.push(value);
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ObservedTotalsV4 {
    semantic_functions: u64,
    declared_semantic_blocks: u64,
    blocks: u64,
    semantic_blocks: u64,
    synthetic_blocks: u64,
    statements: u64,
    operations: u64,
}

struct ReaderV4<'a, 'budget> {
    bytes: &'a [u8],
    offset: usize,
    limits: LineageDecodeLimitsV4,
    budget: &'budget mut WorkBudgetV4,
    observed: ObservedTotalsV4,
    last_tag_offset: usize,
}

impl<'a, 'budget> ReaderV4<'a, 'budget> {
    const fn new(
        bytes: &'a [u8],
        limits: LineageDecodeLimitsV4,
        budget: &'budget mut WorkBudgetV4,
    ) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
            budget,
            observed: ObservedTotalsV4 {
                semantic_functions: 0,
                declared_semantic_blocks: 0,
                blocks: 0,
                semantic_blocks: 0,
                synthetic_blocks: 0,
                statements: 0,
                operations: 0,
            },
            last_tag_offset: 0,
        }
    }

    fn raw(&mut self, length: usize) -> Result<&'a [u8], LineageDecodeErrorV4> {
        self.charge_parse_work(
            u64::try_from(length)
                .map_err(|_| LineageDecodeErrorV4::LengthOverflow { context: "field" })?,
        )?;
        let end = self
            .offset
            .checked_add(length)
            .ok_or(LineageDecodeErrorV4::LengthOverflow { context: "field" })?;
        let Some(value) = self.bytes.get(self.offset..end) else {
            return Err(LineageDecodeErrorV4::UnexpectedEnd {
                offset: self.offset,
                requested: length,
            });
        };
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], LineageDecodeErrorV4> {
        let offset = self.offset;
        self.raw(N)?
            .try_into()
            .map_err(|_| LineageDecodeErrorV4::UnexpectedEnd {
                offset,
                requested: N,
            })
    }

    fn varint(&mut self) -> Result<u64, LineageDecodeErrorV4> {
        let start = self.offset;
        let mut value = 0_u64;
        for index in 0_u32..10 {
            let byte = self.raw(1)?[0];
            if index == 9 && byte > 1 {
                return Err(LineageDecodeErrorV4::VarintOverflow { offset: start });
            }
            value |= u64::from(byte & 0x7f) << (index * 7);
            if byte & 0x80 == 0 {
                let actual_length = self.offset - start;
                if actual_length != usize::from(varint_length(value)) {
                    return Err(LineageDecodeErrorV4::NonShortestVarint { offset: start });
                }
                return Ok(value);
            }
        }
        Err(LineageDecodeErrorV4::VarintOverflow { offset: start })
    }

    fn tag(&mut self, _context: &'static str) -> Result<u64, LineageDecodeErrorV4> {
        self.last_tag_offset = self.offset;
        self.varint()
    }

    const fn invalid_tag(&self, context: &'static str, value: u64) -> LineageDecodeErrorV4 {
        LineageDecodeErrorV4::InvalidTag {
            context,
            offset: self.last_tag_offset,
            value,
        }
    }

    fn admit_declared_totals(
        &mut self,
        totals: LineageTotalsV4,
    ) -> Result<(), LineageDecodeErrorV4> {
        let blocks = totals
            .total_blocks()
            .ok_or(LineageDecodeErrorV4::CountOverflow {
                resource: LineageResourceV4::Blocks,
            })?;
        for (resource, actual) in [
            (
                LineageResourceV4::SemanticFunctions,
                totals.semantic_functions(),
            ),
            (LineageResourceV4::KirFunctions, totals.kir_functions()),
            (LineageResourceV4::Kernels, totals.kernels()),
            (LineageResourceV4::Blocks, blocks),
            (LineageResourceV4::Statements, totals.statements()),
            (LineageResourceV4::Operations, totals.operations()),
        ] {
            self.require_count(resource, actual)?;
        }
        if totals.terminators() > totals.semantic_blocks() {
            return Err(LineageDecodeErrorV4::CountMismatch {
                context: "semantic terminators",
                declared: totals.semantic_blocks(),
                observed: totals.terminators(),
            });
        }
        let record_work = totals
            .kir_functions()
            .checked_add(totals.kernels())
            .and_then(|value| value.checked_add(blocks))
            .and_then(|value| value.checked_add(totals.statements()))
            .and_then(|value| value.checked_add(totals.terminators()))
            .ok_or(LineageDecodeErrorV4::WorkOverflow {
                stage: LineageWorkStageV4::Parse,
            })?;
        self.charge_parse_work(record_work)?;
        Ok(())
    }

    fn charge_semantic_function(
        &mut self,
        semantic_block_count: u64,
        declared: LineageTotalsV4,
    ) -> Result<(), LineageDecodeErrorV4> {
        self.observed.semantic_functions = self.observed.semantic_functions.checked_add(1).ok_or(
            LineageDecodeErrorV4::CountOverflow {
                resource: LineageResourceV4::SemanticFunctions,
            },
        )?;
        self.require_count(
            LineageResourceV4::SemanticFunctions,
            self.observed.semantic_functions,
        )?;
        self.observed.declared_semantic_blocks = self
            .observed
            .declared_semantic_blocks
            .checked_add(semantic_block_count)
            .ok_or(LineageDecodeErrorV4::CountOverflow {
                resource: LineageResourceV4::Blocks,
            })?;
        self.require_count(
            LineageResourceV4::Blocks,
            self.observed.declared_semantic_blocks,
        )?;
        if self.observed.declared_semantic_blocks > declared.semantic_blocks() {
            return Err(LineageDecodeErrorV4::CountMismatch {
                context: "declared semantic blocks",
                declared: declared.semantic_blocks(),
                observed: self.observed.declared_semantic_blocks,
            });
        }
        Ok(())
    }

    fn charge_blocks(
        &mut self,
        amount: u64,
        declared: LineageTotalsV4,
    ) -> Result<(), LineageDecodeErrorV4> {
        self.observed.blocks = self.observed.blocks.checked_add(amount).ok_or(
            LineageDecodeErrorV4::CountOverflow {
                resource: LineageResourceV4::Blocks,
            },
        )?;
        self.require_count(LineageResourceV4::Blocks, self.observed.blocks)?;
        let declared_blocks =
            declared
                .total_blocks()
                .ok_or(LineageDecodeErrorV4::CountOverflow {
                    resource: LineageResourceV4::Blocks,
                })?;
        if self.observed.blocks > declared_blocks {
            return Err(LineageDecodeErrorV4::CountMismatch {
                context: "blocks",
                declared: declared_blocks,
                observed: self.observed.blocks,
            });
        }
        Ok(())
    }

    fn charge_semantic_block(&mut self) -> Result<(), LineageDecodeErrorV4> {
        self.observed.semantic_blocks = self.observed.semantic_blocks.checked_add(1).ok_or(
            LineageDecodeErrorV4::CountOverflow {
                resource: LineageResourceV4::Blocks,
            },
        )?;
        Ok(())
    }

    fn charge_synthetic_block(&mut self) -> Result<(), LineageDecodeErrorV4> {
        self.observed.synthetic_blocks = self.observed.synthetic_blocks.checked_add(1).ok_or(
            LineageDecodeErrorV4::CountOverflow {
                resource: LineageResourceV4::Blocks,
            },
        )?;
        Ok(())
    }

    fn charge_statements(
        &mut self,
        amount: u64,
        declared: LineageTotalsV4,
    ) -> Result<(), LineageDecodeErrorV4> {
        self.observed.statements = self.observed.statements.checked_add(amount).ok_or(
            LineageDecodeErrorV4::CountOverflow {
                resource: LineageResourceV4::Statements,
            },
        )?;
        self.require_count(LineageResourceV4::Statements, self.observed.statements)?;
        if self.observed.statements > declared.statements() {
            return Err(LineageDecodeErrorV4::CountMismatch {
                context: "statements",
                declared: declared.statements(),
                observed: self.observed.statements,
            });
        }
        Ok(())
    }

    fn charge_operations(
        &mut self,
        amount: u64,
        declared: LineageTotalsV4,
    ) -> Result<(), LineageDecodeErrorV4> {
        self.observed.operations = self.observed.operations.checked_add(amount).ok_or(
            LineageDecodeErrorV4::CountOverflow {
                resource: LineageResourceV4::Operations,
            },
        )?;
        self.require_count(LineageResourceV4::Operations, self.observed.operations)?;
        if self.observed.operations > declared.operations() {
            return Err(LineageDecodeErrorV4::CountMismatch {
                context: "operations",
                declared: declared.operations(),
                observed: self.observed.operations,
            });
        }
        Ok(())
    }

    fn verify_observed_totals(
        &self,
        declared: LineageTotalsV4,
    ) -> Result<(), LineageDecodeErrorV4> {
        for (context, declared, observed) in [
            (
                "semantic functions",
                declared.semantic_functions(),
                self.observed.semantic_functions,
            ),
            (
                "blocks",
                declared
                    .total_blocks()
                    .ok_or(LineageDecodeErrorV4::CountOverflow {
                        resource: LineageResourceV4::Blocks,
                    })?,
                self.observed.blocks,
            ),
            (
                "declared semantic blocks",
                declared.semantic_blocks(),
                self.observed.declared_semantic_blocks,
            ),
            (
                "semantic blocks",
                declared.semantic_blocks(),
                self.observed.semantic_blocks,
            ),
            (
                "synthetic blocks",
                declared.synthetic_blocks(),
                self.observed.synthetic_blocks,
            ),
            (
                "statements",
                declared.statements(),
                self.observed.statements,
            ),
            (
                "operations",
                declared.operations(),
                self.observed.operations,
            ),
        ] {
            if declared != observed {
                return Err(LineageDecodeErrorV4::CountMismatch {
                    context,
                    declared,
                    observed,
                });
            }
        }
        Ok(())
    }

    fn require_count(
        &self,
        resource: LineageResourceV4,
        actual: u64,
    ) -> Result<(), LineageDecodeErrorV4> {
        let max = self.limits.count_limit(resource);
        if actual > max {
            Err(LineageDecodeErrorV4::CountLimitExceeded {
                resource,
                actual,
                max,
            })
        } else {
            Ok(())
        }
    }

    fn charge_parse_work(&mut self, amount: u64) -> Result<(), LineageDecodeErrorV4> {
        self.budget
            .charge(LineageWorkStageV4::Parse, amount)
            .map_err(LineageDecodeErrorV4::from_work_budget)
    }

    fn count_to_usize(
        &self,
        context: &'static str,
        count: u64,
    ) -> Result<usize, LineageDecodeErrorV4> {
        usize::try_from(count).map_err(|_| LineageDecodeErrorV4::LengthOverflow { context })
    }

    fn require_minimum_remaining(
        &self,
        context: &'static str,
        count: usize,
    ) -> Result<(), LineageDecodeErrorV4> {
        if count > self.remaining() {
            Err(LineageDecodeErrorV4::DeclaredCountExceedsInput {
                context,
                count: u64::try_from(count).unwrap_or(u64::MAX),
                remaining: self.remaining(),
            })
        } else {
            Ok(())
        }
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn finish(&self) -> Result<(), LineageDecodeErrorV4> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(LineageDecodeErrorV4::TrailingBytes {
                offset: self.offset,
                trailing: self.bytes.len() - self.offset,
            })
        }
    }
}

struct WriterV4<'a> {
    bytes: Vec<u8>,
    max_input_bytes: u64,
    budget: &'a mut WorkBudgetV4,
}

impl<'a> WriterV4<'a> {
    const fn new(max_input_bytes: u64, budget: &'a mut WorkBudgetV4) -> Self {
        Self {
            bytes: Vec::new(),
            max_input_bytes: if max_input_bytes < MAX_LINEAGE_BYTES_V4 {
                max_input_bytes
            } else {
                MAX_LINEAGE_BYTES_V4
            },
            budget,
        }
    }

    fn raw(&mut self, bytes: &[u8]) -> Result<(), LineageEncodeErrorV4> {
        let current = u64::try_from(self.bytes.len())
            .map_err(|_| LineageEncodeErrorV4::LengthOverflow { context: "output" })?;
        let additional = u64::try_from(bytes.len())
            .map_err(|_| LineageEncodeErrorV4::LengthOverflow { context: "field" })?;
        let next = current
            .checked_add(additional)
            .ok_or(LineageEncodeErrorV4::LengthOverflow { context: "output" })?;
        if next > self.max_input_bytes {
            return Err(LineageEncodeErrorV4::OutputLimitExceeded {
                actual: next,
                max: self.max_input_bytes,
            });
        }
        self.budget
            .charge(LineageWorkStageV4::CanonicalEncoding, additional)
            .map_err(LineageEncodeErrorV4::from_work_budget)?;
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| LineageEncodeErrorV4::AllocationFailed)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn varint(&mut self, mut value: u64) -> Result<(), LineageEncodeErrorV4> {
        let mut encoded = [0_u8; 10];
        let mut length = 0_usize;
        loop {
            let mut byte = u8::try_from(value & 0x7f).expect("seven bits always fit in u8");
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            encoded[length] = byte;
            length += 1;
            if value == 0 {
                return self.raw(&encoded[..length]);
            }
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

const fn varint_length(mut value: u64) -> u8 {
    let mut length = 1_u8;
    while value >= 0x80 {
        value >>= 7;
        length += 1;
    }
    length
}

/// Failure to encode a canonical inert V4 lineage model.
#[derive(Debug)]
#[non_exhaustive]
pub enum LineageEncodeErrorV4 {
    /// The typed model failed structural validation.
    Validation(LineageValidationErrorV4),
    /// Aggregate model count arithmetic overflowed before encoding.
    CountOverflow {
        /// Affected resource.
        resource: LineageResourceV4,
    },
    /// A model count exceeded the caller's supplied encoding limit.
    CountLimitExceeded {
        /// Affected resource.
        resource: LineageResourceV4,
        /// Supplied model count.
        actual: u64,
        /// Effective caller limit.
        max: u64,
    },
    /// A host length conversion overflowed.
    LengthOverflow {
        /// Stable field name.
        context: &'static str,
    },
    /// Canonical output exceeded its exact byte bound.
    OutputLimitExceeded {
        /// Required output length at rejection.
        actual: u64,
        /// Maximum output length.
        max: u64,
    },
    /// A fallible output allocation failed.
    AllocationFailed,
    /// Shared work accounting overflowed.
    WorkOverflow {
        /// Stage at which accounting overflowed.
        stage: LineageWorkStageV4,
    },
    /// Canonical construction exceeded its single work budget.
    WorkLimitExceeded {
        /// Stage that consumed the first rejected unit.
        stage: LineageWorkStageV4,
        /// Aggregate work after the rejected charge.
        actual: u64,
        /// Shared aggregate maximum.
        max: u64,
    },
}

impl fmt::Display for LineageEncodeErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => write!(formatter, "invalid V4 lineage model: {error}"),
            Self::CountOverflow { resource } => {
                write!(formatter, "{resource:?} count overflowed before encoding")
            }
            Self::CountLimitExceeded {
                resource,
                actual,
                max,
            } => write!(
                formatter,
                "{resource:?} count {actual} exceeds encoding limit {max}"
            ),
            Self::LengthOverflow { context } => {
                write!(formatter, "{context} length overflows canonical encoding")
            }
            Self::OutputLimitExceeded { actual, max } => {
                write!(
                    formatter,
                    "canonical output needs {actual} bytes, exceeding {max}"
                )
            }
            Self::AllocationFailed => formatter.write_str("canonical output allocation failed"),
            Self::WorkOverflow { stage } => {
                write!(formatter, "{stage:?} work accounting overflowed")
            }
            Self::WorkLimitExceeded { stage, actual, max } => write!(
                formatter,
                "{stage:?} raised aggregate work to {actual}, exceeding {max}"
            ),
        }
    }
}

impl std::error::Error for LineageEncodeErrorV4 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LineageValidationErrorV4> for LineageEncodeErrorV4 {
    fn from(value: LineageValidationErrorV4) -> Self {
        Self::Validation(value)
    }
}

impl LineageEncodeErrorV4 {
    const fn from_work_budget(error: WorkBudgetErrorV4) -> Self {
        match error {
            WorkBudgetErrorV4::Overflow { stage } => Self::WorkOverflow { stage },
            WorkBudgetErrorV4::LimitExceeded { stage, actual, max } => {
                Self::WorkLimitExceeded { stage, actual, max }
            }
        }
    }
}

/// Failure to strictly decode canonical inert V4 lineage bytes.
#[derive(Debug)]
#[non_exhaustive]
pub enum LineageDecodeErrorV4 {
    /// Input length did not fit the wire's `u64` accounting.
    InputLengthOverflow,
    /// Input exceeded the independent parser byte bound.
    InputLimitExceeded {
        /// Observed input bytes.
        actual: u64,
        /// Maximum input bytes.
        max: u64,
    },
    /// Input ended inside a declared field.
    UnexpectedEnd {
        /// Byte offset at which the field began or ended.
        offset: usize,
        /// Requested bytes.
        requested: usize,
    },
    /// Canonical magic was absent.
    InvalidMagic,
    /// The wire version is unsupported.
    UnsupportedVersion(u64),
    /// Header flags were nonzero.
    UnsupportedFlags(u64),
    /// An unsigned LEB128 integer exceeded `u64`.
    VarintOverflow {
        /// Starting byte offset.
        offset: usize,
    },
    /// An unsigned LEB128 integer was not shortest-form.
    NonShortestVarint {
        /// Starting byte offset.
        offset: usize,
    },
    /// A closed enum tag was unknown.
    InvalidTag {
        /// Stable tag kind.
        context: &'static str,
        /// Starting byte offset.
        offset: usize,
        /// Unknown value.
        value: u64,
    },
    /// A declared record count could not fit remaining input.
    DeclaredCountExceedsInput {
        /// Stable record kind.
        context: &'static str,
        /// Declared count.
        count: u64,
        /// Remaining bytes.
        remaining: usize,
    },
    /// A bounded parser count overflowed while accumulating nested records.
    CountOverflow {
        /// Affected resource.
        resource: LineageResourceV4,
    },
    /// A declared or observed count exceeded parser admission limits.
    CountLimitExceeded {
        /// Affected resource.
        resource: LineageResourceV4,
        /// Observed count.
        actual: u64,
        /// Parser maximum.
        max: u64,
    },
    /// A nested observed count disagreed with its declared aggregate.
    CountMismatch {
        /// Stable resource name.
        context: &'static str,
        /// Header count.
        declared: u64,
        /// Reconstructed count.
        observed: u64,
    },
    /// Validation work accounting overflowed.
    WorkOverflow {
        /// Stage at which accounting overflowed.
        stage: LineageWorkStageV4,
    },
    /// Validation work exceeded its independent parser bound.
    WorkLimitExceeded {
        /// Stage that consumed the first rejected unit.
        stage: LineageWorkStageV4,
        /// Observed work.
        actual: u64,
        /// Maximum work.
        max: u64,
    },
    /// A host length conversion overflowed.
    LengthOverflow {
        /// Stable field name.
        context: &'static str,
    },
    /// A fallible parser allocation failed.
    AllocationFailed {
        /// Stable collection name.
        context: &'static str,
    },
    /// Bytes remained after the complete declared record graph.
    TrailingBytes {
        /// First trailing byte offset.
        offset: usize,
        /// Number of trailing bytes.
        trailing: usize,
    },
    /// The decoded typed model failed structural validation.
    Validation(LineageValidationErrorV4),
    /// Exact canonical re-encoding differed from the input.
    NonCanonicalReencoding,
}

impl fmt::Display for LineageDecodeErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputLengthOverflow => formatter.write_str("input length does not fit u64"),
            Self::InputLimitExceeded { actual, max } => {
                write!(formatter, "input uses {actual} bytes, exceeding {max}")
            }
            Self::UnexpectedEnd { offset, requested } => write!(
                formatter,
                "input ended at byte {offset} while reading {requested} bytes"
            ),
            Self::InvalidMagic => formatter.write_str("invalid V4 lineage magic"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported V4 lineage version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported V4 lineage flags {flags:#x}")
            }
            Self::VarintOverflow { offset } => {
                write!(formatter, "varint at byte {offset} overflows u64")
            }
            Self::NonShortestVarint { offset } => {
                write!(formatter, "varint at byte {offset} is not shortest-form")
            }
            Self::InvalidTag {
                context,
                offset,
                value,
            } => write!(formatter, "invalid {context} tag {value} at byte {offset}"),
            Self::DeclaredCountExceedsInput {
                context,
                count,
                remaining,
            } => write!(
                formatter,
                "{context} declares {count} records with only {remaining} input bytes remaining"
            ),
            Self::CountOverflow { resource } => {
                write!(formatter, "{resource:?} count overflowed")
            }
            Self::CountLimitExceeded {
                resource,
                actual,
                max,
            } => write!(
                formatter,
                "{resource:?} count {actual} exceeds parser limit {max}"
            ),
            Self::CountMismatch {
                context,
                declared,
                observed,
            } => write!(
                formatter,
                "{context} declares {declared} records but reconstructs {observed}"
            ),
            Self::WorkOverflow { stage } => {
                write!(formatter, "{stage:?} work accounting overflowed")
            }
            Self::WorkLimitExceeded { stage, actual, max } => write!(
                formatter,
                "{stage:?} raised aggregate work to {actual}, exceeding {max}"
            ),
            Self::LengthOverflow { context } => {
                write!(formatter, "{context} length does not fit this host")
            }
            Self::AllocationFailed { context } => {
                write!(formatter, "{context} allocation failed")
            }
            Self::TrailingBytes { offset, trailing } => write!(
                formatter,
                "input has {trailing} trailing bytes after byte {offset}"
            ),
            Self::Validation(error) => write!(formatter, "invalid V4 lineage model: {error}"),
            Self::NonCanonicalReencoding => {
                formatter.write_str("V4 lineage does not reproduce its canonical encoding")
            }
        }
    }
}

impl std::error::Error for LineageDecodeErrorV4 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LineageValidationErrorV4> for LineageDecodeErrorV4 {
    fn from(value: LineageValidationErrorV4) -> Self {
        Self::Validation(value)
    }
}

impl LineageDecodeErrorV4 {
    const fn from_work_budget(error: WorkBudgetErrorV4) -> Self {
        match error {
            WorkBudgetErrorV4::Overflow { stage } => Self::WorkOverflow { stage },
            WorkBudgetErrorV4::LimitExceeded { stage, actual, max } => {
                Self::WorkLimitExceeded { stage, actual, max }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LineageEncodeErrorV4, MAX_LINEAGE_BYTES_V4, WriterV4};
    use crate::model::WorkBudgetV4;

    #[test]
    fn writer_hard_cap_cannot_be_widened() {
        let mut budget = WorkBudgetV4::unbounded();
        let mut writer = WriterV4::new(u64::MAX, &mut budget);
        let exact = vec![0; usize::try_from(MAX_LINEAGE_BYTES_V4).unwrap()];
        writer.raw(&exact).unwrap();
        assert!(matches!(
            writer.raw(&[0]),
            Err(LineageEncodeErrorV4::OutputLimitExceeded {
                actual,
                max: MAX_LINEAGE_BYTES_V4,
            }) if actual == MAX_LINEAGE_BYTES_V4 + 1
        ));
    }
}
