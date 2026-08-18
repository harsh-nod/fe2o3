use fe2o3_kernel_ir::{
    BasicBlock, BlockId, DiagnosticCode, Function, Kernel, LaunchDomain, LaunchExtent, Module,
    Signature, TargetCapability, Terminator, encode_module_v1,
};
use fe2o3_kir_pliron_bridge::{
    BRIDGE_SCHEMA_V1, BridgeError, BridgeLimits, CANONICAL_BYTES_ATTR_KEY, CanonicalKirRecord,
    HARD_MAX_CANONICAL_BYTES, HARD_MAX_SHELL_OPERATIONS, KirVersion, LimitError, LimitResource,
    MODULE_IDENTITY_ATTR_KEY, MetadataField, SCHEMA_ATTR_KEY, ShellOperationKind,
    WIRE_VERSION_ATTR_KEY, recover_canonical,
};
use pliron::{
    builtin::{
        attributes::{BytesAttr, StringAttr, UnitAttr},
        op_interfaces::{SingleBlockRegionInterface, SymbolOpInterface},
        ops::ModuleOp,
    },
    context::Context,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
};

fn key(value: &str) -> Identifier {
    value.try_into().expect("test key is valid")
}

fn kernel_module(identity: &str) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new(identity);
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn projected() -> (Context, pliron::builtin::ops::ModuleOp, CanonicalKirRecord) {
    let limits = BridgeLimits::default();
    let record =
        CanonicalKirRecord::from_module(&kernel_module("hostile"), KirVersion::V5, limits).unwrap();
    let mut context = Context::new();
    let shell = record.project_to_pliron(&mut context, limits).unwrap();
    (context, shell, record)
}

#[test]
fn limit_configuration_and_payload_preflights_fail_closed() {
    assert_eq!(
        BridgeLimits::new(0, 1),
        Err(LimitError::Zero(LimitResource::CanonicalBytes))
    );
    assert_eq!(
        BridgeLimits::new(1, 0),
        Err(LimitError::Zero(LimitResource::ShellOperations))
    );
    assert_eq!(
        BridgeLimits::new(HARD_MAX_CANONICAL_BYTES + 1, 1),
        Err(LimitError::AboveHardCap(LimitResource::CanonicalBytes))
    );
    assert_eq!(
        BridgeLimits::new(1, HARD_MAX_SHELL_OPERATIONS + 1),
        Err(LimitError::AboveHardCap(LimitResource::ShellOperations))
    );

    let bytes = encode_module_v1(&Module::new("bounded")).unwrap();
    let limits = BridgeLimits::new(bytes.len() - 1, 1).unwrap();
    assert!(matches!(
        CanonicalKirRecord::parse(&bytes, limits),
        Err(BridgeError::CanonicalBytesLimit { .. })
    ));

    let limits = BridgeLimits::new(HARD_MAX_CANONICAL_BYTES, 1).unwrap();
    assert!(matches!(
        CanonicalKirRecord::from_module(
            &kernel_module("too-many-shell-ops"),
            KirVersion::V1,
            limits
        ),
        Err(BridgeError::ShellOperationsLimit { actual: 2, max: 1 })
    ));
}

