use super::*;

#[test]
fn ordinary_identity_is_byte_for_byte_unchanged() {
    for byte in 0..=255 {
        let original = [byte; 32];
        assert_eq!(
            dispatch_identity(original, Gfx942RuntimeInvocationBindingV1::OrdinaryV1),
            original
        );
    }
}

#[test]
fn conditional_identity_has_pinned_domain_family_and_payload_bytes() {
    let binding = Gfx942RuntimeInvocationBindingV1::ConditionalNominalV4 {
        contract_identity: [2; 32],
        premise_identity: [3; 32],
    };
    // Independent SHA-256 of domain || u16_le(4) || base || contract || premises.
    let expected = [
        0x6b, 0xb0, 0x66, 0xd8, 0x8f, 0x54, 0xab, 0xae, 0x74, 0xb3, 0x5f, 0xe8, 0x2f, 0x1d, 0x31,
        0xd1, 0x55, 0x07, 0x89, 0xc9, 0xc0, 0x60, 0x1e, 0xce, 0xc0, 0xac, 0x13, 0x36, 0x75, 0xf7,
        0x7a, 0x51,
    ];
    assert_eq!(dispatch_identity([1; 32], binding), expected);
    for (base, contract, premise) in [(4, 2, 3), (1, 4, 3), (1, 2, 4), (1, 3, 2)] {
        assert_ne!(
            dispatch_identity(
                [base; 32],
                Gfx942RuntimeInvocationBindingV1::ConditionalNominalV4 {
                    contract_identity: [contract; 32],
                    premise_identity: [premise; 32],
                }
            ),
            expected
        );
    }
    assert_ne!(
        dispatch_identity([1; 32], Gfx942RuntimeInvocationBindingV1::OrdinaryV1),
        expected
    );
}
