use super::*;

#[test]
fn journal_layout_shrinks_real_records_without_narrowing_ssa_values() {
    assert_eq!(
        size_of::<ScopedChange>(),
        size_of::<(u32, Option<SsaValueV1>)>()
    );
    assert!(size_of::<ScopedChange>() <= size_of::<(usize, Option<SsaValueV1>)>());
    if usize::BITS == 64 {
        assert_eq!(size_of::<ScopedChange>(), 16);
        assert_eq!(size_of::<(usize, Option<SsaValueV1>)>(), 24);
    }
}

#[test]
fn journal_checked_index_and_all_value_bits_round_trip() {
    let values = [
        None,
        Some(SsaValueV1::Definition(SsaDefinitionIdV1::new(u32::MAX))),
        Some(SsaValueV1::BlockArgument {
            block: SsaBlockIdV1::new(u32::MAX),
            variable: SsaVariableIdV1::new(u32::MAX),
        }),
    ];
    for index in [0, 1, HARD_MAX_SSA_VARIABLES_V1 - 1, u32::MAX as usize] {
        for previous in values {
            let item = ScopedChange::new(index, previous).unwrap();
            assert_eq!(item.variable as usize, index);
            assert_eq!(item.previous, previous);
        }
    }
}

#[test]
fn journal_unrepresentable_index_is_not_truncated() {
    if usize::BITS > 32 {
        assert!(matches!(
            ScopedChange::new(u32::MAX as usize + 1, None),
            Err(SsaPlannerErrorV1::IdentityOverflow)
        ));
        assert!(matches!(
            ScopedChange::new(usize::MAX, None),
            Err(SsaPlannerErrorV1::IdentityOverflow)
        ));
    }
}

#[test]
fn journal_keeps_each_repeated_change_and_restores_nested_scopes() {
    let mut old_values = vec![None; 3];
    let mut new_values = old_values.clone();
    let mut old = Vec::new();
    let mut new = Vec::new();
    let mut scopes = Vec::new();
    for depth in 0..16 {
        scopes.push(old.len());
        for step in 0..19 {
            let index = (depth + step) % old_values.len();
            let value = match step % 3 {
                0 => None,
                1 => Some(SsaValueV1::Definition(SsaDefinitionIdV1::new(
                    (depth * 19 + step) as u32,
                ))),
                _ => Some(SsaValueV1::BlockArgument {
                    block: SsaBlockIdV1::new(depth as u32),
                    variable: SsaVariableIdV1::new(index as u32),
                }),
            };
            old.push((index, old_values[index]));
            new.push(ScopedChange::new(index, new_values[index]).unwrap());
            old_values[index] = value;
            new_values[index] = value;
        }
    }
    assert_eq!(old.len(), 16 * 19, "no journal entries coalesced");
    assert_eq!(new.len(), old.len());
    while let Some(restore) = scopes.pop() {
        while old.len() > restore {
            let (index, value) = old.pop().unwrap();
            let change = new.pop().unwrap();
            assert_eq!((change.variable as usize, change.previous), (index, value));
            old_values[index] = value;
            new_values[change.variable as usize] = change.previous;
            assert_eq!(old_values, new_values);
        }
    }
    assert_eq!(new_values, [None; 3]);
}

#[test]
fn journal_allocation_retains_the_same_capacity_bound() {
    for count in [0, 1, 63, 64, 65, 4097] {
        let old = Vec::<(usize, Option<SsaValueV1>)>::with_capacity(count);
        let new = Vec::<ScopedChange>::with_capacity(count);
        assert_eq!(old.capacity(), count);
        assert_eq!(new.capacity(), count);
        if usize::BITS == 64 {
            assert_eq!(
                old.capacity() * size_of::<(usize, Option<SsaValueV1>)>()
                    - new.capacity() * size_of::<ScopedChange>(),
                count * 8
            );
        }
    }
}
