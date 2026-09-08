use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_rustc_front::*;

const ROOT_NAME: &str = "root_a";
const HELPER_NAME: &str = "helper";
const MARKER_NAME: &str = "marker";

fn fixture() -> KernelContextFrontendContractV1 {
    KernelContextFrontendContractV1::for_generated_kernel(ROOT_NAME, HELPER_NAME, MARKER_NAME)
        .unwrap()
}

fn item(role: KernelContextGeneratedItemRoleV1, name: &str) -> KernelContextGeneratedItemV1 {
    KernelContextGeneratedItemV1::for_generated_name(role, name).unwrap()
}

fn entry_offsets(bytes: &[u8]) -> [usize; 3] {
    let diagnostic_length = usize::from(u16::from_le_bytes(bytes[24..26].try_into().unwrap()));
    let mut offset = 32 + diagnostic_length;
    std::array::from_fn(|_| {
        let current = offset;
        let name_length = usize::from(u16::from_le_bytes(
            bytes[offset + 36..offset + 38].try_into().unwrap(),
        ));
        offset += 40 + name_length;
        current
    })
}

#[test]
fn generated_contract_round_trips_and_exposes_exact_bindings() {
    let contract = fixture();
    let encoded = encode_kernel_context_frontend_contract_v1(&contract);
    let convenience =
        encode_generated_kernel_context_frontend_contract_v1(ROOT_NAME, HELPER_NAME, MARKER_NAME)
            .unwrap();
    assert_eq!(encoded, convenience);
    assert!(encoded.len() <= MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1);

    let decoded = decode_kernel_context_frontend_contract_v1(&encoded).unwrap();
    assert_eq!(decoded, contract);
    assert_eq!(
        encode_kernel_context_frontend_contract_v1(&decoded),
        encoded
    );
    assert_eq!(decoded.physical_kernel_root().name(), ROOT_NAME);
    assert_eq!(decoded.logical_helper().name(), HELPER_NAME);
    assert_eq!(decoded.nominal_kernel_marker().name(), MARKER_NAME);
    assert_eq!(decoded.context_source_ordinal(), 0);
    assert_eq!(decoded.required_issuance_count(), 1);
    assert_eq!(
        decoded.issuance_diagnostic().identity(),
        KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_IDENTITY_V1
    );
    assert_eq!(decoded.issuance_diagnostic().version(), 1);

    for binding in [
        decoded.physical_kernel_root(),
        decoded.logical_helper(),
        decoded.nominal_kernel_marker(),
    ] {
        assert_eq!(
            binding.identity(),
            derive_kernel_context_generated_item_identity_v1(binding.role(), binding.name())
        );
    }
}

#[test]
fn frozen_launch_contract_v1_bytes_are_unchanged() {
    const GOLDEN: &str = "4645324f334b46000100030040000000000000000700000000010000010000000100000000010000010000000100000002000000010005001500000000000000";
    let dimensions = FrontendWorkgroupDimensionsV1::new([256, 1, 1]).unwrap();
    let contract = KernelFrontendContractV1::new(
        Some(FrontendLaunchBoundsV1::new(Some(dimensions), Some(dimensions), Some(2)).unwrap()),
        Some(
            FrontendUnsafeAssemblyDeclarationV1::new(
                FrontendUnsafeAssemblyTargetV1::AmdGpuGfx942,
                ASSEMBLY_OPERAND_SGPR_V1 | ASSEMBLY_OPERAND_IMMEDIATE_V1,
                ASSEMBLY_OPTION_NOMEM_V1 | ASSEMBLY_OPTION_PURE_V1 | ASSEMBLY_OPTION_NOSTACK_V1,
                0,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let encoded = encode_kernel_frontend_contract_v1(contract);
    let actual = encoded
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, GOLDEN);
    assert_eq!(
        decode_kernel_frontend_contract_v1(&encoded).unwrap(),
        contract
    );
}

#[test]
fn malformed_headers_and_every_truncation_fail_closed() {
    let encoded = encode_kernel_context_frontend_contract_v1(&fixture());
    for end in 0..encoded.len() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            decode_kernel_context_frontend_contract_v1(&encoded[..end])
        }))
        .expect("decoder must be total");
        assert!(result.is_err(), "prefix of length {end} was accepted");
    }

    let mut invalid = encoded.clone();
    invalid[0] ^= 1;
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::InvalidMagic)
    );

    let mut invalid = encoded.clone();
    invalid[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::UnknownVersion(
            2
        ))
    );

    let mut invalid = encoded.clone();
    invalid[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::UnsupportedFlags(1))
    );

    let mut invalid = encoded.clone();
    invalid[12..16].copy_from_slice(&31_u32.to_le_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::InvalidLength(
            31
        ))
    );

    let mut invalid = encoded.clone();
    invalid[26] = 1;
    assert!(matches!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::NonzeroReserved(
            _
        ))
    ));

    let mut invalid = encoded.clone();
    invalid.push(0);
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::TrailingBytes)
    );

    let mut invalid = encoded;
    invalid.resize(MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1 + 1, 0);
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::TooLarge)
    );
}

