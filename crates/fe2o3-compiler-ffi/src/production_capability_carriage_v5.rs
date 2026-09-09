//! Exact V13 capability carriage beside the frozen semantic handoff schemas.

use std::{error::Error, fmt, mem::size_of};

use fe2o3_amd_target::{
    PRODUCTION_AMD_NEUTRAL_CAPABILITY_MODEL_REVISION_V1, ProductionAmdTargetProfileV1,
    TargetProfileSpecV1,
};
use fe2o3_compiler_lineage::{
    InertCanonicalKernelIrV13ReceiptErrorV5, InertCanonicalKernelIrV13ReceiptV5,
    InertCapabilityRefinementReceiptKindV1, InertCapabilityRefinementReceiptV1,
    InertCompilerProofOwnerErrorV5, InertCompilerProofOwnerV5, InertLineageContentIdentityV3,
    InertMultiRootProofLineageV3, InertMultiRootStaticCapabilityEvidenceAssociationV1,
    InertProofBindingAssociationV4, InertStaticCapabilityEvidenceAssociationErrorV1,
    InertStaticCapabilityEvidenceAssociationV1, MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3,
    MultiRootProofLineageErrorV3, MultiRootProofRosterKindV3, MultiRootTargetBindingTranscriptV2,
    TargetBindingTranscriptV3, TargetLineageIdentityV3,
};
use fe2o3_proof_contracts::{
    CapabilityCodecErrorV1, CapabilitySubjectV1, DigestV1, ExecutableKirIdentityV1,
    InertCapabilityObligationSetV1, KernelIdentityV1, KernelRootIdentityV1,
    LaunchContractIdentityV1, TargetModelIdentityV1,
};
use sha2::{Digest as _, Sha256};

use crate::{InertSemanticCompilerModuleHandoffErrorV3, InertSemanticCompilerModuleHandoffV3};

/// Magic for the native V13 production capability input handoff.
pub const INERT_PRODUCTION_CAPABILITY_HANDOFF_MAGIC_V5: [u8; 8] = *b"F2CPHOV5";
/// Exact input-handoff wire version.
pub const INERT_PRODUCTION_CAPABILITY_HANDOFF_VERSION_V5: u16 = 5;
/// Magic for one native V5 handoff and exact simulation Bundle V8 transaction.
pub const INERT_PRODUCTION_CAPABILITY_TRANSACTION_MAGIC_V5: [u8; 8] = *b"F2CPTXV5";
/// Exact composite transaction wire version.
pub const INERT_PRODUCTION_CAPABILITY_TRANSACTION_VERSION_V5: u16 = 5;
/// Magic for the completed V13 production capability result.
pub const INERT_PRODUCTION_CAPABILITY_RESULT_MAGIC_V5: [u8; 8] = *b"F2CPRSV5";
/// Exact completion-result wire version.
pub const INERT_PRODUCTION_CAPABILITY_RESULT_VERSION_V5: u16 = 5;
/// Maximum complete V5 input handoff, including its frozen V3 handoff.
pub const MAX_INERT_PRODUCTION_CAPABILITY_HANDOFF_BYTES_V5: usize = 256 * 1024 * 1024;
/// Maximum complete V5 result, including exact same-transaction Bundle V8 custody.
pub const MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5: usize =
    MAX_INERT_PRODUCTION_CAPABILITY_TRANSACTION_BYTES_V5 + (16 * 1024 * 1024);
/// Maximum canonical W4 witness retained by the V5 transaction.
pub const MAX_INERT_PRODUCTION_W4_WITNESS_BYTES_V5: usize = 32 * 1024 * 1024;
/// Maximum exact canonical simulation Bundle V8 retained without depending on kernel IR.
///
/// This is the Bundle V8 416-byte header plus its exact kernel-IR, source-map, semantic-MIR,
/// scalar-storage-map, aggregate-storage-map, and target maxima.
pub const MAX_INERT_SIMULATION_BUNDLE_BYTES_V8: usize = 416
    + (16 * 1024 * 1024)
    + (4 * 1024 * 1024)
    + (128 * 1024 * 1024)
    + (8 * 1024 * 1024)
    + (8 * 1024 * 1024)
    + 4096;
/// Maximum canonical same-transaction V5 handoff plus Bundle V8 custody.
pub const MAX_INERT_PRODUCTION_CAPABILITY_TRANSACTION_BYTES_V5: usize =
    MAX_INERT_PRODUCTION_CAPABILITY_HANDOFF_BYTES_V5
        + MAX_INERT_SIMULATION_BUNDLE_BYTES_V8
        + (1024 * 1024);

const HEADER_BYTES: usize = 32;
const FIELD_HEADER_BYTES: usize = 4;
const TERMINAL_BYTES: usize = 32;
const HANDOFF_KIND: u16 = 1;
const RESULT_KIND: u16 = 2;
const TARGET_CLOSURE_KIND: u16 = 3;
const FINAL_GRAPH_REPORT_KIND: u16 = 4;
const OUTPUT_RECEIPT_KIND: u16 = 5;
const TRANSACTION_KIND: u16 = 6;
const HANDOFF_FIELDS: usize = 12;
const RESULT_FIELDS: usize = 8;
const TRANSACTION_FIELDS: usize = 5;
const TARGET_CLOSURE_FIELDS: usize = 8;
const FINAL_GRAPH_REPORT_FIELDS: usize = 10;
const OUTPUT_RECEIPT_FIELDS: usize = 4;
const MAX_TARGET_DECISION_BYTES: usize = 2 * 1024 * 1024;
const MAX_OUTPUT_RECEIPT_BYTES: usize = 512;
const HANDOFF_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-HANDOFF/EXACT-V13/V5\0";
const TRANSACTION_DOMAIN: &[u8] =
    b"FE2O3/PRODUCTION-CAPABILITY-AND-SIMULATION-BUNDLE-TRANSACTION/V5+V8\0";
const RESULT_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-RESULT/EXACT-V13/V5\0";
const TARGET_CLOSURE_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-TARGET-CAPABILITY-CLOSURE/V5\0";
const FINAL_GRAPH_REPORT_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-FINAL-GRAPH-REPORT/V5\0";
const OUTPUT_RECEIPT_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-COMPILER-OUTPUT-RECEIPT/V5\0";
const HANDOFF_IDENTITY_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-HANDOFF-IDENTITY/V5\0";
const TRANSACTION_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/PRODUCTION-CAPABILITY-AND-SIMULATION-BUNDLE-IDENTITY/V5+V8\0";
const RESULT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-RESULT-IDENTITY/V5\0";
const TARGET_CLOSURE_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/PRODUCTION-TARGET-CAPABILITY-CLOSURE-IDENTITY/V5\0";
const FINAL_GRAPH_REPORT_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/PRODUCTION-FINAL-GRAPH-REPORT-IDENTITY/V5\0";
const OUTPUT_RECEIPT_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/PRODUCTION-COMPILER-OUTPUT-RECEIPT-IDENTITY/V5\0";
const LAUNCH_ROSTER_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/PRODUCTION-TARGET-CAPABILITY-LAUNCH-ROSTER/V5\0";
const W4_WITNESS_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-W4-FINAL-GRAPH-CAPABILITY-WITNESS/V1\0";
const W4_WITNESS_CHECKSUM_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-W4-WITNESS-CHECKSUM/V1\0";
const W4_WITNESS_VERSION_V1: u16 = 1;
/// Frozen number of independently checked obligations in the production W4 schedule.
pub const PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5: usize = 19;
const LIVE_TARGET_CLOSURE_MAGIC_V1: &[u8; 8] = b"F2TCAP01";
const LIVE_TARGET_CLOSURE_VERSION_V1: u16 = 1;
const LIVE_TARGET_CLOSURE_HEADER_BYTES_V1: usize = 157;
const LIVE_TARGET_CLOSURE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION-TARGET-CAPABILITY-CLOSURE/V1\0";
const CAPABILITY_TARGET_MODEL_DOMAIN_V5: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-TARGET-MODEL/V5\0";
const CAPABILITY_ROOT_LAUNCH_DOMAIN_V5: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-ROOT-LAUNCH/V5\0";
const SIMULATION_BUNDLE_MAGIC_V8: &[u8; 8] = b"F2SIMB08";
const SIMULATION_BUNDLE_VERSION_V8: u16 = 8;
const SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V8: u16 = 13;
const SIMULATION_BUNDLE_HEADER_BYTES_V8: usize = 416;
const SIMULATION_BUNDLE_IDENTITY_DOMAIN_V8: &[u8] = b"FE2O3/SIMULATION-BUNDLE-CONTENT/V8\0";

macro_rules! content_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            sha256: [u8; 32],
            byte_len: u64,
        }

        impl $name {
            /// Returns the exact domain-separated digest.
            pub const fn sha256(self) -> [u8; 32] {
                self.sha256
            }

            /// Returns the exact canonical byte length.
            pub const fn byte_len(self) -> u64 {
                self.byte_len
            }
        }
    };
}

content_identity!(
    /// Identity of one exact V5 production capability handoff.
    InertProductionCapabilityHandoffIdentityV5
);
content_identity!(
    /// Identity of one exact V5 production capability result.
    InertProductionCapabilityResultIdentityV5
);
content_identity!(
    /// Identity of one canonical V5 handoff plus simulation Bundle V8 transaction payload.
    InertProductionCapabilityTransactionIdentityV5
);
content_identity!(
    /// Native Bundle V8 content digest and exact canonical byte length.
    InertSimulationBundleIdentityV8
);
content_identity!(
    /// Identity of one exact compiler-stage output receipt.
    InertCompilerStageOutputReceiptIdentityV5
);
content_identity!(
    /// Identity of the exact canonical W4 witness retained by V5.
    InertProductionW4WitnessIdentityV5
);

/// Strict, authority-free custody of the canonical W4 witness payload.
///
/// Decoding this value validates only the frozen W4 envelope. It deliberately cannot recreate
/// the live, move-only W4 witness or grant W6/W7 authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProductionW4WitnessV5 {
    identity: InertProductionW4WitnessIdentityV5,
    canonical_encoding: Box<[u8]>,
}

impl InertProductionW4WitnessV5 {
    /// Retains one exact canonical W4 envelope after validating its domain, version, and checksum.
    pub fn from_canonical_encoding(
        canonical_encoding: impl Into<Vec<u8>>,
    ) -> Result<Self, InertProductionW4WitnessErrorV5> {
        let canonical_encoding = canonical_encoding.into();
        validate_w4_witness_encoding(&canonical_encoding)?;
        let mut digest = Sha256::new();
        digest.update(W4_WITNESS_DOMAIN_V1);
        digest.update(&canonical_encoding);
        let identity = InertProductionW4WitnessIdentityV5 {
            sha256: digest.finalize().into(),
            byte_len: canonical_encoding.len() as u64,
        };
        Ok(Self {
            identity,
            canonical_encoding: canonical_encoding.into_boxed_slice(),
        })
    }

    /// Returns the W4-native, domain-separated identity of these exact bytes.
    pub const fn identity(&self) -> InertProductionW4WitnessIdentityV5 {
        self.identity
    }

    /// Returns the complete canonical W4 witness envelope.
    pub fn canonical_encoding(&self) -> &[u8] {
        &self.canonical_encoding
    }
}

/// Exact immutable coordinates supplied before the sole production worker transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertProductionCapabilityHandoffInputsV5 {
    semantic_mir_identity: [u8; 32],
    subject: CapabilitySubjectV1,
    compiler_policy: [u8; 32],
}

impl InertProductionCapabilityHandoffInputsV5 {
    /// Constructs nonzero source, final-graph, target, launch, and policy coordinates.
    pub fn new(
        semantic_mir_identity: [u8; 32],
        subject: CapabilitySubjectV1,
        compiler_policy: [u8; 32],
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        if semantic_mir_identity == [0; 32] || compiler_policy == [0; 32] {
            return Err(InertProductionCapabilityHandoffErrorV5::ZeroIdentity);
        }
        Ok(Self {
            semantic_mir_identity,
            subject,
            compiler_policy,
        })
    }

    /// Returns the exact decoded semantic-MIR identity.
    pub const fn semantic_mir_identity(self) -> [u8; 32] {
        self.semantic_mir_identity
    }

    /// Returns the exact kernel/root/KIR/epoch/target/launch subject.
    pub const fn subject(self) -> CapabilitySubjectV1 {
        self.subject
    }

    /// Returns the exact protected compiler-policy identity.
    pub const fn compiler_policy(self) -> [u8; 32] {
        self.compiler_policy
    }
}

/// Exact target capability closure retained as canonical decision bytes.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProductionTargetCapabilityClosureV5 {
    closure_identity: [u8; 32],
    neutral_graph: [u8; 32],
    neutral_graph_bytes: u64,
    neutral_epoch: u64,
    target_model: TargetModelIdentityV1,
    launch_contract: LaunchContractIdentityV1,
    canonical_bytes: Box<[u8]>,
}

