//! Private, non-authoritative V2 wire custody under the intact descriptor owner.
//! Only the boxed canonical payload escapes. Existing symbol/envelope/codec
//! engines and their temporary trees/Vec/String capacities remain excluded from
//! this selected logical account, as do allocator overhead, stack and RSS.
use super::*;
use crate::production_worker_handoff::ProductionWorkerHandoffError as HandoffError;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, DeviceTargetV1, MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
};
use std::mem::size_of;

struct PrivateHandoffPayloadV1 {
    canonical: Box<[u8]>,
    retained: usize,
}

/// No raw-parts constructor, Clone, extraction, publication or authority bridge.
#[allow(dead_code)]
pub(crate) struct PrivateBf16WorkerHandoffV1 {
    // Rust drops the new canonical bytes before every inherited owner/account.
    handoff: PrivateHandoffPayloadV1,
    descriptor: PrivateBf16DescriptorV1,
}
type HandoffResult = Result<PrivateHandoffPayloadV1, PrivateBf16LlvmErrorV1>;

const BYTE_MISMATCH: &str = "private BF16 handoff full canonical/module/manifest replay differs";
const DOMAIN_MISMATCH: &str = "private BF16 handoff requires exact gfx942 COV6 FFI-free text";
const PANICKED: &str = "private BF16 handoff construction panicked";

fn mismatch(reason: &'static str) -> PrivateBf16LlvmErrorV1 {
    private_bf16_descriptor_context_v1(reason)
}
fn handoff_error(error: HandoffError) -> PrivateBf16LlvmErrorV1 {
    PrivateBf16LlvmErrorV1::WorkerHandoff(error)
}
fn arithmetic() -> PrivateBf16LlvmErrorV1 {
    private_bf16_target_resource_v1(Resource::Arithmetic)
}
fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx942:xnack-").expect("closed private BF16 target")
}
fn wrapper_delta() -> Result<usize, PrivateBf16LlvmErrorV1> {
    size_of::<PrivateBf16WorkerHandoffV1>()
        .checked_sub(size_of::<PrivateBf16DescriptorV1>())
        .ok_or_else(arithmetic)
}
fn retained_bytes(length: usize) -> Result<usize, PrivateBf16LlvmErrorV1> {
    wrapper_delta()?.checked_add(length).ok_or_else(arithmetic)
}
fn reservation() -> Result<usize, PrivateBf16LlvmErrorV1> {
    retained_bytes(MAX_COMPILER_MODULE_HANDOFF_BYTES_V2)?
        .checked_add(size_of::<Option<HandoffResult>>())
        .ok_or_else(arithmetic)
}
fn reserve_output(budget: &mut Budget<'_>) -> Result<usize, PrivateBf16LlvmErrorV1> {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    budget
        .charge_work(3)
        .map_err(private_bf16_target_resource_v1)?;
    let amount = reservation()?;
    budget
        .reserve_storage(amount)
        .map_err(private_bf16_target_resource_v1)?;
    Ok(amount)
}

/// Returns only this closed inert payload, never a generic T or an authority
/// owner. Uncalled captures and caught payloads die before known credit refund.
fn capture<'work>(
    budget: &mut Budget<'work>,
    construct: impl FnOnce(&mut Budget<'work>) -> HandoffResult,
) -> HandoffResult {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let mut prepaid = 0usize;
    let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepaid = reserve_output(budget)?;
        let result = construct(budget);
        #[cfg(test)]
        if result.is_ok() {
            controls::after_capture();
        }
        result
    }));
    let result = match attempted {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(mismatch(PANICKED))
        }
    };
    finish(result, floor, ledger, slot, prepaid, budget)
}

fn finish(
    result: HandoffResult,
    floor: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    prepaid: usize,
    budget: &mut Budget<'_>,
) -> HandoffResult {
    let prior = budget.check_prior_denials_v1();
    if slot != budget as *const Budget<'_> as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage().checked_sub(floor) != Some(prepaid)
    {
        drop(result);
        prior.map_err(private_bf16_target_resource_v1)?;
        return Err(private_bf16_target_resource_v1(Resource::Accounting));
    }
    match (result, prior) {
        (Ok(payload), Ok(())) => {
            if payload.canonical.len() > MAX_COMPILER_MODULE_HANDOFF_BYTES_V2
                || retained_bytes(payload.canonical.len()).ok() != Some(payload.retained)
            {
                drop(payload);
                budget
                    .release_storage(prepaid)
                    .map_err(private_bf16_target_resource_v1)?;
                return Err(private_bf16_target_resource_v1(Resource::Accounting));
            }
            let Some(unused) = prepaid.checked_sub(payload.retained) else {
                drop(payload);
                budget
                    .release_storage(prepaid)
                    .map_err(private_bf16_target_resource_v1)?;
                return Err(private_bf16_target_resource_v1(Resource::Accounting));
            };
            if let Err(error) = budget.release_storage(unused) {
                drop(payload);
                return Err(private_bf16_target_resource_v1(error));
            }
            Ok(payload)
        }
        (result, prior) => {
            let error = match (result, prior) {
                (Ok(payload), Err(error)) => {
                    drop(payload);
                    private_bf16_target_resource_v1(error)
                }
                (Err(error), Ok(())) => error,
                (Err(_), Err(error)) => private_bf16_target_resource_v1(error),
                (Ok(_), Ok(())) => unreachable!("success handled above"),
            };
            budget
                .release_storage(prepaid)
                .map_err(private_bf16_target_resource_v1)?;
            Err(error)
        }
    }
}

fn check_domain(value: &CompilerModuleHandoffV2) -> Result<(), PrivateBf16LlvmErrorV1> {
    let directions = value.envelope().directional_symbols();
    if value.kind() != CompilerModuleKindV1::LlvmTextIr
        || value.target() != target()
        || value.code_object_version() != CodeObjectVersion::V6
        || directions.import_count() != 0
        || directions.export_count() != 0
        || value.module_bytes().len() > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
    {
        return Err(mismatch(DOMAIN_MISMATCH));
    }
    Ok(())
}