#[test]
fn hostile_count_ordinal_and_diagnostic_mutations_are_rejected() {
    let encoded = encode_kernel_context_frontend_contract_v1(&fixture());

    let mut invalid = encoded.clone();
    invalid[16..18].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::Validation(
            KernelContextFrontendContractValidationErrorV1::ItemCount(2)
        ))
    );

    let mut invalid = encoded.clone();
    invalid[18..20].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::Validation(
            KernelContextFrontendContractValidationErrorV1::ContextSourceOrdinal(1)
        ))
    );

    for count in [0_u16, 2, u16::MAX] {
        let mut invalid = encoded.clone();
        invalid[20..22].copy_from_slice(&count.to_le_bytes());
        assert_eq!(
            decode_kernel_context_frontend_contract_v1(&invalid),
            Err(KernelContextFrontendContractDecodeErrorV1::Validation(
                KernelContextFrontendContractValidationErrorV1::RequiredIssuanceCount(count)
            ))
        );
    }

    let mut invalid = encoded.clone();
    invalid[22..24].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::Validation(
            KernelContextFrontendContractValidationErrorV1::IssuanceDiagnosticVersion(2)
        ))
    );

    let mut invalid = encoded;
    invalid[32] ^= 1;
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid),
        Err(KernelContextFrontendContractDecodeErrorV1::Validation(
            KernelContextFrontendContractValidationErrorV1::IssuanceDiagnosticIdentity
        ))
    );
}

#[test]
fn duplicate_roles_names_and_identity_name_substitution_are_rejected() {
    let encoded = encode_kernel_context_frontend_contract_v1(&fixture());
    let offsets = entry_offsets(&encoded);

    let mut duplicate_role = encoded.clone();
    duplicate_role[offsets[1]..offsets[1] + 2].copy_from_slice(&1_u16.to_le_bytes());
    let identity = derive_kernel_context_generated_item_identity_v1(
        KernelContextGeneratedItemRoleV1::PhysicalKernelRoot,
        HELPER_NAME,
    );
    duplicate_role[offsets[1] + 4..offsets[1] + 36].copy_from_slice(identity.as_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&duplicate_role),
        Err(KernelContextFrontendContractDecodeErrorV1::Validation(
            KernelContextFrontendContractValidationErrorV1::DuplicateItemRole(
                KernelContextGeneratedItemRoleV1::PhysicalKernelRoot
            )
        ))
    );

    let mut duplicate_name = encoded.clone();
    let helper_name = offsets[1] + 40;
    duplicate_name[helper_name..helper_name + ROOT_NAME.len()]
        .copy_from_slice(ROOT_NAME.as_bytes());
    let identity = derive_kernel_context_generated_item_identity_v1(
        KernelContextGeneratedItemRoleV1::LogicalHelper,
        ROOT_NAME,
    );
    duplicate_name[offsets[1] + 4..offsets[1] + 36].copy_from_slice(identity.as_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&duplicate_name),
        Err(KernelContextFrontendContractDecodeErrorV1::Validation(
            KernelContextFrontendContractValidationErrorV1::DuplicateItemName
        ))
    );

    let mut substituted_identity = encoded;
    substituted_identity[offsets[0] + 4] ^= 1;
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&substituted_identity),
        Err(KernelContextFrontendContractDecodeErrorV1::Validation(
            KernelContextFrontendContractValidationErrorV1::ItemIdentityNameMismatch(
                KernelContextGeneratedItemRoleV1::PhysicalKernelRoot
            )
        ))
    );
}