#[test]
fn unknown_malformed_noncanonical_and_truncated_bytes_are_rejected_without_panics() {
    let bytes = encode_module_v1(&Module::new("wire-hostile")).unwrap();
    for length in 0..bytes.len() {
        let outcome = std::panic::catch_unwind(|| {
            CanonicalKirRecord::parse(&bytes[..length], BridgeLimits::default())
        });
        assert!(outcome.is_ok(), "decoder panicked at truncation {length}");
        assert!(outcome.unwrap().is_err(), "accepted truncation {length}");
    }

    let mut unknown = bytes.clone();
    unknown[8..10].copy_from_slice(&6_u16.to_le_bytes());
    assert!(matches!(
        CanonicalKirRecord::parse(&unknown, BridgeLimits::default()),
        Err(BridgeError::Decode(
            fe2o3_kernel_ir::KernelIrDecodeError::UnknownVersion(6)
        ))
    ));

    let mut flags = bytes.clone();
    flags[10] = 1;
    assert!(matches!(
        CanonicalKirRecord::parse(&flags, BridgeLimits::default()),
        Err(BridgeError::Decode(
            fe2o3_kernel_ir::KernelIrDecodeError::UnsupportedFlags(1)
        ))
    ));

    let mut reserved = bytes;
    reserved[16] = 1;
    assert!(matches!(
        CanonicalKirRecord::parse(&reserved, BridgeLimits::default()),
        Err(BridgeError::Decode(
            fe2o3_kernel_ir::KernelIrDecodeError::ReservedNonZero { .. }
        ))
    ));

    let mut noncanonical_module = Module::new("m");
    noncanonical_module.required_capabilities =
        [TargetCapability::Float16, TargetCapability::BFloat16]
            .into_iter()
            .collect();
    let mut noncanonical = encode_module_v1(&noncanonical_module).unwrap();
    let first_capability = 20 + 5 + 12;
    noncanonical[first_capability + 1] = noncanonical[first_capability];
    assert!(matches!(
        CanonicalKirRecord::parse(&noncanonical, BridgeLimits::default()),
        Err(BridgeError::Decode(
            fe2o3_kernel_ir::KernelIrDecodeError::NonCanonical
        ))
    ));
}

#[test]
fn duplicate_and_conflicting_kir_identities_are_rejected_after_wire_validation() {
    let mut duplicates = Module::new("duplicate-functions");
    duplicates.functions.extend([
        Function::declaration("same", Signature::new(vec![], vec![])),
        Function::declaration("same", Signature::new(vec![], vec![])),
    ]);
    let bytes = encode_module_v1(&duplicates).expect("wire codec preserves duplicate records");
    let error = CanonicalKirRecord::parse(&bytes, BridgeLimits::default()).unwrap_err();
    assert!(matches!(
        error,
        BridgeError::InvalidKir(summary)
            if summary.first_code() == Some(DiagnosticCode::DuplicateFunction)
    ));

    let mut duplicate_kernels = kernel_module("duplicate-kernels");
    duplicate_kernels
        .kernels
        .push(duplicate_kernels.kernels[0].clone());
    let bytes =
        encode_module_v1(&duplicate_kernels).expect("wire codec preserves duplicate records");
    let error = CanonicalKirRecord::parse(&bytes, BridgeLimits::default()).unwrap_err();
    assert!(matches!(
        error,
        BridgeError::InvalidKir(summary)
            if summary.first_code() == Some(DiagnosticCode::DuplicateKernel)
    ));

    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut conflicting_roles = Module::new("conflicting-function-roles");
    conflicting_roles.functions.extend([
        Function::declaration("same", Signature::new(vec![], vec![])),
        Function::definition("same", Signature::new(vec![], vec![]), vec![], vec![block]),
    ]);
    let bytes =
        encode_module_v1(&conflicting_roles).expect("wire codec preserves conflicting records");
    let error = CanonicalKirRecord::parse(&bytes, BridgeLimits::default()).unwrap_err();
    assert!(matches!(
        error,
        BridgeError::InvalidKir(summary)
            if summary.first_code() == Some(DiagnosticCode::DuplicateFunction)
                && summary.diagnostic_count() >= 2
    ));
}

#[test]
fn missing_or_type_confused_canonical_payload_never_falls_back_to_shell_reconstruction() {
    let (context, shell, _) = projected();
    shell
        .get_operation()
        .deref_mut(&context)
        .attributes
        .0
        .remove(&key(CANONICAL_BYTES_ATTR_KEY));
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::LossyConversion {
            missing: MetadataField::CanonicalBytes
        })
    ));

    let (context, shell, _) = projected();
    shell.get_operation().deref_mut(&context).attributes.set(
        key(CANONICAL_BYTES_ATTR_KEY),
        StringAttr::new("not bytes".into()),
    );
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::MetadataTypeConfusion(
            MetadataField::CanonicalBytes
        ))
    ));
}

