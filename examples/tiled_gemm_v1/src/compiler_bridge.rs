//! Inert binding between the checked general GEMM plan and compiler API.
//!
//! This adapter commits the exact plan bytes to a bounded frontend snapshot
//! and complete compile request. It authenticates neither MIR nor a kernel and
//! grants no proof, artifact, publication, load, dispatch, or launch authority.

use core::fmt;

use fe2o3_compiler_api::{
    CompileLimitsV1, CompileRequestErrorV1, CompileRequestV1, CompilerProfileIdentityV1,
    CompilerStageV1, KernelInstanceIdentityV1, ObligationSetIdentityV1,
    PipelineConfigurationIdentityV1, PipelineSelectorV1, RequestIdentityV1,
    SnapshotFormatIdentityV1, SnapshotIdentityV1, StageSnapshotErrorV1, StageSnapshotV1,
    TargetProfileIdentityV1,
};
use fe2o3_compiler_driver::{
    GEMM_REQUIRED_SAFETY_PROPERTIES_V1, GemmProofDiagnosticV1, GemmSafetyPropertyV1,
};
use sha2::{Digest, Sha256};

use crate::{
    GEMM_REQUIRED_PROPERTIES_V1, GemmRequiredPropertyV1, GemmVerificationStageV1, GeneralGemmPlanV1,
};

/// Canonical frontend-envelope schema for an inert general GEMM plan.
pub const GENERAL_GEMM_FRONTEND_SCHEMA_V1: &str = "fe2o3-general-gemm-frontend-input-v1";

const FORMAT_IDENTITY_DOMAIN_V1: &str = "fe2o3.general-gemm.frontend-format-identity.v1";
const SNAPSHOT_IDENTITY_DOMAIN_V1: &str = "fe2o3.general-gemm.frontend-snapshot-identity.v1";
const KERNEL_IDENTITY_DOMAIN_V1: &str = "fe2o3.general-gemm.inert-kernel-instance-identity.v1";
const OBLIGATION_IDENTITY_DOMAIN_V1: &str = "fe2o3.general-gemm.required-obligations-identity.v1";
const REQUEST_IDENTITY_DOMAIN_V1: &str = "fe2o3.general-gemm.compile-request-identity.v1";

/// A mirrored corpus property disagrees with the compiler driver's schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmPropertySchemaErrorV1 {
    /// The mirror and compiler enumerate different property counts.
    Cardinality {
        /// Number of mirrored properties.
        mirrored: usize,
        /// Number of compiler-required properties.
        compiler: usize,
    },
    /// The compiler-required order or variant differs from the mirror.
    Property {
        /// Mirrored property.
        mirrored: GemmRequiredPropertyV1,
        /// Compiler property found at the same position.
        compiler: GemmSafetyPropertyV1,
    },
    /// The stable diagnostic spelling differs.
    Spelling {
        /// Mirrored property.
        property: GemmRequiredPropertyV1,
    },
    /// The stable diagnostic code differs.
    DiagnosticCode {
        /// Mirrored property.
        property: GemmRequiredPropertyV1,
        /// Mirrored code.
        mirrored: u32,
        /// Compiler-owned code.
        compiler: u32,
    },
    /// The earliest verification stage differs.
    VerificationStage {
        /// Mirrored property.
        property: GemmRequiredPropertyV1,
        /// Mirrored stage tag.
        mirrored: u8,
        /// Compiler stage tag.
        compiler: u8,
    },
}

impl fmt::Display for GemmPropertySchemaErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "general GEMM property schema mismatch: {self:?}")
    }
}

impl std::error::Error for GemmPropertySchemaErrorV1 {}

