//! Attempt-scoped publication of one inert compiler module for the Worker V2 pipeline.

use crate::kernel_ir_codegen::{
    CompilerModuleConstructionError, InertCompilerModuleTextV1,
    construct_inert_compiler_module_text_v1,
};
use fe2o3_amd_target::{CapabilityDerivationError, WavefrontWidth};
use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffErrorV1 as HandoffPublicationErrorV1,
    CompilerModuleHandoffReceiptV1, ProducerIdentity, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CompilerFfiEnvelopeV1, CompilerModuleHandoffErrorV1, CompilerModuleHandoffV1,
    CompilerModuleKindV1,
};
use fe2o3_kernel_ir::{Module, TargetCapability, WaveWidth, WorkgroupSize};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::path::Path;

const G1_WORKGROUP_X: u32 = 256;

/// Constructs and publishes one canonical, inert compiler-module handoff.
///
/// The handoff remains coordination data. Publication proves possession of the cooperative build
/// attempt and exact byte identity; it does not grant artifact, link, load, or launch authority.
pub(crate) fn publish_worker_v2_compiler_module(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: Option<BuildAttempt>,
    envelope: Option<&CompilerFfiEnvelopeV1>,
    module: &Module,
) -> Result<CompilerModuleHandoffReceiptV1, WorkerV2ProducerError> {
    let attempt = attempt.ok_or(WorkerV2ProducerError::MissingBuildAttempt)?;
    let envelope = envelope.ok_or(WorkerV2ProducerError::MissingCompilerFfiEnvelope)?;
    let module = bind_g1_launch_contract(module)?;
    let module = bind_exact_target_wave_mode(envelope, &module)?;
    let compiler_module = construct_inert_compiler_module_text_v1(&module)
        .map_err(WorkerV2ProducerError::CompilerModule)?;
    validate_envelope_module_roles(envelope, &compiler_module)?;

    let handoff = CompilerModuleHandoffV1::new(
        CompilerModuleKindV1::LlvmTextIr,
        envelope.target(),
        envelope.code_object_version(),
        envelope.clone(),
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(WorkerV2ProducerError::Handoff)?;

    publish_compiler_module_handoff_v1(output_dir, producer, attempt, handoff.canonical_bytes())
        .map_err(WorkerV2ProducerError::Publication)
}

fn bind_g1_launch_contract(module: &Module) -> Result<Module, WorkerV2ProducerError> {
    let required = WorkgroupSize::new(G1_WORKGROUP_X, 1, 1);
    let mut bound = module.clone();
    for kernel in &mut bound.kernels {
        match kernel.workgroup_size {
            None => kernel.workgroup_size = Some(required),
            Some(declared) if declared == required => {}
            Some(declared) => {
                return Err(WorkerV2ProducerError::ConflictingWorkgroupSize {
                    kernel: kernel.id.as_str().to_owned(),
                    declared,
                    required,
                });
            }
        }
    }
    Ok(bound)
}

fn bind_exact_target_wave_mode(
    envelope: &CompilerFfiEnvelopeV1,
    module: &Module,
) -> Result<Module, WorkerV2ProducerError> {
    let target = envelope.target().as_amd_target_id();
    let capabilities = target
        .capabilities()
        .map_err(WorkerV2ProducerError::TargetCapabilities)?;
    let mut declared = BTreeSet::new();
    for capability in module
        .required_capabilities
        .iter()
        .chain(
            module
                .functions
                .iter()
                .flat_map(|function| &function.required_capabilities),
        )
        .chain(
            module
                .kernels
                .iter()
                .flat_map(|kernel| &kernel.required_capabilities),
        )
    {
        if let TargetCapability::WaveWidth(width) = capability {
            declared.insert(*width);
        }
    }
    for width in &declared {
        let target_width = match width {
            WaveWidth::Wave32 => WavefrontWidth::Wave32,
            WaveWidth::Wave64 => WavefrontWidth::Wave64,
        };
        if !capabilities.wavefront_widths().contains(target_width) {
            return Err(WorkerV2ProducerError::UnsupportedWaveMode {
                target: envelope.target().to_string(),
                width: *width,
            });
        }
    }

    // A single selected mode can safely govern standalone exports and helper
    // SCCs. Mixed-mode modules retain their per-root claims; an unclaimed
    // standalone SCC then remains an explicit lowering error.
    if declared.len() > 1 {
        return Ok(module.clone());
    }
    let width = declared.into_iter().next().unwrap_or_else(|| {
        match capabilities.default_wavefront_width() {
            WavefrontWidth::Wave32 => WaveWidth::Wave32,
            WavefrontWidth::Wave64 => WaveWidth::Wave64,
        }
    });
    let mut bound = module.clone();
    bound
        .required_capabilities
        .insert(TargetCapability::WaveWidth(width));
    Ok(bound)
}

fn validate_envelope_module_roles(
    envelope: &CompilerFfiEnvelopeV1,
    module: &InertCompilerModuleTextV1,
) -> Result<(), WorkerV2ProducerError> {
    let symbols = envelope.directional_symbols();
    for symbol in symbols.imports() {
        if module
            .external_declarations()
            .binary_search_by(|candidate| candidate.as_str().cmp(symbol))
            .is_err()
        {
            return Err(WorkerV2ProducerError::MissingExternalDeclaration(
                symbol.to_owned(),
            ));
        }
    }
    for symbol in symbols.exports() {
        if module
            .device_ffi_exports()
            .binary_search_by(|candidate| candidate.as_str().cmp(symbol))
            .is_err()
        {
            return Err(WorkerV2ProducerError::MissingCompilerDefinition(
                symbol.to_owned(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum WorkerV2ProducerError {
    MissingBuildAttempt,
    MissingCompilerFfiEnvelope,
    MissingExternalDeclaration(String),
    MissingCompilerDefinition(String),
    TargetCapabilities(CapabilityDerivationError),
    UnsupportedWaveMode {
        target: String,
        width: WaveWidth,
    },
    ConflictingWorkgroupSize {
        kernel: String,
        declared: WorkgroupSize,
        required: WorkgroupSize,
    },
    CompilerModule(CompilerModuleConstructionError),
    Handoff(CompilerModuleHandoffErrorV1),
    Publication(HandoffPublicationErrorV1),
}

impl fmt::Display for WorkerV2ProducerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBuildAttempt => {
                formatter.write_str("kernel-ir-worker-v2 requires a managed FE2O3_BUILD_ATTEMPT_V1")
            }
            Self::MissingCompilerFfiEnvelope => {
                formatter.write_str("kernel-ir-worker-v2 requires a complete compiler FFI envelope")
            }
            Self::MissingExternalDeclaration(symbol) => write!(
                formatter,
                "compiler FFI import {symbol:?} is absent from the whole kernel IR module's external declarations"
            ),
            Self::MissingCompilerDefinition(symbol) => write!(
                formatter,
                "compiler FFI export {symbol:?} is absent from the whole kernel IR module's device FFI definitions"
            ),
            Self::TargetCapabilities(error) => {
                write!(
                    formatter,
                    "cannot derive exact target capabilities: {error}"
                )
            }
            Self::UnsupportedWaveMode { target, width } => write!(
                formatter,
                "compiler module requires {width:?}, which target {target} does not support"
            ),
            Self::ConflictingWorkgroupSize {
                kernel,
                declared,
                required,
            } => write!(
                formatter,
                "kernel {kernel:?} declares workgroup size ({}, {}, {}), but the Worker V2 G1 profile requires ({}, {}, {})",
                declared.x, declared.y, declared.z, required.x, required.y, required.z
            ),
            Self::CompilerModule(error) => {
                write!(
                    formatter,
                    "whole compiler-module construction failed: {error}"
                )
            }
            Self::Handoff(error) => {
                write!(
                    formatter,
                    "compiler-module handoff construction failed: {error}"
                )
            }
            Self::Publication(error) => {
                write!(
                    formatter,
                    "compiler-module handoff publication failed: {error}"
                )
            }
        }
    }
}

impl Error for WorkerV2ProducerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CompilerModule(error) => Some(error),
            Self::TargetCapabilities(error) => Some(error),
            Self::Handoff(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::MissingBuildAttempt
            | Self::MissingCompilerFfiEnvelope
            | Self::MissingExternalDeclaration(_)
            | Self::MissingCompilerDefinition(_)
            | Self::UnsupportedWaveMode { .. }
            | Self::ConflictingWorkgroupSize { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifact_transaction::{
        BuildInvocation, BuildSession, CompilerModuleHandoffErrorV1 as PublicationError,
        begin_build_attempt, consume_compiler_module_handoff_v1,
    };
    use fe2o3_compiler_ffi::{
        CodeObjectVersion, CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1,
        CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1, CompilerModuleHandoffV1, DeviceTargetV1,
    };
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Signature,
        TargetCapability, Terminator, WaveWidth, WorkgroupSize,
    };
    use reserved_fe2o3_symbols::{
        DeviceFfiContractFieldsV1, DeviceFfiDirectionV1, derive_device_ffi_contract_id_v1,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const IMPORT_ABI: &str =
        "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";
    const EXPORT_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-worker-v2-producer-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn target() -> DeviceTargetV1 {
        DeviceTargetV1::parse("gfx942:xnack-").unwrap()
    }

    fn producer() -> ProducerIdentity {
        ProducerIdentity::from_codegen(
            "worker_v2_fixture",
            Some(Path::new("/workspace/worker-v2-fixture/src/lib.rs")),
        )
        .unwrap()
    }

    fn begin_attempt(directory: &Path, producer: &ProducerIdentity) -> BuildAttempt {
        begin_build_attempt(
            directory,
            producer,
            BuildInvocation::from_bytes([0x42; 32]),
            BuildSession::from_bytes([0x31; 16]),
        )
        .unwrap()
    }

    fn owner(byte: u8, item: &str) -> CompilerFfiSourceOwnerV1 {
        CompilerFfiSourceOwnerV1::new(
            "worker_v2_fixture",
            &format!("worker_v2_fixture::{item}"),
            [byte; 16],
            &format!("_RINvNtCs1234_worker_v2_fixture{item}"),
        )
        .unwrap()
    }

    fn contract(
        direction: DeviceFfiDirectionV1,
        symbol: &str,
        abi: &str,
        effects: &str,
        semantic_byte: u8,
    ) -> CompilerFfiContractV1 {
        let semantic_identity = [semantic_byte; 32];
        let semantic_text = semantic_identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: direction.tag(),
            symbol,
            calling_convention: "C",
            code_object_version: 5,
            target: "gfx942:xnack-",
            physical_abi: abi,
            effects,
            semantic_identity: &semantic_text,
        });
        CompilerFfiContractV1::new(
            id,
            direction,
            match direction {
                DeviceFfiDirectionV1::Import => CompilerFfiLinkRoleV1::RequiresExternalDefinition,
                DeviceFfiDirectionV1::Export => {
                    CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition
                }
            },
            target(),
            CodeObjectVersion::V5,
            owner(semantic_byte, symbol),
            symbol,
            abi,
            effects,
            semantic_identity,
        )
        .unwrap()
    }

    fn envelope() -> CompilerFfiEnvelopeV1 {
        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(target(), CodeObjectVersion::V5, 2).unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Import,
                "external_add",
                IMPORT_ABI,
                "read_global",
                0x11,
            ))
            .unwrap();
        builder
            .push(contract(
                DeviceFfiDirectionV1::Export,
                "rust_helper",
                EXPORT_ABI,
                "none",
                0x22,
            ))
            .unwrap();
        builder.finish().unwrap()
    }

    fn returning_block() -> BasicBlock {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        block
    }

    fn complete_module() -> Module {
        let entry = Function::kernel_entry(
            "entry_impl",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returning_block()],
        );
        let mut export = Function::device_ffi_export(
            "rust_helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returning_block()],
        );
        export
            .required_capabilities
            .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
        let import = Function::declaration("external_add", Signature::new(vec![], vec![]));
        let mut kernel = Kernel::new(
            "entry",
            "entry_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(G1_WORKGROUP_X, 1, 1));

        let mut module = Module::new("tests::worker_v2_producer");
        module.functions = vec![entry, export, import];
        module.kernels.push(kernel);
        module
    }

    #[test]
    fn publishes_exact_text_handoff_without_artifact_authority() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();

        let receipt = publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &complete_module(),
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        let handoff = CompilerModuleHandoffV1::decode(consumed.bytes()).unwrap();

        assert_eq!(receipt.attempt(), attempt);
        assert_eq!(receipt.identity(), consumed.identity());
        assert_eq!(handoff.kind(), CompilerModuleKindV1::LlvmTextIr);
        assert_eq!(handoff.target(), envelope.target());
        assert_eq!(
            handoff.code_object_version(),
            envelope.code_object_version()
        );
        let module_text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(module_text.contains("define amdgpu_kernel void @entry"));
        assert!(module_text.contains("define void @rust_helper"));
        assert!(module_text.contains("declare void @external_add"));
        assert!(!receipt.grants_publication_authority());
        assert!(!receipt.grants_compiler_authority());
        assert!(!consumed.grants_link_authority());
        assert!(!consumed.grants_load_authority());
        assert!(!consumed.grants_launch_authority());
    }

    #[test]
    fn rejects_missing_attempt_or_envelope_before_publication() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();

        assert!(matches!(
            publish_worker_v2_compiler_module(
                &directory.0,
                &producer,
                None,
                Some(&envelope),
                &complete_module(),
            ),
            Err(WorkerV2ProducerError::MissingBuildAttempt)
        ));
        assert!(matches!(
            publish_worker_v2_compiler_module(
                &directory.0,
                &producer,
                Some(attempt),
                None,
                &complete_module(),
            ),
            Err(WorkerV2ProducerError::MissingCompilerFfiEnvelope)
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::NotPublished)
        ));
    }

    #[test]
    fn rejects_envelope_roles_missing_from_the_compiler_module() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module
            .functions
            .retain(|function| function.id.as_str() != "rust_helper");

        assert!(matches!(
            publish_worker_v2_compiler_module(
                &directory.0,
                &producer,
                Some(attempt),
                Some(&envelope),
                &module,
            ),
            Err(WorkerV2ProducerError::MissingCompilerDefinition(symbol))
                if symbol == "rust_helper"
        ));
        let mut module = complete_module();
        module
            .functions
            .retain(|function| function.id.as_str() != "external_add");
        assert!(matches!(
            publish_worker_v2_compiler_module(
                &directory.0,
                &producer,
                Some(attempt),
                Some(&envelope),
                &module,
            ),
            Err(WorkerV2ProducerError::MissingExternalDeclaration(symbol))
                if symbol == "external_add"
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::NotPublished)
        ));
    }

    #[test]
    fn binds_missing_and_accepts_exact_g1_workgroup_sizes() {
        let explicit = complete_module();
        assert_eq!(
            bind_g1_launch_contract(&explicit).unwrap().kernels[0].workgroup_size,
            Some(WorkgroupSize::new(G1_WORKGROUP_X, 1, 1))
        );

        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module.kernels[0].workgroup_size = None;

        publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &module,
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        let handoff = CompilerModuleHandoffV1::decode(consumed.bytes()).unwrap();
        let text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(text.contains("\"amdgpu-flat-work-group-size\"=\"256,256\""));
        assert!(text.contains("!reqd_work_group_size"));
    }

    #[test]
    fn rejects_a_conflicting_workgroup_size_without_publishing() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

        let error = publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &module,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            WorkerV2ProducerError::ConflictingWorkgroupSize {
                kernel,
                declared: WorkgroupSize { x: 64, y: 1, z: 1 },
                required: WorkgroupSize { x: 256, y: 1, z: 1 },
            } if kernel == "entry"
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::NotPublished)
        ));
    }

    #[test]
    fn binds_the_target_default_wave_mode_for_a_standalone_export() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "rust_helper")
            .unwrap()
            .required_capabilities
            .clear();

        publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &module,
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        let handoff = CompilerModuleHandoffV1::decode(consumed.bytes()).unwrap();
        let text = std::str::from_utf8(handoff.module_bytes()).unwrap();
        assert!(text.contains("-wavefrontsize32,+wavefrontsize64"));
    }

    #[test]
    fn rejects_a_wave_mode_unsupported_by_the_exact_target() {
        let directory = TestDirectory::new();
        let producer = producer();
        let attempt = begin_attempt(&directory.0, &producer);
        let envelope = envelope();
        let mut module = complete_module();
        module
            .required_capabilities
            .insert(TargetCapability::WaveWidth(
                fe2o3_kernel_ir::WaveWidth::Wave32,
            ));

        let error = publish_worker_v2_compiler_module(
            &directory.0,
            &producer,
            Some(attempt),
            Some(&envelope),
            &module,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            WorkerV2ProducerError::UnsupportedWaveMode {
                width: WaveWidth::Wave32,
                ..
            }
        ));
        assert!(matches!(
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt),
            Err(PublicationError::NotPublished)
        ));
    }
}
