use fe2o3_contracts::{
    KERNEL_IDENTITY_COMPONENT_BYTES_V1, KERNEL_IDENTITY_VERSION_V1,
    KERNEL_INST_ID_CANONICAL_BYTES_V1, KERNEL_INST_ID_MAGIC_V1, KERNEL_ITEM_ID_CANONICAL_BYTES_V1,
    KERNEL_ITEM_ID_MAGIC_V1, KernelCfgIdentityV1, KernelConstArgumentsIdentityV1,
    KernelCrateIdentityV1, KernelGenericDefinitionIdentityV1, KernelInstId,
    KernelInstIdDecodeErrorV1, KernelItemId, KernelItemIdDecodeErrorV1, KernelRustItemIdentityV1,
    KernelTypeArgumentsIdentityV1, decode_kernel_inst_id, decode_kernel_item_id,
    encode_kernel_inst_id, encode_kernel_item_id,
};

fn item() -> KernelItemId {
    KernelItemId::new(
        KernelCrateIdentityV1::from_untrusted_bytes([0x11; 32]),
        KernelRustItemIdentityV1::from_untrusted_bytes([0x22; 32]),
        KernelGenericDefinitionIdentityV1::from_untrusted_bytes([0x33; 32]),
    )
}

fn instance() -> KernelInstId {
    KernelInstId::new(
        item(),
        KernelTypeArgumentsIdentityV1::from_untrusted_bytes([0x44; 32]),
        KernelConstArgumentsIdentityV1::from_untrusted_bytes([0x55; 32]),
        KernelCfgIdentityV1::from_untrusted_bytes([0x66; 32]),
    )
}

#[test]
fn item_codec_is_fixed_width_deterministic_and_round_trippable() {
    let id = item();
    let bytes = id.encode_canonical();

    assert_eq!(bytes, encode_kernel_item_id(id));
    assert_eq!(bytes.len(), KERNEL_ITEM_ID_CANONICAL_BYTES_V1);
    assert_eq!(&bytes[..8], &KERNEL_ITEM_ID_MAGIC_V1);
    assert_eq!(
        u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        KERNEL_IDENTITY_VERSION_V1
    );
    assert_eq!(&bytes[10..12], &[0, 0]);
    assert_eq!(
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        (3 * KERNEL_IDENTITY_COMPONENT_BYTES_V1) as u32
    );
    assert_eq!(&bytes[16..48], &[0x11; 32]);
    assert_eq!(&bytes[48..80], &[0x22; 32]);
    assert_eq!(&bytes[80..112], &[0x33; 32]);
    assert_eq!(KernelItemId::decode_canonical(&bytes).unwrap(), id);
    assert_eq!(decode_kernel_item_id(&bytes).unwrap(), id);
}

#[test]
fn instance_codec_nests_the_complete_item_and_separates_arguments() {
    let id = instance();
    let bytes = id.encode_canonical();
    let item_bytes = item().encode_canonical();

    assert_eq!(bytes, encode_kernel_inst_id(id));
    assert_eq!(bytes.len(), KERNEL_INST_ID_CANONICAL_BYTES_V1);
    assert_eq!(&bytes[..8], &KERNEL_INST_ID_MAGIC_V1);
    assert_eq!(
        u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        KERNEL_IDENTITY_VERSION_V1
    );
    assert_eq!(&bytes[10..12], &[0, 0]);
    assert_eq!(
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        (KERNEL_ITEM_ID_CANONICAL_BYTES_V1 + 3 * KERNEL_IDENTITY_COMPONENT_BYTES_V1) as u32
    );
    assert_eq!(&bytes[16..128], &item_bytes);
    assert_eq!(&bytes[128..160], &[0x44; 32]);
    assert_eq!(&bytes[160..192], &[0x55; 32]);
    assert_eq!(&bytes[192..224], &[0x66; 32]);
    assert_eq!(KernelInstId::decode_canonical(&bytes).unwrap(), id);
    assert_eq!(decode_kernel_inst_id(&bytes).unwrap(), id);
}

#[test]
fn item_decoder_rejects_malformed_envelopes() {
    let bytes = item().encode_canonical();
    assert_eq!(
        decode_kernel_item_id(&bytes[..15]),
        Err(KernelItemIdDecodeErrorV1::Truncated {
            actual: 15,
            expected: 16,
        })
    );
    assert_eq!(
        decode_kernel_item_id(&bytes[..bytes.len() - 1]),
        Err(KernelItemIdDecodeErrorV1::Truncated {
            actual: KERNEL_ITEM_ID_CANONICAL_BYTES_V1 - 1,
            expected: KERNEL_ITEM_ID_CANONICAL_BYTES_V1,
        })
    );

    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_kernel_item_id(&trailing),
        Err(KernelItemIdDecodeErrorV1::TrailingBytes {
            actual: KERNEL_ITEM_ID_CANONICAL_BYTES_V1 + 1,
            expected: KERNEL_ITEM_ID_CANONICAL_BYTES_V1,
        })
    );

    let mut bad_magic = bytes;
    bad_magic[0] ^= 1;
    assert_eq!(
        decode_kernel_item_id(&bad_magic),
        Err(KernelItemIdDecodeErrorV1::InvalidMagic)
    );

    let mut bad_version = bytes;
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_item_id(&bad_version),
        Err(KernelItemIdDecodeErrorV1::UnknownVersion(2))
    );

    let mut bad_flags = bytes;
    bad_flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_item_id(&bad_flags),
        Err(KernelItemIdDecodeErrorV1::UnsupportedFlags(1))
    );

    let mut bad_length = bytes;
    bad_length[12..16].copy_from_slice(&95_u32.to_le_bytes());
    assert_eq!(
        decode_kernel_item_id(&bad_length),
        Err(KernelItemIdDecodeErrorV1::InvalidPayloadLength {
            actual: 95,
            expected: 96,
        })
    );
}

