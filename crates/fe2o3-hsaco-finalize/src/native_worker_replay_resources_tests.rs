use super::*;
use std::time::Duration;

use fe2o3_compiler_ffi::{
    CompilerFfiEnvelopeV1, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

use crate::{MAX_LINK_INPUTS, MAX_LINK_OPTIONS};

fn module() -> CompilerModuleHandoffV2 {
    let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let version = CodeObjectVersion::V6;
    let envelope = CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, version).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            "replay_resource_test",
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "replay_resource_test.kd",
        ),
    ])
    .unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        version,
        envelope,
        manifest,
        b"define amdgpu_kernel void @replay_resource_test() { ret void }\n",
    )
    .unwrap()
}

fn limits(stdout: usize) -> WorkerExecutionLimitsV1 {
    WorkerExecutionLimitsV1::new(Duration::from_secs(1), stdout, 512).unwrap()
}

fn metadata() -> WorkerResponseReplayMetadataV1<'static> {
    WorkerResponseReplayMetadataV1::from_bodies(&[0, 0, 0, 0], None, None)
}

fn response_inputs() -> NativeWorkerReplayResponseInputs<'static> {
    NativeWorkerReplayResponseInputs {
        raw_output_bytes: 64,
        worker_build_identity_bytes: 10,
        bootstrap_metadata: metadata(),
        replay_metadata: metadata(),
    }
}

fn quote() -> NativeWorkerReplayResourceQuote {
    NativeWorkerReplayResourceQuote::new(
        &module(),
        &[],
        &[],
        &WorkerOutputConstraintsV1::new(1024).unwrap(),
        limits(8192),
        response_inputs(),
    )
    .unwrap()
}

fn v2_dimensions() -> ResponseDimensions {
    ResponseDimensions::from_metadata(metadata())
}

#[test]
fn original_quote_is_preserved_and_additions_are_enumerated() {
    let quote = quote();
    let original = NativeWorkerResourceQuote::new(
        &module(),
        &[],
        &[],
        &WorkerOutputConstraintsV1::new(1024).unwrap(),
        limits(8192),
    )
    .unwrap();
    assert_eq!(quote.first_build, original);
    let response = ResponseEncoding::new(v2_dimensions(), 64, 10, 1024, 8192).unwrap();
    let (metadata_work, metadata_storage) = original.response_metadata_resources();
    let shell = size_of::<InertNativeFirstBuildWorkerEvidenceV1>();
    let kind_work = 41 + 2 * size_of::<KindRow>() + size_of::<WorkerInputKindV1>();
    let kind_storage = size_of::<KindRow>()
        + size_of::<WorkerInputKindV1>()
        + size_of::<Vec<KindRow>>()
        + size_of::<Vec<WorkerInputKindV1>>();
    assert_eq!(
        quote.reconstruction_work,
        original.preflight_work
            + original.execution_work
            + 2 * metadata_work
            + 2 * response.work
            + 64
            + kind_work
            + shell,
    );
    assert_eq!(
        quote.reconstruction_storage,
        original.preflight_storage
            + original.execution_storage
            + 2 * metadata_storage
            + 2 * response.storage
            + kind_storage
            + shell,
    );
    assert!(original.returned_retained_storage() + shell <= quote.reconstruction_storage);
}

