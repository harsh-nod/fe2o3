use super::*;

fn inputs(profile: ProductionAmdTargetProfileV1) -> NativeLoweringAssociationInputsV1 {
    let axis = |seed, len| TargetLineageIdentityV3::new([seed; 32], len).unwrap();
    NativeLoweringAssociationInputsV1 {
        final_native: InertNativeNeutralSubjectV1::new([1; 32], 11, [2; 32], 17).unwrap(),
        carrier: axis(3, 23),
        descriptor: axis(4, 29),
        pre_descriptor_llvm: axis(5, 31),
        final_llvm: axis(6, 37),
        module_handoff: axis(7, 41),
        profile,
    }
}

fn rehash(bytes: &mut [u8]) {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/NATIVE-F-TEXT-DESCRIPTOR-ASSOCIATION/V1\0");
    hash.update(320_u64.to_le_bytes());
    hash.update(&bytes[..320]);
    bytes[320..].copy_from_slice(&hash.finalize());
}

#[test]
fn native_lowering_v1_independent_literal_goldens_pin_all_identity_domains() {
    use crate::InertAmdgpuLoweringReceiptV3;
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    for (profile, terminal, raw, receipt) in [
        (
            ProductionAmdTargetProfileV1::Gfx942,
            "1b20a6cc7933ce78f65e94060c7430702ca2679ac4626db44f6f7f79ab9c5a10",
            "37ccc3fa763b67d4b95360733757ef8b557b52584ca59672470c1eb95925bf57",
            "c24b96a3a1748e6982736ec2f7c53be3e8342d369b6d46cec0742b4d069c83f7",
        ),
        (
            ProductionAmdTargetProfileV1::Gfx950,
            "cfa1d464a10631d9db87f3ec6d22b219caf483f93db42865fc8a76970c19806d",
            "51caefdfe9ec154062e37b1b4b9f51edf9573348f5de826e3677179df93f06ef",
            "ae2812e3ac10fb8d9b1606dba41b1db6ff62e6de0cebd7a0d4dfd17e59e63b8d",
        ),
    ] {
        let record = InertNativeLoweringAssociationV1::new(inputs(profile));
        assert_eq!(hex(&record.canonical_bytes()[320..]), terminal);
        assert_eq!(hex(&Sha256::digest(record.canonical_bytes())), raw);
        let wrapped =
            InertAmdgpuLoweringReceiptV3::from_canonical_preimage(record.canonical_bytes())
                .unwrap();
        assert_eq!(hex(wrapped.identity().sha256()), receipt);
        assert_eq!(wrapped.identity().byte_len(), 352);
    }
}

#[test]
fn native_lowering_v1_fixed_roles_and_profiles_are_inert() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let values = inputs(profile);
        let record = InertNativeLoweringAssociationV1::new(values);
        assert_eq!(record.canonical_bytes().len(), 352);
        assert_eq!(
            &record.canonical_bytes()[..16],
            b"F2NLOW1\0\x01\0\x01\0\x60\x01\0\0"
        );
        assert_eq!(&record.canonical_bytes()[18..24], &[12, 0, 6, 0, 0, 0]);
        assert_eq!(
            &record.canonical_bytes()[24..120],
            values.final_native.canonical_bytes()
        );
        for (i, axis) in [
            values.carrier,
            values.descriptor,
            values.pre_descriptor_llvm,
            values.final_llvm,
            values.module_handoff,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                &record.canonical_bytes()[120 + 40 * i..160 + 40 * i],
                &axis.encode()
            );
        }
        let decoded = InertNativeLoweringAssociationV1::decode(record.canonical_bytes()).unwrap();
        assert_eq!(decoded, record);
        assert_eq!(decoded.inputs(), values);
        assert!(!decoded.grants_authority());
    }
}

#[test]
fn native_lowering_v1_rejects_every_unsealed_byte_mutation_and_extent() {
    let record =
        InertNativeLoweringAssociationV1::new(inputs(ProductionAmdTargetProfileV1::Gfx942));
    for i in 0..352 {
        let mut bytes = record.canonical_bytes().to_vec();
        bytes[i] ^= 1;
        assert!(
            InertNativeLoweringAssociationV1::decode(&bytes).is_err(),
            "byte {i}"
        );
    }
    for i in 0..352 {
        assert!(matches!(
            InertNativeLoweringAssociationV1::decode(&record.canonical_bytes()[..i]),
            Err(NativeLoweringAssociationErrorV1::Length)
        ));
    }
    let mut trailing = record.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(InertNativeLoweringAssociationV1::decode(&trailing).is_err());
}

#[test]
fn native_lowering_v1_resealed_bad_headers_and_zero_coordinates_still_fail() {
    let record =
        InertNativeLoweringAssociationV1::new(inputs(ProductionAmdTargetProfileV1::Gfx942));
    for i in (0..24).chain(24..40) {
        let mut bytes = record.canonical_bytes().to_vec();
        bytes[i] ^= 0x80;
        rehash(&mut bytes);
        assert!(
            InertNativeLoweringAssociationV1::decode(&bytes).is_err(),
            "header {i}"
        );
    }
    for range in [
        40..48,
        48..80,
        80..88,
        88..120,
        120..152,
        152..160,
        160..192,
        192..200,
        200..232,
        232..240,
        240..272,
        272..280,
        280..312,
        312..320,
    ] {
        let mut bytes = record.canonical_bytes().to_vec();
        bytes[range.clone()].fill(0);
        rehash(&mut bytes);
        assert!(
            InertNativeLoweringAssociationV1::decode(&bytes).is_err(),
            "axis {range:?}"
        );
    }
}

#[test]
fn native_lowering_v1_resealing_different_content_is_not_semantic_admission() {
    let values = inputs(ProductionAmdTargetProfileV1::Gfx942);
    let record = InertNativeLoweringAssociationV1::new(values);
    for range in [
        40..48,
        48..80,
        80..88,
        88..120,
        120..152,
        152..160,
        160..192,
        192..200,
        200..232,
        232..240,
        240..272,
        272..280,
        280..312,
        312..320,
    ] {
        let mut bytes = record.canonical_bytes().to_vec();
        bytes[range.start] ^= 0x10;
        rehash(&mut bytes);
        let decoded = InertNativeLoweringAssociationV1::decode(&bytes).unwrap();
        assert_ne!(decoded.inputs(), values);
        assert!(!decoded.grants_authority());
    }
}
