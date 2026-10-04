//! Inert construction controls only, not genuine source occurrence evidence.

use super::*;

#[test]
fn every_physical_binding_is_bounded_and_pairwise_distinct() {
    for registers in [[32, 33, 34, 35, 36], [0, 63, 1, 62, 2], [63, 0, 62, 1, 61]] {
        let value = SemanticGfx942OrderedRegionRegistersV31::new(
            registers[0],
            registers[1],
            [registers[2], registers[3], registers[4]],
        )
        .unwrap();
        assert_eq!(value.scratch(), registers[0]);
        assert_eq!(value.output(), registers[1]);
        assert_eq!(value.inputs(), [registers[2], registers[3], registers[4]]);
        assert_eq!(
            value.vgpr_high_water(),
            registers.into_iter().max().unwrap() + 1
        );
    }
    for left in 0..5 {
        for right in left + 1..5 {
            let mut registers = [0, 1, 2, 3, 4];
            registers[right] = registers[left];
            assert_eq!(
                SemanticGfx942OrderedRegionRegistersV31::new(
                    registers[0],
                    registers[1],
                    [registers[2], registers[3], registers[4]],
                ),
                Err(SemanticMirErrorV1::InvalidOrderedRegionV31)
            );
        }
    }
    for slot in 0..5 {
        for invalid in 64..=255_u8 {
            let mut registers = [0, 1, 2, 3, 4];
            registers[slot] = invalid;
            assert!(
                SemanticGfx942OrderedRegionRegistersV31::new(
                    registers[0],
                    registers[1],
                    [registers[2], registers[3], registers[4]],
                )
                .is_err()
            );
        }
    }
}

#[test]
fn source_references_and_profile_are_closed_but_not_authenticated() {
    let source = SemanticOrderedRegionSourceV31::new(
        [1; 32],
        SemanticFunctionIdentityV1([2; 32]),
        [3; 32],
        [4; 32],
    )
    .unwrap();
    assert_eq!(source.frontend_unit(), [1; 32]);
    assert_eq!(source.function().as_bytes(), &[2; 32]);
    assert_eq!(source.contract(), [3; 32]);
    assert_eq!(source.statement(), [4; 32]);
    for slot in 0..4 {
        let mut references = [[1; 32], [2; 32], [3; 32], [4; 32]];
        references[slot] = [0; 32];
        assert_eq!(
            SemanticOrderedRegionSourceV31::new(
                references[0],
                SemanticFunctionIdentityV1(references[1]),
                references[2],
                references[3],
            ),
            Err(SemanticMirErrorV1::InvalidOrderedRegionV31)
        );
    }
    assert_eq!(
        SemanticGfx942OrderedRegionProfileV31::XorAddU32E32.wire_tag(),
        0
    );
    assert_eq!(
        SemanticGfx942OrderedRegionProfileV31::from_wire_tag(0),
        Some(SemanticGfx942OrderedRegionProfileV31::XorAddU32E32)
    );
    for invalid in 1..=255 {
        assert_eq!(
            SemanticGfx942OrderedRegionProfileV31::from_wire_tag(invalid),
            None
        );
    }
}