#[test]
fn response_families_pay_their_exact_framing_and_hash_prefix() {
    let v2 = ResponseEncoding::new(v2_dimensions(), 64, 10, 1024, 8192).unwrap();
    assert_eq!(v2.wire, 8 + 7 * 6 + 3 * 32 + 1 + 10 + 4 + 41 + 64);
    assert_eq!(v2.work, 64 + 41 + 64 + v2.wire);
    assert_eq!(v2.storage, 41 + 64 + v2.wire);
    for (provider, derivation, extra_headers) in [
        (Some(9), None, 2),
        (None, Some(11), 3),
        (Some(9), Some(11), 3),
    ] {
        let d = ResponseDimensions {
            provider,
            derivation,
            ..v2_dimensions()
        };
        let encoded = ResponseEncoding::new(d, 64, 10, 1024, 8192).unwrap();
        assert_eq!(
            encoded.wire,
            v2.wire + extra_headers * 6 + 32 + provider.unwrap_or(0) + derivation.unwrap_or(0)
        );
        let prefix = if derivation.is_some() {
            b"FE2O3/DIRECT-LLVM-WORKER-RESPONSE/V4\0".len()
        } else {
            b"FE2O3/DIRECT-LLVM-WORKER-RESPONSE/V3\0".len()
        };
        assert_eq!(
            encoded.work,
            64 + 41 + 64 + encoded.wire + encoded.wire - 38 + prefix + 8
        );
        assert_eq!(encoded.storage, 41 + 64 + encoded.wire);
    }
}

#[test]
fn response_directions_are_charged_independently() {
    let inputs = NativeWorkerReplayResponseInputs {
        // The quote measures bodies; canonical validity belongs to the prepaid
        // shared codec. These opaque bodies are NOT claimed to be evidence.
        replay_metadata: WorkerResponseReplayMetadataV1::from_bodies(
            &[0; 4],
            Some(&[0; 9]),
            Some(&[0; 11]),
        ),
        ..response_inputs()
    };
    let actual = NativeWorkerReplayResourceQuote::new(
        &module(),
        &[],
        &[],
        &WorkerOutputConstraintsV1::new(1024).unwrap(),
        limits(8192),
        inputs,
    )
    .unwrap();
    let bootstrap = ResponseEncoding::new(v2_dimensions(), 64, 10, 1024, 8192).unwrap();
    let replay = ResponseEncoding::new(
        ResponseDimensions::from_metadata(inputs.replay_metadata),
        64,
        10,
        1024,
        8192,
    )
    .unwrap();
    assert_eq!(
        actual.reconstruction_work - quote().reconstruction_work,
        replay.work - bootstrap.work
    );
    assert_eq!(
        actual.reconstruction_storage - quote().reconstruction_storage,
        replay.storage - bootstrap.storage
    );
}

#[test]
fn stdout_accepts_the_exact_wire_and_refuses_one_short_in_either_direction() {
    let response = ResponseEncoding::new(v2_dimensions(), 64, 10, 1024, 8192).unwrap();
    assert!(
        NativeWorkerReplayResourceQuote::new(
            &module(),
            &[],
            &[],
            &WorkerOutputConstraintsV1::new(1024).unwrap(),
            limits(response.wire),
            response_inputs(),
        )
        .is_ok()
    );
    assert_eq!(
        NativeWorkerReplayResourceQuote::new(
            &module(),
            &[],
            &[],
            &WorkerOutputConstraintsV1::new(1024).unwrap(),
            limits(response.wire - 1),
            response_inputs(),
        ),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "replay response stdout",
            actual: response.wire,
            maximum: response.wire - 1,
        })
    );
    let larger = WorkerResponseReplayMetadataV1::from_bodies(&[0; 5], None, None);
    for inputs in [
        NativeWorkerReplayResponseInputs {
            bootstrap_metadata: larger,
            ..response_inputs()
        },
        NativeWorkerReplayResponseInputs {
            replay_metadata: larger,
            ..response_inputs()
        },
    ] {
        assert_eq!(
            NativeWorkerReplayResourceQuote::new(
                &module(),
                &[],
                &[],
                &WorkerOutputConstraintsV1::new(1024).unwrap(),
                limits(response.wire),
                inputs,
            ),
            Err(NativeWorkerResourceQuoteError::HardBound {
                component: "replay response stdout",
                actual: response.wire + 1,
                maximum: response.wire,
            })
        );
    }
}

