use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_rustc_front::*;

fn dimensions(value: [u32; 3]) -> FrontendWorkgroupDimensionsV1 {
    FrontendWorkgroupDimensionsV1::new(value).unwrap()
}

fn fixture() -> KernelFrontendContractV1 {
    KernelFrontendContractV1::new(
        Some(
            FrontendLaunchBoundsV1::new(
                Some(dimensions([256, 1, 1])),
                Some(dimensions([256, 1, 1])),
                Some(2),
            )
            .unwrap(),
        ),
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
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn canonical_round_trip_and_golden_are_stable() {
    const GOLDEN: &str = "4645324f334b46000100030040000000000000000700000000010000010000000100000000010000010000000100000002000000010005001500000000000000";
    let encoded = encode_kernel_frontend_contract_v1(fixture());
    assert_eq!(encoded.len(), MAX_FRONTEND_KERNEL_CONTRACT_BYTES_V1);
    assert_eq!(hex(&encoded), GOLDEN);
    let decoded = decode_kernel_frontend_contract_v1(&encoded).unwrap();
    assert_eq!(decoded, fixture());
    assert_eq!(encode_kernel_frontend_contract_v1(decoded), encoded);
    assert_eq!(
        decoded.unsafe_assembly().unwrap().target().canonical_name(),
        "gfx942"
    );
}

#[test]
fn old_frontend_unit_domain_remains_distinct() {
    assert_ne!(FRONTEND_UNIT_MAGIC_V1, FRONTEND_KERNEL_CONTRACT_MAGIC_V1);
    assert_eq!(FRONTEND_UNIT_VERSION_V1, 1);
    assert_eq!(FRONTEND_KERNEL_CONTRACT_VERSION_V1, 1);
    assert_eq!(
        KERNEL_FRONTEND_REGISTRATION_MAGIC_V1.to_le_bytes(),
        *b"FE2O3KFA"
    );
}

#[test]
fn constructors_reject_conflicts_and_unknown_authority_bits() {
    assert!(matches!(
        FrontendWorkgroupDimensionsV1::new([1_025, 1, 1]),
        Err(KernelFrontendContractValidationErrorV1::WorkgroupVolumeTooLarge(_))
    ));
    assert_eq!(
        FrontendLaunchBoundsV1::new(
            Some(dimensions([256, 1, 1])),
            Some(dimensions([64, 1, 1])),
            None,
        ),
        Err(KernelFrontendContractValidationErrorV1::RequiredExceedsMaximum)
    );
    assert_eq!(
        FrontendLaunchBoundsV1::new(Some(dimensions([64, 1, 1])), None, Some(2)),
        Err(KernelFrontendContractValidationErrorV1::OccupancyRequiresMaximum)
    );
    assert!(matches!(
        FrontendUnsafeAssemblyDeclarationV1::new(
            FrontendUnsafeAssemblyTargetV1::AmdGpuGfx942,
            0x8000,
            ASSEMBLY_OPTION_NOMEM_V1,
            0,
        ),
        Err(KernelFrontendContractValidationErrorV1::UnsupportedAssemblyOperands(_))
    ));
    assert_eq!(
        FrontendUnsafeAssemblyDeclarationV1::new(
            FrontendUnsafeAssemblyTargetV1::AmdGpuGfx942,
            ASSEMBLY_OPERAND_ADDRESS_V1,
            ASSEMBLY_OPTION_READONLY_V1,
            ASSEMBLY_EFFECT_WRITE_GLOBAL_V1,
        ),
        Err(KernelFrontendContractValidationErrorV1::AssemblyEffectsConflictWithOptions)
    );
}

#[test]
fn malformed_headers_and_fields_fail_closed() {
    let encoded = encode_kernel_frontend_contract_v1(fixture());
    for end in 0..encoded.len() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            decode_kernel_frontend_contract_v1(&encoded[..end])
        }))
        .expect("decoder must be total");
        assert!(result.is_err());
    }

    let mut invalid = encoded.clone();
    invalid[0] ^= 1;
    assert_eq!(
        decode_kernel_frontend_contract_v1(&invalid),
        Err(KernelFrontendContractDecodeErrorV1::InvalidMagic)
    );
    let mut invalid = encoded.clone();
    invalid[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_frontend_contract_v1(&invalid),
        Err(KernelFrontendContractDecodeErrorV1::UnknownVersion(2))
    );
    let mut invalid = encoded.clone();
    invalid[10..12].copy_from_slice(&0x8000_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_frontend_contract_v1(&invalid),
        Err(KernelFrontendContractDecodeErrorV1::UnsupportedFlags(
            0x8000
        ))
    );
    let mut invalid = encoded.clone();
    invalid[16] = 1;
    assert!(matches!(
        decode_kernel_frontend_contract_v1(&invalid),
        Err(KernelFrontendContractDecodeErrorV1::NonzeroReserved(_))
    ));
    let mut invalid = encoded.clone();
    invalid[20..22].copy_from_slice(&0x8000_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_frontend_contract_v1(&invalid),
        Err(KernelFrontendContractDecodeErrorV1::UnsupportedFlags(
            0x8000
        ))
    );
    let mut invalid = encoded.clone();
    invalid[52..54].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        decode_kernel_frontend_contract_v1(&invalid),
        Err(KernelFrontendContractDecodeErrorV1::UnknownTag { .. })
    ));
    let launch_only = KernelFrontendContractV1::new(fixture().launch(), None).unwrap();
    let mut invalid = encode_kernel_frontend_contract_v1(launch_only);
    invalid.push(0);
    assert_eq!(
        decode_kernel_frontend_contract_v1(&invalid),
        Err(KernelFrontendContractDecodeErrorV1::TrailingBytes)
    );

    let mut invalid = encoded;
    invalid.push(0);
    assert_eq!(
        decode_kernel_frontend_contract_v1(&invalid),
        Err(KernelFrontendContractDecodeErrorV1::TooLarge)
    );
}