const fn compiler_property(property: GemmRequiredPropertyV1) -> GemmSafetyPropertyV1 {
    match property {
        GemmRequiredPropertyV1::MemorySafe => GemmSafetyPropertyV1::MemorySafe,
        GemmRequiredPropertyV1::BoundsSafe => GemmSafetyPropertyV1::BoundsSafe,
        GemmRequiredPropertyV1::Initialized => GemmSafetyPropertyV1::Initialized,
        GemmRequiredPropertyV1::RaceFree => GemmSafetyPropertyV1::RaceFree,
        GemmRequiredPropertyV1::BarrierConvergent => GemmSafetyPropertyV1::BarrierConvergent,
        GemmRequiredPropertyV1::OutputRegionInjective => {
            GemmSafetyPropertyV1::OutputRegionInjective
        }
        GemmRequiredPropertyV1::LdsEpochCorrect => GemmSafetyPropertyV1::LdsEpochCorrect,
        GemmRequiredPropertyV1::AccumulatorPhaseRefinement => {
            GemmSafetyPropertyV1::AccumulatorPhaseRefinement
        }
        GemmRequiredPropertyV1::TailRefinement => GemmSafetyPropertyV1::TailRefinement,
        GemmRequiredPropertyV1::EpilogueRefinement => GemmSafetyPropertyV1::EpilogueRefinement,
        GemmRequiredPropertyV1::NumericalContract => GemmSafetyPropertyV1::NumericalContract,
        GemmRequiredPropertyV1::MachineRefinementBoundary => {
            GemmSafetyPropertyV1::MachineRefinementBoundary
        }
    }
}

const fn compiler_diagnostic(property: GemmRequiredPropertyV1) -> GemmProofDiagnosticV1 {
    match property {
        GemmRequiredPropertyV1::MemorySafe => GemmProofDiagnosticV1::MemorySafe,
        GemmRequiredPropertyV1::BoundsSafe => GemmProofDiagnosticV1::BoundsSafe,
        GemmRequiredPropertyV1::Initialized => GemmProofDiagnosticV1::Initialized,
        GemmRequiredPropertyV1::RaceFree => GemmProofDiagnosticV1::RaceFree,
        GemmRequiredPropertyV1::BarrierConvergent => GemmProofDiagnosticV1::BarrierConvergent,
        GemmRequiredPropertyV1::OutputRegionInjective => {
            GemmProofDiagnosticV1::OutputRegionInjective
        }
        GemmRequiredPropertyV1::LdsEpochCorrect => GemmProofDiagnosticV1::LdsEpochCorrect,
        GemmRequiredPropertyV1::AccumulatorPhaseRefinement => {
            GemmProofDiagnosticV1::AccumulatorPhaseRefinement
        }
        GemmRequiredPropertyV1::TailRefinement => GemmProofDiagnosticV1::TailRefinement,
        GemmRequiredPropertyV1::EpilogueRefinement => GemmProofDiagnosticV1::EpilogueRefinement,
        GemmRequiredPropertyV1::NumericalContract => GemmProofDiagnosticV1::NumericalContract,
        GemmRequiredPropertyV1::MachineRefinementBoundary => {
            GemmProofDiagnosticV1::MachineRefinementBoundary
        }
    }
}

const fn mirrored_stage(property: GemmRequiredPropertyV1) -> GemmVerificationStageV1 {
    match property {
        GemmRequiredPropertyV1::MemorySafe
        | GemmRequiredPropertyV1::Initialized
        | GemmRequiredPropertyV1::RaceFree
        | GemmRequiredPropertyV1::BarrierConvergent
        | GemmRequiredPropertyV1::LdsEpochCorrect => GemmVerificationStageV1::Gpu,
        GemmRequiredPropertyV1::BoundsSafe | GemmRequiredPropertyV1::OutputRegionInjective => {
            GemmVerificationStageV1::Tile
        }
        GemmRequiredPropertyV1::AccumulatorPhaseRefinement
        | GemmRequiredPropertyV1::TailRefinement
        | GemmRequiredPropertyV1::EpilogueRefinement
        | GemmRequiredPropertyV1::NumericalContract => GemmVerificationStageV1::Kernel,
        GemmRequiredPropertyV1::MachineRefinementBoundary => GemmVerificationStageV1::Amdgcn,
    }
}

fn validate_property_entry(
    mirrored: GemmRequiredPropertyV1,
    compiler: GemmSafetyPropertyV1,
    diagnostic: GemmProofDiagnosticV1,
    stage: CompilerStageV1,
) -> Result<(), GemmPropertySchemaErrorV1> {
    let expected_compiler = compiler_property(mirrored);
    if compiler != expected_compiler {
        return Err(GemmPropertySchemaErrorV1::Property { mirrored, compiler });
    }
    if mirrored.as_str() != compiler.as_str() {
        return Err(GemmPropertySchemaErrorV1::Spelling { property: mirrored });
    }
    let mirrored_code = mirrored.diagnostic_code();
    let compiler_code = diagnostic.code();
    if mirrored_code != compiler_code {
        return Err(GemmPropertySchemaErrorV1::DiagnosticCode {
            property: mirrored,
            mirrored: mirrored_code,
            compiler: compiler_code,
        });
    }
    let mirrored_stage = mirrored_stage(mirrored).wire_tag();
    let compiler_stage = stage as u8;
    if mirrored_stage != compiler_stage {
        return Err(GemmPropertySchemaErrorV1::VerificationStage {
            property: mirrored,
            mirrored: mirrored_stage,
            compiler: compiler_stage,
        });
    }
    Ok(())
}

