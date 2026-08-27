use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, DiagnosticCode, Function,
    Gfx950LdsTransposeFormatV1, Gfx950LdsTransposeOperationKindV1, Gfx950LdsTransposeOperationV1,
    Kernel, LaunchDomain, LaunchExtent, MAX_FUNCTIONS_V1, MAX_TEXT_BYTES_V1, Module, Operation,
    OperationKind, ScalarType, Signature, TargetCapability, Terminator, Type, ValueDef, ValueId,
    encode_module_v1,
};
use fe2o3_kir_pliron_bridge::{
    BRIDGE_SCHEMA_V1, BridgeEnvelope, BridgeError, BridgeLimits, CANONICAL_BYTES_ATTR_KEY,
    CanonicalKirRecord, HARD_MAX_CANONICAL_BYTES, HARD_MAX_SHELL_OPERATIONS, KirVersion,
    LimitError, LimitResource, MODULE_IDENTITY_ATTR_KEY, MetadataField, SCHEMA_ATTR_KEY,
    ShellOperationKind, WIRE_VERSION_ATTR_KEY, recover_canonical,
};
use fe2o3_pliron::{CONTEXT_IDENTITY_MARKER_KEY, ContextIdentityError};
use pliron::{
    builtin::{
        attributes::{BytesAttr, StringAttr, UnitAttr},
        op_interfaces::{SingleBlockRegionInterface, SymbolOpInterface},
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

fn projected() -> (Context, BridgeEnvelope, CanonicalKirRecord) {
    let limits = BridgeLimits::default();
    let record =
        CanonicalKirRecord::from_module(&kernel_module("hostile"), KirVersion::V5, limits).unwrap();
    let mut context = Context::new();
    let envelope = record.project_to_pliron(&mut context, limits).unwrap();
    (context, envelope, record)
}

#[test]
fn v5_bridge_rejects_gfx950_lds_transpose_before_encoding() {
    let mut module = kernel_module("gfx950-transpose");
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![ValueDef::new(
                ValueId(0),
                Type::pointer(
                    Type::Scalar(ScalarType::U8),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Current {
                    format: Gfx950LdsTransposeFormatV1::Fp8E4M3,
                },
            )),
        ));

    assert!(matches!(
        CanonicalKirRecord::from_module(&module, KirVersion::V5, BridgeLimits::default()),
        Err(BridgeError::Encode(
            fe2o3_kernel_ir::KernelIrEncodeError::UnsupportedInVersion {
                version: 5,
                feature: "gfx950 LDS transpose operation",
            }
        ))
    ));
}

fn recover_projected(
    context: &Context,
    envelope: &BridgeEnvelope,
    record: &CanonicalKirRecord,
    limits: BridgeLimits,
) -> Result<CanonicalKirRecord, BridgeError> {
    recover_canonical(context, envelope, record.canonical_bytes(), limits)
}

fn context_identity_key() -> Identifier {
    key(CONTEXT_IDENTITY_MARKER_KEY)
}

fn take_context_identity_marker(context: &mut Context) -> Box<dyn std::any::Any> {
    let index = context
        .aux_data_map
        .remove(&context_identity_key())
        .expect("context identity marker exists");
    context
        .aux_data
        .remove(index)
        .expect("context identity marker is live")
}

fn install_context_identity_marker(context: &mut Context, marker: Box<dyn std::any::Any>) {
    let index = context.aux_data.insert(marker);
    context.aux_data_map.insert(context_identity_key(), index);
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

    let oversized_text = Module::new("x".repeat(MAX_TEXT_BYTES_V1 + 1));
    assert!(matches!(
        CanonicalKirRecord::from_module(
            &oversized_text,
            KirVersion::V5,
            BridgeLimits::default()
        ),
        Err(BridgeError::Encode(
            fe2o3_kernel_ir::KernelIrEncodeError::LimitExceeded {
                field: "module ID",
                actual,
                max: MAX_TEXT_BYTES_V1,
            }
        )) if actual == MAX_TEXT_BYTES_V1 + 1
    ));

    let declaration = Function::declaration("f", Signature::new(vec![], vec![]));
    let mut oversized_shape = Module::new("oversized-shape");
    oversized_shape.functions = vec![declaration; MAX_FUNCTIONS_V1 + 1];
    assert!(matches!(
        CanonicalKirRecord::from_module(
            &oversized_shape,
            KirVersion::V5,
            BridgeLimits::default()
        ),
        Err(BridgeError::Encode(
            fe2o3_kernel_ir::KernelIrEncodeError::LimitExceeded {
                field: "module functions",
                actual,
                max: MAX_FUNCTIONS_V1,
            }
        )) if actual == MAX_FUNCTIONS_V1 + 1
    ));

    let mut semantically_invalid = Module::new("active-limit-precedes-verification");
    semantically_invalid.functions.extend([
        Function::declaration("same", Signature::new(vec![], vec![])),
        Function::declaration("same", Signature::new(vec![], vec![])),
    ]);
    let low_byte_limit = BridgeLimits::new(20, HARD_MAX_SHELL_OPERATIONS).unwrap();
    assert!(matches!(
        CanonicalKirRecord::from_module(&semantically_invalid, KirVersion::V5, low_byte_limit),
        Err(BridgeError::CanonicalBytesLimit { max: 20, .. })
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
    let (context, envelope, record) = projected();
    envelope
        .shell()
        .get_operation()
        .deref_mut(&context)
        .attributes
        .0
        .remove(&key(CANONICAL_BYTES_ATTR_KEY));
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::LossyConversion {
            missing: MetadataField::CanonicalBytes
        })
    ));

    let (context, envelope, record) = projected();
    envelope
        .shell()
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(
            key(CANONICAL_BYTES_ATTR_KEY),
            StringAttr::new("not bytes".into()),
        );
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::MetadataTypeConfusion(
            MetadataField::CanonicalBytes
        ))
    ));
}