#[test]
fn hostile_role_name_and_order_mutations_are_rejected() {
    let encoded = encode_kernel_context_frontend_contract_v1(&fixture());
    let offsets = entry_offsets(&encoded);

    let mut unknown_role = encoded.clone();
    unknown_role[offsets[0]..offsets[0] + 2].copy_from_slice(&4_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&unknown_role),
        Err(KernelContextFrontendContractDecodeErrorV1::UnknownItemRole(
            4
        ))
    );

    let mut invalid_name = encoded.clone();
    let name = "9oot_a";
    invalid_name[offsets[0] + 40..offsets[0] + 40 + name.len()].copy_from_slice(name.as_bytes());
    let identity = derive_kernel_context_generated_item_identity_v1(
        KernelContextGeneratedItemRoleV1::PhysicalKernelRoot,
        name,
    );
    invalid_name[offsets[0] + 4..offsets[0] + 36].copy_from_slice(identity.as_bytes());
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&invalid_name),
        Err(KernelContextFrontendContractDecodeErrorV1::Validation(
            KernelContextFrontendContractValidationErrorV1::InvalidItemName
        ))
    );

    let mut noncanonical_order = encoded;
    let entry_length = offsets[1] - offsets[0];
    assert_eq!(entry_length, offsets[2] - offsets[1]);
    for index in 0..entry_length {
        noncanonical_order.swap(offsets[0] + index, offsets[1] + index);
    }
    assert_eq!(
        decode_kernel_context_frontend_contract_v1(&noncanonical_order),
        Err(KernelContextFrontendContractDecodeErrorV1::NonCanonical)
    );
}

#[test]
fn constructors_reject_noncanonical_names_duplicates_and_counts() {
    for name in ["", "_", "9root", "root-name", "root::name", "root\u{e9}"] {
        assert!(
            KernelContextGeneratedItemV1::for_generated_name(
                KernelContextGeneratedItemRoleV1::PhysicalKernelRoot,
                name,
            )
            .is_err(),
            "noncanonical name {name:?} was accepted"
        );
    }
    assert_eq!(
        KernelContextGeneratedItemV1::for_generated_name(
            KernelContextGeneratedItemRoleV1::PhysicalKernelRoot,
            "a".repeat(MAX_KERNEL_CONTEXT_GENERATED_ITEM_NAME_BYTES_V1 + 1),
        ),
        Err(KernelContextFrontendContractValidationErrorV1::ItemNameTooLong)
    );

    let duplicate_names = [
        item(KernelContextGeneratedItemRoleV1::PhysicalKernelRoot, "same"),
        item(KernelContextGeneratedItemRoleV1::LogicalHelper, "same"),
        item(
            KernelContextGeneratedItemRoleV1::NominalKernelMarker,
            MARKER_NAME,
        ),
    ];
    assert_eq!(
        KernelContextFrontendContractV1::new(
            duplicate_names,
            0,
            KernelContextIssuanceDiagnosticV1::exact(),
            1,
        ),
        Err(KernelContextFrontendContractValidationErrorV1::DuplicateItemName)
    );

    assert_eq!(
        KernelContextIssuanceDiagnosticV1::new("lookalike", 1),
        Err(KernelContextFrontendContractValidationErrorV1::IssuanceDiagnosticIdentity)
    );
    assert_eq!(
        KernelContextIssuanceDiagnosticV1::new(KERNEL_CONTEXT_ISSUANCE_DIAGNOSTIC_IDENTITY_V1, 2,),
        Err(KernelContextFrontendContractValidationErrorV1::IssuanceDiagnosticVersion(2))
    );
}
