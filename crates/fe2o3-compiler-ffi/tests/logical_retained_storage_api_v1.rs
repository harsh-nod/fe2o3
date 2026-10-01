//! Compile/run this as an external crate consumer, not an internal-only method test.
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerDescriptorSourceV1, CompilerFfiEnvelopeV1,
    CompilerFfiLogicalStorageErrorV1 as Error, CompilerFfiLogicalStorageLimitsV1 as Limits,
    CompilerFfiLogicalStorageV1 as Report, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use fe2o3_kernel_descriptor::*;
use std::mem::size_of;

fn limits() -> Limits {
    Limits {
        max_bytes: None,
        max_items: 100_000,
    }
}

#[test]
fn public_ffi_owner_reports_are_callable_and_compose_without_embedded_header_duplication() {
    let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V5)
            .unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "entry"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "entry.kd"),
    ])
    .unwrap();
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V5,
        envelope,
        manifest,
        b"; module\ndefine amdgpu_kernel void @entry() { ret void }\n",
    )
    .unwrap();
    let r: Report = handoff.logical_retained_storage_v1(limits()).unwrap();
    assert_eq!(r.header_bytes(), size_of::<CompilerModuleHandoffV2>());
    let envelope = handoff
        .envelope()
        .logical_retained_storage_v1(limits())
        .unwrap();
    let manifest = handoff
        .symbol_manifest()
        .logical_retained_storage_v1(limits())
        .unwrap();
    assert_eq!(
        r.heap_bytes(),
        envelope.heap_bytes() + manifest.heap_bytes() + handoff.backing_capacity()
    );
    let parts = handoff.into_parts();
    let r = parts.logical_retained_storage_v1(limits()).unwrap();
    let module = parts
        .module()
        .logical_retained_storage_v1(limits())
        .unwrap();
    assert_eq!(
        r.heap_bytes(),
        envelope.heap_bytes() + manifest.heap_bytes() + module.heap_bytes()
    );
    assert_eq!(
        parts.logical_retained_storage_v1(Limits {
            max_bytes: Some(r.total_bytes()),
            max_items: r.visited_items()
        }),
        Ok(r)
    );
    assert_eq!(
        parts.logical_retained_storage_v1(Limits {
            max_bytes: Some(r.total_bytes() - 1),
            max_items: r.visited_items()
        }),
        Err(Error::ByteLimit)
    );
    assert_eq!(
        parts.logical_retained_storage_v1(Limits {
            max_bytes: None,
            max_items: 0
        }),
        Err(Error::ItemLimit)
    );
    assert!(!parts.grants_launch_authority());
}

#[test]
fn public_descriptor_visitor_and_source_report_are_available_without_kir_dependency() {
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([1; 32]),
        EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([3; 32]),
        ValidName::new("logical").unwrap(),
        ValidName::new("entry").unwrap(),
        ValidName::new("entry.kd").unwrap(),
        evidence,
        evidence,
        Vec::new(),
        KernelAbiLayoutV1::new(0, 0, 1).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Any,
            DimensionsV1::new(1, 1, 1).unwrap(),
            64,
            0,
            0,
        )
        .unwrap(),
        Vec::new(),
    )
    .unwrap();
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V5,
        CompilerIdentityV1::new(
            Text::new("compiler").unwrap(),
            Text::new("release").unwrap(),
            [4; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("producer").unwrap(),
            Text::new("version").unwrap(),
        ),
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        Vec::new(),
        Vec::new(),
        vec![kernel],
    )
    .unwrap();
    let mut bytes = 0usize;
    table
        .visit_logical_retained_heap_v1(&mut |count, width| {
            bytes = count
                .checked_mul(width)
                .and_then(|b| bytes.checked_add(b))
                .ok_or(Error::Arithmetic)?;
            Ok::<_, Error>(())
        })
        .unwrap();
    assert_eq!(
        table.visit_logical_retained_heap_v1(&mut |_, _| Err::<(), _>("refused")),
        Err("refused")
    );
    let source = CompilerDescriptorSourceV1::new(table).unwrap();
    let r = source.logical_retained_storage_v1(limits()).unwrap();
    assert_eq!(r.header_bytes(), size_of::<CompilerDescriptorSourceV1>());
    // The constructor may reserve extra canonical capacity; do not replace it
    // with the serialized length or infer the decoded table's size from bytes.
    assert!(r.heap_bytes() >= bytes + source.canonical_bytes().len());
    assert!(!source.grants_link_authority());
}