/// Checks every mirrored corpus property against compiler-owned spelling,
/// diagnostic code, verification stage, variant, and required order.
pub fn validate_gemm_property_schema_v1() -> Result<(), GemmPropertySchemaErrorV1> {
    if GEMM_REQUIRED_PROPERTIES_V1.len() != GEMM_REQUIRED_SAFETY_PROPERTIES_V1.len() {
        return Err(GemmPropertySchemaErrorV1::Cardinality {
            mirrored: GEMM_REQUIRED_PROPERTIES_V1.len(),
            compiler: GEMM_REQUIRED_SAFETY_PROPERTIES_V1.len(),
        });
    }
    for (mirrored, compiler) in GEMM_REQUIRED_PROPERTIES_V1
        .into_iter()
        .zip(GEMM_REQUIRED_SAFETY_PROPERTIES_V1)
    {
        validate_property_entry(
            mirrored,
            compiler,
            compiler_diagnostic(mirrored),
            compiler.verification_stage(),
        )?;
    }
    Ok(())
}

/// Caller-selected compiler identities and route for an inert binding.
///
/// These values remain untrusted commitments. This type does not authenticate
/// the selected compiler, target, or pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmCompilerProfilesV1 {
    compiler: CompilerProfileIdentityV1,
    target: TargetProfileIdentityV1,
    pipeline: PipelineConfigurationIdentityV1,
    selector: PipelineSelectorV1,
}

impl GeneralGemmCompilerProfilesV1 {
    /// Records caller-supplied identities and the explicit compiler route.
    pub const fn new(
        compiler: CompilerProfileIdentityV1,
        target: TargetProfileIdentityV1,
        pipeline: PipelineConfigurationIdentityV1,
        selector: PipelineSelectorV1,
    ) -> Self {
        Self {
            compiler,
            target,
            pipeline,
            selector,
        }
    }

    /// Returns the caller-supplied compiler profile commitment.
    pub const fn compiler(self) -> CompilerProfileIdentityV1 {
        self.compiler
    }

    /// Returns the caller-supplied target profile commitment.
    pub const fn target(self) -> TargetProfileIdentityV1 {
        self.target
    }

    /// Returns the caller-supplied pipeline configuration commitment.
    pub const fn pipeline(self) -> PipelineConfigurationIdentityV1 {
        self.pipeline
    }

    /// Returns the explicit compiler route.
    pub const fn selector(self) -> PipelineSelectorV1 {
        self.selector
    }
}

/// A general GEMM frontend binding failed closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmCompilerBindingErrorV1 {
    /// The mirrored property schema does not match the compiler driver.
    PropertySchema(GemmPropertySchemaErrorV1),
    /// The checked plan's stored identity does not hash its canonical bytes.
    PlanIdentityMismatch,
    /// The frontend snapshot is not at the required input stage.
    InputStageMismatch,
    /// The frontend format identity was substituted.
    SnapshotFormatIdentityMismatch,
    /// The exact plan envelope bytes were malformed or substituted.
    FrontendPayloadMismatch,
    /// The frontend snapshot commitment was malformed or substituted.
    SnapshotIdentityMismatch,
    /// The inert kernel-instance commitment was substituted.
    KernelInstanceIdentityMismatch,
    /// The required-property-set commitment was substituted.
    ObligationSetIdentityMismatch,
    /// The complete request commitment was malformed or substituted.
    RequestIdentityMismatch,
    /// The compiler API rejected the bounded snapshot.
    Snapshot(StageSnapshotErrorV1),
    /// The compiler API rejected the bounded request.
    Request(CompileRequestErrorV1),
}

impl fmt::Display for GeneralGemmCompilerBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM compiler binding rejected: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmCompilerBindingErrorV1 {}

impl From<GemmPropertySchemaErrorV1> for GeneralGemmCompilerBindingErrorV1 {
    fn from(value: GemmPropertySchemaErrorV1) -> Self {
        Self::PropertySchema(value)
    }
}

