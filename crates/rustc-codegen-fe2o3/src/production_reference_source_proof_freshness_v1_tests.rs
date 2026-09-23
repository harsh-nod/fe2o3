//! Test-only replay of inert receipts from normal actual-source proof production.
//! No owner, imported proof, lease, or executable escapes the live callback.
use super::*;
use fe2o3_functional_proof::{
    FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2, FunctionalRefinementBindingV2 as Binding,
    FunctionalRefinementBoundaryV2 as Boundary, FunctionalRefinementImportErrorV2 as ImportError,
    FunctionalRefinementImportExpectationV2 as Expectation,
    FunctionalRefinementImportPolicyV2 as Policy,
    FunctionalRefinementReceiptImporterV2 as Importer,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

const TRANSPORT_SCHEMA: &str = "fe2o3-test-source-proof-inert-receipt-v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingRecord {
    reference_identity: [u8; 32],
    reference_mir: [u8; 32],
    kernel_identity: [u8; 32],
    kernel_mir: [u8; 32],
    normalized_obligation: [u8; 32],
}
impl BindingRecord {
    fn from_binding(binding: Binding) -> Self {
        assert_eq!(binding.safe_reference_kind(), SafeReferenceKindV2::Mir);
        assert_eq!(binding.safe_reference_source_hash(), DigestV1::ZERO);
        Self {
            reference_identity: *binding.safe_reference_identity().as_bytes(),
            reference_mir: *binding.safe_reference_mir_hash().as_bytes(),
            kernel_identity: *binding.kernel_subject_identity().as_bytes(),
            kernel_mir: *binding.kernel_mir_hash().as_bytes(),
            normalized_obligation: *binding.normalized_obligation_effect_ir_hash().as_bytes(),
        }
    }
    fn binding(&self) -> Result<Binding, ImportError> {
        Binding::new(
            SafeReferenceKindV2::Mir,
            DigestV1::from_untrusted_bytes(self.reference_identity),
            DigestV1::ZERO,
            DigestV1::from_untrusted_bytes(self.reference_mir),
            DigestV1::from_untrusted_bytes(self.kernel_identity),
            DigestV1::from_untrusted_bytes(self.kernel_mir),
            DigestV1::from_untrusted_bytes(self.normalized_obligation),
        )
    }
}

/// Inert public signature and exact original subjects, not admitted authority.
/// The enclosing child record independently joins this to a normal positive run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Transport {
    schema: String,
    binding: BindingRecord,
    wire: Vec<u8>,
    verifying_key: [u8; 32],
}
impl Transport {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.schema != TRANSPORT_SCHEMA {
            return Err("wrong inert receipt schema");
        }
        if self.wire.len() != FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 {
            return Err("wrong inert receipt length");
        }
        self.binding
            .binding()
            .map_err(|_| "invalid original binding")?;
        Ok(())
    }
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Observation {
    pub(crate) request_count: usize,
    pub(crate) normal_import_count: usize,
    pub(crate) binding: Option<BindingRecord>,
    pub(crate) original_receipt: Option<Transport>,
    pub(crate) stale_error: Option<&'static str>,
    pub(crate) rejected_import_count: usize,
    pub(crate) original_reimport_count: usize,
    pub(crate) normalized_obligation_changed: bool,
    pub(crate) kernel_mir_changed: bool,
}
enum Mode {
    Capture,
    Replay(Transport),
}
struct Active {
    mode: Mode,
    observation: Observation,
}
thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}
struct Restore(Option<Active>);
impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}
pub(crate) fn observe<R>(original: Option<Transport>, run: impl FnOnce() -> R) -> (R, Observation) {
    if let Some(receipt) = &original {
        receipt.validate().unwrap();
    }
    let mode = original.map_or(Mode::Capture, Mode::Replay);
    let restore = Restore(ACTIVE.with(|slot| {
        assert!(
            slot.borrow().is_none(),
            "source proof observer must not nest"
        );
        slot.replace(Some(Active {
            mode,
            observation: Observation::default(),
        }))
    }));
    let result = run();
    let observation = ACTIVE.with(|slot| slot.borrow_mut().take().unwrap().observation);
    drop(restore);
    (result, observation)
}

fn current_binding(request: &CompilerOwnedReferenceEffectRequestV2) -> Binding {
    let [site] = request.requests.as_slice() else {
        panic!("exactly one actual source effect request is required");
    };
    let operation = request
        .kernel
        .blocks()
        .get(site.block)
        .and_then(|block| block.operations().get(site.operation))
        .unwrap();
    let ProductionRankedOperationV1::RequestEffectRefinement { contract, subjects } = operation
    else {
        panic!("prepared site must select its real effect request");
    };
    assert_eq!(*subjects, site.subjects);
    assert_eq!(subjects.safe_reference_kind(), SafeReferenceKindV2::Mir);
    assert_eq!(subjects.safe_reference_source_hash(), DigestV1::ZERO);
    assert_eq!(
        request
            .kernel
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .filter(|operation| matches!(
                operation,
                ProductionRankedOperationV1::RequestEffectRefinement { .. }
            ))
            .count(),
        1
    );
    let digest = fe2o3_pliron::normalized_effect_refinement_hash_for_kernel_v2(
        &request.kernel,
        site.block,
        site.operation,
        contract,
        *subjects,
    )
    .unwrap();
    Binding::from_subjects(*subjects, digest).unwrap()
}

