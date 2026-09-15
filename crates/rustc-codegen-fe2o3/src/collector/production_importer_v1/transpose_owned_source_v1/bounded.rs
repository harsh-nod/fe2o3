use super::{Error, Result};

pub(super) const MAX_DEPTH: usize = 128;

pub(super) fn charge(work: &mut usize, amount: usize) -> Result<()> {
    *work = work.checked_sub(amount).ok_or(Error::Work)?;
    Ok(())
}

// Logical retained words, not an allocator/physical-memory cap. Traversal work
// is charged separately, and a failed construction never refunds either charge.
pub(super) fn reserve<T>(out: &mut Vec<T>, count: usize, work: &mut usize) -> Result<()> {
    if count != 0 && out.capacity() == 0 {
        charge(
            work,
            std::mem::size_of::<Vec<T>>().div_ceil(std::mem::size_of::<usize>()),
        )?;
    }
    let words = std::mem::size_of::<T>().div_ceil(std::mem::size_of::<usize>());
    charge(work, words.checked_mul(count).ok_or(Error::Work)?)?;
    out.try_reserve_exact(count).map_err(|_| Error::Allocation)
}

pub(super) fn search_work(len: usize) -> usize {
    1 + (usize::BITS - len.leading_zeros()) as usize
}

pub(super) fn insert_unique<T: Ord>(out: &mut Vec<T>, value: T, work: &mut usize) -> Result<()> {
    insert_unique_by(out, value, T::cmp, work)
}

pub(super) fn insert_unique_by<T>(
    out: &mut Vec<T>,
    value: T,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
    work: &mut usize,
) -> Result<()> {
    charge(work, search_work(out.len()))?;
    let index = out
        .binary_search_by(|before| compare(before, &value))
        .err()
        .ok_or(Error::Source("duplicate source occurrence key"))?;
    // Charge element shifts as logical words before mutating the vector.
    let words = std::mem::size_of::<T>().div_ceil(std::mem::size_of::<usize>());
    charge(
        work,
        (out.len() - index).checked_mul(words).ok_or(Error::Work)?,
    )?;
    reserve(out, 1, work)?;
    out.insert(index, value);
    Ok(())
}

pub(super) fn push<T>(out: &mut Vec<T>, value: T, work: &mut usize) -> Result<()> {
    reserve(out, 1, work)?;
    out.push(value);
    Ok(())
}