#[test]
fn output_build_and_metadata_dimensions_enforce_inclusive_caps() {
    let d = v2_dimensions();
    assert_eq!(
        ResponseEncoding::new(d, 0, 10, 1024, 8192),
        Err(NativeWorkerResourceQuoteError::Empty("replay raw output"))
    );
    assert_eq!(
        ResponseEncoding::new(d, 1025, 10, 1024, 8192),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "replay bootstrap output",
            actual: 1025,
            maximum: 1024,
        })
    );
    assert!(ResponseEncoding::new(d, 1024, 10, 1024, 8192).is_ok());
    assert_eq!(
        ResponseEncoding::new(d, MAX_WORKER_OUTPUT_BYTES + 1, 10, usize::MAX, usize::MAX),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "replay raw output",
            actual: MAX_WORKER_OUTPUT_BYTES + 1,
            maximum: MAX_WORKER_OUTPUT_BYTES,
        })
    );
    assert!(
        ResponseEncoding::new(
            d,
            MAX_WORKER_OUTPUT_BYTES,
            10,
            MAX_WORKER_OUTPUT_BYTES,
            MAX_WORKER_RESPONSE_BYTES
        )
        .is_ok()
    );
    assert_eq!(
        ResponseEncoding::new(d, 64, 0, 1024, 8192),
        Err(NativeWorkerResourceQuoteError::Empty(
            "replay worker build identity"
        ))
    );
    assert!(ResponseEncoding::new(d, 64, MAX_WORKER_TOOLCHAIN_ID_BYTES, 1024, 8192).is_ok());
    assert_eq!(
        ResponseEncoding::new(d, 64, MAX_WORKER_TOOLCHAIN_ID_BYTES + 1, 1024, 8192),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "replay worker build identity",
            actual: MAX_WORKER_TOOLCHAIN_ID_BYTES + 1,
            maximum: MAX_WORKER_TOOLCHAIN_ID_BYTES,
        })
    );
    let cap = MAX_WORKER_RESPONSE_REPLAY_METADATA_SHELL_BYTES_V1;
    let at_cap = ResponseDimensions {
        diagnostics: 4,
        provider: Some(cap - 5),
        derivation: Some(1),
    };
    assert!(ResponseEncoding::new(at_cap, 64, 10, 1024, MAX_WORKER_RESPONSE_BYTES).is_ok());
    let over_cap = ResponseDimensions {
        derivation: Some(2),
        ..at_cap
    };
    assert_eq!(
        ResponseEncoding::new(over_cap, 64, 10, 1024, MAX_WORKER_RESPONSE_BYTES),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "replay response metadata",
            actual: cap + 1,
            maximum: cap,
        })
    );
}

#[test]
fn checked_overflow_never_wraps_or_uses_saturation() {
    assert_eq!(
        sum([usize::MAX, 1], "sum"),
        Err(NativeWorkerResourceQuoteError::Arithmetic("sum"))
    );
    assert_eq!(
        product(usize::MAX, 2, "product"),
        Err(NativeWorkerResourceQuoteError::Arithmetic("product"))
    );
    let d = ResponseDimensions {
        diagnostics: usize::MAX,
        provider: Some(1),
        derivation: None,
    };
    assert_eq!(
        ResponseEncoding::new(d, 64, 10, 1024, 8192),
        Err(NativeWorkerResourceQuoteError::Arithmetic(
            "replay response metadata"
        ))
    );
    let original = quote().first_build;
    let response = ResponseEncoding::new(v2_dimensions(), 64, 10, 1024, 8192).unwrap();
    assert_eq!(
        NativeWorkerReplayResourceQuote::compose(original, usize::MAX, 64, [response; 2]),
        Err(NativeWorkerResourceQuoteError::Arithmetic(
            "replay input count"
        ))
    );
    for storage in [false, true] {
        let mut excessive = original;
        if storage {
            excessive.execution_storage = usize::MAX;
        } else {
            excessive.execution_work = usize::MAX;
        }
        assert_eq!(
            NativeWorkerReplayResourceQuote::compose(excessive, 0, 64, [response; 2]),
            Err(NativeWorkerResourceQuoteError::Arithmetic(if storage {
                "replay reconstruction storage"
            } else {
                "replay reconstruction work"
            }))
        );
    }
}

