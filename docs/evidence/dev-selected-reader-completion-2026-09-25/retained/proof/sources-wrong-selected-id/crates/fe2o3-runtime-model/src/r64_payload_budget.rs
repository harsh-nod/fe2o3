//! Pure payload accounting decisions. Not a proof of leases, atomics or native custody.

pub const fn r64_payload_reserve_v1(used: usize, charge: usize, capacity: usize) -> Option<usize> {
    match used.checked_add(charge) {
        Some(next) if next <= capacity => Some(next),
        _ => None,
    }
}

pub const fn r64_payload_release_v1(used: usize, charge: usize) -> Option<usize> {
    used.checked_sub(charge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r64_budget_arithmetic_matches_wide_integer_model() {
        let values = [0, 1, 2, 7, 64, usize::MAX / 2, usize::MAX - 1, usize::MAX];
        for used in values {
            for charge in values {
                for limit in values {
                    let sum = used as u128 + charge as u128;
                    let expected =
                        (sum <= limit as u128 && sum <= usize::MAX as u128).then_some(sum as usize);
                    assert_eq!(r64_payload_reserve_v1(used, charge, limit), expected);
                    if let Some(next) = expected {
                        assert_eq!(r64_payload_release_v1(next, charge), Some(used));
                    }
                }
                let expected = if charge <= used {
                    Some((used as u128 - charge as u128) as usize)
                } else {
                    None
                };
                assert_eq!(r64_payload_release_v1(used, charge), expected);
            }
        }
    }
}
