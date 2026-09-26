//! Framing-only fixture. Opaque history/source leaves are not genuine proofs,
//! decoded F, compiler origin, nominal custody or native execution evidence.
use super::*;
use crate::{BuildInvocation, BuildSession, begin_build_attempt};
use fe2o3_compiler_ffi::*;
use fe2o3_compiler_lineage::*;

pub(super) struct Fixture {
    pub path: PathBuf,
    pub producer: ProducerIdentity,
    pub attempt: BuildAttempt,
    pub handoff: Handoff,
}
impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-conditional-transaction-v5-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let producer =
            ProducerIdentity::from_codegen("conditional", Some(Path::new("/conditional.rs")))
                .unwrap();
        let attempt = begin_build_attempt(
            &path,
            &producer,
            BuildInvocation::from_bytes([3; 32]),
            BuildSession::from_bytes([4; 16]),
        )
        .unwrap();
        Self {
            path,
            producer,
            attempt,
            handoff: outer(),
        }
    }
    pub fn reserve(&self, budget: &mut Budget<'_>) -> usize {
        let storage = payload_storage(&self.handoff).unwrap();
        budget.reserve_storage(storage).unwrap();
        storage
    }
    pub fn publish(&self, budget: &mut Budget<'_>) -> Result<CompilerModuleHandoffReceiptV5> {
        publish_compiler_module_handoff_v5(
            &self.path,
            &self.producer,
            self.attempt,
            &self.handoff,
            budget,
        )
    }
    pub fn recover(&self, budget: &mut Budget<'_>) -> Result<CompilerModuleHandoffReceiptV5> {
        recover_compiler_module_handoff_receipt_v5(&self.path, &self.producer, self.attempt, budget)
    }
    pub fn lease(
        &self,
        receipt: CompilerModuleHandoffReceiptV5,
        budget: &mut Budget<'_>,
    ) -> CompilerModuleHandoffCurrentnessLeaseV5 {
        let (lease, storage) = acquire_compiler_module_handoff_currentness_lease_v5(
            &self.path,
            &self.producer,
            receipt,
            budget,
        )
        .unwrap();
        budget.reserve_storage(storage.0).unwrap();
        lease
    }
    pub fn slot(&self) -> PathBuf {
        let producer = producer_identity_for::<Schema>(&self.producer);
        let slot = slot_identity_for::<Schema>(
            producer,
            self.attempt,
            CompilerModuleHandoffSlotV5::Production,
        );
        self.path
            .join(format!("{}{}", Schema::PARENT_PREFIX, hex(&producer)))
            .join(format!("{}{}", Schema::SLOT_PREFIX, hex(&slot)))
    }
    pub fn ready(&self) {
        assert!(self.slot().join(READY_ENTRY).exists());
        assert!(!self.slot().join(CONSUMED_ENTRY).exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}

pub(super) fn token(
    lease: &CompilerModuleHandoffCurrentnessLeaseV5,
    budget: &mut Budget<'_>,
) -> CompilerModuleHandoffConsumptionTokenV5 {
    let (token, storage) = lease.acquire_current_token(budget).unwrap();
    budget.reserve_storage(storage.0).unwrap();
    token
}

fn outer() -> Handoff {
    // Reuse only public inert invocation/receipt fixture content, not V3 authority
    // or an ordinary base inside V5. Construct the V5 tree independently.
    let old = super::super::super::semantic_v3::tests::outer(7);
    let invocation =
        fe2o3_rustc_invocation::encode_descriptor_v3(old.capsule().invocation()).unwrap();
    let target = old.module_handoff().target();
    let llvm = old.module_handoff().module_bytes();
    let module = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap(),
        CompilerModuleSymbolManifestV1::new([
            (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
            (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
        ])
        .unwrap(),
        llvm,
    )
    .unwrap();
    let commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(&module).unwrap();
    let output_layout = NativeConditionalOutputLayoutV1::new::<()>(19, 23, 29).unwrap();
    let mut output = vec![7; output_layout.encoded_len()];
    seal_native_conditional_output_v1(output_layout, &mut output, LIMIT, |_| Ok::<_, ()>(()))
        .unwrap();
    let carrier_layout = NativeConditionalCarrierLayoutV1::new::<()>(output.len(), 31).unwrap();
    let mut carrier = vec![11; carrier_layout.encoded_len()];
    carrier[carrier_layout.output_range()].copy_from_slice(&output);
    let carrier_id = seal_native_conditional_carrier_v1(
        carrier_layout,
        &mut carrier,
        LIMIT,
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    let lowering = lowering_fixture(
        TargetLineageIdentityV3::new(*carrier_id.sha256(), carrier_id.byte_len()).unwrap(),
        &output[output_layout.descriptor_range()],
        llvm,
        module.canonical_bytes(),
    );
    let layout = canonical_semantic_target_layout_transcript_v1(
        "amdgcn-amd-amdhsa",
        "e-p:64:64",
        64,
        "gfx942",
        "-wavefrontsize32,+wavefrontsize64,-xnack",
    )
    .unwrap();
    let inputs = NativeConditionalMetadataInputV1 {
        invocation: &invocation,
        rustc_inventory: old
            .capsule()
            .receipts()
            .rustc_identity_inventory()
            .canonical_preimage(),
        rustc_preflight: old
            .capsule()
            .receipts()
            .rustc_preflight_plan()
            .canonical_preimage(),
        semantic_target_layout: &layout,
        native_lowering: lowering.canonical_bytes(),
        final_module_commitment: commitment.canonical_bytes(),
    };
    let metadata_layout = NativeConditionalMetadataLayoutV1::new::<()>(inputs).unwrap();
    let mut metadata = vec![0; metadata_layout.encoded_len()];
    for (range, bytes) in [
        (metadata_layout.invocation_range(), inputs.invocation),
        (
            metadata_layout.rustc_inventory_range(),
            inputs.rustc_inventory,
        ),
        (
            metadata_layout.rustc_preflight_range(),
            inputs.rustc_preflight,
        ),
        (
            metadata_layout.semantic_target_layout_range(),
            inputs.semantic_target_layout,
        ),
        (
            metadata_layout.native_lowering_range(),
            inputs.native_lowering,
        ),
        (
            metadata_layout.final_module_commitment_range(),
            inputs.final_module_commitment,
        ),
    ] {
        metadata[range].copy_from_slice(bytes);
    }
    seal_native_conditional_metadata_v1(metadata_layout, &mut metadata, LIMIT, |_| Ok::<_, ()>(()))
        .unwrap();
    let capsule_layout =
        InertProductionSemanticCapsuleLayoutV5::new::<()>(metadata.len(), carrier.len()).unwrap();
    let mut bytes = vec![0; capsule_layout.encoded_len()];
    bytes[capsule_layout.metadata_range()].copy_from_slice(&metadata);
    bytes[capsule_layout.carrier_range()].copy_from_slice(&carrier);
    seal_inert_production_semantic_capsule_v5(capsule_layout, &mut bytes, LIMIT, |_| {
        Ok::<_, ()>(())
    })
    .unwrap();
    let capsule = InertProductionSemanticCapsuleV5::decode_owned(bytes).unwrap();
    preflight_inert_semantic_compiler_module_handoff_v5(&capsule, &module).unwrap();
    let outer = InertSemanticCompilerModuleHandoffLayoutV5::new(
        capsule.canonical_bytes().len(),
        module.canonical_bytes().len(),
    )
    .unwrap();
    let mut bytes = vec![0; outer.encoded_len()];
    bytes[outer.capsule_range()].copy_from_slice(capsule.canonical_bytes());
    bytes[outer.module_handoff_range()].copy_from_slice(module.canonical_bytes());
    seal_inert_semantic_compiler_module_handoff_v5(
        outer,
        &mut bytes,
        capsule.identity(),
        module.identity(),
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    Handoff::decode_owned(bytes).unwrap()
}

fn lowering_fixture(
    carrier: TargetLineageIdentityV3,
    descriptor: &[u8],
    llvm: &[u8],
    module: &[u8],
) -> InertNativeLoweringAssociationV1 {
    // Literal inherited V1 header avoids adding a production target dependency
    // just for this inert fixture. Every coordinate uses the public inert codec.
    let mut bytes = [0; NATIVE_LOWERING_ASSOCIATION_BYTES_V1];
    bytes[..16].copy_from_slice(b"F2NLOW1\0\x01\0\x01\0\x60\x01\0\0");
    bytes[16..24].copy_from_slice(&[1, 0, 12, 0, 6, 0, 0, 0]);
    let subject = InertNativeNeutralSubjectV1::new([1; 32], 19, [2; 32], 23).unwrap();
    bytes[24..120].copy_from_slice(subject.canonical_bytes());
    let axis =
        |b: &[u8]| TargetLineageIdentityV3::new(Sha256::digest(b).into(), b.len() as u64).unwrap();
    for (slot, value) in bytes[120..320].chunks_exact_mut(40).zip([
        carrier,
        axis(descriptor),
        axis(llvm),
        axis(llvm),
        axis(module),
    ]) {
        slot.copy_from_slice(&value.encode());
    }
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/NATIVE-F-TEXT-DESCRIPTOR-ASSOCIATION/V1\0");
    hash.update(320_u64.to_le_bytes());
    hash.update(&bytes[..320]);
    bytes[320..].copy_from_slice(&hash.finalize());
    InertNativeLoweringAssociationV1::decode(&bytes).unwrap()
}