#[test]
fn instance_decoder_rejects_malformed_and_ambiguous_envelopes() {
    let bytes = instance().encode_canonical();
    assert_eq!(
        decode_kernel_inst_id(&bytes[..15]),
        Err(KernelInstIdDecodeErrorV1::Truncated {
            actual: 15,
            expected: 16,
        })
    );
    assert_eq!(
        decode_kernel_inst_id(&bytes[..bytes.len() - 1]),
        Err(KernelInstIdDecodeErrorV1::Truncated {
            actual: KERNEL_INST_ID_CANONICAL_BYTES_V1 - 1,
            expected: KERNEL_INST_ID_CANONICAL_BYTES_V1,
        })
    );

    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_kernel_inst_id(&trailing),
        Err(KernelInstIdDecodeErrorV1::TrailingBytes {
            actual: KERNEL_INST_ID_CANONICAL_BYTES_V1 + 1,
            expected: KERNEL_INST_ID_CANONICAL_BYTES_V1,
        })
    );

    let mut bad_magic = bytes;
    bad_magic[0] ^= 1;
    assert_eq!(
        decode_kernel_inst_id(&bad_magic),
        Err(KernelInstIdDecodeErrorV1::InvalidMagic)
    );

    let mut bad_version = bytes;
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_inst_id(&bad_version),
        Err(KernelInstIdDecodeErrorV1::UnknownVersion(2))
    );

    let mut bad_flags = bytes;
    bad_flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_kernel_inst_id(&bad_flags),
        Err(KernelInstIdDecodeErrorV1::UnsupportedFlags(1))
    );

    let mut bad_length = bytes;
    bad_length[12..16].copy_from_slice(&207_u32.to_le_bytes());
    assert_eq!(
        decode_kernel_inst_id(&bad_length),
        Err(KernelInstIdDecodeErrorV1::InvalidPayloadLength {
            actual: 207,
            expected: 208,
        })
    );

    let mut malformed_item = bytes;
    malformed_item[16] ^= 1;
    assert_eq!(
        decode_kernel_inst_id(&malformed_item),
        Err(KernelInstIdDecodeErrorV1::InvalidKernelItem(
            KernelItemIdDecodeErrorV1::InvalidMagic
        ))
    );

    assert_eq!(
        decode_kernel_item_id(&bytes),
        Err(KernelItemIdDecodeErrorV1::InvalidMagic)
    );
    assert_eq!(
        decode_kernel_inst_id(&item().encode_canonical()),
        Err(KernelInstIdDecodeErrorV1::InvalidMagic)
    );
}

#[test]
fn every_accepted_mutation_has_one_exact_reencoding() {
    let item_bytes = item().encode_canonical();
    for index in 0..item_bytes.len() {
        let mut mutated = item_bytes;
        mutated[index] ^= 1;
        if let Ok(decoded) = decode_kernel_item_id(&mutated) {
            assert_eq!(decoded.encode_canonical(), mutated, "item byte {index}");
        }
    }

    let instance_bytes = instance().encode_canonical();
    for index in 0..instance_bytes.len() {
        let mut mutated = instance_bytes;
        mutated[index] ^= 1;
        if let Ok(decoded) = decode_kernel_inst_id(&mutated) {
            assert_eq!(decoded.encode_canonical(), mutated, "instance byte {index}");
        }
    }
}

#[test]
fn property_specific_mutations_change_only_their_identity_layer() {
    let base_item = item();
    let other_item = KernelItemId::new(
        base_item.crate_identity(),
        base_item.rust_item_identity(),
        KernelGenericDefinitionIdentityV1::from_untrusted_bytes([0x34; 32]),
    );
    assert_ne!(base_item, other_item);

    let base = instance();
    let other_type = KernelInstId::new(
        base.item(),
        KernelTypeArgumentsIdentityV1::from_untrusted_bytes([0x45; 32]),
        base.const_arguments_identity(),
        base.cfg_identity(),
    );
    let other_const = KernelInstId::new(
        base.item(),
        base.type_arguments_identity(),
        KernelConstArgumentsIdentityV1::from_untrusted_bytes([0x56; 32]),
        base.cfg_identity(),
    );
    let other_cfg = KernelInstId::new(
        base.item(),
        base.type_arguments_identity(),
        base.const_arguments_identity(),
        KernelCfgIdentityV1::from_untrusted_bytes([0x67; 32]),
    );
    let other_definition = KernelInstId::new(
        other_item,
        base.type_arguments_identity(),
        base.const_arguments_identity(),
        base.cfg_identity(),
    );

    for distinct in [other_type, other_const, other_cfg, other_definition] {
        assert_ne!(base, distinct);
        assert_ne!(base.encode_canonical(), distinct.encode_canonical());
    }
}
