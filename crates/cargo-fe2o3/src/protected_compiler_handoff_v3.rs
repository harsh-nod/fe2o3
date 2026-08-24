use std::error::Error;
use std::fmt;
use std::path::Path;

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffErrorV2, CompilerModuleHandoffErrorV3,
    CompilerModuleHandoffReceiptV3, ConsumedCompilerModuleHandoffV2,
    ConsumedCompilerModuleHandoffV3, ProducerIdentity,
    acquire_compiler_module_handoff_currentness_lease_v3, consume_compiler_module_handoff_v2,
    consume_compiler_module_handoff_with_currentness_v3,
    recover_compiler_module_handoff_receipt_v3,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1;
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_hsaco_finalize::ProtectedFirstBuildWorkerV3Error;
use fe2o3_rustc_invocation::RustcInvocationDescriptorV3;

use crate::inert_rustc_invocation_capture::{
    InertPreparedRustcInvocationCapture, InertRustcInvocationCaptureV2,
    InertRustcInvocationCaptureV3,
};

/// Move-only parent custody selected for one exactly prepared rustc child.
pub(crate) enum ParentRustcInvocationCustody {
    InertV2(InertRustcInvocationCaptureV2),
    ProtectedV3(Box<ParentProtectedRustcInvocationCustodyV3>),
}

impl ParentRustcInvocationCustody {
    pub(crate) fn retain(
        capture: Option<InertPreparedRustcInvocationCapture>,
        capability: Option<RustcInvocationCapabilityV1>,
    ) -> Result<Option<Self>, ParentProtectedRustcInvocationCustodyErrorV3> {
        match (capture, capability) {
            (Some(InertPreparedRustcInvocationCapture::V3(invocation)), Some(capability)) => {
                let custody = ParentProtectedRustcInvocationCustodyV3 {
                    invocation,
                    capability,
                };
                custody.revalidate()?;
                Ok(Some(Self::ProtectedV3(Box::new(custody))))
            }
            (Some(InertPreparedRustcInvocationCapture::V2(capture)), None) => {
                Ok(Some(Self::InertV2(capture)))
            }
            (None, None) => Ok(None),
            (Some(InertPreparedRustcInvocationCapture::V3(_)), None) => {
                Err(ParentProtectedRustcInvocationCustodyErrorV3::MissingCapability)
            }
            (Some(InertPreparedRustcInvocationCapture::V2(_)), Some(_)) => {
                Err(ParentProtectedRustcInvocationCustodyErrorV3::CapabilityForV2)
            }
            (None, Some(_)) => Err(ParentProtectedRustcInvocationCustodyErrorV3::MissingCapture),
        }
    }

    pub(crate) fn revalidate(&self) -> Result<(), ParentProtectedRustcInvocationCustodyErrorV3> {
        match self {
            Self::InertV2(capture) => {
                debug_assert_eq!(capture.descriptor().amd_target(), "gfx942:xnack-");
                Ok(())
            }
            Self::ProtectedV3(custody) => custody.revalidate(),
        }
    }

    fn protected_v3(&self) -> Option<&ParentProtectedRustcInvocationCustodyV3> {
        match self {
            Self::InertV2(_) => None,
            Self::ProtectedV3(custody) => Some(custody.as_ref()),
        }
    }

    /// Runs one operation while the exact selected parent custody remains live.
    pub(crate) fn retain_through<T>(self, operation: impl FnOnce(&Self) -> T) -> T {
        operation(&self)
    }

    pub(crate) const fn grants_compiler_authority(&self) -> bool {
        false
    }
}

/// Move-only parent custody of the exact protected invocation prepared for one rustc child.
///
/// The capture and sealed capability remain inert. Retaining them proves neither that rustc ran nor
/// that it authored an artifact, and grants no compiler, link, load, or launch authority.
pub(crate) struct ParentProtectedRustcInvocationCustodyV3 {
    invocation: Box<InertRustcInvocationCaptureV3>,
    capability: RustcInvocationCapabilityV1,
}

impl ParentProtectedRustcInvocationCustodyV3 {
    pub(crate) fn revalidate(&self) -> Result<(), ParentProtectedRustcInvocationCustodyErrorV3> {
        self.capability
            .revalidate()
            .map_err(ParentProtectedRustcInvocationCustodyErrorV3::Capability)?;
        if self.invocation.descriptor() != self.capability.descriptor() {
            return Err(ParentProtectedRustcInvocationCustodyErrorV3::DescriptorMismatch);
        }
        Ok(())
    }