#[test]
fn redundant_schema_version_identity_and_metadata_cardinality_are_enforced() {
    let (context, shell, _) = projected();
    let mut wrong_schema = BRIDGE_SCHEMA_V1;
    wrong_schema[0] ^= 1;
    shell
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(key(SCHEMA_ATTR_KEY), BytesAttr::new(wrong_schema.to_vec()));
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::MetadataConflict(MetadataField::Schema))
    ));

    let (context, shell, _) = projected();
    shell.get_operation().deref_mut(&context).attributes.set(
        key(WIRE_VERSION_ATTR_KEY),
        BytesAttr::new(4_u16.to_le_bytes().to_vec()),
    );
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::MetadataConflict(MetadataField::WireVersion))
    ));

    let (context, shell, _) = projected();
    shell.get_operation().deref_mut(&context).attributes.set(
        key(WIRE_VERSION_ATTR_KEY),
        BytesAttr::new(99_u16.to_le_bytes().to_vec()),
    );
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::UnknownVersion(99))
    ));

    let (context, shell, _) = projected();
    shell.get_operation().deref_mut(&context).attributes.set(
        key(MODULE_IDENTITY_ATTR_KEY),
        StringAttr::new("substituted".into()),
    );
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::MetadataConflict(MetadataField::ModuleIdentity))
    ));

    let (context, shell, _) = projected();
    shell
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(key("hostile_extra"), UnitAttr);
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::UnexpectedMetadata)
    ));

    let (mut context, shell, _) = projected();
    shell.set_symbol_name(&mut context, "confused_shell_identity".try_into().unwrap());
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::MetadataConflict(MetadataField::ModuleSymbol))
    ));
}

#[test]
fn duplicate_extra_and_valid_but_conflicting_shell_operations_are_rejected() {
    let (mut context, shell, _) = projected();
    let duplicate = dialect_gpu::HierarchyIdOp::new(&mut context, dialect_gpu::HierarchyAttr::Grid);
    shell.append_operation(&mut context, duplicate.get_operation(), 0);
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::ShellOperationCount {
            expected: 2,
            actual: 3
        })
    ));

    let (mut context, shell, _) = projected();
    let body = shell.get_body(&context, 0);
    let original_grid = body
        .deref(&context)
        .iter(&context)
        .nth(1)
        .expect("grid projection");
    original_grid.unlink(&context);
    let lane = dialect_gpu::HierarchyIdOp::new(&mut context, dialect_gpu::HierarchyAttr::Lane);
    shell.append_operation(&mut context, lane.get_operation(), 0);
    assert!(matches!(
        recover_canonical(&context, &shell, BridgeLimits::default()),
        Err(BridgeError::ShellOperationConflict {
            index: 1,
            expected: ShellOperationKind::GpuGridHierarchy
        })
    ));

    let (mut context, shell, _) = projected();
    let foreign = dialect_kernel::AlgorithmOp::new(&mut context, 1).unwrap();
    shell.append_operation(&mut context, foreign.get_operation(), 0);
    let tight = BridgeLimits::new(HARD_MAX_CANONICAL_BYTES, 2).unwrap();
    assert!(matches!(
        recover_canonical(&context, &shell, tight),
        Err(BridgeError::ShellOperationsLimit { actual: 3, max: 2 })
    ));
}

#[test]
fn crate_manifest_contains_only_representation_dependencies() {
    let manifest = include_str!("../Cargo.toml").to_ascii_lowercase();
    for forbidden in ["comgr", "pliron-llvm", "hsa", "hip", "rustix", "libc"] {
        assert!(
            !manifest.contains(forbidden),
            "forbidden authority dependency {forbidden}"
        );
    }
}

#[test]
fn a_foreign_operation_wrapped_as_a_module_is_rejected_before_traversal() {
    let (mut context, _, _) = projected();
    let hierarchy = dialect_gpu::HierarchyIdOp::new(&mut context, dialect_gpu::HierarchyAttr::Grid);
    let forged = ModuleOp::from_operation(hierarchy.get_operation());
    assert!(matches!(
        recover_canonical(&context, &forged, BridgeLimits::default()),
        Err(BridgeError::MalformedShell)
    ));
}
