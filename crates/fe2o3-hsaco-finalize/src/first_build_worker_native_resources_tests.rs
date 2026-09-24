use super::*;
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

fn dimensions() -> Dimensions {
    Dimensions {
        handoff: 4096,
        module: 1024,
        envelope: 256,
        manifest: 512,
        providers: 128,
        provider_count: 1,
        option_text: 32,
        option_count: 1,
        symbols: 2,
        contracts: 0,
        output: 1024,
        stdout: 8192,
        stderr: 512,
        timeout: Duration::from_secs(1),
    }
}

fn module() -> CompilerModuleHandoffV2 {
    let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let version = CodeObjectVersion::V6;
    let envelope = CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, version).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "resource_test"),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "resource_test.kd",
        ),
    ])
    .unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        version,
        envelope,
        manifest,
        b"define amdgpu_kernel void @resource_test() { ret void }\n",
    )
    .unwrap()
}

#[test]
fn arithmetic_errors_do_not_wrap_or_saturate() {
    assert_eq!(
        sum([usize::MAX, 1], "sum"),
        Err(NativeWorkerResourceQuoteError::Arithmetic("sum"))
    );
    assert_eq!(
        product(usize::MAX, 2, "copies"),
        Err(NativeWorkerResourceQuoteError::Arithmetic("copies"))
    );
    let mut d = dimensions();
    d.providers = usize::MAX;
    assert_eq!(
        NativeWorkerResourceQuote::from_dimensions(d),
        Err(NativeWorkerResourceQuoteError::Arithmetic(
            "aggregate input payload"
        ))
    );
}

#[test]
fn count_caps_are_checked_before_payload_arithmetic() {
    let mut d = dimensions();
    d.provider_count = MAX_LINK_INPUTS;
    d.providers = usize::MAX;
    assert_eq!(
        NativeWorkerResourceQuote::from_dimensions(d),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "provider count",
            actual: MAX_LINK_INPUTS,
            maximum: MAX_LINK_INPUTS - 1,
        })
    );
    d = dimensions();
    d.option_count = MAX_LINK_OPTIONS + 1;
    d.option_text = usize::MAX;
    assert_eq!(
        NativeWorkerResourceQuote::from_dimensions(d),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "option count",
            actual: MAX_LINK_OPTIONS + 1,
            maximum: MAX_LINK_OPTIONS,
        })
    );
    d = dimensions();
    d.provider_count = MAX_LINK_INPUTS - 1;
    d.option_count = MAX_LINK_OPTIONS;
    assert!(NativeWorkerResourceQuote::from_dimensions(d).is_ok());
}

#[test]
fn declared_byte_bounds_and_extent_are_not_silently_clamped() {
    let mut d = dimensions();
    d.providers = MAX_WORKER_TOTAL_INPUT_BYTES;
    assert!(matches!(
        NativeWorkerResourceQuote::from_dimensions(d),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "aggregate input payload",
            ..
        })
    ));
    d = dimensions();
    d.handoff = d.module;
    assert_eq!(
        NativeWorkerResourceQuote::from_dimensions(d),
        Err(NativeWorkerResourceQuoteError::HandoffExtent)
    );
    d = dimensions();
    d.output = MAX_WORKER_OUTPUT_BYTES + 1;
    assert!(matches!(
        NativeWorkerResourceQuote::from_dimensions(d),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "output",
            ..
        })
    ));
    d = dimensions();
    d.option_text = MAX_LINK_OPTION_NAME_BYTES + MAX_LINK_OPTION_VALUE_BYTES + 1;
    assert!(matches!(
        NativeWorkerResourceQuote::from_dimensions(d),
        Err(NativeWorkerResourceQuoteError::HardBound {
            component: "option text",
            ..
        })
    ));
}