impl From<StageSnapshotErrorV1> for GeneralGemmCompilerBindingErrorV1 {
    fn from(value: StageSnapshotErrorV1) -> Self {
        Self::Snapshot(value)
    }
}

impl From<CompileRequestErrorV1> for GeneralGemmCompilerBindingErrorV1 {
    fn from(value: CompileRequestErrorV1) -> Self {
        Self::Request(value)
    }
}

/// Privately validated inert compiler request for one exact plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralGemmCompilerBindingV1 {
    request: CompileRequestV1,
}

impl GeneralGemmCompilerBindingV1 {
    /// Borrows the bounded compiler API request.
    pub const fn request(&self) -> &CompileRequestV1 {
        &self.request
    }

    /// Revalidates this request against the exact checked plan.
    pub fn validate(
        &self,
        plan: &GeneralGemmPlanV1,
    ) -> Result<(), GeneralGemmCompilerBindingErrorV1> {
        validate_general_gemm_compiler_request_v1(plan, &self.request)
    }

    /// Releases the inert request without granting downstream authority.
    pub fn into_request(self) -> CompileRequestV1 {
        self.request
    }
}

fn append_text(output: &mut Vec<u8>, value: &str) {
    append_bytes(output, value.as_bytes());
}

fn append_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}

fn digest(domain: &str, fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u32).to_le_bytes());
    hasher.update(domain.as_bytes());
    for field in fields {
        hasher.update((field.len() as u32).to_le_bytes());
        hasher.update(field);
    }
    hasher.finalize().into()
}

fn plan_bytes_and_identity(
    plan: &GeneralGemmPlanV1,
) -> Result<(Vec<u8>, [u8; 32]), GeneralGemmCompilerBindingErrorV1> {
    let canonical = plan.encode_canonical();
    let identity: [u8; 32] = Sha256::digest(&canonical).into();
    if &identity != plan.identity().as_bytes() {
        return Err(GeneralGemmCompilerBindingErrorV1::PlanIdentityMismatch);
    }
    Ok((canonical, identity))
}

fn frontend_bytes(plan_bytes: &[u8], plan_identity: &[u8; 32]) -> Vec<u8> {
    let mut output = Vec::with_capacity(
        GENERAL_GEMM_FRONTEND_SCHEMA_V1.len() + plan_identity.len() + plan_bytes.len() + 12,
    );
    append_text(&mut output, GENERAL_GEMM_FRONTEND_SCHEMA_V1);
    append_bytes(&mut output, plan_identity);
    append_bytes(&mut output, plan_bytes);
    output
}

fn snapshot_format_identity() -> SnapshotFormatIdentityV1 {
    SnapshotFormatIdentityV1::from_untrusted_bytes(digest(
        FORMAT_IDENTITY_DOMAIN_V1,
        &[GENERAL_GEMM_FRONTEND_SCHEMA_V1.as_bytes()],
    ))
}

fn snapshot_identity(format: SnapshotFormatIdentityV1, bytes: &[u8]) -> SnapshotIdentityV1 {
    SnapshotIdentityV1::from_untrusted_bytes(digest(
        SNAPSHOT_IDENTITY_DOMAIN_V1,
        &[format.as_bytes(), bytes],
    ))
}

fn kernel_identity(
    plan_identity: &[u8; 32],
    snapshot: SnapshotIdentityV1,
) -> KernelInstanceIdentityV1 {
    KernelInstanceIdentityV1::from_untrusted_bytes(digest(
        KERNEL_IDENTITY_DOMAIN_V1,
        &[plan_identity, snapshot.as_bytes()],
    ))
}

fn obligation_identity() -> ObligationSetIdentityV1 {
    let mut bytes = Vec::with_capacity(512);
    for property in GEMM_REQUIRED_PROPERTIES_V1 {
        let compiler = compiler_property(property);
        bytes.push(compiler as u8);
        append_text(&mut bytes, compiler.as_str());
        bytes.extend_from_slice(&compiler_diagnostic(property).code().to_le_bytes());
        bytes.push(compiler.verification_stage() as u8);
    }
    ObligationSetIdentityV1::from_untrusted_bytes(digest(OBLIGATION_IDENTITY_DOMAIN_V1, &[&bytes]))
}

#[derive(Clone, Copy)]
struct RequestIdentityFields {
    kernel: KernelInstanceIdentityV1,
    profiles: GeneralGemmCompilerProfilesV1,
    obligations: ObligationSetIdentityV1,
    snapshot: SnapshotIdentityV1,
    format: SnapshotFormatIdentityV1,
    limits: CompileLimitsV1,
}