impl InertProductionTargetCapabilityClosureV5 {
    /// Retains the exact closure identity, model, launch evidence, and canonical decisions.
    pub fn new(
        closure_identity: [u8; 32],
        neutral_graph: [u8; 32],
        neutral_graph_bytes: u64,
        neutral_epoch: u64,
        target_model: TargetModelIdentityV1,
        launch_contract: LaunchContractIdentityV1,
        canonical_decisions: impl Into<Vec<u8>>,
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let canonical_decisions = canonical_decisions.into();
        if closure_identity == [0; 32]
            || neutral_graph == [0; 32]
            || neutral_graph_bytes == 0
            || neutral_epoch == 0
            || target_model.digest().is_zero()
            || launch_contract.digest().is_zero()
        {
            return Err(InertProductionCapabilityHandoffErrorV5::ZeroIdentity);
        }
        require_blob(&canonical_decisions, MAX_TARGET_DECISION_BYTES)?;
        let target = target_model.digest();
        let launch = launch_contract.digest();
        let neutral_len = neutral_graph_bytes.to_le_bytes();
        let neutral_epoch_bytes = neutral_epoch.to_le_bytes();
        let fields: [&[u8]; TARGET_CLOSURE_FIELDS] = [
            TARGET_CLOSURE_DOMAIN,
            &closure_identity,
            &neutral_graph,
            &neutral_len,
            &neutral_epoch_bytes,
            target.as_bytes(),
            launch.as_bytes(),
            &canonical_decisions,
        ];
        let canonical_bytes = encode_record(
            INERT_PRODUCTION_CAPABILITY_HANDOFF_VERSION_V5,
            TARGET_CLOSURE_KIND,
            &fields,
            MAX_TARGET_DECISION_BYTES + 1024,
            TARGET_CLOSURE_IDENTITY_DOMAIN,
        )?;
        Ok(Self {
            closure_identity,
            neutral_graph,
            neutral_graph_bytes,
            neutral_epoch,
            target_model,
            launch_contract,
            canonical_bytes,
        })
    }

    /// Retains a closure for a canonical subject roster, including per-root launch contracts.
    #[allow(clippy::too_many_arguments)]
    pub fn new_for_subject_roster(
        closure_identity: [u8; 32],
        neutral_graph: [u8; 32],
        neutral_graph_bytes: u64,
        neutral_epoch: u64,
        target_model: TargetModelIdentityV1,
        subjects: &[CapabilitySubjectV1],
        canonical_decisions: impl Into<Vec<u8>>,
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        Self::new(
            closure_identity,
            neutral_graph,
            neutral_graph_bytes,
            neutral_epoch,
            target_model,
            launch_roster_identity(subjects)?,
            canonical_decisions,
        )
    }

    /// Strictly decodes an exact V5 target closure with no legacy fallback.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let record = decode_record(
            bytes,
            INERT_PRODUCTION_CAPABILITY_HANDOFF_VERSION_V5,
            TARGET_CLOSURE_KIND,
            TARGET_CLOSURE_FIELDS,
            MAX_TARGET_DECISION_BYTES + 1024,
            TARGET_CLOSURE_DOMAIN,
            TARGET_CLOSURE_IDENTITY_DOMAIN,
        )?;
        require_lengths(
            &record.fields[..7],
            &[TARGET_CLOSURE_DOMAIN.len(), 32, 32, 8, 8, 32, 32],
        )?;
        let decoded = Self::new(
            copy_32(record.fields[1])?,
            copy_32(record.fields[2])?,
            read_u64_exact(record.fields[3])?,
            read_u64_exact(record.fields[4])?,
            TargetModelIdentityV1::from_untrusted_digest(decode_digest(record.fields[5])?),
            LaunchContractIdentityV1::from_untrusted_digest(decode_digest(record.fields[6])?),
            record.fields[7].to_vec(),
        )?;
        require_canonical(decoded.canonical_bytes(), bytes)?;
        Ok(decoded)
    }

    /// Returns the producer's exact target-closure identity.
    pub const fn closure_identity(&self) -> [u8; 32] {
        self.closure_identity
    }

    /// Returns the optimized target-neutral graph answered by this closure.
    pub const fn neutral_graph(&self) -> [u8; 32] {
        self.neutral_graph
    }

    /// Returns the exact optimized graph byte length.
    pub const fn neutral_graph_bytes(&self) -> u64 {
        self.neutral_graph_bytes
    }

    /// Returns the exact optimized graph epoch answered by this closure.
    pub const fn neutral_epoch(&self) -> u64 {
        self.neutral_epoch
    }

    /// Returns the exact selected target-model identity.
    pub const fn target_model(&self) -> TargetModelIdentityV1 {
        self.target_model
    }

    /// Returns the exact launch-contract identity used by the closure.
    pub const fn launch_contract(&self) -> LaunchContractIdentityV1 {
        self.launch_contract
    }

    /// Returns the complete canonical V5 closure bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Exact final-graph analysis report and its canonical result bytes.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProductionFinalGraphReportV5 {
    final_graph: [u8; 32],
    final_graph_bytes: u64,
    final_epoch: u64,
    analysis_epoch: u64,
    checker_identity: [u8; 32],
    schedule_identity: [u8; 32],
    checked_evidence: Box<[[u8; 32]]>,
    report_identity: [u8; 32],
    w4_witness: InertProductionW4WitnessV5,
    canonical_bytes: Box<[u8]>,
}

impl InertProductionFinalGraphReportV5 {
    /// Retains one exact final graph identity, epoch, and canonical analysis result.
    pub fn new(
        final_graph: [u8; 32],
        final_graph_bytes: u64,
        final_epoch: u64,
        analysis_epoch: u64,
        checker_identity: [u8; 32],
        schedule_identity: [u8; 32],
        checked_evidence: impl Into<Vec<[u8; 32]>>,
        canonical_results: impl Into<Vec<u8>>,
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let checked_evidence = checked_evidence.into();
        let w4_witness =
            InertProductionW4WitnessV5::from_canonical_encoding(canonical_results.into())?;
        if final_graph == [0; 32]
            || final_graph_bytes == 0
            || final_epoch == 0
            || analysis_epoch == 0
            || checker_identity == [0; 32]
            || schedule_identity == [0; 32]
            || checked_evidence.len() != PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5
            || checked_evidence.iter().any(|identity| identity == &[0; 32])
        {
            return Err(InertProductionCapabilityHandoffErrorV5::ZeroIdentity);
        }
        let canonical_results = w4_witness.canonical_encoding();
        let report_identity = derive_blob_identity(FINAL_GRAPH_REPORT_DOMAIN, canonical_results);
        let graph_len = final_graph_bytes.to_le_bytes();
        let epoch = final_epoch.to_le_bytes();
        let analysis_epoch_bytes = analysis_epoch.to_le_bytes();
        let checked_evidence_bytes = encode_checked_evidence_roster(&checked_evidence);
        let fields: [&[u8]; FINAL_GRAPH_REPORT_FIELDS] = [
            FINAL_GRAPH_REPORT_DOMAIN,
            &final_graph,
            &graph_len,
            &epoch,
            &analysis_epoch_bytes,
            &checker_identity,
            &schedule_identity,
            &checked_evidence_bytes,
            &report_identity,
            &canonical_results,
        ];
        let canonical_bytes = encode_record(
            INERT_PRODUCTION_CAPABILITY_HANDOFF_VERSION_V5,
            FINAL_GRAPH_REPORT_KIND,
            &fields,
            MAX_INERT_PRODUCTION_W4_WITNESS_BYTES_V5 + 1024,
            FINAL_GRAPH_REPORT_IDENTITY_DOMAIN,
        )?;
        Ok(Self {
            final_graph,
            final_graph_bytes,
            final_epoch,
            analysis_epoch,
            checker_identity,
            schedule_identity,
            checked_evidence: checked_evidence.into_boxed_slice(),
            report_identity,
            w4_witness,
            canonical_bytes,
        })
    }

    /// Strictly decodes an exact V5 final-graph report.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let record = decode_record(
            bytes,
            INERT_PRODUCTION_CAPABILITY_HANDOFF_VERSION_V5,
            FINAL_GRAPH_REPORT_KIND,
            FINAL_GRAPH_REPORT_FIELDS,
            MAX_INERT_PRODUCTION_W4_WITNESS_BYTES_V5 + 1024,
            FINAL_GRAPH_REPORT_DOMAIN,
            FINAL_GRAPH_REPORT_IDENTITY_DOMAIN,
        )?;
        require_lengths(
            &record.fields[..7],
            &[FINAL_GRAPH_REPORT_DOMAIN.len(), 32, 8, 8, 8, 32, 32],
        )?;
        let decoded = Self::new(
            copy_32(record.fields[1])?,
            read_u64_exact(record.fields[2])?,
            read_u64_exact(record.fields[3])?,
            read_u64_exact(record.fields[4])?,
            copy_32(record.fields[5])?,
            copy_32(record.fields[6])?,
            decode_checked_evidence_roster(record.fields[7])?,
            record.fields[9].to_vec(),
        )?;
        if decoded.report_identity != copy_32(record.fields[8])? {
            return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
                "final graph report results",
            ));
        }
        require_canonical(decoded.canonical_bytes(), bytes)?;
        Ok(decoded)
    }

    /// Returns the exact final canonical KIR digest.
    pub const fn final_graph(&self) -> [u8; 32] {
        self.final_graph
    }

    /// Returns the exact final canonical KIR byte length.
    pub const fn final_graph_bytes(&self) -> u64 {
        self.final_graph_bytes
    }

    /// Returns the exact final optimization epoch.
    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    /// Returns the exact live PLIRON epoch checked by W4.
    pub const fn analysis_epoch(&self) -> u64 {
        self.analysis_epoch
    }

    /// Returns the exact W4 checker implementation identity.
    pub const fn checker_identity(&self) -> [u8; 32] {
        self.checker_identity
    }

    /// Returns the identity of the complete ordered W4 obligation schedule result.
    pub const fn schedule_identity(&self) -> [u8; 32] {
        self.schedule_identity
    }

    /// Returns each exact W4 obligation evidence identity in frozen schedule order.
    pub fn checked_evidence(&self) -> &[[u8; 32]] {
        &self.checked_evidence
    }

    /// Returns the identity of the exact canonical analysis result bytes.
    pub const fn report_identity(&self) -> [u8; 32] {
        self.report_identity
    }

    /// Returns the exact authority-free analysis result bytes bound by this report.
    pub fn canonical_results(&self) -> &[u8] {
        self.w4_witness.canonical_encoding()
    }

    /// Returns typed custody of the exact W4 witness payload.
    pub const fn w4_witness(&self) -> &InertProductionW4WitnessV5 {
        &self.w4_witness
    }

    /// Returns the complete canonical report record.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Closed compiler outputs measured by the production worker stages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProductionCompilerOutputStageV5 {
    /// Exact final LLVM module bytes.
    Llvm = 1,
    /// Exact linked object/code-object bytes.
    Object = 2,
}

/// Move-only measurement receipt for one exact compiler-stage output.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCompilerStageOutputReceiptV5 {
    stage: ProductionCompilerOutputStageV5,
    output_sha256: [u8; 32],
    output_bytes: u64,
    identity: InertCompilerStageOutputReceiptIdentityV5,
    canonical_bytes: Box<[u8]>,
}

impl InertCompilerStageOutputReceiptV5 {
    /// Measures exact nonempty bytes at the LLVM or object-producing stage.
    pub fn from_stage_output(
        stage: ProductionCompilerOutputStageV5,
        output: &[u8],
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        if output.is_empty() {
            return Err(InertProductionCapabilityHandoffErrorV5::EmptyField);
        }
        let output_bytes = u64::try_from(output.len())
            .map_err(|_| InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
        let output_sha256 = Sha256::digest(output).into();
        Self::from_measurement(stage, output_sha256, output_bytes)
    }

    fn from_measurement(
        stage: ProductionCompilerOutputStageV5,
        output_sha256: [u8; 32],
        output_bytes: u64,
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        if output_sha256 == [0; 32] || output_bytes == 0 {
            return Err(InertProductionCapabilityHandoffErrorV5::ZeroIdentity);
        }
        let stage_bytes = [stage as u8];
        let output_len = output_bytes.to_le_bytes();
        let fields: [&[u8]; OUTPUT_RECEIPT_FIELDS] = [
            OUTPUT_RECEIPT_DOMAIN,
            &stage_bytes,
            &output_sha256,
            &output_len,
        ];
        let canonical_bytes = encode_record(
            INERT_PRODUCTION_CAPABILITY_RESULT_VERSION_V5,
            OUTPUT_RECEIPT_KIND,
            &fields,
            MAX_OUTPUT_RECEIPT_BYTES,
            OUTPUT_RECEIPT_IDENTITY_DOMAIN,
        )?;
        let identity = InertCompilerStageOutputReceiptIdentityV5 {
            sha256: terminal_identity(&canonical_bytes)?,
            byte_len: canonical_bytes.len() as u64,
        };
        Ok(Self {
            stage,
            output_sha256,
            output_bytes,
            identity,
            canonical_bytes,
        })
    }

    /// Strictly decodes one exact output receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let record = decode_record(
            bytes,
            INERT_PRODUCTION_CAPABILITY_RESULT_VERSION_V5,
            OUTPUT_RECEIPT_KIND,
            OUTPUT_RECEIPT_FIELDS,
            MAX_OUTPUT_RECEIPT_BYTES,
            OUTPUT_RECEIPT_DOMAIN,
            OUTPUT_RECEIPT_IDENTITY_DOMAIN,
        )?;
        require_lengths(&record.fields, &[OUTPUT_RECEIPT_DOMAIN.len(), 1, 32, 8])?;
        let stage = match record.fields[1][0] {
            1 => ProductionCompilerOutputStageV5::Llvm,
            2 => ProductionCompilerOutputStageV5::Object,
            _ => return Err(InertProductionCapabilityHandoffErrorV5::WrongOutputStage),
        };
        let decoded = Self::from_measurement(
            stage,
            copy_32(record.fields[2])?,
            read_u64_exact(record.fields[3])?,
        )?;
        require_canonical(decoded.canonical_bytes(), bytes)?;
        Ok(decoded)
    }

    /// Returns the exact stage that produced the measured bytes.
    pub const fn stage(&self) -> ProductionCompilerOutputStageV5 {
        self.stage
    }

    /// Returns the raw SHA-256 of the exact output bytes.
    pub const fn output_sha256(&self) -> [u8; 32] {
        self.output_sha256
    }

    /// Returns the exact output byte length.
    pub const fn output_bytes(&self) -> u64 {
        self.output_bytes
    }

    /// Returns the domain-separated receipt identity.
    pub const fn identity(&self) -> InertCompilerStageOutputReceiptIdentityV5 {
        self.identity
    }

    /// Returns the complete canonical receipt bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Move-only input to the sole production worker transaction for native KIR V13.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProductionCapabilityHandoffV5 {
    legacy_handoff: InertSemanticCompilerModuleHandoffV3,
    executable_kir: InertCanonicalKernelIrV13ReceiptV5,
    proof_lineage: InertMultiRootProofLineageV3,
    inputs: InertProductionCapabilityHandoffInputsV5,
    source_refinement: InertCapabilityRefinementReceiptV1,
    subjects: Box<[CapabilitySubjectV1]>,
    obligations: Box<[InertCapabilityObligationSetV1]>,
    target_closure: InertProductionTargetCapabilityClosureV5,
    final_graph_report: InertProductionFinalGraphReportV5,
    identity: InertProductionCapabilityHandoffIdentityV5,
    canonical_bytes: Box<[u8]>,
}