#[test]
fn bounded_collection_admission_is_inherited_before_response_work() {
    let provider = WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![1]).unwrap();
    let mut providers = vec![provider; MAX_LINK_INPUTS - 1];
    let option = LinkOptionV1::new("optimization", "2").unwrap();
    let mut options = vec![option; MAX_LINK_OPTIONS];
    let module = module();
    let output = WorkerOutputConstraintsV1::new(1024).unwrap();
    // Duplicate identities/options are semantic checks, not length census.
    assert!(
        NativeWorkerReplayResourceQuote::new(
            &module,
            &providers,
            &options,
            &output,
            limits(8192),
            response_inputs(),
        )
        .is_ok()
    );
    providers.push(providers[0].clone());
    let invalid_response = NativeWorkerReplayResponseInputs {
        raw_output_bytes: 0,
        ..response_inputs()
    };
    assert_eq!(
        NativeWorkerReplayResourceQuote::new(
            &module,
            &providers,
            &options,
            &output,
            limits(8192),
            invalid_response,
        ),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "provider count",
            actual: MAX_LINK_INPUTS,
            maximum: MAX_LINK_INPUTS - 1,
        })
    );
    providers.pop();
    options.push(options[0].clone());
    assert_eq!(
        NativeWorkerReplayResourceQuote::new(
            &module,
            &providers,
            &options,
            &output,
            limits(8192),
            invalid_response,
        ),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "option count",
            actual: MAX_LINK_OPTIONS + 1,
            maximum: MAX_LINK_OPTIONS,
        })
    );
}

#[test]
fn actual_lengths_not_caller_capacity_control_the_quote_without_cloning_inputs() {
    let mut spacious = Vec::with_capacity(4096);
    spacious.extend_from_slice(&[1, 2, 3]);
    let small = WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![1, 2, 3]).unwrap();
    let large = WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, spacious).unwrap();
    let module = module();
    let pointer = module.canonical_bytes().as_ptr();
    let small_pointer = small.bytes().as_ptr();
    let large_pointer = large.bytes().as_ptr();
    let output = WorkerOutputConstraintsV1::new(1024).unwrap();
    let make = |provider: &WorkerInputV1| {
        NativeWorkerReplayResourceQuote::new(
            &module,
            std::slice::from_ref(provider),
            &[],
            &output,
            limits(8192),
            response_inputs(),
        )
        .unwrap()
    };
    assert_eq!(make(&small), make(&large));
    assert_eq!(module.canonical_bytes().as_ptr(), pointer);
    assert_eq!(small.bytes().as_ptr(), small_pointer);
    assert_eq!(large.bytes().as_ptr(), large_pointer);
}

#[test]
fn larger_response_dimensions_increase_only_the_enumerated_additions() {
    let base = quote();
    let module = module();
    let output = WorkerOutputConstraintsV1::new(1024).unwrap();
    let bigger_output = NativeWorkerReplayResponseInputs {
        raw_output_bytes: 65,
        ..response_inputs()
    };
    let actual = NativeWorkerReplayResourceQuote::new(
        &module,
        &[],
        &[],
        &output,
        limits(8192),
        bigger_output,
    )
    .unwrap();
    assert_eq!(actual.first_build, base.first_build);
    // Each V2 encode adds output hash + body write + wire write; helper adds 1 hash byte.
    assert_eq!(
        actual.reconstruction_work - base.reconstruction_work,
        2 * 3 + 1
    );
    assert_eq!(
        actual.reconstruction_storage - base.reconstruction_storage,
        2 * 2
    );
    let bigger_build = NativeWorkerReplayResponseInputs {
        worker_build_identity_bytes: 11,
        ..response_inputs()
    };
    let actual = NativeWorkerReplayResourceQuote::new(
        &module,
        &[],
        &[],
        &output,
        limits(8192),
        bigger_build,
    )
    .unwrap();
    assert_eq!(actual.first_build, base.first_build);
    assert_eq!(actual.reconstruction_work - base.reconstruction_work, 2);
    assert_eq!(
        actual.reconstruction_storage - base.reconstruction_storage,
        2
    );
}