fn request_identity(fields: RequestIdentityFields) -> RequestIdentityV1 {
    let selector = [fields.profiles.selector() as u8];
    let mut limit_bytes = Vec::with_capacity(18);
    limit_bytes.extend_from_slice(&fields.limits.max_stage_snapshots().to_le_bytes());
    limit_bytes.extend_from_slice(&fields.limits.max_stage_receipts().to_le_bytes());
    limit_bytes.extend_from_slice(&fields.limits.max_diagnostics().to_le_bytes());
    limit_bytes.extend_from_slice(&fields.limits.max_snapshot_bytes().to_le_bytes());
    limit_bytes.extend_from_slice(&fields.limits.max_total_snapshot_bytes().to_le_bytes());
    limit_bytes.extend_from_slice(&fields.limits.max_candidate_bytes().to_le_bytes());
    RequestIdentityV1::from_untrusted_bytes(digest(
        REQUEST_IDENTITY_DOMAIN_V1,
        &[
            fields.kernel.as_bytes(),
            fields.profiles.compiler().as_bytes(),
            fields.profiles.target().as_bytes(),
            fields.profiles.pipeline().as_bytes(),
            fields.obligations.as_bytes(),
            &selector,
            fields.snapshot.as_bytes(),
            fields.format.as_bytes(),
            &limit_bytes,
        ],
    ))
}

/// Creates a bounded, inert compiler request binding the exact checked plan.
pub fn bind_general_gemm_compiler_request_v1(
    plan: &GeneralGemmPlanV1,
    profiles: GeneralGemmCompilerProfilesV1,
    limits: CompileLimitsV1,
) -> Result<GeneralGemmCompilerBindingV1, GeneralGemmCompilerBindingErrorV1> {
    validate_gemm_property_schema_v1()?;
    let (plan_bytes, plan_identity) = plan_bytes_and_identity(plan)?;
    let bytes = frontend_bytes(&plan_bytes, &plan_identity);
    let format = snapshot_format_identity();
    let snapshot_id = snapshot_identity(format, &bytes);
    let snapshot =
        StageSnapshotV1::new(CompilerStageV1::FrontendInput, snapshot_id, format, bytes)?;
    let kernel = kernel_identity(&plan_identity, snapshot_id);
    let obligations = obligation_identity();
    let request_id = request_identity(RequestIdentityFields {
        kernel,
        profiles,
        obligations,
        snapshot: snapshot_id,
        format,
        limits,
    });
    let request = CompileRequestV1::new(
        request_id,
        kernel,
        profiles.compiler(),
        profiles.target(),
        profiles.pipeline(),
        obligations,
        profiles.selector(),
        snapshot,
        limits,
    )?;
    validate_general_gemm_compiler_request_v1(plan, &request)?;
    Ok(GeneralGemmCompilerBindingV1 { request })
}