impl InertProductionCapabilityHandoffV5 {
    /// Joins exact source-stage custody to exact target-lowered V13 graph custody.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        legacy_handoff: InertSemanticCompilerModuleHandoffV3,
        executable_kir: InertCanonicalKernelIrV13ReceiptV5,
        proof_lineage: InertMultiRootProofLineageV3,
        inputs: InertProductionCapabilityHandoffInputsV5,
        source_refinement: InertCapabilityRefinementReceiptV1,
        obligations: InertCapabilityObligationSetV1,
        target_closure: InertProductionTargetCapabilityClosureV5,
        final_graph_report: InertProductionFinalGraphReportV5,
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        Self::new_multi_root(
            legacy_handoff,
            executable_kir,
            proof_lineage,
            inputs,
            source_refinement,
            vec![obligations],
            target_closure,
            final_graph_report,
        )
    }

    /// Joins a complete canonical root roster to the exact final V13 graph.
    #[allow(clippy::too_many_arguments)]
    pub fn new_multi_root(
        legacy_handoff: InertSemanticCompilerModuleHandoffV3,
        executable_kir: InertCanonicalKernelIrV13ReceiptV5,
        proof_lineage: InertMultiRootProofLineageV3,
        inputs: InertProductionCapabilityHandoffInputsV5,
        source_refinement: InertCapabilityRefinementReceiptV1,
        obligations: Vec<InertCapabilityObligationSetV1>,
        target_closure: InertProductionTargetCapabilityClosureV5,
        final_graph_report: InertProductionFinalGraphReportV5,
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let subjects = obligations
            .iter()
            .map(InertCapabilityObligationSetV1::subject)
            .collect::<Vec<_>>();
        validate_handoff_parts(
            &legacy_handoff,
            &executable_kir,
            &proof_lineage,
            inputs,
            &source_refinement,
            &subjects,
            &obligations,
            &target_closure,
            &final_graph_report,
        )?;
        let semantic = inputs.semantic_mir_identity;
        let subject_roster = encode_subject_roster(&subjects)?;
        let obligation_roster = encode_obligation_roster(&obligations)?;
        let policy = inputs.compiler_policy;
        let source_kind = [source_refinement.kind() as u8];
        let fields: [&[u8]; HANDOFF_FIELDS] = [
            HANDOFF_DOMAIN,
            legacy_handoff.canonical_bytes(),
            executable_kir.canonical_preimage(),
            proof_lineage.canonical_bytes(),
            &semantic,
            &subject_roster,
            &policy,
            &source_kind,
            source_refinement.canonical_preimage(),
            &obligation_roster,
            target_closure.canonical_bytes(),
            final_graph_report.canonical_bytes(),
        ];
        let canonical_bytes = encode_record(
            INERT_PRODUCTION_CAPABILITY_HANDOFF_VERSION_V5,
            HANDOFF_KIND,
            &fields,
            MAX_INERT_PRODUCTION_CAPABILITY_HANDOFF_BYTES_V5,
            HANDOFF_IDENTITY_DOMAIN,
        )?;
        let identity = InertProductionCapabilityHandoffIdentityV5 {
            sha256: terminal_identity(&canonical_bytes)?,
            byte_len: canonical_bytes.len() as u64,
        };
        Ok(Self {
            legacy_handoff,
            executable_kir,
            proof_lineage,
            inputs,
            source_refinement,
            subjects: subjects.into_boxed_slice(),
            obligations: obligations.into_boxed_slice(),
            target_closure,
            final_graph_report,
            identity,
            canonical_bytes,
        })
    }

    /// Strictly decodes one complete native V5 handoff without V3/V8 projection.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let record = decode_record(
            bytes,
            INERT_PRODUCTION_CAPABILITY_HANDOFF_VERSION_V5,
            HANDOFF_KIND,
            HANDOFF_FIELDS,
            MAX_INERT_PRODUCTION_CAPABILITY_HANDOFF_BYTES_V5,
            HANDOFF_DOMAIN,
            HANDOFF_IDENTITY_DOMAIN,
        )?;
        require_lengths(
            &record.fields[..5],
            &[
                HANDOFF_DOMAIN.len(),
                record.fields[1].len(),
                record.fields[2].len(),
                record.fields[3].len(),
                32,
            ],
        )?;
        require_lengths(&record.fields[6..8], &[32, 1])?;
        let subjects = decode_subject_roster(record.fields[5])?;
        let obligations = decode_obligation_roster(record.fields[9])?;
        let source_kind = match record.fields[7][0] {
            0 => InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
            1 => InertCapabilityRefinementReceiptKindV1::Machine,
            _ => return Err(InertProductionCapabilityHandoffErrorV5::WrongRefinementKind),
        };
        let decoded = Self::new_multi_root(
            InertSemanticCompilerModuleHandoffV3::decode(record.fields[1])?,
            InertCanonicalKernelIrV13ReceiptV5::from_canonical_preimage(record.fields[2].to_vec())?,
            InertMultiRootProofLineageV3::decode(record.fields[3])?,
            InertProductionCapabilityHandoffInputsV5::new(
                copy_32(record.fields[4])?,
                *subjects
                    .first()
                    .ok_or(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount)?,
                copy_32(record.fields[6])?,
            )?,
            InertCapabilityRefinementReceiptV1::from_canonical_preimage(
                source_kind,
                record.fields[8].to_vec(),
            )?,
            obligations,
            InertProductionTargetCapabilityClosureV5::decode(record.fields[10])?,
            InertProductionFinalGraphReportV5::decode(record.fields[11])?,
        )?;
        require_canonical(decoded.canonical_bytes(), bytes)?;
        Ok(decoded)
    }

    /// Returns the exact terminal handoff identity.
    pub const fn identity(&self) -> InertProductionCapabilityHandoffIdentityV5 {
        self.identity
    }

    /// Returns the complete canonical V5 handoff bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the frozen V3 semantic/module owner carried without reinterpretation.
    pub const fn legacy_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        &self.legacy_handoff
    }

    /// Returns exact canonical KIR V13 receipt custody.
    pub const fn executable_kir(&self) -> &InertCanonicalKernelIrV13ReceiptV5 {
        &self.executable_kir
    }

    /// Returns the native multi-root V13 lineage owner.
    pub const fn proof_lineage(&self) -> &InertMultiRootProofLineageV3 {
        &self.proof_lineage
    }

    /// Returns the exact immutable handoff coordinates.
    pub const fn inputs(&self) -> InertProductionCapabilityHandoffInputsV5 {
        self.inputs
    }

    /// Returns the exact source refinement receipt supplied by its source stage.
    pub const fn source_refinement(&self) -> &InertCapabilityRefinementReceiptV1 {
        &self.source_refinement
    }

    /// Returns every exact subject in canonical descriptor-kernel order.
    pub fn subjects(&self) -> &[CapabilitySubjectV1] {
        &self.subjects
    }

    /// Returns the exact capability obligation set.
    pub const fn obligations(&self) -> &InertCapabilityObligationSetV1 {
        &self.obligations[0]
    }

    /// Returns one exact obligation set for every canonical root.
    pub fn obligation_roster(&self) -> &[InertCapabilityObligationSetV1] {
        &self.obligations
    }

    /// Returns exact target closure custody.
    pub const fn target_closure(&self) -> &InertProductionTargetCapabilityClosureV5 {
        &self.target_closure
    }

    /// Returns exact final-graph report custody.
    pub const fn final_graph_report(&self) -> &InertProductionFinalGraphReportV5 {
        &self.final_graph_report
    }
}

/// Exact, authority-free canonical simulation Bundle V8 custody.
///
/// This crate deliberately does not depend on `fe2o3-kernel-ir`. The producer must pass the
/// digest from `VerifiedSimulationBundleV8::identity()` with its exact canonical bytes. This
/// type rederives that native digest and validates the closed outer V8 envelope and bounds. A
/// downstream sealed verifier must still consume these bytes through
/// `VerifiedSimulationBundleV8::from_canonical_bytes`; possession of this inert value grants no
/// simulation, compiler, publication, load, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertSimulationBundleV8 {
    identity: InertSimulationBundleIdentityV8,
    canonical_bytes: Box<[u8]>,
}

impl InertSimulationBundleV8 {
    /// Retains the exact canonical bytes from one already verified Bundle V8.
    pub fn from_verified_canonical_bytes(
        native_identity: [u8; 32],
        canonical_bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, InertSimulationBundleErrorV8> {
        let canonical_bytes = canonical_bytes.into();
        validate_simulation_bundle_envelope_v8(&canonical_bytes)?;
        let derived = derive_simulation_bundle_identity_v8(&canonical_bytes);
        if native_identity == [0; 32] || native_identity != derived {
            return Err(InertSimulationBundleErrorV8::IdentityMismatch);
        }
        Ok(Self {
            identity: InertSimulationBundleIdentityV8 {
                sha256: derived,
                byte_len: canonical_bytes.len() as u64,
            },
            canonical_bytes: canonical_bytes.into_boxed_slice(),
        })
    }

    /// Returns the native Bundle V8 content identity and exact byte length.
    pub const fn identity(&self) -> InertSimulationBundleIdentityV8 {
        self.identity
    }

    /// Returns the exact canonical bytes for strict downstream Bundle V8 decoding.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Move-only, authority-free input payload for one durable V5 plus Bundle V8 occurrence.
///
/// Its single canonical identity commits to both complete children. Neither child can be omitted,
/// reordered, independently replaced, or downgraded while preserving this payload identity.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProductionCapabilityTransactionV5 {
    handoff: InertProductionCapabilityHandoffV5,
    simulation_bundle: InertSimulationBundleV8,
    identity: InertProductionCapabilityTransactionIdentityV5,
    canonical_bytes: Box<[u8]>,
}

impl InertProductionCapabilityTransactionV5 {
    /// Joins exact V5 handoff and Bundle V8 custody without creating authority.
    pub fn new(
        handoff: InertProductionCapabilityHandoffV5,
        simulation_bundle: InertSimulationBundleV8,
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let bundle_identity = simulation_bundle.identity();
        let bundle_sha256 = bundle_identity.sha256();
        let bundle_length = bundle_identity.byte_len().to_le_bytes();
        let fields: [&[u8]; TRANSACTION_FIELDS] = [
            TRANSACTION_DOMAIN,
            handoff.canonical_bytes(),
            &bundle_sha256,
            &bundle_length,
            simulation_bundle.canonical_bytes(),
        ];
        let canonical_bytes = encode_record(
            INERT_PRODUCTION_CAPABILITY_TRANSACTION_VERSION_V5,
            TRANSACTION_KIND,
            &fields,
            MAX_INERT_PRODUCTION_CAPABILITY_TRANSACTION_BYTES_V5,
            TRANSACTION_IDENTITY_DOMAIN,
        )?;
        let identity = InertProductionCapabilityTransactionIdentityV5 {
            sha256: terminal_identity(&canonical_bytes)?,
            byte_len: canonical_bytes.len() as u64,
        };
        Ok(Self {
            handoff,
            simulation_bundle,
            identity,
            canonical_bytes,
        })
    }

