//! Synthetic metadata and custody controls, not compiler or execution qualification.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ExecutionRoleV15, FixedVectorTypeV12, Function,
    Kernel, LaunchDomain, LaunchExtent, ScalarType, Signature, StorageLayoutIdV1, Terminator,
    ValueId, VectorLayoutV12, WorkgroupSize,
};
use serde_json::Value;
#[path = "../../tests/fixtures/diagnostic_kir_v18.rs"]
mod fixture;

const FLOOR: usize = 97;
fn options(files: &fixture::Files) -> Options {
    Options {
        kir: files.kir.clone().into_os_string(),
        output: None,
    }
}
fn module() -> Module {
    let mut module = fixture::module();
    module.functions.clear();
    module.kernels.clear();
    for (id, parameters, workgroup) in [
        (
            "actual$alpha",
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::Scalar(ScalarType::U64),
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
            ],
            Some(WorkgroupSize::new(64, 1, 1)),
        ),
        ("actual$beta", vec![Type::INDEX], None),
    ] {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let body_parameters = (0..parameters.len())
            .map(|index| ValueId(7 + index as u32))
            .collect();
        let mut function = Function::kernel_entry(
            id,
            Signature::new(parameters, vec![]),
            body_parameters,
            vec![block],
        );
        function.required_capabilities = function.derived_capabilities();
        let mut kernel = Kernel::new(
            id,
            id,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = workgroup;
        kernel.required_capabilities = function.required_capabilities.clone();
        module.functions.push(function);
        module.kernels.push(kernel);
    }
    module.required_capabilities = module.derived_capabilities();
    module
}
fn run_files(files: &fixture::Files) -> Vec<u8> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let mut output = Vec::new();
    run_with_budget(&options(files), &mut budget, &mut output).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
    output
}
fn tag(error: &Failure) -> String {
    serialized_tag(error.0.kind)
}

#[test]
fn all_kernel_ids_and_ordered_request_parameters_are_actual_metadata() {
    let module = module();
    let bytes = fixture::bytes(&module);
    let files = fixture::Files::new(&bytes, b"not a request");
    std::fs::remove_file(&files.request).unwrap();
    let report: Value = serde_json::from_slice(&run_files(&files)).unwrap();
    assert_eq!(report["schema"], "fe2o3-kernel-inventory-v1");
    assert_eq!(report["storage_layouts"], module.storage_layouts.len());
    assert_eq!(
        report["kernels"].as_array().unwrap().len(),
        module.kernels.len()
    );
    for (index, kernel) in module.kernels.iter().enumerate() {
        let actual = &report["kernels"][index];
        assert_eq!(actual["id"], kernel.id.as_str());
        assert_eq!(actual["entry"], kernel.entry.as_str());
        assert_eq!(actual["request_abi"], "entry_parameter_order");
        assert_eq!(
            actual["parameters"].as_array().unwrap().len(),
            module
                .function(&kernel.entry)
                .unwrap()
                .signature
                .parameters
                .len()
        );
    }
    let first = &report["kernels"][0];
    assert_eq!(first["workgroup_size"], serde_json::json!([64, 1, 1]));
    assert!(report["kernels"][1]["workgroup_size"].is_null());
    assert_eq!(first["parameters"][0]["type"]["kind"], "slice");
    assert_eq!(first["parameters"][0]["type"]["access"], "read_only");
    assert_eq!(
        first["parameters"][0]["request_encoding"]["kind"],
        "buffer_or_buffer_view"
    );
    assert_eq!(
        first["parameters"][1]["type"],
        serde_json::json!({"kind":"scalar","type":"u64","bits":64})
    );
    assert_eq!(first["parameters"][2]["type"]["access"], "read_write");
    assert_eq!(
        report["kernels"][1]["parameters"][0]["type"],
        serde_json::json!({"kind":"scalar","type":"index","bits":null})
    );
    assert_eq!(first["results"], serde_json::json!([]));
}

#[test]
fn exact_domain_identity_and_raw_digest_are_distinct_and_bound() {
    let bytes = fixture::bytes(&module());
    let files = fixture::Files::new(&bytes, b"");
    let report: Value = serde_json::from_slice(&run_files(&files)).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    let (owner, receipt) = Owner::from_canonical_bytes_with_verification_budget_v18(
        &bytes,
        diagnostic_kir_v18::LAYOUT_LIMITS,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|value| format!("{value:02x}"))
            .collect::<String>()
    };
    assert_eq!(report["kir"]["wire_version"], 18);
    assert_eq!(report["kir"]["canonical_bytes"], bytes.len());
    assert_eq!(
        report["kir"]["identity_sha256"],
        hex(owner.identity().digest())
    );
    assert_eq!(report["kir"]["raw_sha256"], hex(&Sha256::digest(&bytes)));
    assert_ne!(
        report["kir"]["identity_sha256"],
        report["kir"]["raw_sha256"]
    );
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn metadata_of_unexecutable_storage_operations_is_not_simulator_admission() {
    let files = fixture::Files::new(&fixture::bytes(&fixture::storage_module(true)), b"");
    let report: Value = serde_json::from_slice(&run_files(&files)).unwrap();
    assert_eq!(report["simulator_admission"], "not_checked");
    assert_eq!(report["native_abi"], "unavailable");
    assert_eq!(report["target_profile"], "not_encoded");
    assert_eq!(
        report["additional_launch_requirements"],
        "unavailable_from_kernel_metadata"
    );
    for flag in [
        "simulated",
        "source_authentication",
        "proof_authority",
        "compiler_execution_authority",
        "launch_authority",
        "hardware_observed",
        "performance_prediction",
    ] {
        assert_eq!(report[flag], false, "{flag}");
    }
}

