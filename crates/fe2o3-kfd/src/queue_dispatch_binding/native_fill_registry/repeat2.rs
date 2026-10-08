//! Two invocations of each original recipe, without replacing common backing.

use super::*;

/// Prepaid metadata for exactly two cycles of the same four original recipes.
/// This is not rolling admission or permission to replace source, DATA, or ABI.
/// The one-shot constructor cannot consume this distinct storage type.
pub struct Gfx942NativeFillRegistryRepeat2StorageV1 {
    pub(in crate::queue) inner: Gfx942NativeFillRegistryStorageV1,
}

impl Gfx942NativeFillRegistryRepeat2StorageV1 {
    pub fn preallocate(
        account: ResourceCreditAccountV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Gfx942NativeFillRegistryStorageV1::preallocate_profile(account, true)
            .map(|inner| Self { inner })
    }
}

impl<const N: usize> Gfx942NativeFillResidentRegistryStorageV1<N> {
    pub(in crate::queue) fn require_profile(
        &self,
        repeat2: bool,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.repeat2 != repeat2
            || (repeat2 && N != 4)
            || self.recipes.len() != N
            || self
                .recipes
                .iter()
                .enumerate()
                .any(|(index, r)| r.repeat2 != repeat2 || r.count != N || r.index != index)
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        Ok(())
    }

    pub(in crate::queue) fn rearm_second_cycle(
        &mut self,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require_profile(true)?;
        if self.recipes.len() != 4 {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        for recipe in self.recipes.iter() {
            recipe.require_second_cycle()?;
        }
        // All four original generation owners were checked before any change.
        // Their monotone epoch counters and recycled occurrences are not reset.
        for recipe in self.recipes.iter_mut() {
            recipe.accepted = false;
            recipe.copied_generation = None;
        }
        Ok(())
    }
}

impl RegistryRecipeV1 {
    fn require_second_cycle(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if !self.repeat2 || !self.accepted || self.accepted_cycles != 1 {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let returned = self.generation.returned_generation()?;
        if self.copied_generation != Some(returned) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "repeat2/tests.rs"]
mod tests;