    /// Strictly decodes the sole complete V5 plus Bundle V8 transaction payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let record = decode_record(
            bytes,
            INERT_PRODUCTION_CAPABILITY_TRANSACTION_VERSION_V5,
            TRANSACTION_KIND,
            TRANSACTION_FIELDS,
            MAX_INERT_PRODUCTION_CAPABILITY_TRANSACTION_BYTES_V5,
            TRANSACTION_DOMAIN,
            TRANSACTION_IDENTITY_DOMAIN,
        )?;
        require_lengths(
            &record.fields[..4],
            &[TRANSACTION_DOMAIN.len(), record.fields[1].len(), 32, 8],
        )?;
        let bundle_length = read_u64_exact(record.fields[3])?;
        if usize::try_from(bundle_length).ok() != Some(record.fields[4].len()) {
            return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
                "simulation Bundle V8 length",
            ));
        }
        let decoded = Self::new(
            InertProductionCapabilityHandoffV5::decode(record.fields[1])?,
            InertSimulationBundleV8::from_verified_canonical_bytes(
                copy_32(record.fields[2])?,
                record.fields[4].to_vec(),
            )?,
        )?;
        require_canonical(decoded.canonical_bytes(), bytes)?;
        Ok(decoded)
    }

    /// Returns the canonical pair identity used by durable transaction binding.
    pub const fn identity(&self) -> InertProductionCapabilityTransactionIdentityV5 {
        self.identity
    }

    /// Returns the exact V5 handoff retained in this transaction.
    pub const fn handoff(&self) -> &InertProductionCapabilityHandoffV5 {
        &self.handoff
    }

    /// Returns the exact Bundle V8 bytes and native identity retained in this transaction.
    pub const fn simulation_bundle(&self) -> &InertSimulationBundleV8 {
        &self.simulation_bundle
    }

    /// Returns the complete canonical pair bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Completed production worker result retaining all input and output proof custody.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProductionCapabilityResultV5 {
    transaction: InertProductionCapabilityTransactionV5,
    llvm_output: InertCompilerStageOutputReceiptV5,
    object_output: InertCompilerStageOutputReceiptV5,
    machine_refinement: InertCapabilityRefinementReceiptV1,
    capability_association: InertMultiRootStaticCapabilityEvidenceAssociationV1,
    proof_owner: InertCompilerProofOwnerV5,
    identity: InertProductionCapabilityResultIdentityV5,
    canonical_bytes: Box<[u8]>,
}

impl InertProductionCapabilityResultV5 {
    /// Completes carriage only when every producer-owned output binds the exact input subject.
    pub fn new(
        transaction: InertProductionCapabilityTransactionV5,
        llvm_output: InertCompilerStageOutputReceiptV5,
        object_output: InertCompilerStageOutputReceiptV5,
        machine_refinement: InertCapabilityRefinementReceiptV1,
        capability_association: InertStaticCapabilityEvidenceAssociationV1,
        proof_owner: InertCompilerProofOwnerV5,
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        Self::new_multi_root(
            transaction,
            llvm_output,
            object_output,
            machine_refinement,
            InertMultiRootStaticCapabilityEvidenceAssociationV1::new(vec![capability_association])?,
            proof_owner,
        )
    }

    /// Completes carriage for one exact association per canonical kernel root.
    pub fn new_multi_root(
        transaction: InertProductionCapabilityTransactionV5,
        llvm_output: InertCompilerStageOutputReceiptV5,
        object_output: InertCompilerStageOutputReceiptV5,
        machine_refinement: InertCapabilityRefinementReceiptV1,
        capability_association: InertMultiRootStaticCapabilityEvidenceAssociationV1,
        proof_owner: InertCompilerProofOwnerV5,
    ) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        validate_result_parts(
            transaction.handoff(),
            &llvm_output,
            &object_output,
            &machine_refinement,
            &capability_association,
            &proof_owner,
        )?;
        let machine_kind = [machine_refinement.kind() as u8];
        let fields: [&[u8]; RESULT_FIELDS] = [
            RESULT_DOMAIN,
            transaction.canonical_bytes(),
            llvm_output.canonical_bytes(),
            object_output.canonical_bytes(),
            &machine_kind,
            machine_refinement.canonical_preimage(),
            capability_association.canonical_bytes(),
            proof_owner.canonical_bytes(),
        ];
        let canonical_bytes = encode_record(
            INERT_PRODUCTION_CAPABILITY_RESULT_VERSION_V5,
            RESULT_KIND,
            &fields,
            MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5,
            RESULT_IDENTITY_DOMAIN,
        )?;
        let identity = InertProductionCapabilityResultIdentityV5 {
            sha256: terminal_identity(&canonical_bytes)?,
            byte_len: canonical_bytes.len() as u64,
        };
        Ok(Self {
            transaction,
            llvm_output,
            object_output,
            machine_refinement,
            capability_association,
            proof_owner,
            identity,
            canonical_bytes,
        })
    }

    /// Strictly decodes one complete V5 result with no incomplete or downgraded form.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertProductionCapabilityHandoffErrorV5> {
        let record = decode_record(
            bytes,
            INERT_PRODUCTION_CAPABILITY_RESULT_VERSION_V5,
            RESULT_KIND,
            RESULT_FIELDS,
            MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5,
            RESULT_DOMAIN,
            RESULT_IDENTITY_DOMAIN,
        )?;
        require_lengths(
            &record.fields[..5],
            &[
                RESULT_DOMAIN.len(),
                record.fields[1].len(),
                record.fields[2].len(),
                record.fields[3].len(),
                1,
            ],
        )?;
        let machine_kind = match record.fields[4][0] {
            0 => InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
            1 => InertCapabilityRefinementReceiptKindV1::Machine,
            _ => return Err(InertProductionCapabilityHandoffErrorV5::WrongRefinementKind),
        };
        let decoded = Self::new_multi_root(
            InertProductionCapabilityTransactionV5::decode(record.fields[1])?,
            InertCompilerStageOutputReceiptV5::decode(record.fields[2])?,
            InertCompilerStageOutputReceiptV5::decode(record.fields[3])?,
            InertCapabilityRefinementReceiptV1::from_canonical_preimage(
                machine_kind,
                record.fields[5].to_vec(),
            )?,
            InertMultiRootStaticCapabilityEvidenceAssociationV1::decode(record.fields[6])?,
            InertCompilerProofOwnerV5::decode(record.fields[7])?,
        )?;
        require_canonical(decoded.canonical_bytes(), bytes)?;
        Ok(decoded)
    }

    /// Returns the exact completed result identity.
    pub const fn identity(&self) -> InertProductionCapabilityResultIdentityV5 {
        self.identity
    }

    /// Returns the exact same-transaction input retained through completion.
    pub const fn transaction(&self) -> &InertProductionCapabilityTransactionV5 {
        &self.transaction
    }

    /// Returns the exact input handoff retained through completion.
    pub const fn handoff(&self) -> &InertProductionCapabilityHandoffV5 {
        self.transaction.handoff()
    }

    /// Returns exact Bundle V8 custody retained through completion.
    pub const fn simulation_bundle(&self) -> &InertSimulationBundleV8 {
        self.transaction.simulation_bundle()
    }

    /// Returns the exact LLVM output receipt.
    pub const fn llvm_output(&self) -> &InertCompilerStageOutputReceiptV5 {
        &self.llvm_output
    }

    /// Returns the exact object output receipt.
    pub const fn object_output(&self) -> &InertCompilerStageOutputReceiptV5 {
        &self.object_output
    }

    /// Returns the exact machine-refinement receipt supplied by its established stage.
    pub const fn machine_refinement(&self) -> &InertCapabilityRefinementReceiptV1 {
        &self.machine_refinement
    }

    /// Returns the exact static capability association.
    pub fn capability_association(&self) -> &InertStaticCapabilityEvidenceAssociationV1 {
        &self.capability_association.entries()[0]
    }

    /// Returns the complete canonical capability-association roster.
    pub const fn capability_associations(
        &self,
    ) -> &InertMultiRootStaticCapabilityEvidenceAssociationV1 {
        &self.capability_association
    }

    /// Returns the native exact-V13 V5 proof owner.
    pub const fn proof_owner(&self) -> &InertCompilerProofOwnerV5 {
        &self.proof_owner
    }

    /// Returns the complete canonical result bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

fn validate_handoff_parts(
    legacy_handoff: &InertSemanticCompilerModuleHandoffV3,
    executable_kir: &InertCanonicalKernelIrV13ReceiptV5,
    proof_lineage: &InertMultiRootProofLineageV3,
    inputs: InertProductionCapabilityHandoffInputsV5,
    source_refinement: &InertCapabilityRefinementReceiptV1,
    subjects: &[CapabilitySubjectV1],
    obligations: &[InertCapabilityObligationSetV1],
    target_closure: &InertProductionTargetCapabilityClosureV5,
    final_graph_report: &InertProductionFinalGraphReportV5,
) -> Result<(), InertProductionCapabilityHandoffErrorV5> {
    if source_refinement.kind() != InertCapabilityRefinementReceiptKindV1::SourceMirToKir {
        return Err(InertProductionCapabilityHandoffErrorV5::WrongRefinementKind);
    }
    if subjects.first().copied() != Some(inputs.subject)
        || subjects.len() != obligations.len()
        || subjects
            .iter()
            .zip(obligations)
            .any(|(subject, obligation)| obligation.subject() != *subject)
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "capability obligation roster",
        ));
    }
    validate_legacy_lineage(legacy_handoff, proof_lineage, inputs)?;
    validate_subject_roster(proof_lineage, subjects)?;
    source_refinement.validate_source_subject_roster_v1(proof_lineage, subjects)?;
    let kir_sha256: [u8; 32] = Sha256::digest(executable_kir.canonical_preimage()).into();
    let kir_len = executable_kir.canonical_preimage().len() as u64;
    let neutral = proof_lineage.neutral_kir();
    if subjects.iter().any(|subject| {
        subject.executable_kir().digest().as_bytes() != &kir_sha256
            || subject.executable_kir_epoch() != neutral.graph_epoch()
    }) || neutral.digest() != kir_sha256
        || neutral.canonical_length() != kir_len
        || final_graph_report.final_graph != kir_sha256
        || final_graph_report.final_graph_bytes != kir_len
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "canonical KIR V13",
        ));
    }
    if neutral.graph_epoch() != final_graph_report.final_epoch {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "final graph epoch",
        ));
    }
    if proof_lineage
        .roster(MultiRootProofRosterKindV3::MiddleEnd)
        .semantic_mir_sha256()
        != inputs.semantic_mir_identity
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "semantic MIR",
        ));
    }
    if subjects
        .iter()
        .any(|subject| target_closure.target_model != subject.target_model())
        || target_closure.launch_contract != launch_roster_identity(subjects)?
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "target capability closure",
        ));
    }
    if target_closure.neutral_graph != final_graph_report.final_graph
        || target_closure.neutral_graph_bytes != final_graph_report.final_graph_bytes
        || target_closure.neutral_epoch != final_graph_report.final_epoch
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "target-closure final graph",
        ));
    }
    validate_live_target_closure(legacy_handoff, proof_lineage, subjects, target_closure)?;
    Ok(())
}

fn validate_legacy_lineage(
    legacy_handoff: &InertSemanticCompilerModuleHandoffV3,
    proof_lineage: &InertMultiRootProofLineageV3,
    inputs: InertProductionCapabilityHandoffInputsV5,
) -> Result<(), InertProductionCapabilityHandoffErrorV5> {
    let capsule = legacy_handoff.capsule();
    let receipts = capsule.receipts();
    let semantic = receipts.semantic_mir().identity();
    if *semantic.sha256() != inputs.semantic_mir_identity {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "legacy semantic MIR",
        ));
    }

    let proof =
        InertProofBindingAssociationV4::decode(receipts.proof_binding().canonical_preimage())
            .map_err(|_| {
                InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
                    "legacy proof-binding lineage",
                )
            })?;
    let proof_inputs = proof.inputs();
    let expected_proof_inputs = [
        lineage_identity(*semantic.sha256(), semantic.byte_len())?,
        lineage_identity(
            *receipts.middle_end().identity().sha256(),
            receipts.middle_end().identity().byte_len(),
        )?,
        lineage_identity(
            *receipts.kernel_ir().identity().sha256(),
            receipts.kernel_ir().identity().byte_len(),
        )?,
        lineage_identity(
            *receipts.mir_to_kir_correspondence().identity().sha256(),
            receipts.mir_to_kir_correspondence().identity().byte_len(),
        )?,
        lineage_identity(
            *receipts.formal_memory().identity().sha256(),
            receipts.formal_memory().identity().byte_len(),
        )?,
    ];
    if [
        proof_inputs.semantic_mir(),
        proof_inputs.middle_end(),
        proof_inputs.kernel_ir(),
        proof_inputs.mir_to_kir_correspondence(),
        proof_inputs.formal_memory(),
    ] != expected_proof_inputs
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "legacy proof-binding lineage",
        ));
    }

    let semantic = TargetLineageIdentityV3::new(*semantic.sha256(), semantic.byte_len())
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::ZeroIdentity)?;
    let neutral_bytes = receipts.kernel_ir().canonical_preimage();
    let neutral = TargetLineageIdentityV3::new(
        Sha256::digest(neutral_bytes).into(),
        neutral_bytes.len() as u64,
    )
    .map_err(|_| InertProductionCapabilityHandoffErrorV5::ZeroIdentity)?;
    let configured_target = capsule.target().to_string();
    let target_binding = receipts.target_binding().canonical_preimage();
    if let Ok(binding) = MultiRootTargetBindingTranscriptV2::decode(target_binding) {
        let roster = proof_lineage.roster(MultiRootProofRosterKindV3::MiddleEnd);
        if binding.semantic_mir() != semantic
            || binding.target_neutral_kir() != neutral
            || binding.configured_target() != configured_target
            || binding.roster_identity() != roster.roster_identity()
            || binding.code_object_version()
                != u16::from(
                    legacy_handoff
                        .module_handoff()
                        .code_object_version()
                        .number(),
                )
            || binding.root_count() != roster.root_count()
        {
            return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
                "legacy target-binding lineage",
            ));
        }
        for (subject_index, root_index) in roster.canonical_kernel_order().iter().enumerate() {
            let root = roster
                .root(*root_index as usize)
                .ok_or(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount)?;
            let workgroup = binding
                .workgroup(subject_index)
                .ok_or(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount)?;
            if workgroup.kernel() != root.kernel_id() || workgroup.workgroup() != root.workgroup() {
                return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
                    "legacy target workgroup roster",
                ));
            }
        }
        return Ok(());
    }

    let binding = TargetBindingTranscriptV3::decode(target_binding).map_err(|_| {
        InertProductionCapabilityHandoffErrorV5::IdentityMismatch("legacy target-binding lineage")
    })?;
    let binding = binding.inputs().map_err(|_| {
        InertProductionCapabilityHandoffErrorV5::IdentityMismatch("legacy target-binding lineage")
    })?;
    let roster = proof_lineage.roster(MultiRootProofRosterKindV3::MiddleEnd);
    let root = roster
        .root(
            *roster
                .canonical_kernel_order()
                .first()
                .ok_or(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount)?
                as usize,
        )
        .ok_or(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount)?;
    if roster.root_count() != 1
        || binding.semantic_mir != semantic
        || binding.target_neutral_kir != neutral
        || binding.configured_target != configured_target
        || binding.code_object_version
            != u16::from(
                legacy_handoff
                    .module_handoff()
                    .code_object_version()
                    .number(),
            )
        || binding.default_workgroup != root.workgroup()
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "legacy target-binding lineage",
        ));
    }
    Ok(())
}

