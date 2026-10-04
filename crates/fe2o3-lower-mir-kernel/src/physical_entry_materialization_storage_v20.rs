//! Logical requested-payload accounting, following existing importer units.
//! Vec/String reservations are fallible. Fixed shallow Type boxes use existing
//! KIR constructors after prepayment; this is NOT a host-OOM recovery promise.
use super::{Budget, Error, Resource};

pub(crate) struct Scope<'a, 'work> {
    budget: &'a mut Budget<'work>,
    reserved: usize,
}
impl<'a, 'work> Scope<'a, 'work> {
    pub(crate) fn new(budget: &'a mut Budget<'work>) -> Self {
        Self {
            budget,
            reserved: 0,
        }
    }
    pub(crate) fn reserve(&mut self, amount: usize) -> Result<(), Error> {
        let next = self
            .reserved
            .checked_add(amount)
            .ok_or(Resource::Arithmetic)?;
        self.budget.reserve_storage(amount)?;
        self.reserved = next;
        Ok(())
    }
    pub(crate) fn vec<T>(&mut self, count: usize) -> Result<Vec<T>, Error> {
        self.reserve(
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        Ok(values)
    }
    pub(crate) fn copy<T: Copy>(&mut self, values: &[T]) -> Result<Vec<T>, Error> {
        let mut output = self.vec(values.len())?;
        output.extend_from_slice(values);
        Ok(output)
    }
    pub(crate) fn string(&mut self, value: &str) -> Result<String, Error> {
        self.reserve(value.len())?;
        let mut output = String::new();
        output
            .try_reserve_exact(value.len())
            .map_err(|_| Resource::Allocation)?;
        output.push_str(value);
        Ok(output)
    }
    pub(crate) fn bytes(&self) -> usize {
        self.reserved
    }
}
impl Drop for Scope<'_, '_> {
    fn drop(&mut self) {
        // This scope holds the only mutable ledger borrow. Its successful
        // reservations are never released elsewhere, so this cannot underflow.
        // It restores the caller's incoming floor on Result and unwind paths;
        // live successful output ownership is transferred with a receipt.
        let _ = self.budget.release_storage(self.reserved);
    }
}