#[test]
fn redundant_schema_version_identity_and_metadata_cardinality_are_enforced() {
    let (context, envelope, record) = projected();
    let mut wrong_schema = BRIDGE_SCHEMA_V1;
    wrong_schema[0] ^= 1;
    envelope
        .shell()
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(key(SCHEMA_ATTR_KEY), BytesAttr::new(wrong_schema.to_vec()));
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::MetadataConflict(MetadataField::Schema))
    ));

    let (context, envelope, record) = projected();
    envelope
        .shell()
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(
            key(WIRE_VERSION_ATTR_KEY),
            BytesAttr::new(4_u16.to_le_bytes().to_vec()),
        );
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::MetadataConflict(MetadataField::WireVersion))
    ));

    let (context, envelope, record) = projected();
    envelope
        .shell()
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(
            key(WIRE_VERSION_ATTR_KEY),
            BytesAttr::new(99_u16.to_le_bytes().to_vec()),
        );
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::UnknownVersion(99))
    ));

    let (context, envelope, record) = projected();
    envelope
        .shell()
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(
            key(MODULE_IDENTITY_ATTR_KEY),
            StringAttr::new("substituted".into()),
        );
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::MetadataConflict(MetadataField::ModuleIdentity))
    ));

    let (context, envelope, record) = projected();
    envelope
        .shell()
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(key("hostile_extra"), UnitAttr);
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::UnexpectedMetadata)
    ));

    let (mut context, envelope, record) = projected();
    envelope
        .shell()
        .set_symbol_name(&mut context, "confused_shell_identity".try_into().unwrap());
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::MetadataConflict(MetadataField::ModuleSymbol))
    ));
}

#[test]
fn duplicate_extra_and_valid_but_conflicting_shell_operations_are_rejected() {
    let (mut context, envelope, record) = projected();
    let duplicate = dialect_gpu::HierarchyIdOp::new(&mut context, dialect_gpu::HierarchyAttr::Grid);
    envelope
        .shell()
        .append_operation(&mut context, duplicate.get_operation(), 0);
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::ShellOperationCount {
            expected: 2,
            actual: 3
        })
    ));

    let (mut context, envelope, record) = projected();
    let body = envelope.shell().get_body(&context, 0);
    let original_grid = body
        .deref(&context)
        .iter(&context)
        .nth(1)
        .expect("grid projection");
    original_grid.unlink(&context);
    let lane = dialect_gpu::HierarchyIdOp::new(&mut context, dialect_gpu::HierarchyAttr::Lane);
    envelope
        .shell()
        .append_operation(&mut context, lane.get_operation(), 0);
    assert!(matches!(
        recover_projected(&context, &envelope, &record, BridgeLimits::default()),
        Err(BridgeError::ShellOperationConflict {
            index: 1,
            expected: ShellOperationKind::GpuGridHierarchy
        })
    ));

    let (mut context, envelope, record) = projected();
    let foreign = dialect_kernel::AlgorithmOp::new(&mut context, 1).unwrap();
    envelope
        .shell()
        .append_operation(&mut context, foreign.get_operation(), 0);
    let tight = BridgeLimits::new(HARD_MAX_CANONICAL_BYTES, 2).unwrap();
    assert!(matches!(
        recover_projected(&context, &envelope, &record, tight),
        Err(BridgeError::ShellOperationsLimit { actual: 3, max: 2 })
    ));
}