#[test]
fn timeout_is_not_reinterpreted_as_logical_reconstruction_work() {
    let base = quote();
    let different_timeout =
        WorkerExecutionLimitsV1::new(Duration::from_secs(2), 8192, 512).unwrap();
    let actual = NativeWorkerReplayResourceQuote::new(
        &module(),
        &[],
        &[],
        &WorkerOutputConstraintsV1::new(1024).unwrap(),
        different_timeout,
        response_inputs(),
    )
    .unwrap();
    assert_eq!(actual.reconstruction_work, base.reconstruction_work);
    assert_eq!(actual.reconstruction_storage, base.reconstruction_storage);
}

#[test]
fn one_short_work_or_storage_refuses_callback_on_original_ledger() {
    let quote = quote();
    for underpay_storage in [false, true] {
        let mut work = Work::new(7 + quote.reconstruction_work - usize::from(!underpay_storage));
        let mut budget = Budget::new(
            &mut work,
            13 + quote.reconstruction_storage - usize::from(underpay_storage),
        );
        budget.charge_work(7).unwrap();
        budget.reserve_storage(13).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut called = false;
        let result = budget.with_prepaid_scope::<(), Resource>(
            13,
            1,
            quote.reconstruction_work,
            quote.reconstruction_storage,
            |_| {
                called = true;
                Ok(())
            },
        );
        assert!(!called);
        assert_eq!(budget.storage(), 13);
        assert!(budget.work_ledger_identity_v1() == ledger);
        if underpay_storage {
            assert!(matches!(result, Err(Resource::Storage(_))));
            assert_eq!(budget.work(), 7 + quote.reconstruction_work);
            assert_eq!(
                budget.failed_storage(),
                Some(13 + quote.reconstruction_storage)
            );
        } else {
            assert!(matches!(result, Err(Resource::Work(_))));
            assert_eq!(budget.work(), 8);
        }
    }
}

#[test]
fn exact_scope_restores_scratch_and_retained_evidence_is_reserved_separately() {
    let quote = quote();
    let floor = 31;
    let retained = quote.first_build.returned_retained_storage()
        + size_of::<InertNativeFirstBuildWorkerEvidenceV1>();
    let mut work = Work::new(7 + quote.reconstruction_work);
    let mut budget = Budget::new(&mut work, floor + quote.reconstruction_storage);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    budget
        .with_prepaid_scope::<(), Resource>(
            floor,
            1,
            quote.reconstruction_work,
            quote.reconstruction_storage,
            |inner| {
                assert!(inner.work_ledger_identity_v1() == ledger);
                assert_eq!(inner.storage(), floor + quote.reconstruction_storage);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 7 + quote.reconstruction_work);
    budget.reserve_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor + retained);
    assert_eq!(budget.peak_storage(), floor + quote.reconstruction_storage);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn callback_failure_keeps_work_charged_and_releases_only_replay_scratch() {
    let quote = quote();
    let mut work = Work::new(quote.reconstruction_work);
    let mut budget = Budget::new(&mut work, 31 + quote.reconstruction_storage);
    budget.reserve_storage(31).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = budget.with_prepaid_scope::<(), Resource>(
        31,
        0,
        quote.reconstruction_work,
        quote.reconstruction_storage,
        |_| Err(Resource::Arithmetic),
    );
    assert!(matches!(result, Err(Resource::Arithmetic)));
    assert_eq!(budget.storage(), 31);
    assert_eq!(budget.work(), quote.reconstruction_work);
    assert!(budget.work_ledger_identity_v1() == ledger);
}