#[test]
fn all_byte_and_count_dimensions_are_monotone() {
    let base = NativeWorkerResourceQuote::from_dimensions(dimensions()).unwrap();
    for index in 0..13 {
        let mut d = dimensions();
        match index {
            0 => d.handoff += 1,
            1 => d.module += 1,
            2 => d.envelope += 1,
            3 => d.manifest += 1,
            4 => d.providers += 1,
            5 => d.provider_count += 1,
            6 => d.option_text += 1,
            7 => d.option_count += 1,
            8 => d.output += 1,
            9 => d.stdout += 1,
            10 => d.stderr += 1,
            11 => d.symbols += 1,
            12 => d.contracts += 1,
            _ => unreachable!(),
        }
        let larger = NativeWorkerResourceQuote::from_dimensions(d).unwrap();
        assert!(
            larger.preflight_storage >= base.preflight_storage,
            "dimension {index}"
        );
        assert!(
            larger.preflight_work >= base.preflight_work,
            "dimension {index}"
        );
        assert!(
            larger.execution_storage >= base.execution_storage,
            "dimension {index}"
        );
        assert!(
            larger.execution_work >= base.execution_work,
            "dimension {index}"
        );
        assert!(
            larger.returned_buffer_storage >= base.returned_buffer_storage,
            "dimension {index}"
        );
        assert!(
            larger.returned_retained_storage() >= base.returned_retained_storage(),
            "dimension {index}"
        );
    }
}

#[test]
fn metadata_count_boundaries_are_checked() {
    for (symbols, contracts) in [
        (MAX_COMPILER_MODULE_SYMBOLS_V1, 0),
        (0, MAX_COMPILER_FFI_CONTRACTS_V1),
    ] {
        let mut d = dimensions();
        d.symbols = symbols;
        d.contracts = contracts;
        assert!(NativeWorkerResourceQuote::from_dimensions(d).is_ok());
        if symbols != 0 {
            d.symbols += 1;
        } else {
            d.contracts += 1;
        }
        assert!(matches!(
            NativeWorkerResourceQuote::from_dimensions(d),
            Err(NativeWorkerResourceQuoteError::HardBound { .. })
        ));
    }
}

#[test]
fn tiny_captures_include_minimum_vec_capacity() {
    for limit in 0..16 {
        assert!(capture_storage(limit).unwrap() >= 8 + limit);
        assert!(capture_storage(limit).unwrap() >= 3 * limit);
    }
    assert!(matches!(
        capture_storage(usize::MAX),
        Err(NativeWorkerResourceQuoteError::Arithmetic(_))
    ));
}

#[test]
fn tiny_exact_output_does_not_underquote_malformed_response_decode() {
    let mut d = dimensions();
    d.output = 1;
    d.stdout = MAX_WORKER_RESPONSE_BYTES;
    let tiny = NativeWorkerResourceQuote::from_dimensions(d).unwrap();
    assert_eq!(tiny.response_capture_bytes, MAX_WORKER_RESPONSE_BYTES);
    assert_eq!(tiny.response_decode_output_bytes, MAX_WORKER_OUTPUT_BYTES);
    assert!(tiny.successful_response_bytes < tiny.response_capture_bytes);
    d.output = MAX_WORKER_OUTPUT_BYTES;
    let large = NativeWorkerResourceQuote::from_dimensions(d).unwrap();
    assert_eq!(tiny.execution_storage, large.execution_storage);
    assert!(tiny.returned_buffer_storage < large.returned_buffer_storage);
    assert!(tiny.execution_storage >= tiny.returned_buffer_storage);
    assert!(large.execution_storage >= large.returned_buffer_storage);
}

#[test]
fn capture_caps_each_response_but_keeps_two_executions() {
    let mut d = dimensions();
    d.stdout = 100;
    let quote = NativeWorkerResourceQuote::from_dimensions(d).unwrap();
    assert_eq!(quote.response_capture_bytes, 100);
    assert_eq!(quote.response_decode_output_bytes, 100);
    assert_eq!(quote.successful_response_bytes, 100);
    assert!(quote.execution_storage >= 2 * (2 * 100 + 100 + 2 * d.stderr));
    assert_eq!(quote.total_worker_timeout, Duration::from_secs(2));
}

#[test]
fn timeout_is_separate_from_logical_codec_work() {
    let base = NativeWorkerResourceQuote::from_dimensions(dimensions()).unwrap();
    let mut d = dimensions();
    d.timeout = MAX_WORKER_TIMEOUT;
    let longer = NativeWorkerResourceQuote::from_dimensions(d).unwrap();
    assert_eq!(longer.preflight_work, base.preflight_work);
    assert_eq!(longer.execution_work, base.execution_work);
    assert_eq!(longer.execution_storage, base.execution_storage);
    assert_eq!(longer.total_worker_timeout, MAX_WORKER_TIMEOUT * 2);
    d.timeout += Duration::from_nanos(1);
    assert_eq!(
        NativeWorkerResourceQuote::from_dimensions(d),
        Err(NativeWorkerResourceQuoteError::InvalidExecutionLimits)
    );
}