#[test]
fn extra_bytes_metadata_is_rejected_on_every_projected_child() {
    for index in 0..2 {
        let (context, envelope, record) = projected();
        let body = envelope.shell().get_body(&context, 0);
        let child = body
            .deref(&context)
            .iter(&context)
            .nth(index)
            .expect("projected child");
        child.deref_mut(&context).attributes.set(
            key("hostile_child_bytes"),
            BytesAttr::new(vec![index as u8]),
        );

        assert!(matches!(
            recover_projected(&context, &envelope, &record, BridgeLimits::default()),
            Err(BridgeError::UnexpectedShellMetadata { index: actual })
                if actual == index
        ));
    }
}

#[test]
fn gpu_registration_failure_is_propagated_fail_closed() {
    let record = CanonicalKirRecord::from_module(
        &kernel_module("gpu-registration"),
        KirVersion::V5,
        BridgeLimits::default(),
    )
    .unwrap();
    let mut context = Context::new();
    let marker = context.aux_data.insert(Box::new(17_u32));
    context
        .aux_data_map
        .insert(key("fe2o3_dialect_gpu_explicit_registration"), marker);

    assert!(matches!(
        record.project_to_pliron(&mut context, BridgeLimits::default()),
        Err(BridgeError::GpuRegistration(
            dialect_gpu::RegistrationError::MarkerCollision
        ))
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
fn matching_foreign_arena_layout_is_rejected_before_shell_traversal() {
    let (_owner, envelope, record) = projected();
    let mut foreign = Context::new();
    let foreign_envelope = record
        .project_to_pliron(&mut foreign, BridgeLimits::default())
        .unwrap();

    assert_eq!(
        envelope.shell().get_operation(),
        foreign_envelope.shell().get_operation(),
        "test requires colliding contextless Pliron arena handles"
    );
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        recover_projected(&foreign, &envelope, &record, BridgeLimits::default())
    }));
    assert!(matches!(outcome, Ok(Err(BridgeError::ContextMismatch))));
}

#[test]
fn transplanted_marker_cannot_transfer_an_envelope_to_an_anchored_context() {
    let (mut owner, envelope, record) = projected();
    let owner_marker = take_context_identity_marker(&mut owner);

    let mut foreign = Context::new();
    let foreign_envelope = record
        .project_to_pliron(&mut foreign, BridgeLimits::default())
        .unwrap();
    drop(take_context_identity_marker(&mut foreign));
    install_context_identity_marker(&mut foreign, owner_marker);

    assert_eq!(
        envelope.shell().get_operation(),
        foreign_envelope.shell().get_operation(),
        "test requires colliding contextless Pliron arena handles"
    );
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        recover_projected(&foreign, &envelope, &record, BridgeLimits::default())
    }));
    assert!(matches!(
        outcome,
        Ok(Err(BridgeError::ContextIdentity(
            ContextIdentityError::CorruptMarker
        )))
    ));
}

#[test]
fn transplanted_marker_into_an_unanchored_context_is_typed_and_panic_free() {
    let (mut owner, envelope, record) = projected();
    let owner_marker = take_context_identity_marker(&mut owner);
    let mut foreign = Context::new();
    install_context_identity_marker(&mut foreign, owner_marker);

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        recover_projected(&foreign, &envelope, &record, BridgeLimits::default())
    }));
    assert!(matches!(
        outcome,
        Ok(Err(BridgeError::ContextIdentity(
            ContextIdentityError::CorruptMarker
        )))
    ));
}