/// The decoded V2 is a private temporary engine result, not retained custody.
fn decode(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<CompilerModuleHandoffV2, PrivateBf16LlvmErrorV1> {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    if bytes.len() > MAX_COMPILER_MODULE_HANDOFF_BYTES_V2 {
        return Err(handoff_error(HandoffError::Handoff(
            fe2o3_compiler_ffi::CompilerModuleHandoffErrorV2::HandoffByteBoundExceeded,
        )));
    }
    let work = bytes
        .len()
        .checked_mul(8)
        .and_then(|n| n.checked_add(12))
        .ok_or_else(arithmetic)?;
    budget
        .charge_work(work)
        .map_err(private_bf16_target_resource_v1)?;
    let decoded = CompilerModuleHandoffV2::decode(bytes)
        .map_err(|error| handoff_error(HandoffError::Handoff(error)))?;
    check_domain(&decoded)?;
    Ok(decoded)
}

/// Called only inside the prepaid closed capture. This is a data encoder, not
/// an admission constructor. Source/Return/actual-O replay precedes its caller.
fn encode_prepaid(
    envelope: CompilerFfiEnvelopeV1,
    manifest: CompilerModuleSymbolManifestV1,
    final_llvm: &str,
    budget: &mut Budget<'_>,
) -> HandoffResult {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    if final_llvm.len() > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES {
        return Err(mismatch(DOMAIN_MISMATCH));
    }
    let work = final_llvm
        .len()
        .checked_add(envelope.canonical_bytes().len())
        .and_then(|n| n.checked_add(manifest.canonical_bytes().len()))
        .and_then(|n| n.checked_mul(8))
        .and_then(|n| n.checked_add(32))
        .ok_or_else(arithmetic)?;
    budget
        .charge_work(work)
        .map_err(private_bf16_target_resource_v1)?;
    let encoded = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target(),
        CodeObjectVersion::V6,
        envelope,
        manifest,
        final_llvm.as_bytes(),
    )
    .map_err(|error| handoff_error(HandoffError::Handoff(error)))?;
    check_domain(&encoded)?;
    let decoded = decode(encoded.canonical_bytes(), budget)?;
    // Full independent codec roundtrip and all public role/module projections,
    // not only a digest match.
    if decoded.canonical_bytes() != encoded.canonical_bytes()
        || decoded.module_bytes() != final_llvm.as_bytes()
        || decoded.symbol_manifest().canonical_bytes()
            != encoded.symbol_manifest().canonical_bytes()
        || !decoded
            .symbol_manifest()
            .entries()
            .eq(encoded.symbol_manifest().entries())
        || decoded.envelope().canonical_bytes() != encoded.envelope().canonical_bytes()
    {
        return Err(mismatch(BYTE_MISMATCH));
    }
    let length = encoded.canonical_bytes().len();
    budget
        .charge_work(length)
        .map_err(private_bf16_target_resource_v1)?;
    let canonical = encoded.canonical_bytes().to_vec().into_boxed_slice();
    let retained = retained_bytes(canonical.len())?;
    drop(decoded);
    drop(encoded);
    #[cfg(test)]
    controls::made();
    Ok(PrivateHandoffPayloadV1 {
        canonical,
        retained,
    })
}

fn derive(
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    worker: &str,
    final_llvm: &str,
    budget: &mut Budget<'_>,
) -> HandoffResult {
    #[cfg(test)]
    controls::entered();
    capture(budget, |budget| {
        let work = worker
            .len()
            .checked_add(output.canonical().canonical_bytes().len())
            .and_then(|n| n.checked_add(8))
            .ok_or_else(arithmetic)?;
        budget
            .charge_work(work)
            .map_err(private_bf16_target_resource_v1)?;
        // Reuse actual O's bounded symbol closure. Keep the already-bound final
        // text separate: no descriptor binder or ordinary assembler is invoked.
        let compiler_module =
            crate::kernel_ir_codegen::retain_private_bf16_checked_compiler_module_text_v1(
                output,
                worker.to_owned(),
            )
            .map_err(|error| handoff_error(HandoffError::CompilerModule(error)))?;
        let envelope = crate::production_worker_handoff::derive_production_compiler_ffi_envelope(
            target(),
            output.module(),
            &compiler_module,
            None,
            *output.canonical().identity().digest(),
        )
        .map_err(handoff_error)?;
        crate::compiler_module_contract::validate_exact_target_binding(target(), output.module())
            .map_err(|error| handoff_error(error.into()))?;
        crate::compiler_module_contract::validate_envelope_module_roles(
            &envelope,
            &compiler_module,
        )
        .map_err(|error| handoff_error(error.into()))?;
        let manifest = crate::compiler_module_contract::construct_symbol_manifest(&compiler_module)
            .map_err(|error| handoff_error(HandoffError::SymbolManifest(error)))?;
        encode_prepaid(envelope, manifest, final_llvm, budget)
    })
}

fn compare(
    retained: &PrivateHandoffPayloadV1,
    fresh: &PrivateHandoffPayloadV1,
    final_llvm: &str,
    budget: &mut Budget<'_>,
) -> Result<(), PrivateBf16LlvmErrorV1> {
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    let left = decode(&retained.canonical, budget)?;
    let right = decode(&fresh.canonical, budget)?;
    let work = retained
        .canonical
        .len()
        .checked_add(fresh.canonical.len())
        .and_then(|n| n.checked_add(final_llvm.len()))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_add(8))
        .ok_or_else(arithmetic)?;
    budget
        .charge_work(work)
        .map_err(private_bf16_target_resource_v1)?;
    if retained.retained != retained_bytes(retained.canonical.len())?
        || fresh.retained != retained_bytes(fresh.canonical.len())?
        || retained.retained != fresh.retained
        || retained.canonical != fresh.canonical
        || left.module_bytes() != final_llvm.as_bytes()
        || right.module_bytes() != final_llvm.as_bytes()
        || left.envelope().canonical_bytes() != right.envelope().canonical_bytes()
        || left.symbol_manifest().canonical_bytes() != right.symbol_manifest().canonical_bytes()
        || !left
            .symbol_manifest()
            .entries()
            .eq(right.symbol_manifest().entries())
    {
        return Err(mismatch(BYTE_MISMATCH));
    }
    Ok(())
}

fn replay_prepared(
    retained: &PrivateHandoffPayloadV1,
    fresh: PrivateHandoffPayloadV1,
    final_llvm: &str,
    floor: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    budget: &mut Budget<'_>,
) -> Result<(), PrivateBf16LlvmErrorV1> {
    let compared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        #[cfg(test)]
        controls::before_compare();
        compare(retained, &fresh, final_llvm, budget)
    }));
    let joined = match compared {
        Ok(joined) => joined,
        Err(payload) => {
            drop(payload);
            Err(mismatch(PANICKED))
        }
    };
    let storage = fresh.retained;
    drop(fresh);
    if budget.storage().checked_sub(floor) != Some(storage)
        || budget.work_ledger_identity_v1() != ledger
        || budget as *const Budget<'_> as usize != slot
    {
        budget
            .check_prior_denials_v1()
            .map_err(private_bf16_target_resource_v1)?;
        return Err(private_bf16_target_resource_v1(Resource::Accounting));
    }
    budget
        .release_storage(storage)
        .map_err(private_bf16_target_resource_v1)?;
    budget
        .check_prior_denials_v1()
        .map_err(private_bf16_target_resource_v1)?;
    if budget.storage() != floor {
        return Err(private_bf16_target_resource_v1(Resource::Accounting));
    }
    joined
}