#[test]
fn staging_before_wire_cap_rejection_is_not_truncated() {
    let mut d = dimensions();
    d.module = MAX_COMPILER_MODULE_BYTES_V1;
    d.providers = 0;
    d.provider_count = 0;
    d.envelope = MAX_COMPILER_FFI_ENVELOPE_BYTES_V1;
    d.manifest = MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1;
    d.handoff = MAX_COMPILER_MODULE_HANDOFF_BYTES_V2;
    let quote = NativeWorkerResourceQuote::from_dimensions(d).unwrap();
    assert_eq!(quote.request_wire_bytes, MAX_WORKER_REQUEST_BYTES);
    assert!(quote.request_encoding_bytes > quote.request_wire_bytes);
    assert!(quote.preflight_storage >= quote.request_encoding_bytes);
    // The separate aggregate V3 guard may reject these dimensions. A buffer
    // estimate deliberately does not imply aggregate admission.
}

#[test]
fn constructor_returns_complete_quote_for_small_valid_handoff() {
    let module = module();
    let output = WorkerOutputConstraintsV1::new(1024).unwrap();
    let limits = WorkerExecutionLimitsV1::default();
    let quote = NativeWorkerResourceQuote::new(&module, &[], &[], &output, limits).unwrap();
    assert!(quote.preflight_work > 0);
    assert!(quote.execution_work > 0);
    assert!(quote.preflight_work < 4_000_000_000);
    assert!(quote.execution_work < 4_000_000_000);
    assert!(quote.returned_retained_storage() > quote.returned_buffer_storage);
    assert!(quote.returned_retained_storage() <= quote.execution_storage);
}

#[test]
fn one_underpayment_refuses_each_leaf_callback_on_original_ledger() {
    let quote = NativeWorkerResourceQuote::from_dimensions(dimensions()).unwrap();
    for (work, scratch) in [
        (quote.preflight_work, quote.preflight_storage),
        (quote.execution_work, quote.execution_storage),
    ] {
        for underpay_storage in [false, true] {
            let mut meter = Work::new(7 + work - usize::from(!underpay_storage));
            let mut budget = Budget::new(&mut meter, 13 + scratch - usize::from(underpay_storage));
            budget.charge_work(7).unwrap();
            budget.reserve_storage(13).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let mut called = false;
            let result = budget.with_prepaid_scope::<(), Resource>(13, 1, work, scratch, |_| {
                called = true;
                Ok(())
            });
            assert!(!called);
            assert_eq!(budget.storage(), 13);
            assert!(budget.work_ledger_identity_v1() == ledger);
            if underpay_storage {
                assert!(matches!(result, Err(Resource::Storage(_))));
                assert_eq!(budget.work(), 7 + work);
                assert_eq!(budget.failed_storage(), Some(13 + scratch));
            } else {
                assert!(matches!(result, Err(Resource::Work(_))));
                assert_eq!(budget.work(), 8);
            }
        }
    }
}

#[test]
fn restored_scratch_and_retained_result_are_distinct_reservations() {
    let quote = NativeWorkerResourceQuote::from_dimensions(dimensions()).unwrap();
    let floor = 31;
    let mut meter = Work::new(7 + quote.execution_work);
    let mut budget = Budget::new(&mut meter, floor + quote.execution_storage);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    budget
        .with_prepaid_scope::<(), Resource>(
            floor,
            1,
            quote.execution_work,
            quote.execution_storage,
            |inner| {
                assert!(inner.work_ledger_identity_v1() == ledger);
                assert_eq!(inner.storage(), floor + quote.execution_storage);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 7 + quote.execution_work);
    budget
        .reserve_storage(quote.returned_retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor + quote.returned_retained_storage());
    assert_eq!(budget.peak_storage(), floor + quote.execution_storage);
    budget
        .release_storage(quote.returned_retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}