fn validate_live_target_closure(
    legacy_handoff: &InertSemanticCompilerModuleHandoffV3,
    proof_lineage: &InertMultiRootProofLineageV3,
    subjects: &[CapabilitySubjectV1],
    target_closure: &InertProductionTargetCapabilityClosureV5,
) -> Result<(), InertProductionCapabilityHandoffErrorV5> {
    let record = decode_record(
        target_closure.canonical_bytes(),
        INERT_PRODUCTION_CAPABILITY_HANDOFF_VERSION_V5,
        TARGET_CLOSURE_KIND,
        TARGET_CLOSURE_FIELDS,
        MAX_TARGET_DECISION_BYTES + 1024,
        TARGET_CLOSURE_DOMAIN,
        TARGET_CLOSURE_IDENTITY_DOMAIN,
    )?;
    let closure = record.fields[7];
    if closure.len() < LIVE_TARGET_CLOSURE_HEADER_BYTES_V1
        || closure.get(..8) != Some(LIVE_TARGET_CLOSURE_MAGIC_V1)
        || read_u16(closure, 8)? != LIVE_TARGET_CLOSURE_VERSION_V1
        || closure[10] != 13
        || read_u16(closure, 155)? == 0
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "live target closure header",
        ));
    }
    let closure_identity = derive_live_target_closure_identity(closure);
    let launch_evidence = copy_32(&closure[123..155])?;
    if closure_identity != target_closure.closure_identity
        || copy_32(&closure[11..43])? != target_closure.neutral_graph
        || read_u64(closure, 43)? != target_closure.neutral_graph_bytes
        || read_u64(closure, 51)? != target_closure.neutral_epoch
        || launch_evidence == [0; 32]
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "live target closure identity",
        ));
    }

    let profile = ProductionAmdTargetProfileV1::from_device_target(
        &legacy_handoff.capsule().target().to_string(),
    )
    .ok_or(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
        "legacy target profile",
    ))?;
    let (profile_fingerprint, revision_fingerprint) = target_model_fingerprints(profile);
    let mut expected_model = [0_u8; 64];
    for (index, word) in profile_fingerprint
        .into_iter()
        .chain(revision_fingerprint)
        .enumerate()
    {
        expected_model[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }
    if closure[59..123] != expected_model
        || derive_capability_target_model(&expected_model)
            != *target_closure.target_model.digest().as_bytes()
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "legacy target capability model",
        ));
    }

    let roster = proof_lineage.roster(MultiRootProofRosterKindV3::MiddleEnd);
    for (subject_index, root_index) in roster.canonical_kernel_order().iter().enumerate() {
        let root = roster
            .root(*root_index as usize)
            .ok_or(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount)?;
        let mut digest = Sha256::new();
        digest.update(CAPABILITY_ROOT_LAUNCH_DOMAIN_V5);
        digest.update(launch_evidence);
        digest.update((subject_index as u64).to_le_bytes());
        digest.update(root.semantic_root().to_le_bytes());
        digest.update(root.semantic_root_identity());
        digest.update(root.kernel_binding());
        digest.update([root.source_rank()]);
        for dimension in root.workgroup() {
            digest.update(dimension.to_le_bytes());
        }
        digest.update((root.kernel_id().len() as u64).to_le_bytes());
        digest.update(root.kernel_id().as_bytes());
        if digest.finalize().as_slice()
            != subjects[subject_index]
                .launch_contract()
                .digest()
                .as_bytes()
        {
            return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
                "per-root launch contract",
            ));
        }
    }
    Ok(())
}

fn derive_live_target_closure_identity(canonical_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((LIVE_TARGET_CLOSURE_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(LIVE_TARGET_CLOSURE_IDENTITY_DOMAIN_V1);
    digest.update((canonical_bytes.len() as u64).to_le_bytes());
    digest.update(canonical_bytes);
    digest.finalize().into()
}

fn derive_capability_target_model(model_fingerprints: &[u8; 64]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CAPABILITY_TARGET_MODEL_DOMAIN_V5);
    digest.update(model_fingerprints);
    digest.finalize().into()
}

fn target_model_fingerprints(profile: ProductionAmdTargetProfileV1) -> ([u64; 4], [u64; 4]) {
    (
        target_profile_fingerprint(profile.target_profile_spec()),
        target_revision_fingerprint(PRODUCTION_AMD_NEUTRAL_CAPABILITY_MODEL_REVISION_V1),
    )
}

// Frozen target-spec V1 identity algorithm, duplicated here to keep this carriage layer from
// depending on the target model implementation that produces the live closure.
fn target_profile_fingerprint(profile: TargetProfileSpecV1) -> [u64; 4] {
    let mut state = [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b1_85eb_ca87,
        0xd6e8_feb8_6659_fd93,
    ];
    state = fingerprint_field(state, b"fe2o3.target-profile-fingerprint.v1");
    state = fingerprint_field(state, profile.vendor().as_str().as_bytes());
    state = fingerprint_field(state, profile.architecture_family().as_str().as_bytes());
    state = fingerprint_field(state, profile.architecture().as_bytes());
    state = fingerprint_optional_field(state, profile.rustc_target());
    state = fingerprint_optional_field(state, profile.llvm_target());
    state = fingerprint_field(state, profile.artifact_format().as_str().as_bytes());
    state = fingerprint_field(state, profile.execution_model().as_str().as_bytes());
    state = fingerprint_optional_field(state, profile.data_layout());
    state = fingerprint_u64(state, profile.features().len() as u64);
    for feature in profile.features() {
        state = fingerprint_field(state, feature.name().as_bytes());
        state = fingerprint_field(state, feature.state().as_str().as_bytes());
    }
    state
}

fn target_revision_fingerprint(revision: &str) -> [u64; 4] {
    let state = [
        0xaf63_bd4c_8601_b7df,
        0x8601_b7df_af63_bd4c,
        0xa076_1d64_78bd_642f,
        0xe703_7ed1_a0b4_28db,
    ];
    fingerprint_field(
        fingerprint_field(state, b"fe2o3.target-model-revision-fingerprint.v1"),
        revision.as_bytes(),
    )
}

fn fingerprint_optional_field(state: [u64; 4], field: Option<&str>) -> [u64; 4] {
    match field {
        Some(field) => fingerprint_field(fingerprint_u64(state, 1), field.as_bytes()),
        None => fingerprint_u64(state, 0),
    }
}

fn fingerprint_field(mut state: [u64; 4], field: &[u8]) -> [u64; 4] {
    state = fingerprint_u64(state, field.len() as u64);
    for byte in field {
        for (lane, lane_state) in state.iter_mut().enumerate() {
            *lane_state ^= u64::from(*byte).wrapping_add((lane as u64) << 8);
            *lane_state = lane_state.wrapping_mul(0x0000_0100_0000_01b3);
            *lane_state ^= *lane_state >> (29 + lane);
        }
    }
    state
}

fn fingerprint_u64(mut state: [u64; 4], value: u64) -> [u64; 4] {
    for byte in value.to_le_bytes() {
        for (lane, lane_state) in state.iter_mut().enumerate() {
            *lane_state ^= u64::from(byte).wrapping_add((lane as u64) << 8);
            *lane_state = lane_state.wrapping_mul(0x0000_0100_0000_01b3);
            *lane_state ^= *lane_state >> (29 + lane);
        }
    }
    state
}

fn validate_subject_roster(
    proof_lineage: &InertMultiRootProofLineageV3,
    subjects: &[CapabilitySubjectV1],
) -> Result<(), InertProductionCapabilityHandoffErrorV5> {
    let roster = proof_lineage.roster(MultiRootProofRosterKindV3::MiddleEnd);
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&subjects.len())
        || subjects.len() != roster.root_count()
    {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount);
    }
    for (subject_index, root_index) in roster.canonical_kernel_order().iter().enumerate() {
        let root = roster
            .root(*root_index as usize)
            .ok_or(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount)?;
        let subject = subjects[subject_index];
        if subject.kernel().digest().as_bytes() != &root.kernel_binding()
            || subject.root().digest().as_bytes() != &root.semantic_root_identity()
            || subject.executable_kir().digest().as_bytes() != &roster.neutral_kir().digest()
            || subject.executable_kir_epoch() != roster.neutral_kir().graph_epoch()
        {
            return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
                "kernel/root proof roster",
            ));
        }
    }
    Ok(())
}

fn validate_result_parts(
    handoff: &InertProductionCapabilityHandoffV5,
    llvm_output: &InertCompilerStageOutputReceiptV5,
    object_output: &InertCompilerStageOutputReceiptV5,
    machine_refinement: &InertCapabilityRefinementReceiptV1,
    capability_association: &InertMultiRootStaticCapabilityEvidenceAssociationV1,
    proof_owner: &InertCompilerProofOwnerV5,
) -> Result<(), InertProductionCapabilityHandoffErrorV5> {
    if llvm_output.stage != ProductionCompilerOutputStageV5::Llvm
        || object_output.stage != ProductionCompilerOutputStageV5::Object
    {
        return Err(InertProductionCapabilityHandoffErrorV5::WrongOutputStage);
    }
    let module_bytes = handoff.legacy_handoff.module_handoff().module_bytes();
    if llvm_output.output_sha256 != Sha256::digest(module_bytes).as_slice()
        || llvm_output.output_bytes != module_bytes.len() as u64
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "LLVM output",
        ));
    }
    if machine_refinement.kind() != InertCapabilityRefinementReceiptKindV1::Machine {
        return Err(InertProductionCapabilityHandoffErrorV5::WrongRefinementKind);
    }
    if capability_association.entries().len() != handoff.subjects.len() {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount);
    }
    let capsule = handoff.legacy_handoff.capsule();
    let receipts = capsule.receipts();
    let expected_lineage = [
        lineage_identity(*capsule.identity().sha256(), capsule.identity().byte_len())?,
        lineage_identity(
            handoff.executable_kir.identity().sha256(),
            handoff.executable_kir.identity().byte_len(),
        )?,
        lineage_identity(
            *receipts.proof_binding().identity().sha256(),
            receipts.proof_binding().identity().byte_len(),
        )?,
        lineage_identity(
            *receipts.target_binding().identity().sha256(),
            receipts.target_binding().identity().byte_len(),
        )?,
        lineage_identity(
            *receipts.amdgpu_lowering().identity().sha256(),
            receipts.amdgpu_lowering().identity().byte_len(),
        )?,
        lineage_identity(
            *receipts.semantic_to_llvm().identity().sha256(),
            receipts.semantic_to_llvm().identity().byte_len(),
        )?,
        lineage_identity(
            *receipts
                .final_compiler_module_commitment()
                .identity()
                .sha256(),
            receipts
                .final_compiler_module_commitment()
                .identity()
                .byte_len(),
        )?,
    ];
    for (index, association) in capability_association.entries().iter().enumerate() {
        let associated = association.inputs();
        let actual_lineage = [
            associated.capsule(),
            associated.kernel_ir(),
            associated.proof_binding(),
            associated.target_binding(),
            associated.target_lowering(),
            associated.semantic_to_llvm(),
            associated.final_compiler_module(),
        ];
        if actual_lineage != expected_lineage
            || association.subject() != handoff.subjects[index]
            || association.obligation_set_bytes() != handoff.obligations[index].canonical_bytes()
            || associated.source_refinement() != Some(handoff.source_refinement.identity())
            || associated.machine_refinement() != Some(machine_refinement.identity())
        {
            return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
                "capability association roster",
            ));
        }
    }
    let owner = proof_owner.inputs();
    if owner.legacy_proof_binding()
        != lineage_identity(
            *receipts.proof_binding().identity().sha256(),
            receipts.proof_binding().identity().byte_len(),
        )?
        || owner.semantic_mir_receipt()
            != lineage_identity(
                *receipts.semantic_mir().identity().sha256(),
                receipts.semantic_mir().identity().byte_len(),
            )?
        || owner.semantic_mir_identity() != handoff.inputs.semantic_mir_identity
        || owner.executable_kir_receipt() != handoff.executable_kir.identity()
        || owner.executable_kir_bytes() != handoff.executable_kir.identity().byte_len()
        || proof_owner.subjects() != handoff.subjects.as_ref()
        || proof_owner.proof_lineage() != Some(handoff.proof_lineage.identity())
        || owner.compiler_policy() != handoff.inputs.compiler_policy
        || owner.source_refinement() != handoff.source_refinement.identity()
        || owner.machine_refinement() != machine_refinement.identity()
        || owner.capability_association() != capability_association.identity()
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "V5 proof owner",
        ));
    }
    Ok(())
}