impl PrivateBf16DescriptorV1 {
    fn prepare_private_bf16_handoff_payload_v1(
        &mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> HandoffResult {
        // Preserve the exact typed source/Return/guard refusal before any wire
        // construction. Nothing here converts raw Incomplete into Complete.
        self.revalidate_private_bf16_descriptor_v1(requested_return, typed_roots, profile)?;
        let output = self.llvm.optimized.optimization.checked.owner();
        let worker = &self.llvm.llvm.worker;
        let final_llvm = &self.descriptor.final_llvm;
        let result = self
            .llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .with_budget(|budget| Ok(derive(output, worker, final_llvm, budget)));
        self.llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
        let handoff = result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)??;
        Ok(handoff)
    }
    #[allow(dead_code)]
    pub(crate) fn prepare_private_bf16_worker_handoff_v1(
        mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<PrivateBf16WorkerHandoffV1, PrivateBf16LlvmErrorV1> {
        let handoff =
            self.prepare_private_bf16_handoff_payload_v1(requested_return, typed_roots, profile)?;
        Ok(PrivateBf16WorkerHandoffV1 {
            handoff,
            descriptor: self,
        })
    }
}
impl PrivateBf16WorkerHandoffV1 {
    #[allow(dead_code)]
    pub(crate) fn revalidate_private_bf16_worker_handoff_v1(
        &mut self,
        requested_return: [u8; 4],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        self.descriptor.revalidate_private_bf16_descriptor_v1(
            requested_return,
            typed_roots,
            profile,
        )?;
        let output = self.descriptor.llvm.optimized.optimization.checked.owner();
        let worker = &self.descriptor.llvm.llvm.worker;
        let final_llvm = &self.descriptor.descriptor.final_llvm;
        let retained = &self.handoff;
        let result = self
            .descriptor
            .llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .with_budget(|budget| {
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let slot = budget as *const Budget<'_> as usize;
                let fresh = match derive(output, worker, final_llvm, budget) {
                    Ok(value) => value,
                    Err(error) => return Ok(Err(error)),
                };
                Ok(replay_prepared(
                    retained, fresh, final_llvm, floor, ledger, slot, budget,
                ))
            });
        self.descriptor
            .llvm
            .optimized
            .target
            .formal
            .verification
            .phase
            .require_clean_v1()
            .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
        result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)?
    }

    #[allow(dead_code)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod controls {
    use super::*;
    use fe2o3_compiler_ffi::{
        CompilerModuleHandoffErrorV1, CompilerModuleHandoffErrorV2,
        CompilerModuleSymbolRoleV1 as Role,
    };
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::cell::Cell;

    // These are inert data/account controls, not source/LLVM/descriptor admission.
    const FINAL: &str = "; independent fixed final text\ndefine amdgpu_kernel void @entry() { ret void }\nmodule asm \"descriptor-once\"\n";
    const OTHER: &str = "; independent fixed final text\ndefine amdgpu_kernel void @entry() { ret void }\nmodule asm \"descriptor-twice\"\n";