#[test]
fn formatter_preserves_every_type_without_inventing_request_encodings() {
    let mut module = module();
    module.functions[0].signature.parameters = vec![
        Type::Unit,
        Type::Scalar(ScalarType::F64),
        Type::StorageObject(StorageLayoutIdV1(1)),
        Type::Execution(ExecutionRoleV15::Context),
        Type::Execution(ExecutionRoleV15::Workgroup),
        Type::Execution(ExecutionRoleV15::MaskedTileU32 {
            lanes: 64,
            elements: 3,
        }),
        Type::Execution(ExecutionRoleV15::LaneFragmentU32 {
            lanes: 64,
            elements: 3,
        }),
        Type::Vector(FixedVectorTypeV12::new(
            ScalarType::U32,
            4,
            VectorLayoutV12::Interleaved { factor: 2 },
        )),
        Type::pointer(
            Type::slice(
                Type::StorageObject(StorageLayoutIdV1(0)),
                AddressSpace::Workgroup,
                AccessMode::ReadOnly,
            ),
            AddressSpace::Generic,
            AccessMode::ReadWrite,
        ),
    ];
    // This is formatter DATA only; deliberately not a claimed canonical admission.
    let report = format::Report::new(&module, [1; 32], 1, [2; 32]);
    let document = serde_json::to_value(&report).unwrap();
    let parameters = document["kernels"][0]["parameters"].as_array().unwrap();
    assert_eq!(parameters[2]["type"]["layout"], 1);
    assert_eq!(parameters[5]["type"]["role"]["elements"], 3);
    assert_eq!(
        parameters[7]["type"]["layout"],
        serde_json::json!({"kind":"interleaved","factor":2})
    );
    assert_eq!(parameters[8]["type"]["pointee"]["element"]["layout"], 0);
    for index in [0, 2, 3, 4, 5, 6, 7, 8] {
        assert_eq!(parameters[index]["request_encoding"]["kind"], "unavailable");
    }
    assert_eq!(document["simulator_admission"], "not_checked");
    let empty = Module::new("empty-metadata");
    let empty = serde_json::to_value(format::Report::new(&empty, [1; 32], 1, [2; 32])).unwrap();
    assert_eq!(empty["kernels"], serde_json::json!([]));
}

#[test]
fn strict_inspect_options_do_not_accept_execution_or_version_fallback() {
    let args = |values: &[&str]| {
        values
            .iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    };
    assert!(parse(args(&["--diagnostic-kir-v18", "input", "--output", "new"])).is_ok());
    for values in [
        vec![],
        vec!["--kir-v7", "input"],
        vec!["--diagnostic-kir-v18"],
        vec!["--diagnostic-kir-v18", ""],
        vec!["--diagnostic-kir-v18", "--output"],
        vec!["--diagnostic-kir-v18", "input", "--output", "--request"],
        vec!["--diagnostic-kir-v18", "input", "--request", "request"],
        vec![
            "--diagnostic-kir-v18",
            "input",
            "--diagnostic-kir-v18",
            "again",
        ],
        vec![
            "--diagnostic-kir-v18",
            "input",
            "--output",
            "a",
            "--output",
            "b",
        ],
        vec!["--diagnostic-kir-v18", "input", "--race-evidence"],
        vec![
            "--diagnostic-kir-v18",
            "input",
            "--replay-schedule",
            "schedule",
        ],
    ] {
        assert_eq!(
            tag(&parse(args(&values)).unwrap_err()),
            "invalid_command_line"
        );
    }
}

#[test]
fn metadata_nesting_is_bounded_before_serializer_recursion() {
    let mut module = module();
    let mut nested = Type::Scalar(ScalarType::U32);
    for _ in 0..MAX_TYPE_DEPTH {
        nested = Type::pointer(nested, AddressSpace::Global, AccessMode::ReadOnly);
    }
    module.functions[0].signature.parameters = vec![nested.clone()];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    assert!(census(&module, &mut budget).is_ok());
    module.functions[0].signature.parameters = vec![Type::pointer(
        nested,
        AddressSpace::Global,
        AccessMode::ReadOnly,
    )];
    let error = census(&module, &mut budget).unwrap_err();
    assert_eq!(tag(&error.0), "output_too_large");
    assert_eq!(budget.storage(), 0);
}