fn lineage_identity(
    sha256: [u8; 32],
    byte_len: u64,
) -> Result<InertLineageContentIdentityV3, InertProductionCapabilityHandoffErrorV5> {
    InertLineageContentIdentityV3::new(sha256, byte_len)
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::ZeroIdentity)
}

fn encode_subject(subject: CapabilitySubjectV1) -> [u8; 168] {
    let mut bytes = [0; 168];
    bytes[..32].copy_from_slice(subject.kernel().digest().as_bytes());
    bytes[32..64].copy_from_slice(subject.root().digest().as_bytes());
    bytes[64..96].copy_from_slice(subject.executable_kir().digest().as_bytes());
    bytes[96..104].copy_from_slice(&subject.executable_kir_epoch().to_le_bytes());
    bytes[104..136].copy_from_slice(subject.target_model().digest().as_bytes());
    bytes[136..168].copy_from_slice(subject.launch_contract().digest().as_bytes());
    bytes
}

fn encode_subject_roster(
    subjects: &[CapabilitySubjectV1],
) -> Result<Vec<u8>, InertProductionCapabilityHandoffErrorV5> {
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&subjects.len()) {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount);
    }
    let mut bytes = Vec::with_capacity(4 + subjects.len() * 168);
    bytes.extend_from_slice(&(subjects.len() as u32).to_le_bytes());
    for subject in subjects {
        bytes.extend_from_slice(&encode_subject(*subject));
    }
    Ok(bytes)
}

fn encode_checked_evidence_roster(evidence: &[[u8; 32]]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + evidence.len() * 32);
    bytes.extend_from_slice(&(evidence.len() as u32).to_le_bytes());
    for identity in evidence {
        bytes.extend_from_slice(identity);
    }
    bytes
}

fn decode_checked_evidence_roster(
    bytes: &[u8],
) -> Result<Vec<[u8; 32]>, InertProductionCapabilityHandoffErrorV5> {
    if bytes.len() != 4 + PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5 * 32
        || read_u32(bytes, 0)? as usize != PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5
    {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount);
    }
    (0..PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5)
        .map(|index| copy_32(&bytes[4 + index * 32..4 + (index + 1) * 32]))
        .collect()
}

fn decode_subject_roster(
    bytes: &[u8],
) -> Result<Vec<CapabilitySubjectV1>, InertProductionCapabilityHandoffErrorV5> {
    let count = usize::try_from(read_u32(bytes, 0)?)
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&count)
        || bytes.len() != 4 + count * 168
    {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount);
    }
    (0..count)
        .map(|index| decode_subject(&bytes[4 + index * 168..4 + (index + 1) * 168]))
        .collect()
}

fn encode_obligation_roster(
    obligations: &[InertCapabilityObligationSetV1],
) -> Result<Vec<u8>, InertProductionCapabilityHandoffErrorV5> {
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&obligations.len()) {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(4)
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::AllocationFailed)?;
    bytes.extend_from_slice(&(obligations.len() as u32).to_le_bytes());
    for obligation in obligations {
        bytes.extend_from_slice(
            &u32::try_from(obligation.canonical_bytes().len())
                .map_err(|_| InertProductionCapabilityHandoffErrorV5::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(obligation.canonical_bytes());
    }
    Ok(bytes)
}

fn decode_obligation_roster(
    bytes: &[u8],
) -> Result<Vec<InertCapabilityObligationSetV1>, InertProductionCapabilityHandoffErrorV5> {
    let count = usize::try_from(read_u32(bytes, 0)?)
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&count) {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount);
    }
    let mut offset = 4;
    let mut obligations = Vec::new();
    obligations
        .try_reserve_exact(count)
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::AllocationFailed)?;
    for _ in 0..count {
        let length = usize::try_from(read_u32(bytes, offset)?)
            .map_err(|_| InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
        let start = offset
            .checked_add(4)
            .ok_or(InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
        let end = start
            .checked_add(length)
            .ok_or(InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
        obligations.push(InertCapabilityObligationSetV1::decode_canonical(
            bytes
                .get(start..end)
                .ok_or(InertProductionCapabilityHandoffErrorV5::Truncated)?,
        )?);
        offset = end;
    }
    if offset != bytes.len() {
        return Err(InertProductionCapabilityHandoffErrorV5::TrailingBytes);
    }
    Ok(obligations)
}

fn launch_roster_identity(
    subjects: &[CapabilitySubjectV1],
) -> Result<LaunchContractIdentityV1, InertProductionCapabilityHandoffErrorV5> {
    if subjects.len() == 1 {
        return Ok(subjects[0].launch_contract());
    }
    if !(2..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&subjects.len()) {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount);
    }
    let mut digest = Sha256::new();
    digest.update(LAUNCH_ROSTER_IDENTITY_DOMAIN);
    digest.update((subjects.len() as u32).to_le_bytes());
    for subject in subjects {
        digest.update(subject.kernel().digest().as_bytes());
        digest.update(subject.launch_contract().digest().as_bytes());
    }
    Ok(LaunchContractIdentityV1::from_untrusted_digest(
        DigestV1::from_untrusted_bytes(digest.finalize().into()),
    ))
}

fn decode_subject(
    bytes: &[u8],
) -> Result<CapabilitySubjectV1, InertProductionCapabilityHandoffErrorV5> {
    if bytes.len() != 168 {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidFieldLength);
    }
    CapabilitySubjectV1::new(
        KernelIdentityV1::from_untrusted_digest(decode_digest(&bytes[..32])?),
        KernelRootIdentityV1::from_untrusted_digest(decode_digest(&bytes[32..64])?),
        ExecutableKirIdentityV1::from_untrusted_digest(decode_digest(&bytes[64..96])?),
        read_u64_exact(&bytes[96..104])?,
        TargetModelIdentityV1::from_untrusted_digest(decode_digest(&bytes[104..136])?),
        LaunchContractIdentityV1::from_untrusted_digest(decode_digest(&bytes[136..168])?),
    )
    .map_err(InertProductionCapabilityHandoffErrorV5::Capability)
}

struct DecodedRecord<'a> {
    fields: Vec<&'a [u8]>,
}

#[allow(clippy::too_many_arguments)]
fn decode_record<'a>(
    bytes: &'a [u8],
    version: u16,
    kind: u16,
    field_count: usize,
    maximum: usize,
    domain: &[u8],
    identity_domain: &[u8],
) -> Result<DecodedRecord<'a>, InertProductionCapabilityHandoffErrorV5> {
    if bytes.len() > maximum {
        return Err(InertProductionCapabilityHandoffErrorV5::TooLarge);
    }
    if bytes.len() < HEADER_BYTES + TERMINAL_BYTES {
        return Err(InertProductionCapabilityHandoffErrorV5::Truncated);
    }
    let expected_magic = match kind {
        HANDOFF_KIND | TARGET_CLOSURE_KIND | FINAL_GRAPH_REPORT_KIND => {
            INERT_PRODUCTION_CAPABILITY_HANDOFF_MAGIC_V5
        }
        RESULT_KIND | OUTPUT_RECEIPT_KIND => INERT_PRODUCTION_CAPABILITY_RESULT_MAGIC_V5,
        TRANSACTION_KIND => INERT_PRODUCTION_CAPABILITY_TRANSACTION_MAGIC_V5,
        _ => return Err(InertProductionCapabilityHandoffErrorV5::WrongRecordKind),
    };
    if bytes[..8] != expected_magic {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidMagic);
    }
    if read_u16(bytes, 8)? != version {
        return Err(InertProductionCapabilityHandoffErrorV5::UnsupportedVersion);
    }
    if read_u16(bytes, 10)? != kind {
        return Err(InertProductionCapabilityHandoffErrorV5::WrongRecordKind);
    }
    if usize::from(read_u16(bytes, 12)?) != field_count {
        return Err(InertProductionCapabilityHandoffErrorV5::WrongFieldCount);
    }
    if read_u16(bytes, 14)? != 0 || read_u64(bytes, 24)? != 0 {
        return Err(InertProductionCapabilityHandoffErrorV5::NonzeroReserved);
    }
    if usize::try_from(read_u64(bytes, 16)?)
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::LengthOverflow)?
        != bytes.len()
    {
        return Err(InertProductionCapabilityHandoffErrorV5::DeclaredLengthMismatch);
    }
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(field_count)
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::AllocationFailed)?;
    let mut offset = HEADER_BYTES;
    for _ in 0..field_count {
        let len = usize::try_from(read_u32(bytes, offset)?)
            .map_err(|_| InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
        let start = offset
            .checked_add(FIELD_HEADER_BYTES)
            .ok_or(InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
        let end = start
            .checked_add(len)
            .ok_or(InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
        fields.push(
            bytes
                .get(start..end)
                .ok_or(InertProductionCapabilityHandoffErrorV5::Truncated)?,
        );
        offset = end;
    }
    if offset + TERMINAL_BYTES != bytes.len() {
        return Err(InertProductionCapabilityHandoffErrorV5::TrailingBytes);
    }
    if fields.first().copied() != Some(domain) {
        return Err(InertProductionCapabilityHandoffErrorV5::WrongDomain);
    }
    let terminal = copy_32(&bytes[offset..])?;
    if terminal == [0; 32] || terminal != derive_record_identity(identity_domain, &bytes[..offset])
    {
        return Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
            "terminal record identity",
        ));
    }
    Ok(DecodedRecord { fields })
}

