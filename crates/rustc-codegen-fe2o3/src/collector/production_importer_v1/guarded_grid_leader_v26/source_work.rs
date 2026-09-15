use super::{ProductionSemanticImportErrorV1, rejected};

pub(super) fn spend(
    work: &mut usize,
    amount: usize,
) -> Result<(), ProductionSemanticImportErrorV1> {
    *work = work
        .checked_sub(amount)
        .ok_or_else(|| rejected("guarded Grid leader source-work exhausted"))?;
    Ok(())
}

pub(super) fn reserve<T>(
    count: usize,
    work: &mut usize,
) -> Result<Vec<T>, ProductionSemanticImportErrorV1> {
    let words = std::mem::size_of::<T>()
        .div_ceil(std::mem::size_of::<usize>())
        .max(1);
    spend(
        work,
        count
            .checked_mul(words)
            .ok_or_else(|| rejected("guarded Grid leader source-work overflow"))?,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| rejected("guarded Grid leader source allocation"))?;
    spend(
        work,
        values
            .capacity()
            .saturating_sub(count)
            .checked_mul(words)
            .ok_or_else(|| rejected("guarded Grid leader source-work overflow"))?,
    )?;
    Ok(values)
}

// The caller precharges the complete borrowed roster. No collection or growth.
pub(super) fn one<'a, T: 'a>(
    mut values: impl Iterator<Item = &'a T>,
    detail: &'static str,
) -> Result<&'a T, ProductionSemanticImportErrorV1> {
    let value = values.next().ok_or_else(|| rejected(detail))?;
    if values.next().is_some() {
        return Err(rejected(detail));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarded_grid_source_buffer_precharge_rejects_before_reserve() {
        let mut work = 31;
        let result = reserve::<[usize; 2]>(16, &mut work);
        assert!(matches!(
            result,
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "guarded Grid leader source-work exhausted"
            ))
        ));
        assert_eq!(work, 31);
        let mut exact = 32;
        let values = reserve::<[usize; 2]>(16, &mut exact).unwrap();
        assert_eq!(values.capacity(), 16);
        assert_eq!(exact, 0);
    }

    #[test]
    fn guarded_grid_source_buffer_overflow_is_not_a_fresh_allowance() {
        let mut work = 100;
        assert!(matches!(
            reserve::<[usize; 2]>(usize::MAX, &mut work),
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "guarded Grid leader source-work overflow"
            ))
        ));
        assert_eq!(work, 100);
    }

    #[test]
    fn guarded_grid_source_rosters_preserve_shared_remaining_work() {
        let mut work = 47;
        let first = reserve::<usize>(32, &mut work).unwrap();
        assert_eq!(first.capacity(), 32);
        assert_eq!(work, 15);
        assert!(matches!(
            reserve::<usize>(16, &mut work),
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "guarded Grid leader source-work exhausted"
            ))
        ));
        assert_eq!(work, 15);
    }

    #[test]
    fn guarded_grid_unique_roster_is_borrowed_and_rejects_second_match() {
        let rows = [0, 1, 2];
        let found = one(rows.iter().filter(|&&n| n == 1), "unique").unwrap();
        assert!(std::ptr::eq(found, &rows[1]));
        assert!(matches!(
            one(rows.iter().filter(|&&n| n > 0), "unique"),
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "unique"
            ))
        ));
        assert!(matches!(
            one(rows.iter().filter(|&&n| n > 2), "unique"),
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "unique"
            ))
        ));
    }
}