    #[derive(Clone, Copy, Default)]
    struct State {
        entered: usize,
        made: usize,
        dropped: usize,
        captures: usize,
        panic_after_capture: bool,
        panic_on_compare: bool,
    }
    thread_local! { static STATE: Cell<Option<State>> = const { Cell::new(None) }; }
    struct Scope(Option<State>);
    impl Scope {
        fn enter(panic_after_capture: bool) -> Self {
            Self(STATE.replace(Some(State {
                panic_after_capture,
                ..State::default()
            })))
        }
        fn snapshot(&self) -> State {
            STATE.with(|s| s.get().unwrap())
        }
    }
    impl Drop for Scope {
        fn drop(&mut self) {
            STATE.set(self.0);
        }
    }
    pub(super) fn entered() {
        STATE.with(|s| {
            if let Some(mut v) = s.get() {
                v.entered += 1;
                s.set(Some(v));
            }
        });
    }
    pub(super) fn made() {
        STATE.with(|s| {
            if let Some(mut v) = s.get() {
                v.made += 1;
                s.set(Some(v));
            }
        });
    }
    pub(super) fn after_capture() {
        let unwind = STATE.with(|s| {
            let Some(mut v) = s.get() else {
                return false;
            };
            let unwind = v.panic_after_capture;
            v.panic_after_capture = false;
            s.set(Some(v));
            unwind
        });
        if unwind {
            // Harness-only payload; do not call rustc's global panic hook.
            std::panic::resume_unwind(Box::new("private handoff capture control"));
        }
    }
    pub(super) fn before_compare() {
        let unwind = STATE.with(|s| {
            let Some(mut v) = s.get() else {
                return false;
            };
            let unwind = v.panic_on_compare;
            v.panic_on_compare = false;
            s.set(Some(v));
            unwind
        });
        if unwind {
            std::panic::resume_unwind(Box::new("private handoff replay control"));
        }
    }
    impl Drop for PrivateHandoffPayloadV1 {
        fn drop(&mut self) {
            STATE.with(|s| {
                if let Some(mut v) = s.get() {
                    v.dropped += 1;
                    s.set(Some(v));
                }
            });
        }
    }
    struct CaptureDrop;
    impl Drop for CaptureDrop {
        fn drop(&mut self) {
            STATE.with(|s| {
                if let Some(mut v) = s.get() {
                    v.captures += 1;
                    s.set(Some(v));
                }
            });
        }
    }
    fn envelope(t: DeviceTargetV1, cov: CodeObjectVersion) -> CompilerFfiEnvelopeV1 {
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(t, cov).unwrap()
    }
    fn manifest(helper: &str) -> CompilerModuleSymbolManifestV1 {
        CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "entry"),
            (Role::KernelDescriptor, "entry.kd"),
            (Role::InternalHelper, helper),
        ])
        .unwrap()
    }
    // Independent public-codec oracle: this does not call derive/encode_prepaid.
    fn oracle(module: &str, helper: &str) -> PrivateHandoffPayloadV1 {
        let wire = CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            target(),
            CodeObjectVersion::V6,
            envelope(target(), CodeObjectVersion::V6),
            manifest(helper),
            module.as_bytes(),
        )
        .unwrap();
        let canonical = wire.canonical_bytes().to_vec().into_boxed_slice();
        let retained = retained_bytes(canonical.len()).unwrap();
        made();
        PrivateHandoffPayloadV1 {
            canonical,
            retained,
        }
    }
    fn inert() -> PrivateHandoffPayloadV1 {
        made();
        PrivateHandoffPayloadV1 {
            canonical: vec![1, 2, 3].into_boxed_slice(),
            retained: retained_bytes(3).unwrap(),
        }
    }
    fn accounting(result: &HandoffResult) -> bool {
        matches!(
            result,
            Err(PrivateBf16LlvmErrorV1::RankedVerification(
                E::ConditionalResource(Resource::Accounting)
            ))
        )
    }
    fn mismatch_is(result: &Result<(), PrivateBf16LlvmErrorV1>, reason: &'static str) -> bool {
        matches!(result, Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
            crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(actual)
        )) if *actual == reason)
    }

    #[test]
    fn boxed_handoff_matches_independent_codec_and_exact_final_module() {
        let scope = Scope::enter(false);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, reservation().unwrap() + 7);
        budget.reserve_storage(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let payload = capture(&mut budget, |b| {
            encode_prepaid(
                envelope(target(), CodeObjectVersion::V6),
                manifest("helper"),
                FINAL,
                b,
            )
        })
        .unwrap();
        let expected = oracle(FINAL, "helper");
        assert_eq!(payload.canonical, expected.canonical);
        assert_eq!(
            payload.retained,
            wrapper_delta().unwrap() + payload.canonical.len()
        );
        let decoded = CompilerModuleHandoffV2::decode(&payload.canonical).unwrap();
        assert_eq!(decoded.module_bytes(), FINAL.as_bytes());
        assert_ne!(decoded.module_bytes(), OTHER.as_bytes());
        assert_eq!(
            decoded.symbol_manifest().entries().collect::<Vec<_>>(),
            vec![
                (Role::KernelEntry, "entry"),
                (Role::KernelDescriptor, "entry.kd"),
                (Role::InternalHelper, "helper"),
            ]
        );
        assert!(!decoded.grants_worker_authority());
        assert!(!decoded.grants_launch_authority());
        drop(decoded);
        drop(expected); // Inert oracle is harness-owned, not the retained payload.
        assert_eq!(scope.snapshot().dropped, 1);
        assert_eq!(budget.storage(), 7 + payload.retained);
        let retained = payload.retained;
        drop(payload);
        assert_eq!(scope.snapshot().dropped, 2);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 7);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }

    #[test]
    fn valid_rehashed_foreign_module_and_manifest_fail_full_owner_comparison() {
        let expected = oracle(FINAL, "helper");
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        for foreign in [oracle(OTHER, "helper"), oracle(FINAL, "xelper")] {
            // The counterexample is valid and self-consistent, not a stale hash.
            assert!(CompilerModuleHandoffV2::decode(&foreign.canonical).is_ok());
            assert!(mismatch_is(
                &compare(&foreign, &expected, FINAL, &mut budget),
                BYTE_MISMATCH
            ));
        }
        assert!(compare(&expected, &expected, FINAL, &mut budget).is_ok());
        assert!(mismatch_is(
            &compare(&expected, &expected, OTHER, &mut budget),
            BYTE_MISMATCH
        ));
    }

    #[test]
    fn actual_box_mutation_is_restored_before_error_inspection_and_clean_replay() {
        let mut retained = oracle(FINAL, "helper");
        let expected = oracle(FINAL, "helper");
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let last = retained.canonical.len() - 1;
        retained.canonical[last] ^= 1;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compare(&retained, &expected, FINAL, &mut budget)
        }));
        retained.canonical[last] ^= 1;
        assert!(matches!(
            result,
            Ok(Err(PrivateBf16LlvmErrorV1::WorkerHandoff(
                HandoffError::Handoff(CompilerModuleHandoffErrorV2::Handoff(
                    CompilerModuleHandoffErrorV1::ModuleIdentityMismatch
                ))
            )))
        ));
        compare(&retained, &expected, FINAL, &mut budget).unwrap();
    }

    #[test]
    fn strict_decode_rejects_each_truncation_trailing_byte_and_wrong_domain() {
        let wire = oracle(FINAL, "helper");
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        for length in 0..wire.canonical.len() {
            assert!(matches!(
                decode(&wire.canonical[..length], &mut budget),
                Err(PrivateBf16LlvmErrorV1::WorkerHandoff(
                    HandoffError::Handoff(_)
                ))
            ));
        }
        let mut trailing = wire.canonical.to_vec();
        trailing.push(0);
        assert!(matches!(
            decode(&trailing, &mut budget),
            Err(PrivateBf16LlvmErrorV1::WorkerHandoff(
                HandoffError::Handoff(CompilerModuleHandoffErrorV2::Handoff(
                    CompilerModuleHandoffErrorV1::TrailingBytes
                ))
            ))
        ));
        for (t, cov) in [
            (
                DeviceTargetV1::parse("gfx950:xnack-").unwrap(),
                CodeObjectVersion::V6,
            ),
            (target(), CodeObjectVersion::V5),
        ] {
            let foreign = CompilerModuleHandoffV2::new(
                CompilerModuleKindV1::LlvmTextIr,
                t,
                cov,
                envelope(t, cov),
                manifest("helper"),
                FINAL.as_bytes(),
            )
            .unwrap();
            let result = decode(foreign.canonical_bytes(), &mut budget).map(|_| ());
            assert!(mismatch_is(&result, DOMAIN_MISMATCH));
        }
    }

    #[test]
    fn boxed_handoff_comparison_detects_retention_lies() {
        let mut retained = oracle(FINAL, "helper");
        let fresh = oracle(FINAL, "helper");
        retained.retained += 1;
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        assert!(mismatch_is(
            &compare(&retained, &fresh, FINAL, &mut budget),
            BYTE_MISMATCH
        ));
    }

    #[test]
    fn capture_prepayment_exact_and_one_short_drops_unused_capture() {
        let cap = reservation().unwrap();
        for available in [cap, cap - 1] {
            let scope = Scope::enter(false);
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 7 + available);
            budget.reserve_storage(7).unwrap();
            let captured = CaptureDrop;
            let result = capture(&mut budget, move |_| {
                drop(captured);
                Ok(inert())
            });
            assert_eq!(scope.snapshot().captures, 1);
            if available == cap {
                let payload = result.unwrap();
                let retained = payload.retained;
                assert_eq!(budget.storage(), 7 + retained);
                assert_eq!(scope.snapshot().dropped, 0);
                drop(payload);
                budget.release_storage(retained).unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(PrivateBf16LlvmErrorV1::RankedVerification(
                        E::ConditionalResource(Resource::Storage(_))
                    ))
                ));
                assert_eq!(scope.snapshot().made, 0);
                assert!(budget.failed_storage().is_some());
            }
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn capture_reservation_exact_work_and_one_short_keep_entry_storage() {
        for quota in [3, 2] {
            let scope = Scope::enter(false);
            let mut work = Work::new(quota);
            let mut budget = Budget::new(&mut work, reservation().unwrap() + 7);
            budget.reserve_storage(7).unwrap();
            let captured = CaptureDrop;
            let result = capture(&mut budget, move |_| {
                drop(captured);
                Ok(inert())
            });
            assert_eq!(scope.snapshot().captures, 1);
            if quota == 3 {
                let payload = result.unwrap();
                let storage = payload.retained;
                drop(payload);
                budget.release_storage(storage).unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(PrivateBf16LlvmErrorV1::RankedVerification(
                        E::ConditionalResource(Resource::Work(_))
                    ))
                ));
                assert_eq!(scope.snapshot().made, 0);
                assert!(budget.failed_work().is_some());
            }
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn capture_typed_refusal_and_unwind_restore_only_known_credit() {
        for unwind in [false, true] {
            let scope = Scope::enter(unwind);
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, reservation().unwrap() + 7);
            budget.reserve_storage(7).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = capture(&mut budget, |_| {
                if unwind {
                    Ok(inert())
                } else {
                    Err(mismatch("typed control refusal"))
                }
            })
            .map(|payload| drop(payload));
            assert!(mismatch_is(
                &result,
                if unwind {
                    PANICKED
                } else {
                    "typed control refusal"
                }
            ));
            assert_eq!(scope.snapshot().dropped, usize::from(unwind));
            assert_eq!(budget.storage(), 7);
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
    }

    #[test]
    fn capture_prior_and_late_denials_do_not_become_clean_success() {
        for late in [false, true] {
            for mode in 0..3 {
                let scope = Scope::enter(false);
                let mut work = Work::new(16);
                let mut budget = Budget::new(&mut work, reservation().unwrap() + 7);
                budget.reserve_storage(7).unwrap();
                if !late {
                    if mode != 1 {
                        let _ = budget.charge_work(100);
                    }
                    if mode != 0 {
                        let _ = budget.reserve_storage(reservation().unwrap() + 1);
                    }
                }
                let captured = CaptureDrop;
                let result = capture(&mut budget, move |b| {
                    drop(captured);
                    if mode != 1 {
                        let _ = b.charge_work(100);
                    }
                    if mode != 0 {
                        let _ = b.reserve_storage(reservation().unwrap() + 1);
                    }
                    Ok(inert())
                });
                let prior = budget.check_prior_denials_v1().unwrap_err();
                assert!(
                    matches!(result, Err(PrivateBf16LlvmErrorV1::RankedVerification(
                    E::ConditionalResource(error)
                )) if error == prior)
                );
                assert_eq!(scope.snapshot().captures, 1);
                assert_eq!(scope.snapshot().made, usize::from(late));
                assert_eq!(scope.snapshot().dropped, usize::from(late));
                assert_eq!(budget.storage(), 7);
                if mode != 1 {
                    assert!(budget.failed_work().is_some());
                }
                if mode == 1 {
                    assert!(budget.failed_storage().is_some());
                }
                assert!(budget.check_prior_denials_v1().is_err());
            }
        }
    }

    #[test]
    fn foreign_ledger_slot_or_surplus_never_receives_a_guessed_refund() {
        for mode in 0..3 {
            let scope = Scope::enter(false);
            let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, reservation().unwrap() + 8);
            budget.reserve_storage(7).unwrap();
            let mut other_work = Work::new(100);
            let other = Budget::new(&mut other_work, 100);
            let ledger = if mode == 0 {
                other.work_ledger_identity_v1()
            } else {
                budget.work_ledger_identity_v1()
            };
            let slot = if mode == 1 {
                1
            } else {
                &budget as *const Budget<'_> as usize
            };
            let prepaid = reserve_output(&mut budget).unwrap();
            if mode == 2 {
                budget.reserve_storage(1).unwrap();
            }
            let floor = budget.storage();
            let result = finish(Ok(inert()), 7, ledger, slot, prepaid, &mut budget);
            assert!(accounting(&result));
            assert_eq!(scope.snapshot().dropped, 1);
            assert_eq!(budget.storage(), floor);
        }
    }

    #[test]
    fn overstated_payload_retention_drops_before_known_credit_release() {
        let scope = Scope::enter(false);
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, reservation().unwrap() + 7);
        budget.reserve_storage(7).unwrap();
        let result = capture(&mut budget, |_| {
            let mut payload = inert();
            payload.retained += 1;
            Ok(payload)
        });
        assert!(accounting(&result));
        assert_eq!(scope.snapshot().dropped, 1);
        assert_eq!(budget.storage(), 7);
    }

    #[test]
    fn comparison_exact_work_and_one_short_preserve_original_floor() {
        let retained = oracle(FINAL, "helper");
        let fresh = oracle(FINAL, "helper");
        let mut measured = 0usize;
        for case in 0..3 {
            let mut work = Work::new(if case == 0 {
                usize::MAX
            } else {
                measured - usize::from(case == 2)
            });
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget
                .reserve_storage(7 + retained.retained + fresh.retained)
                .unwrap();
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let result = compare(&retained, &fresh, FINAL, &mut budget);
            if case == 0 {
                measured = budget.work();
            }
            if case < 2 {
                result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(PrivateBf16LlvmErrorV1::RankedVerification(
                        E::ConditionalResource(Resource::Work(_))
                    ))
                ));
                assert!(budget.failed_work().is_some());
            }
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
    }

    #[test]
    fn replay_drops_fresh_before_refund_on_success_mismatch_or_unwind() {
        for mode in 0..3 {
            let scope = Scope::enter(false);
            let retained = oracle(FINAL, "helper");
            let fresh = oracle(if mode == 1 { OTHER } else { FINAL }, "helper");
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let floor = 7 + retained.retained;
            budget.reserve_storage(floor + fresh.retained).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let slot = &budget as *const Budget<'_> as usize;
            if mode == 2 {
                STATE.with(|s| {
                    let mut v = s.get().unwrap();
                    v.panic_on_compare = true;
                    s.set(Some(v));
                });
            }
            let result = replay_prepared(&retained, fresh, FINAL, floor, ledger, slot, &mut budget);
            match mode {
                0 => result.unwrap(),
                1 => assert!(mismatch_is(&result, BYTE_MISMATCH)),
                2 => assert!(mismatch_is(&result, PANICKED)),
                _ => unreachable!(),
            }
            assert_eq!(scope.snapshot().dropped, 1);
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            let charge = retained.retained;
            drop(retained);
            assert_eq!(scope.snapshot().dropped, 2);
            budget.release_storage(charge).unwrap();
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn source_continuation_replays_typed_owner_before_wire_and_has_no_binder() {
        // Lexical/source tripwire only; genuine rustc owner coverage is separate.
        let source = include_str!("bf16_private_worker_handoff_v1.rs");
        let production = source.split("#[cfg(test)]\nmod controls").next().unwrap();
        let constructor = production
            .split("impl PrivateBf16DescriptorV1 {")
            .nth(1)
            .unwrap()
            .split("impl PrivateBf16WorkerHandoffV1 {")
            .next()
            .unwrap();
        assert!(
            constructor
                .find("self.revalidate_private_bf16_descriptor_v1(")
                .unwrap()
                < constructor.find("Ok(derive(").unwrap()
        );
        let compact: String = constructor.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.contains(
            "self.revalidate_private_bf16_descriptor_v1(requested_return,typed_roots,profile)?"
        ));
        assert!(!production.contains("bind_compiler_descriptor_source_v1("));
        assert!(!production.contains("assemble_production_worker_handoff("));
        assert!(!production.contains("observe_engineering_hsaco_v1("));
        assert!(!production.contains("pub(crate) fn into_"));
        assert!(!production.contains("pub(crate) fn canonical_bytes"));
        assert!(
            production
                .find("handoff: PrivateHandoffPayloadV1,")
                .unwrap()
                < production
                    .find("descriptor: PrivateBf16DescriptorV1,")
                    .unwrap()
        );
        assert!(production.contains("drop(fresh);"));
    }
    // Genuine-owner controls use the exact private constructor/replay paths.
    // Test scopes/counters, public-codec fixtures and panic payloads are harness
    // state, not a claim of whole-engine heap or RSS accounting.
    fn exact_refusal(
        result: Result<(), PrivateBf16LlvmErrorV1>,
        kind: u8,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        let accepted = match (&result, kind) {
            (Err(PrivateBf16LlvmErrorV1::RankedVerification(E::FormalMemory(
                fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::SemanticKir(
                    fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch)))), 0) => true,
            (Err(PrivateBf16LlvmErrorV1::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure)), 1 | 2) => true,
            (Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
                crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(
                    "private BF16 descriptor/final LLVM bytes differ from owned replay"))), 3 | 4) => true,
            (Err(PrivateBf16LlvmErrorV1::WorkerHandoff(HandoffError::Handoff(
                CompilerModuleHandoffErrorV2::Handoff(CompilerModuleHandoffErrorV1::ModuleIdentityMismatch)))), 5) => true,
            (Err(PrivateBf16LlvmErrorV1::DescriptorEvidence(
                crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(reason))), 6 | 7) if *reason == BYTE_MISMATCH => true,
            _ => false,
        };
        if accepted {
            Ok(())
        } else {
            match result {
                Err(e) => Err(e),
                Ok(()) => Err(mismatch(
                    "genuine handoff negative control unexpectedly succeeded",
                )),
            }
        }
    }
    fn caught(
        result: std::thread::Result<Result<(), PrivateBf16LlvmErrorV1>>,
    ) -> Result<(), PrivateBf16LlvmErrorV1> {
        match result {
            Ok(value) => value,
            Err(payload) => {
                drop(payload);
                Err(mismatch(
                    "genuine handoff control unwound after restoration",
                ))
            }
        }
    }
    impl PrivateBf16DescriptorV1 {
        fn handoff_checkpoint_for_test(
            &mut self,
        ) -> Result<
            (
                fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
                usize,
                usize,
                usize,
            ),
            PrivateBf16LlvmErrorV1,
        > {
            self.llvm
                .optimized
                .target
                .formal
                .verification
                .phase
                .with_budget(|b| {
                    b.check_prior_denials_v1().map_err(resource)?;
                    Ok((
                        b.work_ledger_identity_v1(),
                        b.storage(),
                        b.work(),
                        b.peak_storage(),
                    ))
                })
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)
        }
        fn require_handoff_checkpoint_for_test(
            &mut self,
            before: (
                fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
                usize,
                usize,
                usize,
            ),
        ) -> Result<(), PrivateBf16LlvmErrorV1> {
            let after = self.handoff_checkpoint_for_test()?;
            if before.0 != after.0
                || before.1 != after.1
                || before.2 >= after.2
                || before.3 > after.3
            {
                return Err(private_bf16_target_resource_v1(Resource::Accounting));
            }
            Ok(())
        }
        pub(crate) fn exercise_private_bf16_handoff_constructor_for_test_v1(
            &mut self,
            requested: [u8; 4],
            typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
            profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        ) -> Result<(), PrivateBf16LlvmErrorV1> {
            self.revalidate_private_bf16_descriptor_v1(requested, typed_roots, profile)?;
            let wrong = if requested == [0, 1, 2, 3] {
                [1, 0, 2, 3]
            } else {
                [0, 1, 2, 3]
            };
            for fault in 0..5 {
                let before = self.handoff_checkpoint_for_test()?;
                let scope = Scope::enter(false);
                let descriptor_last = self
                    .descriptor
                    .canonical_descriptor
                    .len()
                    .checked_sub(1)
                    .ok_or_else(|| mismatch("empty genuine descriptor"))?;
                let text_at = self
                    .descriptor
                    .final_llvm
                    .find("gfx942")
                    .ok_or_else(|| mismatch("genuine final target absent"))?;
                if fault == 3 {
                    self.descriptor.canonical_descriptor[descriptor_last] ^= 1;
                }
                if fault == 4 {
                    self.descriptor
                        .final_llvm
                        .get_mut(text_at..text_at + 6)
                        .unwrap()
                        .make_ascii_uppercase();
                }
                let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.prepare_private_bf16_handoff_payload_v1(
                        if fault == 0 { wrong } else { requested },
                        if fault == 2 { &[] } else { typed_roots },
                        if fault == 1 {
                            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950
                        } else {
                            profile
                        },
                    )
                    .map(drop)
                }));
                // Restore the same allocations before inspecting either Result or panic.
                if fault == 3 {
                    self.descriptor.canonical_descriptor[descriptor_last] ^= 1;
                }
                if fault == 4 {
                    self.descriptor
                        .final_llvm
                        .get_mut(text_at..text_at + 6)
                        .unwrap()
                        .make_ascii_lowercase();
                }
                exact_refusal(caught(attempted), fault)?;
                if scope.snapshot().entered != 0
                    || scope.snapshot().made != 0
                    || scope.snapshot().dropped != 0
                {
                    return Err(mismatch("refused source entered handoff builder"));
                }
                self.require_handoff_checkpoint_for_test(before)?;
                drop(scope);
                self.revalidate_private_bf16_descriptor_v1(requested, typed_roots, profile)?;
            }
            Ok(())
        }
    }
    impl PrivateBf16WorkerHandoffV1 {
        pub(crate) fn exercise_private_bf16_handoff_for_test_v1(
            &mut self,
            requested: [u8; 4],
            typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
            profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        ) -> Result<(), PrivateBf16LlvmErrorV1> {
            self.revalidate_private_bf16_worker_handoff_v1(requested, typed_roots, profile)?;
            let before = self.descriptor.handoff_checkpoint_for_test()?;
            let last = self
                .handoff
                .canonical
                .len()
                .checked_sub(1)
                .ok_or_else(|| mismatch("empty genuine handoff"))?;
            self.handoff.canonical[last] ^= 1;
            let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.revalidate_private_bf16_worker_handoff_v1(requested, typed_roots, profile)
            }));
            self.handoff.canonical[last] ^= 1;
            exact_refusal(caught(attempted), 5)?;
            self.descriptor
                .require_handoff_checkpoint_for_test(before)?;
            self.revalidate_private_bf16_worker_handoff_v1(requested, typed_roots, profile)?;
            let retained = &self.handoff;
            let final_llvm = &self.descriptor.descriptor.final_llvm;
            let result = self
                .descriptor
                .llvm
                .optimized
                .target
                .formal
                .verification
                .phase
                .with_budget(|budget| {
                    Ok((|| -> Result<(), PrivateBf16LlvmErrorV1> {
                        // Build valid foreign packets from genuine metadata, not stale digests.
                        // Only the boxed foreign copy is newly selected retained custody;
                        // public-codec/Vec/String fixtures are explicitly harness temporaries.
                        for mode in 0..2 {
                            let floor = budget.storage();
                            let ledger = budget.work_ledger_identity_v1();
                            let slot = budget as *const Budget<'_> as usize;
                            let fresh = capture(budget, |budget| {
                                let actual = decode(&retained.canonical, budget)?;
                                budget
                                    .charge_work(
                                        retained
                                            .canonical
                                            .len()
                                            .checked_mul(8)
                                            .ok_or_else(arithmetic)?,
                                    )
                                    .map_err(private_bf16_target_resource_v1)?;
                                let mut module = actual.module_bytes().to_vec();
                                let mut roles: Vec<_> = actual
                                    .symbol_manifest()
                                    .entries()
                                    .map(|(r, s)| (r, s.to_owned()))
                                    .collect();
                                if mode == 0 {
                                    let at = module
                                        .iter()
                                        .rposition(|b| *b == b'\n')
                                        .ok_or_else(|| mismatch("genuine module newline absent"))?;
                                    module[at] = b' ';
                                } else {
                                    let (_, name) = roles
                                        .iter_mut()
                                        .find(|(r, _)| *r == Role::InternalHelper)
                                        .ok_or_else(|| {
                                            mismatch("genuine helper manifest role absent")
                                        })?;
                                    let at = name
                                        .len()
                                        .checked_sub(1)
                                        .ok_or_else(|| mismatch("empty genuine helper symbol"))?;
                                    let replacement = if &name[at..] == "x" { "y" } else { "x" };
                                    name.replace_range(at.., replacement);
                                }
                                let manifest = CompilerModuleSymbolManifestV1::new(roles)
                                    .map_err(|e| handoff_error(HandoffError::SymbolManifest(e)))?;
                                let encoded = CompilerModuleHandoffV2::new(
                                    actual.kind(),
                                    actual.target(),
                                    actual.code_object_version(),
                                    actual.envelope().clone(),
                                    manifest,
                                    &module,
                                )
                                .map_err(|e| handoff_error(HandoffError::Handoff(e)))?;
                                let decoded = decode(encoded.canonical_bytes(), budget)?;
                                if decoded.canonical_bytes() == retained.canonical.as_ref() {
                                    return Err(mismatch("foreign genuine control unchanged"));
                                }
                                budget
                                    .charge_work(encoded.canonical_bytes().len())
                                    .map_err(private_bf16_target_resource_v1)?;
                                let canonical =
                                    encoded.canonical_bytes().to_vec().into_boxed_slice();
                                let retained = retained_bytes(canonical.len())?;
                                drop(decoded);
                                drop(encoded);
                                drop(actual);
                                drop(module);
                                made();
                                Ok(PrivateHandoffPayloadV1 {
                                    canonical,
                                    retained,
                                })
                            })?;
                            exact_refusal(
                                replay_prepared(
                                    retained, fresh, final_llvm, floor, ledger, slot, budget,
                                ),
                                6 + mode,
                            )?;
                            if budget.storage() != floor
                                || budget.work_ledger_identity_v1() != ledger
                            {
                                return Err(private_bf16_target_resource_v1(Resource::Accounting));
                            }
                        }
                        Ok(())
                    })())
                });
            self.descriptor
                .llvm
                .optimized
                .target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
            result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)??;
            self.revalidate_private_bf16_worker_handoff_v1(requested, typed_roots, profile)
        }
    }
}
#[cfg(test)]
mod genuine_observation {
    use super::*;
    use sha2::{Digest, Sha256};
    struct Hex<'a>(&'a [u8; 32]);
    impl std::fmt::Display for Hex<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for b in self.0 {
                write!(f, "{b:02x}")?;
            }
            Ok(())
        }
    }
    struct Observation {
        output_sha: [u8; 32],
        output_bytes: u64,
        descriptor_sha: [u8; 32],
        descriptor_bytes: usize,
        final_sha: [u8; 32],
        final_bytes: usize,
        handoff_sha: [u8; 32],
        handoff_bytes: usize,
        manifest_sha: [u8; 32],
        manifest_bytes: usize,
        retained_handoff: usize,
        retained_descriptor: usize,
        retained_llvm: usize,
        work: usize,
        storage: usize,
        peak: usize,
    }
    impl Observation {
        fn emit(self, requested: [u8; 4], scratch: usize) {
            eprintln!("fe2o3-bf16-private-owning-handoff-collected-v1 requested={},{},{},{} output_sha256={} output_bytes={} descriptor_sha256={} descriptor_bytes={} final_llvm_sha256={} final_llvm_bytes={} handoff_sha256={} handoff_bytes={} manifest_sha256={} manifest_bytes={} retained_handoff_storage={} retained_descriptor_storage={} retained_llvm_storage={} observer_storage={} work={} storage={} peak={} same_ledger=true actual_output_owner=true full_handoff_replayed=true full_module_replayed=true full_manifest_replayed=true handoff_bytes_mutation_refused=true foreign_module_refused=true foreign_manifest_refused=true constructor_refusals_proved=true replay_storage_restored=true runtime_bounds_alias_duties_preserved=true cleanup_pending=true llvm_emitted=true descriptor_constructed=true handoff_constructed=true worker_invoked=false normal_admission=false launch_authenticated=false artifact_authority=false handoff_authority=false",
                requested[0],requested[1],requested[2],requested[3],Hex(&self.output_sha),self.output_bytes,
                Hex(&self.descriptor_sha),self.descriptor_bytes,Hex(&self.final_sha),self.final_bytes,
                Hex(&self.handoff_sha),self.handoff_bytes,Hex(&self.manifest_sha),self.manifest_bytes,
                self.retained_handoff,self.retained_descriptor,self.retained_llvm,scratch,self.work,self.storage,self.peak);
        }
    }
    impl PrivateBf16WorkerHandoffV1 {
        /// Only called after all genuine controls; this prints a paid inert row.
        /// It does not expose bytes, a detached owner, Worker or launch authority.
        pub(crate) fn observe_private_bf16_handoff_for_test_v1(
            &mut self,
            requested: [u8; 4],
            roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
            profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        ) -> Result<(), PrivateBf16LlvmErrorV1> {
            self.revalidate_private_bf16_worker_handoff_v1(requested, roots, profile)?;
            let output = self.descriptor.llvm.optimized.optimization.checked.owner();
            let descriptor = &self.descriptor.descriptor;
            let llvm = &self.descriptor.llvm.llvm;
            let handoff = &self.handoff;
            let result = self
                .descriptor
                .llvm
                .optimized
                .target
                .formal
                .verification
                .phase
                .with_budget(|budget| {
                    let run = (|| -> Result<(), PrivateBf16LlvmErrorV1> {
                        budget
                            .check_prior_denials_v1()
                            .map_err(private_bf16_target_resource_v1)?;
                        budget
                            .charge_work(4)
                            .map_err(private_bf16_target_resource_v1)?;
                        let scratch = size_of::<Observation>();
                        let floor = budget.storage();
                        let ledger = budget.work_ledger_identity_v1();
                        let slot = budget as *const Budget<'_> as usize;
                        budget
                            .reserve_storage(scratch)
                            .map_err(private_bf16_target_resource_v1)?;
                        let protected = floor.checked_add(scratch).ok_or_else(arithmetic)?;
                        let inspected = (|| -> Result<Observation, PrivateBf16LlvmErrorV1> {
                            let bytes = handoff
                                .canonical
                                .len()
                                .checked_add(descriptor.canonical_descriptor.len())
                                .and_then(|n| n.checked_add(descriptor.final_llvm.len()))
                                .ok_or_else(arithmetic)?;
                            budget
                                .charge_work(
                                    bytes
                                        .checked_mul(8)
                                        .and_then(|n| n.checked_add(128))
                                        .ok_or_else(arithmetic)?,
                                )
                                .map_err(private_bf16_target_resource_v1)?;
                            let decoded = decode(&handoff.canonical, budget)?;
                            if decoded.module_bytes() != descriptor.final_llvm.as_bytes()
                                || decoded.canonical_bytes() != handoff.canonical.as_ref()
                                || handoff.retained != retained_bytes(handoff.canonical.len())?
                            {
                                return Err(mismatch(BYTE_MISMATCH));
                            }
                            let row = Observation {
                                output_sha: *output.canonical().identity().digest(),
                                output_bytes: output.canonical().identity().canonical_length(),
                                descriptor_sha: Sha256::digest(
                                    descriptor.canonical_descriptor.as_ref(),
                                )
                                .into(),
                                descriptor_bytes: descriptor.canonical_descriptor.len(),
                                final_sha: Sha256::digest(descriptor.final_llvm.as_bytes()).into(),
                                final_bytes: descriptor.final_llvm.len(),
                                handoff_sha: Sha256::digest(handoff.canonical.as_ref()).into(),
                                handoff_bytes: handoff.canonical.len(),
                                manifest_sha: Sha256::digest(
                                    decoded.symbol_manifest().canonical_bytes(),
                                )
                                .into(),
                                manifest_bytes: decoded.symbol_manifest().canonical_bytes().len(),
                                retained_handoff: handoff.retained,
                                retained_descriptor: descriptor.retained,
                                retained_llvm: llvm.retained,
                                work: budget.work(),
                                storage: budget.storage(),
                                peak: budget.peak_storage(),
                            };
                            drop(decoded);
                            Ok(row)
                        })();
                        if budget.storage() != protected
                            || budget.work_ledger_identity_v1() != ledger
                            || budget as *const Budget<'_> as usize != slot
                        {
                            drop(inspected);
                            return Err(private_bf16_target_resource_v1(Resource::Accounting));
                        }
                        let result = match inspected {
                            Ok(row) => {
                                row.emit(requested, scratch);
                                Ok(())
                            }
                            Err(e) => Err(e),
                        };
                        // Row/decoded temporaries are gone before known credit release.
                        budget
                            .release_storage(scratch)
                            .map_err(private_bf16_target_resource_v1)?;
                        budget
                            .check_prior_denials_v1()
                            .map_err(private_bf16_target_resource_v1)?;
                        if budget.storage() != floor {
                            return Err(private_bf16_target_resource_v1(Resource::Accounting));
                        }
                        result
                    })();
                    Ok(run)
                });
            self.descriptor
                .llvm
                .optimized
                .target
                .formal
                .verification
                .phase
                .require_clean_v1()
                .map_err(PrivateBf16LlvmErrorV1::RankedVerification)?;
            result.map_err(PrivateBf16LlvmErrorV1::RankedVerification)?
        }
    }
}