fn encode_record(
    version: u16,
    kind: u16,
    fields: &[&[u8]],
    maximum: usize,
    identity_domain: &[u8],
) -> Result<Box<[u8]>, InertProductionCapabilityHandoffErrorV5> {
    let payload = fields.iter().try_fold(0_usize, |total, field| {
        total
            .checked_add(FIELD_HEADER_BYTES)
            .and_then(|value| value.checked_add(field.len()))
            .ok_or(InertProductionCapabilityHandoffErrorV5::LengthOverflow)
    })?;
    let total = HEADER_BYTES
        .checked_add(payload)
        .and_then(|value| value.checked_add(TERMINAL_BYTES))
        .ok_or(InertProductionCapabilityHandoffErrorV5::LengthOverflow)?;
    if total > maximum {
        return Err(InertProductionCapabilityHandoffErrorV5::TooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::AllocationFailed)?;
    let magic = match kind {
        HANDOFF_KIND | TARGET_CLOSURE_KIND | FINAL_GRAPH_REPORT_KIND => {
            INERT_PRODUCTION_CAPABILITY_HANDOFF_MAGIC_V5
        }
        RESULT_KIND | OUTPUT_RECEIPT_KIND => INERT_PRODUCTION_CAPABILITY_RESULT_MAGIC_V5,
        TRANSACTION_KIND => INERT_PRODUCTION_CAPABILITY_TRANSACTION_MAGIC_V5,
        _ => return Err(InertProductionCapabilityHandoffErrorV5::WrongRecordKind),
    };
    bytes.extend_from_slice(&magic);
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(
        &u16::try_from(fields.len())
            .map_err(|_| InertProductionCapabilityHandoffErrorV5::LengthOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    for field in fields {
        bytes.extend_from_slice(
            &u32::try_from(field.len())
                .map_err(|_| InertProductionCapabilityHandoffErrorV5::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(field);
    }
    let identity = derive_record_identity(identity_domain, &bytes);
    bytes.extend_from_slice(&identity);
    Ok(bytes.into_boxed_slice())
}

fn require_blob(
    bytes: &[u8],
    maximum: usize,
) -> Result<(), InertProductionCapabilityHandoffErrorV5> {
    if bytes.is_empty() {
        return Err(InertProductionCapabilityHandoffErrorV5::EmptyField);
    }
    if bytes.len() > maximum {
        return Err(InertProductionCapabilityHandoffErrorV5::TooLarge);
    }
    Ok(())
}

fn validate_w4_witness_encoding(bytes: &[u8]) -> Result<(), InertProductionW4WitnessErrorV5> {
    if bytes.len() > MAX_INERT_PRODUCTION_W4_WITNESS_BYTES_V5 {
        return Err(InertProductionW4WitnessErrorV5::TooLarge);
    }
    let header = W4_WITNESS_DOMAIN_V1
        .len()
        .checked_add(size_of::<u16>())
        .ok_or(InertProductionW4WitnessErrorV5::LengthOverflow)?;
    let minimum = header
        .checked_add(32)
        .ok_or(InertProductionW4WitnessErrorV5::LengthOverflow)?;
    if bytes.len() < minimum {
        return Err(InertProductionW4WitnessErrorV5::Truncated);
    }
    if !bytes.starts_with(W4_WITNESS_DOMAIN_V1) {
        return Err(InertProductionW4WitnessErrorV5::WrongDomain);
    }
    let version = u16::from_le_bytes(
        bytes[W4_WITNESS_DOMAIN_V1.len()..header]
            .try_into()
            .map_err(|_| InertProductionW4WitnessErrorV5::Truncated)?,
    );
    if version != W4_WITNESS_VERSION_V1 {
        return Err(InertProductionW4WitnessErrorV5::UnsupportedVersion);
    }
    let payload_end = bytes.len() - 32;
    let mut checksum = Sha256::new();
    checksum.update(W4_WITNESS_CHECKSUM_DOMAIN_V1);
    checksum.update(&bytes[..payload_end]);
    let expected: [u8; 32] = checksum.finalize().into();
    if bytes[payload_end..] != expected {
        return Err(InertProductionW4WitnessErrorV5::ChecksumMismatch);
    }
    Ok(())
}

fn require_lengths(
    fields: &[&[u8]],
    expected: &[usize],
) -> Result<(), InertProductionCapabilityHandoffErrorV5> {
    if fields.len() != expected.len()
        || fields
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.len() != *expected)
    {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidFieldLength);
    }
    Ok(())
}

fn require_canonical(
    actual: &[u8],
    supplied: &[u8],
) -> Result<(), InertProductionCapabilityHandoffErrorV5> {
    if actual == supplied {
        Ok(())
    } else {
        Err(InertProductionCapabilityHandoffErrorV5::NonCanonical)
    }
}

fn derive_record_identity(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_le_bytes());
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn derive_blob_identity(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_le_bytes());
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn terminal_identity(bytes: &[u8]) -> Result<[u8; 32], InertProductionCapabilityHandoffErrorV5> {
    copy_32(
        bytes
            .get(bytes.len().saturating_sub(TERMINAL_BYTES)..)
            .ok_or(InertProductionCapabilityHandoffErrorV5::Truncated)?,
    )
}

fn decode_digest(bytes: &[u8]) -> Result<DigestV1, InertProductionCapabilityHandoffErrorV5> {
    let digest = DigestV1::from_untrusted_bytes(copy_32(bytes)?);
    if digest.is_zero() {
        return Err(InertProductionCapabilityHandoffErrorV5::ZeroIdentity);
    }
    Ok(digest)
}

fn copy_32(bytes: &[u8]) -> Result<[u8; 32], InertProductionCapabilityHandoffErrorV5> {
    bytes
        .try_into()
        .map_err(|_| InertProductionCapabilityHandoffErrorV5::InvalidFieldLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, InertProductionCapabilityHandoffErrorV5> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(InertProductionCapabilityHandoffErrorV5::Truncated)?
            .try_into()
            .map_err(|_| InertProductionCapabilityHandoffErrorV5::Truncated)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, InertProductionCapabilityHandoffErrorV5> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(InertProductionCapabilityHandoffErrorV5::Truncated)?
            .try_into()
            .map_err(|_| InertProductionCapabilityHandoffErrorV5::Truncated)?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, InertProductionCapabilityHandoffErrorV5> {
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .ok_or(InertProductionCapabilityHandoffErrorV5::Truncated)?
            .try_into()
            .map_err(|_| InertProductionCapabilityHandoffErrorV5::Truncated)?,
    ))
}

fn read_u64_exact(bytes: &[u8]) -> Result<u64, InertProductionCapabilityHandoffErrorV5> {
    if bytes.len() != 8 {
        return Err(InertProductionCapabilityHandoffErrorV5::InvalidFieldLength);
    }
    read_u64(bytes, 0)
}

fn validate_simulation_bundle_envelope_v8(
    bytes: &[u8],
) -> Result<(), InertSimulationBundleErrorV8> {
    if bytes.len() > MAX_INERT_SIMULATION_BUNDLE_BYTES_V8 {
        return Err(InertSimulationBundleErrorV8::TooLarge);
    }
    let header = bytes
        .get(..SIMULATION_BUNDLE_HEADER_BYTES_V8)
        .ok_or(InertSimulationBundleErrorV8::Truncated)?;
    if header.get(..8) != Some(SIMULATION_BUNDLE_MAGIC_V8) {
        return Err(InertSimulationBundleErrorV8::InvalidMagic);
    }
    let u16_at = |offset: usize| {
        u16::from_le_bytes(
            header[offset..offset + 2]
                .try_into()
                .expect("bounded Bundle V8 header"),
        )
    };
    let u32_at = |offset: usize| {
        u32::from_le_bytes(
            header[offset..offset + 4]
                .try_into()
                .expect("bounded Bundle V8 header"),
        )
    };
    let u64_at = |offset: usize| {
        u64::from_le_bytes(
            header[offset..offset + 8]
                .try_into()
                .expect("bounded Bundle V8 header"),
        )
    };
    if u16_at(8) != SIMULATION_BUNDLE_VERSION_V8 {
        return Err(InertSimulationBundleErrorV8::UnsupportedVersion);
    }
    if u16_at(10) != 0
        || header[22..28] != [0; 6]
        || u16_at(12) != SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V8
        || u16_at(14) != SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V8
        || u32_at(16) == 0
        || u64_at(56) == 0
    {
        return Err(InertSimulationBundleErrorV8::InvalidHeader);
    }

    let target_length = usize::from(u16_at(20));
    let kir_length =
        usize::try_from(u64_at(28)).map_err(|_| InertSimulationBundleErrorV8::InvalidLength)?;
    let source_length =
        usize::try_from(u32_at(36)).map_err(|_| InertSimulationBundleErrorV8::InvalidLength)?;
    let semantic_length =
        usize::try_from(u64_at(40)).map_err(|_| InertSimulationBundleErrorV8::InvalidLength)?;
    let storage_length =
        usize::try_from(u32_at(48)).map_err(|_| InertSimulationBundleErrorV8::InvalidLength)?;
    let aggregate_length =
        usize::try_from(u32_at(52)).map_err(|_| InertSimulationBundleErrorV8::InvalidLength)?;
    if target_length == 0
        || target_length > 4096
        || kir_length == 0
        || kir_length > 16 * 1024 * 1024
        || source_length == 0
        || source_length > 4 * 1024 * 1024
        || semantic_length == 0
        || semantic_length > 128 * 1024 * 1024
        || storage_length == 0
        || storage_length > 8 * 1024 * 1024
        || aggregate_length == 0
        || aggregate_length > 8 * 1024 * 1024
        || u64_at(216) != kir_length as u64
        || u64_at(176) == 0
        || u64_at(96) == 0
        || u64_at(136) == 0
    {
        return Err(InertSimulationBundleErrorV8::InvalidLength);
    }
    for identity in [
        &header[64..96],
        &header[104..136],
        &header[144..176],
        &header[184..216],
        &header[224..256],
        &header[256..288],
        &header[288..320],
        &header[320..352],
        &header[352..384],
        &header[384..416],
    ] {
        if identity.iter().all(|byte| *byte == 0) {
            return Err(InertSimulationBundleErrorV8::ZeroIdentity);
        }
    }
    let expected_length = [
        target_length,
        kir_length,
        source_length,
        semantic_length,
        storage_length,
        aggregate_length,
    ]
    .into_iter()
    .try_fold(SIMULATION_BUNDLE_HEADER_BYTES_V8, usize::checked_add)
    .ok_or(InertSimulationBundleErrorV8::InvalidLength)?;
    if expected_length != bytes.len() {
        return Err(InertSimulationBundleErrorV8::TrailingOrMissingBytes);
    }
    let target = &bytes
        [SIMULATION_BUNDLE_HEADER_BYTES_V8..SIMULATION_BUNDLE_HEADER_BYTES_V8 + target_length];
    if target
        .iter()
        .any(|byte| !byte.is_ascii() || byte.is_ascii_control())
    {
        return Err(InertSimulationBundleErrorV8::InvalidTarget);
    }
    Ok(())
}

fn derive_simulation_bundle_identity_v8(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(SIMULATION_BUNDLE_IDENTITY_DOMAIN_V8);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

/// Invalid dependency-neutral Bundle V8 custody.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InertSimulationBundleErrorV8 {
    TooLarge,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    InvalidHeader,
    InvalidLength,
    InvalidTarget,
    ZeroIdentity,
    TrailingOrMissingBytes,
    IdentityMismatch,
}

impl fmt::Display for InertSimulationBundleErrorV8 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid exact simulation Bundle V8 custody: {self:?}"
        )
    }
}

impl Error for InertSimulationBundleErrorV8 {}

/// Invalid frozen W4 witness payload presented to the V5 carrier.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InertProductionW4WitnessErrorV5 {
    TooLarge,
    LengthOverflow,
    Truncated,
    WrongDomain,
    UnsupportedVersion,
    ChecksumMismatch,
}

impl fmt::Display for InertProductionW4WitnessErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid canonical W4 witness payload: {self:?}")
    }
}

impl Error for InertProductionW4WitnessErrorV5 {}

/// Strict construction or decoding failure for V5 capability carriage.
#[derive(Debug)]
#[non_exhaustive]
pub enum InertProductionCapabilityHandoffErrorV5 {
    /// A nested frozen semantic handoff failed strict V3 decoding.
    LegacyHandoff(InertSemanticCompilerModuleHandoffErrorV3),
    /// A canonical KIR V13 receipt was malformed or unbounded.
    KernelIrReceipt(InertCanonicalKernelIrV13ReceiptErrorV5),
    /// Native multi-root V13 lineage was malformed or inconsistent.
    ProofLineage(MultiRootProofLineageErrorV3),
    /// A capability obligation/result record was malformed.
    Capability(CapabilityCodecErrorV1),
    /// A refinement or static capability association was malformed.
    CapabilityAssociation(InertStaticCapabilityEvidenceAssociationErrorV1),
    /// The V5 proof owner was malformed.
    ProofOwner(InertCompilerProofOwnerErrorV5),
    /// The retained W4 witness envelope was not exact and canonical.
    W4Witness(InertProductionW4WitnessErrorV5),
    /// The exact dependency-neutral Bundle V8 envelope or native identity was invalid.
    SimulationBundle(InertSimulationBundleErrorV8),
    /// A canonical record exceeded its hard maximum.
    TooLarge,
    /// A required exact byte field was empty.
    EmptyField,
    /// Allocation failed.
    AllocationFailed,
    /// Length arithmetic overflowed.
    LengthOverflow,
    /// A record ended before all declared fields were present.
    Truncated,
    /// The wire magic was not the exact expected V5 magic.
    InvalidMagic,
    /// The wire version was not exactly V5.
    UnsupportedVersion,
    /// The closed record kind was wrong.
    WrongRecordKind,
    /// The exact field count was wrong.
    WrongFieldCount,
    /// A reserved field was nonzero.
    NonzeroReserved,
    /// The declared and actual record lengths differed.
    DeclaredLengthMismatch,
    /// Bytes followed the terminal identity.
    TrailingBytes,
    /// The domain was not the exact record domain.
    WrongDomain,
    /// A fixed-width field had the wrong length.
    InvalidFieldLength,
    /// A decoded record was not its unique canonical encoding.
    NonCanonical,
    /// Source and machine refinement receipt kinds were crossed.
    WrongRefinementKind,
    /// LLVM and object output stages were crossed.
    WrongOutputStage,
    /// A required digest or epoch was zero.
    ZeroIdentity,
    /// A subject, obligation, or result roster is empty, oversized, or incomplete.
    InvalidRosterCount,
    /// One exact cross-stage coordinate was substituted.
    IdentityMismatch(&'static str),
}

impl fmt::Display for InertProductionCapabilityHandoffErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityMismatch(field) => {
                write!(
                    formatter,
                    "production capability carriage substituted {field}"
                )
            }
            error => write!(
                formatter,
                "invalid production capability carriage V5: {error:?}"
            ),
        }
    }
}

impl Error for InertProductionCapabilityHandoffErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LegacyHandoff(error) => Some(error),
            Self::KernelIrReceipt(error) => Some(error),
            Self::ProofLineage(error) => Some(error),
            Self::CapabilityAssociation(error) => Some(error),
            Self::ProofOwner(error) => Some(error),
            Self::W4Witness(error) => Some(error),
            Self::SimulationBundle(error) => Some(error),
            _ => None,
        }
    }
}

impl From<InertSemanticCompilerModuleHandoffErrorV3> for InertProductionCapabilityHandoffErrorV5 {
    fn from(error: InertSemanticCompilerModuleHandoffErrorV3) -> Self {
        Self::LegacyHandoff(error)
    }
}

impl From<InertCanonicalKernelIrV13ReceiptErrorV5> for InertProductionCapabilityHandoffErrorV5 {
    fn from(error: InertCanonicalKernelIrV13ReceiptErrorV5) -> Self {
        Self::KernelIrReceipt(error)
    }
}

impl From<InertProductionW4WitnessErrorV5> for InertProductionCapabilityHandoffErrorV5 {
    fn from(error: InertProductionW4WitnessErrorV5) -> Self {
        Self::W4Witness(error)
    }
}

impl From<InertSimulationBundleErrorV8> for InertProductionCapabilityHandoffErrorV5 {
    fn from(error: InertSimulationBundleErrorV8) -> Self {
        Self::SimulationBundle(error)
    }
}

impl From<MultiRootProofLineageErrorV3> for InertProductionCapabilityHandoffErrorV5 {
    fn from(error: MultiRootProofLineageErrorV3) -> Self {
        Self::ProofLineage(error)
    }
}

impl From<CapabilityCodecErrorV1> for InertProductionCapabilityHandoffErrorV5 {
    fn from(error: CapabilityCodecErrorV1) -> Self {
        Self::Capability(error)
    }
}

impl From<InertStaticCapabilityEvidenceAssociationErrorV1>
    for InertProductionCapabilityHandoffErrorV5
{
    fn from(error: InertStaticCapabilityEvidenceAssociationErrorV1) -> Self {
        Self::CapabilityAssociation(error)
    }
}