#[test]
fn wrong_version_and_corrupt_input_leave_no_report_or_credit_escape() {
    let bytes = fixture::bytes(&module());
    for changed in [
        {
            let mut copy = bytes.clone();
            copy[8..10].copy_from_slice(&17_u16.to_le_bytes());
            copy
        },
        bytes[..bytes.len() - 1].to_vec(),
        [bytes.as_slice(), &[0]].concat(),
    ] {
        let files = fixture::Files::new(&changed, b"");
        let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
        let mut budget = Budget::new(&mut work, fixture::BOUND);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut output = Vec::new();
        assert!(run_with_budget(&options(&files), &mut budget, &mut output).is_err());
        assert!(output.is_empty());
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn original_budget_exact_and_one_short_limits_preserve_floor_and_no_output() {
    let files = fixture::Files::new(&fixture::bytes(&module()), b"");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let mut expected = Vec::new();
    run_with_budget(&options(&files), &mut budget, &mut expected).unwrap();
    let used = budget.work();
    let peak = budget.peak_storage();
    assert!(used > 1 && peak > FRAME_BYTES + FLOOR);
    for (work_limit, storage_limit, succeeds) in [
        (used, peak, true),
        (0, peak, false),
        (used - 1, peak, false),
        (used, peak - 1, false),
        (used, FLOOR + FRAME_BYTES - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let address = &budget as *const Budget<'_>;
        let mut output = Vec::new();
        let result = run_with_budget(&options(&files), &mut budget, &mut output);
        assert_eq!(
            result.is_ok(),
            succeeds,
            "{work_limit}/{storage_limit}: {result:?}"
        );
        if succeeds {
            assert_eq!(output, expected);
        } else {
            assert!(output.is_empty());
        }
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(&budget as *const Budget<'_>, address);
    }
}

#[test]
fn escaped_metadata_output_bound_refuses_before_stdout_or_publication() {
    let mut module = Module::new("\"".repeat(MAX_REPORT_BYTES / 2));
    let files = fixture::Files::new(b"", b"");
    let destination = files.path("inventory.json");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    let mut output = Vec::new();
    for destination in [None, Some(destination.as_path())] {
        let error = publish_report(
            &format::Report::new(&module, [1; 32], 1, [2; 32]),
            destination,
            &mut budget,
            &mut output,
            MAX_REPORT_BYTES,
        )
        .unwrap_err();
        assert_eq!(tag(&error.0), "output_too_large");
        assert!(output.is_empty());
    }
    assert!(!destination.exists());
    module.id = "short".into();
    assert!(
        measure(
            &format::Report::new(&module, [1; 32], 1, [2; 32]),
            &mut budget,
            1
        )
        .is_err()
    );
}

#[test]
fn secure_inputs_and_transactional_outputs_preserve_foreign_entries() {
    let files = fixture::Files::new(&fixture::bytes(&module()), b"");
    let expected = run_files(&files);
    let output = files.path("inventory.json");
    let mut opts = options(&files);
    opts.output = Some(output.clone().into_os_string());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    let mut stdout = Vec::new();
    run_with_budget(&opts, &mut budget, &mut stdout).unwrap();
    assert!(stdout.is_empty());
    assert_eq!(std::fs::read(&output).unwrap(), expected);
    assert!(run_with_budget(&opts, &mut budget, &mut stdout).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), expected);
    let link = files.path("output-link");
    std::os::unix::fs::symlink(&output, &link).unwrap();
    opts.output = Some(link.clone().into_os_string());
    assert!(run_with_budget(&opts, &mut budget, &mut stdout).is_err());
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&output).unwrap(), expected);
    for path in [link.as_path(), Path::new("/dev/null")] {
        opts = Options {
            kir: path.as_os_str().into(),
            output: None,
        };
        assert!(run_with_budget(&opts, &mut budget, &mut stdout).is_err());
    }
    assert_eq!(budget.storage(), 0);
    assert!(stdout.is_empty());
}

struct FailedOutput(bool);
impl Write for FailedOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        if self.0 {
            panic!("injected output panic");
        }
        Err(io::Error::other("injected output error"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn writer_error_and_panic_drop_owners_before_original_storage_cleanup() {
    let files = fixture::Files::new(&fixture::bytes(&module()), b"");
    for panic in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(fixture::BOUND);
        let mut budget = Budget::new(&mut work, fixture::BOUND);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_with_budget(&options(&files), &mut budget, &mut FailedOutput(panic))
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(budget.work() > 0);
    }
}