    pub(crate) const fn descriptor(&self) -> &RustcInvocationDescriptorV3 {
        self.invocation.descriptor()
    }
}

#[derive(Debug)]
pub(crate) enum ParentProtectedRustcInvocationCustodyErrorV3 {
    MissingCapture,
    MissingCapability,
    CapabilityForV2,
    DescriptorMismatch,
    Capability(String),
}

impl fmt::Display for ParentProtectedRustcInvocationCustodyErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCapture => formatter.write_str(
                "protected rustc invocation capability has no exact parent invocation capture",
            ),
            Self::MissingCapability => formatter.write_str(
                "protected parent invocation capture has no retained sealed capability",
            ),
            Self::CapabilityForV2 => formatter
                .write_str("unprotected V2 invocation capture unexpectedly has a V3 capability"),
            Self::DescriptorMismatch => formatter.write_str(
                "parent invocation capture and retained sealed capability describe different rustc invocations",
            ),
            Self::Capability(error) => write!(formatter, "retained rustc invocation capability is invalid: {error}"),
        }
    }
}

impl Error for ParentProtectedRustcInvocationCustodyErrorV3 {}

/// Move-only result of parent-authorized current V3 consumption.
///
/// The exact recovered receipt remains paired with the consumed transaction so
/// downstream worker execution never reconstructs or drops its transaction
/// identity. This remains inert and grants no compiler or runtime authority.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct ParentConsumedCompilerModuleHandoffV3 {
    receipt: CompilerModuleHandoffReceiptV3,
    consumed: ConsumedCompilerModuleHandoffV3,
    compiler_closure: CompilerClosureV2,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ParentConsumedCompilerModuleHandoffV3 {
    pub(crate) const fn receipt(&self) -> CompilerModuleHandoffReceiptV3 {
        self.receipt
    }

    pub(crate) const fn consumed(&self) -> &ConsumedCompilerModuleHandoffV3 {
        &self.consumed
    }

    pub(crate) const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        CompilerModuleHandoffReceiptV3,
        ConsumedCompilerModuleHandoffV3,
        CompilerClosureV2,
    ) {
        (self.receipt, self.consumed, self.compiler_closure)
    }

    pub(crate) const fn grants_compiler_authority(&self) -> bool {
        false
    }
}

/// Explicit selection of the protected compiler-module transport schema.
///
/// Production preselects `ProtectedV3`; legacy protected qualification routes remain on
/// `ProtectedV2`. V3 derives the expected terminal identity from the exact durable receipt under
/// the cooperative lock. Neither variant authenticates compiler authorship.
pub(crate) enum ProtectedCompilerModuleHandoffIntake {
    ProtectedV2 {
        compiler_closure: Box<CompilerClosureV2>,
    },
    ProtectedV3,
}

impl ProtectedCompilerModuleHandoffIntake {
    pub(crate) fn protected_v2(compiler_closure: CompilerClosureV2) -> Self {
        Self::ProtectedV2 {
            compiler_closure: Box::new(compiler_closure),
        }
    }

    pub(crate) const fn protected_v3() -> Self {
        Self::ProtectedV3
    }

