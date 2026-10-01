//! Exact read-only retained storage for one inert catalog, not a budget receipt.
//!
//! The parent owns all five fields. Its three vectors contain bytes or Copy
//! scalar rows, so no payload traversal, allocation, encoding or replay is needed.
//! Enclosing owners use the heap-only method after charging their own header once.

use super::{
    InertCanonicalKernelIrContractCatalogV1 as Catalog, KernelIrPipelineContractDefinitionV1,
    KernelIrPipelineStorageBindingV1,
};
use crate::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error,
    LogicalStorageLimitsV1 as Limits,
};
use std::mem::size_of;

/// Actual logical extent of one currently retained catalog.
///
/// Counts its inline header and actual capacities of its three owned vectors.
/// It excludes allocator metadata/rounding, observer stack, constructor scratch,
/// borrowed input rows and all other compiler owners. This is not a peak, RSS,
/// a whole-action storage result, or authority to reuse a compiler receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrContractCatalogRetainedStorageV1 {
    pub inline_bytes: usize,
    pub canonical_owned_bytes: usize,
    pub definitions_owned_bytes: usize,
    pub bindings_owned_bytes: usize,
    pub total_bytes: usize,
    /// One root and three constant-time collection-capacity observations.
    pub visited_items: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Capacities {
    canonical: usize,
    definitions: usize,
    bindings: usize,
    heap: usize,
}

impl Capacities {
    fn from_counts(
        canonical: usize,
        definition_capacity: usize,
        binding_capacity: usize,
    ) -> Result<Self, Error> {
        let definitions = definition_capacity
            .checked_mul(size_of::<KernelIrPipelineContractDefinitionV1>())
            .ok_or(Error::Arithmetic)?;
        let bindings = binding_capacity
            .checked_mul(size_of::<KernelIrPipelineStorageBindingV1>())
            .ok_or(Error::Arithmetic)?;
        let heap = canonical
            .checked_add(definitions)
            .and_then(|bytes| bytes.checked_add(bindings))
            .ok_or(Error::Arithmetic)?;
        Ok(Self {
            canonical,
            definitions,
            bindings,
            heap,
        })
    }

    fn charge_heap(self, counter: &mut Counter) -> Result<(), Error> {
        // One atomic counter operation: an enclosing owner never receives a
        // partially charged catalog on byte/item/arithmetic refusal.
        counter.charge(self.heap, 3)
    }
}

// These bounds prevent an unnoticed change to heap-owning row elements. Their
// complete current definitions contain only scalar fields; no row has a child
// allocation requiring a per-element visit.
fn require_copy_rows<T: Copy>() {}

impl Catalog {
    fn retained_capacities_v1(&self) -> Result<Capacities, Error> {
        require_copy_rows::<KernelIrPipelineContractDefinitionV1>();
        require_copy_rows::<KernelIrPipelineStorageBindingV1>();
        let Self {
            canonical,
            digest: _,
            semantic_source: _,
            definitions,
            bindings,
        } = self;
        Capacities::from_counts(
            canonical.capacity(),
            definitions.capacity(),
            bindings.capacity(),
        )
    }

    /// Measures this standalone catalog's header and actual owned capacities.
    ///
    /// Limits constrain this observation only. No constructor, canonical wire,
    /// existing reservation receipt or compilation budget is changed. The
    /// returned result has no source, proof, compiler or execution authority.
    pub fn retained_logical_storage_v1(
        &self,
        limits: Limits,
    ) -> Result<KernelIrContractCatalogRetainedStorageV1, Error> {
        let capacities = self.retained_capacities_v1()?;
        let mut counter = Counter::new(limits);
        let inline_bytes = size_of::<Self>();
        counter.charge(inline_bytes, 1)?;
        capacities.charge_heap(&mut counter)?;
        Ok(KernelIrContractCatalogRetainedStorageV1 {
            inline_bytes,
            canonical_owned_bytes: capacities.canonical,
            definitions_owned_bytes: capacities.definitions,
            bindings_owned_bytes: capacities.bindings,
            total_bytes: counter.bytes(),
            visited_items: counter.items(),
        })
    }

    /// Adds only this catalog's three vector payloads to an enclosing owner's
    /// logical-storage counter, atomically on success.
    ///
    /// The caller must already count this catalog's inline fields as part of its
    /// enclosing header (or charge a standalone catalog header once). Never add
    /// this payload charge after already adding a standalone catalog total.
    /// Equal bytes in separately owned catalogs still represent distinct storage.
    /// On every refusal the supplied counter is unchanged.
    pub fn charge_retained_heap_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        self.retained_capacities_v1()?.charge_heap(counter)
    }
}

#[cfg(test)]
#[path = "contract_catalog_retained_storage_v1_tests.rs"]
mod tests;