/// Validates an opaque compiler API request against one exact checked plan.
///
/// Validation recomputes every adapter-owned identity. Caller-owned profile
/// identities remain untrusted, but substitution without a new request
/// commitment fails closed.
pub fn validate_general_gemm_compiler_request_v1(
    plan: &GeneralGemmPlanV1,
    request: &CompileRequestV1,
) -> Result<(), GeneralGemmCompilerBindingErrorV1> {
    validate_gemm_property_schema_v1()?;
    let (plan_bytes, plan_identity) = plan_bytes_and_identity(plan)?;
    let expected_bytes = frontend_bytes(&plan_bytes, &plan_identity);
    let input = request.input();
    if input.stage() != CompilerStageV1::FrontendInput {
        return Err(GeneralGemmCompilerBindingErrorV1::InputStageMismatch);
    }
    let format = snapshot_format_identity();
    if input.format_identity() != format {
        return Err(GeneralGemmCompilerBindingErrorV1::SnapshotFormatIdentityMismatch);
    }
    if input.canonical_bytes() != expected_bytes {
        return Err(GeneralGemmCompilerBindingErrorV1::FrontendPayloadMismatch);
    }
    let snapshot = snapshot_identity(format, &expected_bytes);
    if input.identity() != snapshot {
        return Err(GeneralGemmCompilerBindingErrorV1::SnapshotIdentityMismatch);
    }
    let kernel = kernel_identity(&plan_identity, snapshot);
    if request.kernel_instance_identity() != kernel {
        return Err(GeneralGemmCompilerBindingErrorV1::KernelInstanceIdentityMismatch);
    }
    let obligations = obligation_identity();
    if request.input_obligations_identity() != obligations {
        return Err(GeneralGemmCompilerBindingErrorV1::ObligationSetIdentityMismatch);
    }
    let profiles = GeneralGemmCompilerProfilesV1::new(
        request.compiler_profile_identity(),
        request.target_profile_identity(),
        request.pipeline_configuration_identity(),
        request.selector(),
    );
    let expected_request = request_identity(RequestIdentityFields {
        kernel,
        profiles,
        obligations,
        snapshot,
        format,
        limits: request.limits(),
    });
    if request.identity() != expected_request {
        return Err(GeneralGemmCompilerBindingErrorV1::RequestIdentityMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_amd_target::AmdTargetId;

    use crate::contract::TARGET_V1;
    use crate::{
        GeneralGemmRequestV1, GeneralLaunchLimitsV1, admit_target_v1, plan_general_gemm_v1,
    };

    #[test]
    fn diagnostic_and_stage_drift_are_rejected() {
        assert!(matches!(
            validate_property_entry(
                GemmRequiredPropertyV1::BoundsSafe,
                GemmSafetyPropertyV1::BoundsSafe,
                GemmProofDiagnosticV1::RaceFree,
                CompilerStageV1::Tile,
            ),
            Err(GemmPropertySchemaErrorV1::DiagnosticCode { .. })
        ));
        assert!(matches!(
            validate_property_entry(
                GemmRequiredPropertyV1::BoundsSafe,
                GemmSafetyPropertyV1::BoundsSafe,
                GemmProofDiagnosticV1::BoundsSafe,
                CompilerStageV1::Gpu,
            ),
            Err(GemmPropertySchemaErrorV1::VerificationStage { .. })
        ));
    }

    #[test]
    fn independent_problem_schedule_and_launch_mutations_change_commitments() {
        fn plan(m: u32) -> GeneralGemmPlanV1 {
            plan_general_gemm_v1(
                admit_target_v1(AmdTargetId::parse(TARGET_V1).unwrap()).unwrap(),
                GeneralGemmRequestV1::new(m, 19, 18, 23, 29, 31, 2.0, -1.0),
                GeneralLaunchLimitsV1::representable(),
            )
            .unwrap()
        }

        fn identities(bytes: &[u8]) -> (SnapshotIdentityV1, RequestIdentityV1) {
            let plan_identity: [u8; 32] = Sha256::digest(bytes).into();
            let format = snapshot_format_identity();
            let snapshot = snapshot_identity(format, &frontend_bytes(bytes, &plan_identity));
            let kernel = kernel_identity(&plan_identity, snapshot);
            let obligations = obligation_identity();
            let request = request_identity(RequestIdentityFields {
                kernel,
                profiles: GeneralGemmCompilerProfilesV1::new(
                    CompilerProfileIdentityV1::from_untrusted_bytes([0x11; 32]),
                    TargetProfileIdentityV1::from_untrusted_bytes([0x22; 32]),
                    PipelineConfigurationIdentityV1::from_untrusted_bytes([0x33; 32]),
                    PipelineSelectorV1::PlironShadow,
                ),
                obligations,
                snapshot,
                format,
                limits: CompileLimitsV1::default(),
            });
            (snapshot, request)
        }

        let base_plan = plan(17);
        let base = base_plan.encode_canonical();
        let problem = plan(18).encode_canonical();

        let mut schedule = base.clone();
        let schedule_start = schedule
            .windows(crate::GENERAL_GEMM_REFERENCE_SCHEDULE_V1.len())
            .position(|window| window == crate::GENERAL_GEMM_REFERENCE_SCHEDULE_V1.as_bytes())
            .unwrap();
        schedule[schedule_start] ^= 1;

        let mut launch = base.clone();
        let launch_bytes: Vec<_> = base_plan
            .aql_grid_work_items()
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let launch_start = launch
            .windows(launch_bytes.len())
            .position(|window| window == launch_bytes)
            .unwrap();
        launch[launch_start] ^= 1;

        let base_identities = identities(&base);
        for mutated in [&problem, &schedule, &launch] {
            let mutated_identities = identities(mutated);
            assert_ne!(mutated_identities.0, base_identities.0);
            assert_ne!(mutated_identities.1, base_identities.1);
        }
    }
}