    pub(crate) fn consume_v2(
        &self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
    ) -> Result<ConsumedCompilerModuleHandoffV2, ProtectedCompilerModuleHandoffIntakeError> {
        let Self::ProtectedV2 { compiler_closure } = self else {
            return Err(ProtectedCompilerModuleHandoffIntakeError::WrongSchema {
                requested: "V2",
                selected: "V3",
            });
        };
        consume_compiler_module_handoff_v2(output_dir, producer, attempt, **compiler_closure)
            .map_err(ProtectedCompilerModuleHandoffIntakeError::V2)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn consume_v3(
        &self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        parent_custody: &ParentRustcInvocationCustody,
    ) -> Result<ParentConsumedCompilerModuleHandoffV3, ProtectedCompilerModuleHandoffIntakeError>
    {
        self.consume_v3_after_preflight(output_dir, producer, attempt, parent_custody, |_, _, _| {
            Ok(())
        })
        .map(|(consumed, ())| consumed)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn consume_v3_after_preflight<T>(
        &self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        parent_custody: &ParentRustcInvocationCustody,
        preflight: impl FnOnce(
            &InertSemanticCompilerModuleHandoffV3,
            CompilerModuleHandoffReceiptV3,
            CompilerClosureV2,
        ) -> Result<T, ProtectedFirstBuildWorkerV3Error>,
    ) -> Result<(ParentConsumedCompilerModuleHandoffV3, T), ProtectedCompilerModuleHandoffIntakeError>
    {
        let Self::ProtectedV3 = self else {
            return Err(ProtectedCompilerModuleHandoffIntakeError::WrongSchema {
                requested: "V3",
                selected: "V2",
            });
        };
        parent_custody
            .revalidate()
            .map_err(ProtectedCompilerModuleHandoffIntakeError::ParentCustody)?;
        let protected_custody = parent_custody
            .protected_v3()
            .ok_or(ProtectedCompilerModuleHandoffIntakeError::UnprotectedParentCustody)?;
        let receipt = recover_compiler_module_handoff_receipt_v3(output_dir, producer, attempt)
            .map_err(ProtectedCompilerModuleHandoffIntakeError::V3)?;
        if receipt.attempt() != attempt || receipt.grants_compiler_authority() {
            return Err(ProtectedCompilerModuleHandoffIntakeError::TransportBindingMismatch);
        }
        let lease =
            acquire_compiler_module_handoff_currentness_lease_v3(output_dir, producer, receipt)
                .map_err(ProtectedCompilerModuleHandoffIntakeError::V3)?;
        if lease.receipt() != receipt {
            return Err(ProtectedCompilerModuleHandoffIntakeError::TransportBindingMismatch);
        }
        parent_custody
            .revalidate()
            .map_err(ProtectedCompilerModuleHandoffIntakeError::ParentCustody)?;
        let token = lease
            .acquire_current_token()
            .map_err(ProtectedCompilerModuleHandoffIntakeError::V3)?;
        if token.handoff().capsule().invocation() != protected_custody.descriptor() {
            return Err(ProtectedCompilerModuleHandoffIntakeError::InvocationMismatch);
        }
        let compiler_closure = *protected_custody.descriptor().compiler_closure();
        let prepared = preflight(token.handoff(), receipt, compiler_closure)
            .map_err(ProtectedCompilerModuleHandoffIntakeError::WorkerPreflight)?;
        parent_custody
            .revalidate()
            .map_err(ProtectedCompilerModuleHandoffIntakeError::ParentCustody)?;
        let consumed = consume_compiler_module_handoff_with_currentness_v3(&lease, token)
            .map_err(ProtectedCompilerModuleHandoffIntakeError::V3)?;
        if consumed.attempt() != receipt.attempt()
            || consumed.slot() != receipt.slot()
            || consumed.transaction_identity() != receipt.transaction_identity()
            || consumed.handoff_identity() != receipt.handoff_identity()
        {
            return Err(ProtectedCompilerModuleHandoffIntakeError::TransportBindingMismatch);
        }
        debug_assert!(!consumed.grants_compiler_authority());
        let consumed = ParentConsumedCompilerModuleHandoffV3 {
            receipt,
            consumed,
            compiler_closure,
        };
        Ok((consumed, prepared))
    }
}

#[derive(Debug)]
pub(crate) enum ProtectedCompilerModuleHandoffIntakeError {
    WrongSchema {
        requested: &'static str,
        selected: &'static str,
    },
    ParentCustody(ParentProtectedRustcInvocationCustodyErrorV3),
    V2(CompilerModuleHandoffErrorV2),
    V3(CompilerModuleHandoffErrorV3),
    WorkerPreflight(ProtectedFirstBuildWorkerV3Error),
    TransportBindingMismatch,
    InvocationMismatch,
    UnprotectedParentCustody,
}

impl fmt::Display for ProtectedCompilerModuleHandoffIntakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSchema {
                requested,
                selected,
            } => write!(
                formatter,
                "protected compiler-module {requested} intake requested from selected {selected} schema",
            ),
            Self::ParentCustody(error) => error.fmt(formatter),
            Self::V2(error) => error.fmt(formatter),
            Self::V3(error) => error.fmt(formatter),
            Self::WorkerPreflight(error) => write!(formatter, "protected V3 worker preflight failed before handoff consumption: {error}"),
            Self::TransportBindingMismatch => formatter.write_str(
                "consumed V3 compiler-module handoff changed its exact transaction binding",
            ),
            Self::InvocationMismatch => formatter.write_str(
                "consumed V3 compiler-module handoff does not retain the exact parent-prepared rustc invocation",
            ),
            Self::UnprotectedParentCustody => formatter.write_str(
                "protected V3 compiler-module intake requires protected parent invocation custody",
            ),
        }
    }
}