impl From<InertCompilerProofOwnerErrorV5> for InertProductionCapabilityHandoffErrorV5 {
    fn from(error: InertCompilerProofOwnerErrorV5) -> Self {
        Self::ProofOwner(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_compiler_lineage::{
        MultiRootCanonicalKirVersionV3, MultiRootNeutralKirIdentityV3,
        MultiRootProofRosterInputsV3, MultiRootProofRosterRootInputV3,
        MultiRootProofRosterTranscriptV3,
    };

    fn digest(byte: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([byte; 32])
    }

    fn w4_witness(payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::from(W4_WITNESS_DOMAIN_V1);
        bytes.extend_from_slice(&W4_WITNESS_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(payload);
        let mut checksum = Sha256::new();
        checksum.update(W4_WITNESS_CHECKSUM_DOMAIN_V1);
        checksum.update(&bytes);
        bytes.extend_from_slice(&checksum.finalize());
        bytes
    }

    fn simulation_bundle_v8(seed: u8) -> Vec<u8> {
        let target = b"gfx950:xnack-";
        let sections: [&[u8]; 5] = [b"kir", b"source", b"semantic", b"storage", b"aggregate"];
        let mut bytes = vec![0_u8; SIMULATION_BUNDLE_HEADER_BYTES_V8];
        bytes[..8].copy_from_slice(SIMULATION_BUNDLE_MAGIC_V8);
        bytes[8..10].copy_from_slice(&SIMULATION_BUNDLE_VERSION_V8.to_le_bytes());
        bytes[12..14].copy_from_slice(&SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V8.to_le_bytes());
        bytes[14..16].copy_from_slice(&SIMULATION_BUNDLE_CANONICAL_KIR_VERSION_V8.to_le_bytes());
        bytes[16..20].copy_from_slice(&1_u32.to_le_bytes());
        bytes[20..22].copy_from_slice(&(target.len() as u16).to_le_bytes());
        bytes[28..36].copy_from_slice(&(sections[0].len() as u64).to_le_bytes());
        bytes[36..40].copy_from_slice(&(sections[1].len() as u32).to_le_bytes());
        bytes[40..48].copy_from_slice(&(sections[2].len() as u64).to_le_bytes());
        bytes[48..52].copy_from_slice(&(sections[3].len() as u32).to_le_bytes());
        bytes[52..56].copy_from_slice(&(sections[4].len() as u32).to_le_bytes());
        bytes[56..64].copy_from_slice(&7_u64.to_le_bytes());
        for (range, value) in [
            (64..96, seed.wrapping_add(1)),
            (104..136, seed.wrapping_add(2)),
            (144..176, seed.wrapping_add(3)),
            (184..216, seed.wrapping_add(4)),
            (224..256, seed.wrapping_add(5)),
            (256..288, seed.wrapping_add(6)),
            (288..320, seed.wrapping_add(7)),
            (320..352, seed.wrapping_add(8)),
            (352..384, seed.wrapping_add(9)),
            (384..416, seed.wrapping_add(10)),
        ] {
            bytes[range].fill(value.max(1));
        }
        bytes[96..104].copy_from_slice(&1_u64.to_le_bytes());
        bytes[136..144].copy_from_slice(&1_u64.to_le_bytes());
        bytes[176..184].copy_from_slice(&(sections[0].len() as u64).to_le_bytes());
        bytes[216..224].copy_from_slice(&(sections[0].len() as u64).to_le_bytes());
        bytes.extend_from_slice(target);
        for section in sections {
            bytes.extend_from_slice(section);
        }
        bytes
    }

    fn subject(kir: [u8; 32], epoch: u64, kernel: u8, root: u8) -> CapabilitySubjectV1 {
        subject_with_coordinates(kir, epoch, kernel, root, 31, 32)
    }

    fn subject_with_coordinates(
        kir: [u8; 32],
        epoch: u64,
        kernel: u8,
        root: u8,
        target: u8,
        launch: u8,
    ) -> CapabilitySubjectV1 {
        CapabilitySubjectV1::new(
            KernelIdentityV1::from_untrusted_digest(digest(kernel)),
            KernelRootIdentityV1::from_untrusted_digest(digest(root)),
            ExecutableKirIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(kir)),
            epoch,
            TargetModelIdentityV1::from_untrusted_digest(digest(target)),
            LaunchContractIdentityV1::from_untrusted_digest(digest(launch)),
        )
        .unwrap()
    }

    fn lineage(
        kir: [u8; 32],
        kir_len: u64,
        epoch: u64,
        semantic_mir: [u8; 32],
        kernel: u8,
        root: u8,
    ) -> InertMultiRootProofLineageV3 {
        let neutral = MultiRootNeutralKirIdentityV3::new(
            MultiRootCanonicalKirVersionV3::V13,
            kir_len,
            kir,
            epoch,
        )
        .unwrap();
        let roster = |kind, payload: &'static [u8]| {
            let roots = [
                MultiRootProofRosterRootInputV3 {
                    semantic_root: 0,
                    semantic_root_identity: [root; 32],
                    kernel_binding: [kernel; 32],
                    source_rank: 1,
                    workgroup: [64, 1, 1],
                    logical_name: "kernel_a",
                    export_symbol: "kernel_a",
                    kernel_id: "kernel_a",
                    payload,
                },
                MultiRootProofRosterRootInputV3 {
                    semantic_root: 1,
                    semantic_root_identity: [root.wrapping_add(1); 32],
                    kernel_binding: [kernel.wrapping_add(1); 32],
                    source_rank: 1,
                    workgroup: [64, 1, 1],
                    logical_name: "kernel_b",
                    export_symbol: "kernel_b",
                    kernel_id: "kernel_b",
                    payload,
                },
            ];
            MultiRootProofRosterTranscriptV3::new(MultiRootProofRosterInputsV3 {
                kind,
                semantic_mir_sha256: semantic_mir,
                neutral_kir: neutral,
                roster_identity: [40; 32],
                canonical_kernel_order: &[0, 1],
                roots: &roots,
            })
            .unwrap()
        };
        InertMultiRootProofLineageV3::new(
            roster(MultiRootProofRosterKindV3::MiddleEnd, b"middle"),
            roster(
                MultiRootProofRosterKindV3::Correspondence,
                b"correspondence",
            ),
            roster(MultiRootProofRosterKindV3::FormalMemory, b"memory"),
            roster(MultiRootProofRosterKindV3::VerusExecution, b"verus"),
        )
        .unwrap()
    }

    #[test]
    fn exact_target_report_and_output_receipts_round_trip() {
        let closure = InertProductionTargetCapabilityClosureV5::new(
            [1; 32],
            [3; 32],
            122,
            8,
            TargetModelIdentityV1::from_untrusted_digest(digest(31)),
            LaunchContractIdentityV1::from_untrusted_digest(digest(32)),
            b"canonical decisions".to_vec(),
        )
        .unwrap();
        assert_eq!(
            InertProductionTargetCapabilityClosureV5::decode(closure.canonical_bytes())
                .unwrap()
                .canonical_bytes(),
            closure.canonical_bytes()
        );
        assert!(matches!(
            InertProductionTargetCapabilityClosureV5::new(
                [1; 32],
                [3; 32],
                122,
                0,
                TargetModelIdentityV1::from_untrusted_digest(digest(31)),
                LaunchContractIdentityV1::from_untrusted_digest(digest(32)),
                b"canonical decisions".to_vec(),
            ),
            Err(InertProductionCapabilityHandoffErrorV5::ZeroIdentity)
        ));

        let report = InertProductionFinalGraphReportV5::new(
            [2; 32],
            123,
            9,
            7,
            [4; 32],
            [5; 32],
            vec![[6; 32]; PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5],
            w4_witness(b"canonical report results"),
        )
        .unwrap();
        let witness = report.w4_witness();
        assert_eq!(witness.canonical_encoding(), report.canonical_results());
        assert_eq!(
            witness.identity().byte_len(),
            witness.canonical_encoding().len() as u64
        );
        assert_eq!(
            InertProductionFinalGraphReportV5::decode(report.canonical_bytes())
                .unwrap()
                .canonical_bytes(),
            report.canonical_bytes()
        );
        assert!(matches!(
            InertProductionFinalGraphReportV5::new(
                [2; 32],
                123,
                9,
                0,
                [4; 32],
                [5; 32],
                vec![[6; 32]; PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5],
                w4_witness(b"canonical report results"),
            ),
            Err(InertProductionCapabilityHandoffErrorV5::ZeroIdentity)
        ));

        for stage in [
            ProductionCompilerOutputStageV5::Llvm,
            ProductionCompilerOutputStageV5::Object,
        ] {
            let receipt =
                InertCompilerStageOutputReceiptV5::from_stage_output(stage, b"output").unwrap();
            assert_eq!(
                InertCompilerStageOutputReceiptV5::decode(receipt.canonical_bytes())
                    .unwrap()
                    .canonical_bytes(),
                receipt.canonical_bytes()
            );
        }
    }

    #[test]
    fn downgrade_omission_and_trailing_bytes_fail_closed() {
        let report = InertProductionFinalGraphReportV5::new(
            [2; 32],
            123,
            9,
            7,
            [4; 32],
            [5; 32],
            vec![[6; 32]; PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5],
            w4_witness(b"canonical report results"),
        )
        .unwrap();
        let bytes = report.canonical_bytes();
        for prefix in 0..bytes.len() {
            assert!(InertProductionFinalGraphReportV5::decode(&bytes[..prefix]).is_err());
        }
        let mut downgraded = bytes.to_vec();
        downgraded[8..10].copy_from_slice(&4_u16.to_le_bytes());
        assert!(matches!(
            InertProductionFinalGraphReportV5::decode(&downgraded),
            Err(InertProductionCapabilityHandoffErrorV5::UnsupportedVersion)
        ));
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(InertProductionFinalGraphReportV5::decode(&trailing).is_err());
    }

    #[test]
    fn typed_w4_payload_rejects_substitution_before_v5_carriage() {
        let exact = w4_witness(b"exact W4 result");
        let witness = InertProductionW4WitnessV5::from_canonical_encoding(exact.clone()).unwrap();
        assert_eq!(witness.canonical_encoding(), exact);

        let mut substituted = exact;
        let payload_index = W4_WITNESS_DOMAIN_V1.len() + size_of::<u16>();
        substituted[payload_index] ^= 1;
        assert_eq!(
            InertProductionW4WitnessV5::from_canonical_encoding(substituted),
            Err(InertProductionW4WitnessErrorV5::ChecksumMismatch)
        );
    }

    #[test]
    fn inert_bundle_v8_rejects_omission_downgrade_trailing_and_malformed_substitution() {
        let bytes = simulation_bundle_v8(19);
        let identity = derive_simulation_bundle_identity_v8(&bytes);
        let bundle =
            InertSimulationBundleV8::from_verified_canonical_bytes(identity, bytes.clone())
                .unwrap();
        assert_eq!(bundle.identity().sha256(), identity);
        assert_eq!(bundle.canonical_bytes(), bytes);

        for prefix in 0..bytes.len() {
            assert!(
                InertSimulationBundleV8::from_verified_canonical_bytes(
                    derive_simulation_bundle_identity_v8(&bytes[..prefix]),
                    bytes[..prefix].to_vec(),
                )
                .is_err()
            );
        }

        let mut downgraded = bytes.clone();
        downgraded[8..10].copy_from_slice(&7_u16.to_le_bytes());
        assert_eq!(
            InertSimulationBundleV8::from_verified_canonical_bytes(
                derive_simulation_bundle_identity_v8(&downgraded),
                downgraded,
            ),
            Err(InertSimulationBundleErrorV8::UnsupportedVersion)
        );

        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            InertSimulationBundleV8::from_verified_canonical_bytes(
                derive_simulation_bundle_identity_v8(&trailing),
                trailing,
            ),
            Err(InertSimulationBundleErrorV8::TrailingOrMissingBytes)
        );

        let mut malformed = bytes.clone();
        malformed[28..36].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(matches!(
            InertSimulationBundleV8::from_verified_canonical_bytes(
                derive_simulation_bundle_identity_v8(&malformed),
                malformed,
            ),
            Err(InertSimulationBundleErrorV8::InvalidLength)
        ));
        assert_eq!(
            InertSimulationBundleV8::from_verified_canonical_bytes([0x55; 32], bytes),
            Err(InertSimulationBundleErrorV8::IdentityMismatch)
        );
    }

    #[test]
    fn multi_root_roster_rejects_nonselected_corruption_omission_reordering_and_stale_graph() {
        let kir_bytes = b"candidate canonical KIR V13".to_vec();
        let kir_digest: [u8; 32] = Sha256::digest(&kir_bytes).into();
        let epoch = 7;
        let semantic_mir = [11; 32];
        let first = subject(kir_digest, epoch, 21, 22);
        let second = subject(kir_digest, epoch, 22, 23);
        let proof_lineage = lineage(
            kir_digest,
            kir_bytes.len() as u64,
            epoch,
            semantic_mir,
            21,
            22,
        );
        assert!(validate_subject_roster(&proof_lineage, &[first, second]).is_ok());
        assert!(matches!(
            validate_subject_roster(&proof_lineage, &[first]),
            Err(InertProductionCapabilityHandoffErrorV5::InvalidRosterCount)
        ));
        for hostile in [
            vec![second, first],
            vec![first, first],
            vec![first, subject(kir_digest, epoch, 22, 24)],
            vec![first, subject(kir_digest, epoch - 1, 22, 23)],
            vec![first, subject([61; 32], epoch, 22, 23)],
        ] {
            assert!(matches!(
                validate_subject_roster(&proof_lineage, &hostile),
                Err(InertProductionCapabilityHandoffErrorV5::IdentityMismatch(
                    "kernel/root proof roster"
                ))
            ));
        }

        let closure = InertProductionTargetCapabilityClosureV5::new_for_subject_roster(
            [1; 32],
            [3; 32],
            kir_bytes.len() as u64,
            epoch - 1,
            first.target_model(),
            &[first, second],
            b"canonical decisions".to_vec(),
        )
        .unwrap();
        assert_ne!(closure.launch_contract(), first.launch_contract());
    }
}