/// Runs after the existing fresh runtime lease opens, before its unchanged proof run.
pub(super) fn observe_request(
    request: &CompilerOwnedReferenceEffectRequestV2,
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
) {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(active) = slot.as_mut() else { return };
        active.observation.request_count += 1;
        assert_eq!(active.observation.request_count, 1);
        let current = current_binding(request);
        active.observation.binding = Some(BindingRecord::from_binding(current));
        let Mode::Replay(original) = &active.mode else {
            return;
        };
        let old = original.binding.binding().unwrap();
        // Feature-selected actual Rust variants also change portable crate metadata.
        // The real importer therefore rejects the reference identity FIRST. Do not
        // invent a hybrid old-subject/new-obligation expectation to bypass that check.
        assert_ne!(
            old.safe_reference_identity(),
            current.safe_reference_identity()
        );
        assert_ne!(
            old.kernel_subject_identity(),
            current.kernel_subject_identity()
        );
        assert_ne!(old.kernel_mir_hash(), current.kernel_mir_hash());
        assert_ne!(
            old.normalized_obligation_effect_ir_hash(),
            current.normalized_obligation_effect_ir_hash()
        );
        active.observation.normalized_obligation_changed = true;
        active.observation.kernel_mir_changed = true;
        let toolchain =
            fe2o3_verifier::functional_refinement_verus_toolchain_identity_v2(runtime).unwrap();
        let policy = Policy::new(
            original.verifying_key,
            toolchain,
            Boundary::SafeReferenceMirToKernelMir,
        )
        .unwrap();
        let mut importer = Importer::new(policy, 1).unwrap();
        assert_eq!(
            importer
                .import(Expectation::new(current), &original.wire)
                .unwrap_err(),
            ImportError::StaleSafeReferenceIdentity,
        );
        assert_eq!(
            importer.imported_count(),
            0,
            "refusal must not consume the single receipt slot"
        );
        active.observation.stale_error = Some("StaleSafeReferenceIdentity");
        active.observation.rejected_import_count = importer.imported_count();
        // Same untouched bytes/key, same freshly derived runtime toolchain, same
        // importer. This positive proves the stale failure was not malformed wire,
        // a bad signature, setup failure, or capacity exhaustion.
        let accepted = importer
            .import(Expectation::new(old), &original.wire)
            .unwrap();
        assert_eq!(accepted.binding(), old);
        assert!(accepted.signature_and_policy_verified());
        assert_eq!(importer.imported_count(), 1);
        active.observation.original_reimport_count = importer.imported_count();
        drop(accepted); // Never supplied to a compiler owner or continuation.
    });
}

pub(super) fn observe_import(
    binding: Binding,
    imported: &ImportedFunctionalRefinementProofV2,
    signed: &InertFunctionalRefinementReceiptSignatureV2,
) {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(active) = slot.as_mut() else { return };
        assert!(
            matches!(active.mode, Mode::Capture),
            "a semantically wrong source must not acquire a fresh normal receipt"
        );
        active.observation.normal_import_count += 1;
        assert_eq!(active.observation.normal_import_count, 1);
        assert_eq!(active.observation.request_count, 1);
        assert_eq!(imported.binding(), binding);
        assert_eq!(imported.boundary(), Boundary::SafeReferenceMirToKernelMir);
        assert!(imported.signature_and_policy_verified());
        let binding = BindingRecord::from_binding(binding);
        assert_eq!(active.observation.binding.as_ref(), Some(&binding));
        active.observation.original_receipt = Some(Transport {
            schema: TRANSPORT_SCHEMA.into(),
            binding,
            wire: signed.wire().to_vec(),
            verifying_key: *signed.verifying_key(),
        });
    });
}

#[test]
fn inert_transport_is_closed_and_validates_before_replay() {
    assert!(serde_json::from_str::<Transport>("{}").is_err());
    let binding = BindingRecord {
        reference_identity: [1; 32],
        reference_mir: [2; 32],
        kernel_identity: [3; 32],
        kernel_mir: [4; 32],
        normalized_obligation: [5; 32],
    };
    let mut transport = Transport {
        schema: TRANSPORT_SCHEMA.into(),
        binding,
        wire: vec![0; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
        verifying_key: [6; 32],
    };
    // Shape validation is expressly NOT signature or compiler-authority validation.
    assert_eq!(transport.validate(), Ok(()));
    let mut json = serde_json::to_value(&transport).unwrap();
    json["authority"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Transport>(json).is_err());
    transport.wire.pop();
    assert_eq!(transport.validate(), Err("wrong inert receipt length"));
    transport.wire.push(0);
    transport.binding.kernel_mir = [0; 32];
    assert_eq!(transport.validate(), Err("invalid original binding"));
    transport.schema = "other".into();
    assert_eq!(transport.validate(), Err("wrong inert receipt schema"));
}