impl Error for ProtectedCompilerModuleHandoffIntakeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ParentCustody(error) => Some(error),
            Self::V2(error) => Some(error),
            Self::V3(error) => Some(error),
            Self::WorkerPreflight(error) => Some(error),
            Self::WrongSchema { .. }
            | Self::TransportBindingMismatch
            | Self::InvocationMismatch
            | Self::UnprotectedParentCustody => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use fe2o3_artifact_transaction::{
        BuildAttempt, BuildInvocation, BuildSession, CompilerModuleHandoffErrorV2,
        CompilerModuleHandoffErrorV3, ProducerIdentity, begin_build_attempt,
        consume_compiler_module_handoff_v2, publish_compiler_module_handoff_v2,
        publish_compiler_module_handoff_v3,
    };
    use fe2o3_build_authority::CompilerClosureV2;
    use fe2o3_compiler_ffi::{
        CompilerFfiEnvelopeV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
        CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
        INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3, INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3,
        INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V3,
        INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3,
        INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V3, InertFinalCompilerModuleCommitmentV3,
        InertSemanticCompilerModuleHandoffV3,
    };
    use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
    use fe2o3_rustc_invocation::{
        InvocationDigestV3, RustcInvocationDescriptorV3, encode_descriptor_v3,
    };
    use sha2::{Digest, Sha256};

    use super::{
        ParentRustcInvocationCustody, ProtectedCompilerModuleHandoffIntake,
        ProtectedCompilerModuleHandoffIntakeError, ProtectedFirstBuildWorkerV3Error,
    };
    use crate::inert_rustc_invocation_capture::{
        InertPreparedRustcInvocationCapture, InertRustcInvocationCaptureV2,
    };

    const TARGET: &str = "gfx942:xnack-";
    const CAPSULE_MAGIC_V3: [u8; 8] = *b"F2O3ISV3";
    const CAPSULE_VERSION_V3: u16 = 3;
    const CAPSULE_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-PRODUCTION-SEMANTIC-CAPSULE/V3\0";
    const PAIR_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V3\0";
    const OUTER_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V3\0";
    const RECEIPT_DOMAINS_V3: [&[u8]; 15] = [
        b"FE2O3/INERT-LINEAGE-CONTENT/RUSTC-IDENTITY-INVENTORY/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/RUSTC-PREFLIGHT-PLAN/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/CANONICAL-SEMANTIC-MIR/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/MIDDLE-END-PASS-CHAIN/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/CANONICAL-KERNEL-IR/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/MIR-TO-KIR-CORRESPONDENCE/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/FORMAL-MEMORY-OBLIGATIONS/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/PROOF-BINDING-SET/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/TARGET-BINDING/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/TARGET-DATA-LAYOUT/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/ABI/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/EXPORT-MANIFEST/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/AMDGPU-LOWERING/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/SEMANTIC-TO-LLVM/V3\0",
        b"FE2O3/INERT-LINEAGE-CONTENT/FINAL-COMPILER-MODULE-COMMITMENT/V3\0",
    ];
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "cargo-fe2o3-protected-v3-{label}-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn compiler_closure(seed: u8) -> CompilerClosureV2 {
        CompilerClosureV2::new(
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
            [seed.wrapping_add(4); 32],
            [seed.wrapping_add(5); 32],
            [seed.wrapping_add(6); 32],
        )
        .unwrap()
    }

    fn protected_parent_custody(seed: u8) -> (ParentRustcInvocationCustody, CompilerClosureV2) {
        let closure = compiler_closure(seed);
        let mut command = Command::new("/proc/self/fd/9");
        command.args([
            "--crate-name",
            "protected_v3_fixture",
            "--crate-type=lib",
            "-Zcodegen-backend=/proc/./self/fd/198",
        ]);
        let environment = [
            (OsString::from("PATH"), OsString::from("/usr/bin")),
            (OsString::from("FE2O3_TARGET"), OsString::from(TARGET)),
            (
                OsString::from("FE2O3_HSACO_DIR"),
                OsString::from("/workspace/target/fe2o3"),
            ),
            (
                OsString::from("FE2O3_VERIFY_KERNEL_IR"),
                OsString::from("1"),
            ),
        ];
        let capture = InertRustcInvocationCaptureV2::capture(
            &command,
            "/toolchains/rustc".as_ref(),
            Path::new("/workspace/fe2o3"),
            &environment,
            closure.rustc_executable_sha256(),
            closure.codegen_backend_sha256(),
        )
        .unwrap();
        let prepared = InertPreparedRustcInvocationCapture::from_v2_and_protected_closure(
            capture,
            Some(closure),
        )
        .unwrap();
        let capability = fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1::create(
            prepared.descriptor_v3().unwrap().clone(),
        )
        .unwrap();
        let custody = ParentRustcInvocationCustody::retain(Some(prepared), Some(capability))
            .unwrap()
            .unwrap();
        (custody, closure)
    }

    fn producer(seed: u8) -> ProducerIdentity {
        ProducerIdentity::from_codegen(
            &format!("protected_v3_{seed:02x}"),
            Some(Path::new("/workspace/protected_v3.rs")),
        )
        .unwrap()
    }

    fn begin(directory: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
        begin_build_attempt(
            directory,
            producer,
            BuildInvocation::from_bytes([seed; 32]),
            BuildSession::from_bytes([seed.wrapping_add(1); 16]),
        )
        .unwrap()
    }

    fn target() -> DeviceTargetV1 {
        DeviceTargetV1::parse(TARGET).unwrap()
    }

    fn payload(label: &str, seed: u8) -> Vec<u8> {
        format!("cargo-fe2o3/protected-v3/{label}/{seed:02x}").into_bytes()
    }

    fn identity(domain: &[u8], preimage: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(domain);
        digest.update((preimage.len() as u64).to_le_bytes());
        digest.update(preimage);
        digest.finalize().into()
    }

    fn push_blob(output: &mut Vec<u8>, bytes: &[u8]) {
        output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        output.extend_from_slice(bytes);
    }

    fn canonical_capsule_bytes(
        invocation: &RustcInvocationDescriptorV3,
        seed: u8,
        final_commitment: &[u8],
    ) -> Vec<u8> {
        let invocation_bytes = encode_descriptor_v3(invocation).unwrap();
        let invocation_digest = InvocationDigestV3::calculate(invocation).unwrap();
        let mut preimages = vec![
            payload("inventory", seed),
            payload("preflight", seed),
            payload("mir", seed),
            payload("middle-end", seed),
            payload("kir", seed),
            payload("correspondence", seed),
            payload("formal-memory", seed),
            payload("proof", seed),
            payload("target", seed),
            payload("layout", seed),
            payload("abi", seed),
            payload("exports", seed),
            payload("lowering", seed),
            payload("semantic-llvm", seed),
        ];
        preimages.push(final_commitment.to_vec());
        let encoded_receipts = preimages
            .iter()
            .map(|bytes| 4 + bytes.len() + 32)
            .sum::<usize>();
        let total_len =
            24 + 4 + invocation_bytes.len() + 32 + 2 + TARGET.len() + encoded_receipts + 32;

        let mut capsule = Vec::with_capacity(total_len);
        capsule.extend_from_slice(&CAPSULE_MAGIC_V3);
        capsule.extend_from_slice(&CAPSULE_VERSION_V3.to_le_bytes());
        capsule.extend_from_slice(&0_u16.to_le_bytes());
        capsule.extend_from_slice(&(total_len as u64).to_le_bytes());
        capsule.extend_from_slice(&0_u32.to_le_bytes());
        push_blob(&mut capsule, &invocation_bytes);
        capsule.extend_from_slice(invocation_digest.as_bytes());
        capsule.extend_from_slice(&(TARGET.len() as u16).to_le_bytes());
        capsule.extend_from_slice(TARGET.as_bytes());
        for (domain, preimage) in RECEIPT_DOMAINS_V3.iter().zip(preimages) {
            push_blob(&mut capsule, &preimage);
            capsule.extend_from_slice(&identity(domain, &preimage));
        }
        let capsule_identity = identity(CAPSULE_IDENTITY_DOMAIN_V3, &capsule);
        capsule.extend_from_slice(&capsule_identity);
        assert_eq!(capsule.len(), total_len);
        capsule
    }

    fn module_handoff(seed: u8) -> CompilerModuleHandoffV2 {
        let envelope =
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target(), CodeObjectVersion::V5)
                .unwrap();
        let manifest = CompilerModuleSymbolManifestV1::new([
            (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
            (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
        ])
        .unwrap();
        let module = format!(
            "; ModuleID = 'cargo-protected-v3-{seed:02x}'\ndefine amdgpu_kernel void @kernel() {{ ret void }}\n"
        );
        CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            target(),
            CodeObjectVersion::V5,
            envelope,
            manifest,
            module.as_bytes(),
        )
        .unwrap()
    }

    fn outer(
        custody: &ParentRustcInvocationCustody,
        seed: u8,
    ) -> InertSemanticCompilerModuleHandoffV3 {
        let module_handoff = module_handoff(seed);
        let commitment =
            InertFinalCompilerModuleCommitmentV3::from_handoff(&module_handoff).unwrap();
        let capsule = canonical_capsule_bytes(
            custody.protected_v3().unwrap().descriptor(),
            seed,
            commitment.canonical_bytes(),
        );
        let capsule_identity: [u8; 32] = capsule[capsule.len() - 32..].try_into().unwrap();
        let module_identity = module_handoff.identity();

        let mut pair = Vec::with_capacity(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3);
        pair.extend_from_slice(&INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3);
        pair.extend_from_slice(&INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V3.to_le_bytes());
        pair.extend_from_slice(&0_u16.to_le_bytes());
        pair.extend_from_slice(&(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3 as u32).to_le_bytes());
        pair.extend_from_slice(&0_u32.to_le_bytes());
        pair.extend_from_slice(&capsule_identity);
        pair.extend_from_slice(&(capsule.len() as u64).to_le_bytes());
        pair.extend_from_slice(module_identity.sha256());
        pair.extend_from_slice(&module_identity.byte_len().to_le_bytes());
        let pair_identity = identity(PAIR_IDENTITY_DOMAIN_V3, &pair);
        pair.extend_from_slice(&pair_identity);
        assert_eq!(pair.len(), INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3);

        let module_bytes = module_handoff.canonical_bytes();
        let total_len = 40 + capsule.len() + module_bytes.len() + pair.len() + 32;
        let mut outer = Vec::with_capacity(total_len);
        outer.extend_from_slice(&INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3);
        outer.extend_from_slice(&INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V3.to_le_bytes());
        outer.extend_from_slice(&0_u16.to_le_bytes());
        outer.extend_from_slice(&(total_len as u64).to_le_bytes());
        outer.extend_from_slice(&0_u32.to_le_bytes());
        outer.extend_from_slice(&(capsule.len() as u64).to_le_bytes());
        outer.extend_from_slice(&(module_bytes.len() as u64).to_le_bytes());
        outer.extend_from_slice(&capsule);
        outer.extend_from_slice(module_bytes);
        outer.extend_from_slice(&pair);
        let outer_identity = identity(OUTER_IDENTITY_DOMAIN_V3, &outer);
        outer.extend_from_slice(&outer_identity);
        assert_eq!(outer.len(), total_len);
        InertSemanticCompilerModuleHandoffV3::decode(&outer).unwrap()
    }

    #[test]
    fn parent_custody_survives_child_exit_and_managed_completion() {
        let (custody, _) = protected_parent_custody(0x10);
        assert!(
            crate::process_execution::status(&mut Command::new("/bin/true"))
                .unwrap()
                .success()
        );
        let completed = custody.retain_through(|custody| {
            custody.revalidate().unwrap();
            assert!(!custody.grants_compiler_authority());
            "managed-completion-finished"
        });
        assert_eq!(completed, "managed-completion-finished");
    }

    #[test]
    fn protected_v3_intake_rejects_wrong_attempt_without_consuming() {
        let directory = TestDirectory::new("wrong-attempt");
        let producer = producer(0x20);
        let attempt = begin(&directory.0, &producer, 0x21);
        let wrong_attempt = BuildAttempt::from_env_value(&format!(
            "{}:{}:{}",
            attempt.generation() + 1,
            attempt.session(),
            attempt.invocation()
        ))
        .unwrap();
        let (custody, _) = protected_parent_custody(0x22);
        let handoff = outer(&custody, 0x23);
        publish_compiler_module_handoff_v3(&directory.0, &producer, attempt, &handoff).unwrap();
        let intake = ProtectedCompilerModuleHandoffIntake::protected_v3();

        assert!(matches!(
            intake.consume_v3(&directory.0, &producer, wrong_attempt, &custody),
            Err(ProtectedCompilerModuleHandoffIntakeError::V3(
                CompilerModuleHandoffErrorV3::Attempt { .. }
            ))
        ));
        let consumed = intake
            .consume_v3(&directory.0, &producer, attempt, &custody)
            .unwrap();
        assert_eq!(consumed.receipt().attempt(), attempt);
        assert_eq!(consumed.consumed().attempt(), attempt);
        assert_eq!(
            consumed.compiler_closure(),
            *handoff.capsule().compiler_closure()
        );
        assert!(!consumed.grants_compiler_authority());
        let (receipt, transaction, compiler_closure) = consumed.into_parts();
        assert_eq!(
            receipt.transaction_identity(),
            transaction.transaction_identity()
        );
        assert_eq!(compiler_closure, *handoff.capsule().compiler_closure());
    }

    #[test]
    fn protected_v3_intake_runs_preflight_before_one_shot_consumption() {
        let directory = TestDirectory::new("preflight-order");
        let producer = producer(0x24);
        let attempt = begin(&directory.0, &producer, 0x25);
        let (custody, _) = protected_parent_custody(0x26);
        let handoff = outer(&custody, 0x27);
        let receipt =
            publish_compiler_module_handoff_v3(&directory.0, &producer, attempt, &handoff).unwrap();
        let intake = ProtectedCompilerModuleHandoffIntake::protected_v3();

        let (consumed, marker) = intake
            .consume_v3_after_preflight(
                &directory.0,
                &producer,
                attempt,
                &custody,
                |observed, observed_receipt, compiler_closure| {
                    assert_eq!(observed, &handoff);
                    assert_eq!(observed_receipt, receipt);
                    assert_eq!(compiler_closure, *handoff.capsule().compiler_closure());
                    Ok("preflight-complete")
                },
            )
            .unwrap();
        assert_eq!(marker, "preflight-complete");
        assert_eq!(consumed.receipt(), receipt);
        assert!(matches!(
            intake.consume_v3(&directory.0, &producer, attempt, &custody),
            Err(ProtectedCompilerModuleHandoffIntakeError::V3(
                CompilerModuleHandoffErrorV3::AlreadyConsumed
            ))
        ));
    }

    #[test]
    fn protected_v3_preflight_rejection_leaves_transaction_unconsumed() {
        let directory = TestDirectory::new("preflight-rejection");
        let producer = producer(0x28);
        let attempt = begin(&directory.0, &producer, 0x29);
        let (custody, _) = protected_parent_custody(0x2a);
        let handoff = outer(&custody, 0x2b);
        publish_compiler_module_handoff_v3(&directory.0, &producer, attempt, &handoff).unwrap();
        let intake = ProtectedCompilerModuleHandoffIntake::protected_v3();

        assert!(matches!(
            intake.consume_v3_after_preflight(
                &directory.0,
                &producer,
                attempt,
                &custody,
                |_, _, _| Err::<(), _>(ProtectedFirstBuildWorkerV3Error::ReplayValidation {
                    field: "fixture deterministic rejection",
                }),
            ),
            Err(ProtectedCompilerModuleHandoffIntakeError::WorkerPreflight(
                _
            ))
        ));
        assert!(
            intake
                .consume_v3(&directory.0, &producer, attempt, &custody)
                .is_ok()
        );
    }

    #[test]
    fn protected_v3_intake_rejects_wrong_producer_without_consuming() {
        let directory = TestDirectory::new("wrong-producer");
        let owner = producer(0x30);
        let intruder = producer(0x31);
        let attempt = begin(&directory.0, &owner, 0x32);
        let (custody, _) = protected_parent_custody(0x33);
        let handoff = outer(&custody, 0x34);
        publish_compiler_module_handoff_v3(&directory.0, &owner, attempt, &handoff).unwrap();
        let intake = ProtectedCompilerModuleHandoffIntake::protected_v3();

        assert!(matches!(
            intake.consume_v3(&directory.0, &intruder, attempt, &custody),
            Err(ProtectedCompilerModuleHandoffIntakeError::V3(
                CompilerModuleHandoffErrorV3::Attempt { .. }
            ))
        ));
        assert!(
            intake
                .consume_v3(&directory.0, &owner, attempt, &custody)
                .is_ok()
        );
    }

    #[test]
    fn protected_v3_intake_derives_identity_and_rejects_conflicting_publication() {
        let directory = TestDirectory::new("wrong-identity");
        let producer = producer(0x40);
        let attempt = begin(&directory.0, &producer, 0x41);
        let (custody, _) = protected_parent_custody(0x42);
        let handoff = outer(&custody, 0x43);
        let unrelated = outer(&custody, 0x44);
        publish_compiler_module_handoff_v3(&directory.0, &producer, attempt, &handoff).unwrap();
        let unrelated_receipt =
            publish_compiler_module_handoff_v3(&directory.0, &producer, attempt, &unrelated);
        assert!(matches!(
            unrelated_receipt,
            Err(CompilerModuleHandoffErrorV3::WrongHandoffIdentity)
        ));
        let exact = ProtectedCompilerModuleHandoffIntake::protected_v3();
        assert!(
            exact
                .consume_v3(&directory.0, &producer, attempt, &custody)
                .is_ok()
        );
    }

    #[test]
    fn protected_v3_intake_rejects_wrong_parent_before_consumption() {
        let directory = TestDirectory::new("wrong-parent");
        let producer = producer(0x48);
        let attempt = begin(&directory.0, &producer, 0x49);
        let (exact_custody, _) = protected_parent_custody(0x4a);
        let (wrong_custody, _) = protected_parent_custody(0x4b);
        let handoff = outer(&exact_custody, 0x4c);
        publish_compiler_module_handoff_v3(&directory.0, &producer, attempt, &handoff).unwrap();
        let intake = ProtectedCompilerModuleHandoffIntake::protected_v3();

        assert!(matches!(
            intake.consume_v3(&directory.0, &producer, attempt, &wrong_custody),
            Err(ProtectedCompilerModuleHandoffIntakeError::InvocationMismatch)
        ));
        assert!(
            intake
                .consume_v3(&directory.0, &producer, attempt, &exact_custody)
                .is_ok()
        );
    }

    #[test]
    fn protected_v3_intake_is_one_shot_and_rejects_replay() {
        let directory = TestDirectory::new("replay");
        let producer = producer(0x50);
        let attempt = begin(&directory.0, &producer, 0x51);
        let (custody, _) = protected_parent_custody(0x52);
        let handoff = outer(&custody, 0x53);
        publish_compiler_module_handoff_v3(&directory.0, &producer, attempt, &handoff).unwrap();
        let intake = ProtectedCompilerModuleHandoffIntake::protected_v3();

        assert!(
            intake
                .consume_v3(&directory.0, &producer, attempt, &custody)
                .is_ok()
        );
        assert!(matches!(
            intake.consume_v3(&directory.0, &producer, attempt, &custody),
            Err(ProtectedCompilerModuleHandoffIntakeError::V3(
                CompilerModuleHandoffErrorV3::AlreadyConsumed
            ))
        ));
    }

    #[test]
    fn protected_v3_intake_rejects_absent_publication() {
        let directory = TestDirectory::new("absent");
        let producer = producer(0x60);
        let attempt = begin(&directory.0, &producer, 0x61);
        let (custody, _) = protected_parent_custody(0x62);
        let intake = ProtectedCompilerModuleHandoffIntake::protected_v3();

        assert!(matches!(
            intake.consume_v3(&directory.0, &producer, attempt, &custody),
            Err(ProtectedCompilerModuleHandoffIntakeError::V3(
                CompilerModuleHandoffErrorV3::NotPublished
            ))
        ));
    }

    #[test]
    fn protected_v3_intake_never_falls_back_to_published_v2() {
        let directory = TestDirectory::new("no-v2-fallback");
        let producer = producer(0x70);
        let attempt = begin(&directory.0, &producer, 0x71);
        let (custody, closure) = protected_parent_custody(0x72);
        let handoff = outer(&custody, 0x73);
        let v2_bytes = handoff.module_handoff().canonical_bytes();
        publish_compiler_module_handoff_v2(&directory.0, &producer, attempt, closure, v2_bytes)
            .unwrap();
        let intake = ProtectedCompilerModuleHandoffIntake::protected_v3();

        assert!(matches!(
            intake.consume_v3(&directory.0, &producer, attempt, &custody),
            Err(ProtectedCompilerModuleHandoffIntakeError::V3(
                CompilerModuleHandoffErrorV3::NotPublished
            ))
        ));
        assert_eq!(
            consume_compiler_module_handoff_v2(&directory.0, &producer, attempt, closure)
                .unwrap()
                .bytes(),
            v2_bytes
        );
    }

    #[test]
    fn selected_v2_intake_preserves_existing_transport_behavior() {
        let directory = TestDirectory::new("v2-preserved");
        let producer = producer(0x80);
        let attempt = begin(&directory.0, &producer, 0x81);
        let closure = compiler_closure(0x82);
        publish_compiler_module_handoff_v2(
            &directory.0,
            &producer,
            attempt,
            closure,
            b"existing protected V2 handoff",
        )
        .unwrap();
        let intake = ProtectedCompilerModuleHandoffIntake::protected_v2(closure);
        assert_eq!(
            intake
                .consume_v2(&directory.0, &producer, attempt)
                .unwrap()
                .bytes(),
            b"existing protected V2 handoff"
        );
        assert!(matches!(
            consume_compiler_module_handoff_v2(&directory.0, &producer, attempt, closure),
            Err(CompilerModuleHandoffErrorV2::AlreadyConsumed)
        ));
    }
}
