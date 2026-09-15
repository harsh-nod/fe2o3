use super::*;

#[test]
fn windows_preserve_interior_zeroes_and_each_dense_word() {
    let (windowed, mut storage) = DefinitionRows::layout(4, 16, 12).unwrap();
    assert!(windowed);
    let mut rows = DefinitionRows::try_new(4, 16, windowed).unwrap();
    for row in 0..4 {
        let mut scratch = vec![0; 16];
        scratch[row + 2] = 1 << row;
        scratch[row + 4] = u64::MAX;
        rows.finish_window(row, &scratch, Some(row + 2..row + 5), &mut storage, 100)
            .unwrap();
        for (word, expected) in scratch.iter().copied().enumerate() {
            assert_eq!(rows.word(row, word), expected);
        }
    }
    assert_eq!(storage, 4 * size_of::<WordWindow>() / 8 + 12);
}

#[test]
fn empty_rows_are_zero_without_payload_allocations() {
    let (windowed, mut storage) = DefinitionRows::layout(5, 16, 0).unwrap();
    assert!(windowed);
    let original = storage;
    let mut rows = DefinitionRows::try_new(5, 16, windowed).unwrap();
    for row in 0..5 {
        rows.finish_window(row, &[0; 16], None, &mut storage, original)
            .unwrap();
        for word in 0..16 {
            assert_eq!(rows.word(row, word), 0);
        }
    }
    assert_eq!(storage, original);
}

#[test]
fn small_or_full_width_rows_fall_back_to_original_dense_representation() {
    for width in [0, 1, 2, 3, 8, 129] {
        let (windowed, storage) = DefinitionRows::layout(7, width, 7 * width).unwrap();
        assert!(!windowed);
        assert_eq!(storage, 7 * width);
        let mut rows = DefinitionRows::try_new(7, width, windowed).unwrap();
        if width > 0 {
            rows.insert_dense(3, width * 64 - 1);
            assert_eq!(rows.word(3, width - 1), 1 << 63);
        }
    }
}

#[test]
fn word_span_bound_covers_sparse_promotable_rank_and_boundary_alignment() {
    for locals in [1, 63, 64, 65, 129, 1025] {
        for stride in [1, 2, 7, 63, 64, 65] {
            let promoted = (0..locals)
                .filter(|local| local % stride == 0 || *local + 1 == locals)
                .collect::<Vec<_>>();
            let width = promoted.len().div_ceil(64);
            for first in 0..promoted.len() {
                for last in first..promoted.len() {
                    let actual = last / 64 - first / 64 + 1;
                    assert!(actual <= word_span_upper(promoted[first], promoted[last], width));
                }
            }
        }
    }
}

#[test]
fn payload_limit_is_inclusive_and_rejection_keeps_the_row_unmodified() {
    let (windowed, base) = DefinitionRows::layout(4, 16, 8).unwrap();
    assert!(windowed);
    for limit in [base + 1, base + 2] {
        let mut rows = DefinitionRows::try_new(4, 16, windowed).unwrap();
        let mut storage = base;
        let result = rows.finish_window(1, &[0, 7, 9, 0], Some(1..3), &mut storage, limit);
        if limit == base + 1 {
            assert!(
                matches!(result, Err(SsaPlannerErrorV1::ResourceLimitExceeded { required, limit: actual, .. })
                if required == base + 2 && actual == limit)
            );
            assert_eq!(rows.word(1, 1), 0);
            assert_eq!(storage, base);
        } else {
            result.unwrap();
            assert_eq!(storage, base + 2);
            assert_eq!(rows.word(1, 1), 7);
            assert_eq!(rows.word(1, 2), 9);
        }
    }
}

#[test]
fn layout_arithmetic_remains_checked() {
    assert!(matches!(
        DefinitionRows::layout(usize::MAX, 2, 0),
        Err(SsaPlannerErrorV1::IdentityOverflow)
    ));
    assert!(matches!(
        DefinitionRows::layout(1, 8, usize::MAX),
        Err(SsaPlannerErrorV1::IdentityOverflow)
    ));
}
