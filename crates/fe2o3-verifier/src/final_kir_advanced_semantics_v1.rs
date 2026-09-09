//! Closed semantic constants shared by direct final-KIR proofs.
//!
//! This module contains no evaluator and issues no authority. The canonical
//! symbolic executor remains `final_kir_output_equivalence_v1`; these helpers
//! only encode typed V13 metadata and the Verus definitions used by it.

use std::collections::BTreeSet;

use fe2o3_kernel_ir::{AddressSpace, MemoryOrdering, ScalarType};

pub(super) const fn float_layout_v1(scalar: ScalarType) -> Option<(u16, u16)> {
    match scalar {
        ScalarType::F16 => Some((5, 10)),
        ScalarType::Bf16 => Some((8, 7)),
        ScalarType::F32 => Some((8, 23)),
        ScalarType::F64 => Some((11, 52)),
        _ => None,
    }
}

pub(super) const fn address_space_tag_v1(space: AddressSpace) -> u8 {
    match space {
        AddressSpace::Private => 0,
        AddressSpace::Workgroup => 1,
        AddressSpace::Global => 2,
        AddressSpace::Constant => 3,
        AddressSpace::Generic => 4,
    }
}

pub(super) fn address_space_mask_v1(spaces: &BTreeSet<AddressSpace>) -> u8 {
    spaces.iter().fold(0, |mask, space| {
        mask | match space {
            AddressSpace::Private => 1,
            AddressSpace::Workgroup => 2,
            AddressSpace::Global => 4,
            AddressSpace::Constant => 8,
            AddressSpace::Generic => 16,
        }
    })
}

pub(super) const fn memory_ordering_tag_v1(ordering: MemoryOrdering) -> u8 {
    match ordering {
        MemoryOrdering::Relaxed => 0,
        MemoryOrdering::Acquire => 1,
        MemoryOrdering::Release => 2,
        MemoryOrdering::AcquireRelease => 3,
        MemoryOrdering::SequentiallyConsistent => 4,
    }
}

pub(super) fn render_wave_reduce_max_f32_v1(mut lanes: Vec<String>) -> Option<Vec<String>> {
    if lanes.is_empty() || !lanes.len().is_power_of_two() {
        return None;
    }
    let mut distance = 1;
    while distance < lanes.len() {
        let previous = lanes;
        lanes = (0..previous.len())
            .map(|lane| {
                let lhs = &previous[lane];
                let rhs = &previous[lane ^ distance];
                format!(
                    "if fe2o3_ieee_compare_v1(2, 8, 23, {lhs}, {rhs}) == 1 {{ {rhs} }} else {{ {lhs} }}"
                )
            })
            .collect();
        distance *= 2;
    }
    Some(lanes)
}

pub(super) const STRICT_FLOAT_BIT_SEMANTICS_V1: &str = r#"
    pub open spec fn fe2o3_ieee_is_nan_v1(bits: int, exponent_bits: nat, fraction_bits: nat) -> bool {
        let fraction_modulus = fe2o3_bv_modulus_v2(fraction_bits);
        let exponent_modulus = fe2o3_bv_modulus_v2(exponent_bits);
        ((bits / fraction_modulus) % exponent_modulus == exponent_modulus - 1)
            && (bits % fraction_modulus != 0)
    }

    pub open spec fn fe2o3_ieee_equal_v1(lhs: int, rhs: int, exponent_bits: nat, fraction_bits: nat) -> bool {
        let sign = fe2o3_bv_modulus_v2(exponent_bits + fraction_bits);
        if fe2o3_ieee_is_nan_v1(lhs, exponent_bits, fraction_bits)
            || fe2o3_ieee_is_nan_v1(rhs, exponent_bits, fraction_bits) {
            false
        } else if lhs % sign == 0 && rhs % sign == 0 {
            true
        } else {
            lhs == rhs
        }
    }

    pub open spec fn fe2o3_ieee_less_v1(lhs: int, rhs: int, exponent_bits: nat, fraction_bits: nat) -> bool {
        let sign = fe2o3_bv_modulus_v2(exponent_bits + fraction_bits);
        let lhs_negative = lhs >= sign;
        let rhs_negative = rhs >= sign;
        if fe2o3_ieee_is_nan_v1(lhs, exponent_bits, fraction_bits)
            || fe2o3_ieee_is_nan_v1(rhs, exponent_bits, fraction_bits)
            || (lhs % sign == 0 && rhs % sign == 0) {
            false
        } else if lhs_negative != rhs_negative {
            lhs_negative
        } else if lhs_negative {
            lhs % sign > rhs % sign
        } else {
            lhs < rhs
        }
    }

    pub open spec fn fe2o3_ieee_compare_v1(operation: int, exponent_bits: nat, fraction_bits: nat, lhs: int, rhs: int) -> int {
        let equal = fe2o3_ieee_equal_v1(lhs, rhs, exponent_bits, fraction_bits);
        let less = fe2o3_ieee_less_v1(lhs, rhs, exponent_bits, fraction_bits);
        let greater = fe2o3_ieee_less_v1(rhs, lhs, exponent_bits, fraction_bits);
        if (operation == 0 && equal)
            || (operation == 1 && !equal)
            || (operation == 2 && less)
            || (operation == 3 && (less || equal))
            || (operation == 4 && greater)
            || (operation == 5 && (greater || equal)) {
            1
        } else {
            0
        }
    }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_encodings_are_closed_and_collision_free() {
        assert_eq!(float_layout_v1(ScalarType::F32), Some((8, 23)));
        assert_eq!(float_layout_v1(ScalarType::U32), None);
        assert_eq!(address_space_tag_v1(AddressSpace::Global), 2);
        assert_eq!(
            address_space_mask_v1(&BTreeSet::from([
                AddressSpace::Workgroup,
                AddressSpace::Global,
            ])),
            6,
        );
        assert_ne!(
            memory_ordering_tag_v1(MemoryOrdering::Acquire),
            memory_ordering_tag_v1(MemoryOrdering::Release),
        );
        let reduced = render_wave_reduce_max_f32_v1(vec![
            "a".to_owned(),
            "b".to_owned(),
            "c".to_owned(),
            "d".to_owned(),
        ])
        .unwrap();
        assert_eq!(reduced.len(), 4);
        assert!(reduced[0].contains("fe2o3_ieee_compare_v1"));
        assert!(render_wave_reduce_max_f32_v1(Vec::new()).is_none());
    }
}
