//! Producer-side logical decode bounds; catalog and source admission stay separate.
use super::*;

/// Bounds one catalog decode above its incoming storage floor. Input bytes are
/// borrowed and excluded. The returned owner is unreserved, as for the decoder.
/// These are logical callback/storage bounds, not CPU, allocator, or authority claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrContractCatalogDecodeQuoteV1 {
    work: usize,
    additional_storage: usize,
    retained_storage: usize,
}

impl KernelIrContractCatalogDecodeQuoteV1 {
    /// Maximum charged work, including canonical re-encoding and comparison.
    pub const fn work(self) -> usize {
        self.work
    }
    /// Maximum simultaneous additional logical storage during decoding.
    pub const fn additional_storage(self) -> usize {
        self.additional_storage
    }
    /// Maximum logical payload of the unreserved returned catalog owner.
    pub const fn retained_storage(self) -> usize {
        self.retained_storage
    }
}

impl InertCanonicalKernelIrContractCatalogV1 {
    /// Quotes any catalog with this complete wire length, without trusting its
    /// encoded row counts. This does not parse or admit that catalog. Independent
    /// row maxima conservatively cover every accepted partition of the bytes;
    /// the unchanged decoder still requires its exact framing and row geometry.
    pub fn decode_quote_for_length_v1(
        length: usize,
    ) -> Result<KernelIrContractCatalogDecodeQuoteV1, CatalogError> {
        if !(HEADER..=MAX_KERNEL_IR_CONTRACT_CATALOG_BYTES_V1).contains(&length) {
            return Err(CatalogError::Invalid("catalog length"));
        }
        let payload = length - HEADER;
        let definitions = payload / DEFINITION_BYTES;
        let bindings = payload / BINDING_BYTES;
        let rows = definitions
            .checked_add(bindings)
            .ok_or(CatalogError::Arithmetic)?;
        let row_storage = definitions
            .checked_mul(size_of::<KernelIrPipelineContractDefinitionV1>())
            .and_then(|n| {
                n.checked_add(bindings.checked_mul(size_of::<KernelIrPipelineStorageBindingV1>())?)
            })
            .ok_or(CatalogError::Arithmetic)?;
        let retained_storage = size_of::<Self>()
            .checked_add(length)
            .and_then(|n| n.checked_add(row_storage))
            .ok_or(CatalogError::Arithmetic)?;
        // The temporary decoded rows coexist with the canonical owner and its
        // copied rows. Four byte visits and two row visits cover the full decode.
        let additional_storage = size_of::<Vec<KernelIrPipelineContractDefinitionV1>>()
            .checked_add(size_of::<Vec<KernelIrPipelineStorageBindingV1>>())
            .and_then(|n| n.checked_add(row_storage))
            .and_then(|n| n.checked_add(retained_storage))
            .ok_or(CatalogError::Arithmetic)?;
        let work = length
            .checked_mul(4)
            .and_then(|n| n.checked_add(rows.checked_mul(2)?))
            .and_then(|n| n.checked_add(DOMAIN.len() + 9))
            .ok_or(CatalogError::Arithmetic)?;
        Ok(KernelIrContractCatalogDecodeQuoteV1 {
            work,
            additional_storage,
            retained_storage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn decode_quote_covers_actual_rows_work_peak_and_retained_owner() {
        for (definition_count, binding_count) in [(0, 0), (1, 1), (8, 32), (64, 1)] {
            let definitions: Vec<_> = (0..definition_count)
                .map(|key| KernelIrPipelineContractDefinitionV1 {
                    key,
                    semantic_pipeline_type: key + 1,
                    semantic_payload_type: 0,
                    buffers: 2,
                    elements: 32,
                    prefetch_distance: 1,
                    packed_bits: 32,
                    source_size_bytes: 4,
                    source_alignment_bytes: 4,
                })
                .collect();
            let bindings: Vec<_> = (0..binding_count)
                .map(|storage| KernelIrPipelineStorageBindingV1 {
                    function: 0,
                    storage,
                    key: storage % definition_count,
                    block: 0,
                    operation: storage,
                })
                .collect();
            let mut setup_work = Work::new(usize::MAX);
            let mut setup = Budget::new(&mut setup_work, usize::MAX);
            let (catalog, input) = InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                [1; 32],
                &definitions,
                &bindings,
                &mut setup,
            )
            .unwrap();
            let quote = InertCanonicalKernelIrContractCatalogV1::decode_quote_for_length_v1(
                catalog.canonical_bytes().len(),
            )
            .unwrap();
            let floor = 19 + input.retained_storage();
            let mut work = Work::new(11 + quote.work());
            let mut b = Budget::new(&mut work, floor + quote.additional_storage());
            b.charge_work(11).unwrap();
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let (decoded, charge) = InertCanonicalKernelIrContractCatalogV1::decode_with_budget(
                catalog.canonical_bytes(),
                &mut b,
            )
            .unwrap();
            assert_eq!(decoded, catalog);
            assert_eq!(b.storage(), floor);
            assert!(b.work() - 11 <= quote.work());
            assert!(b.peak_storage() - floor <= quote.additional_storage());
            assert!(charge.retained_storage() <= quote.retained_storage());
            assert!(b.work_ledger_identity_v1() == ledger);
        }
    }

    #[test]
    fn decode_quote_empty_is_exact_and_existing_extent_is_closed() {
        let quote =
            InertCanonicalKernelIrContractCatalogV1::decode_quote_for_length_v1(HEADER).unwrap();
        assert_eq!(quote.work(), 4 * HEADER + DOMAIN.len() + 9);
        assert_eq!(
            quote.retained_storage(),
            size_of::<InertCanonicalKernelIrContractCatalogV1>() + HEADER
        );
        assert_eq!(
            quote.additional_storage(),
            quote.retained_storage()
                + size_of::<Vec<KernelIrPipelineContractDefinitionV1>>()
                + size_of::<Vec<KernelIrPipelineStorageBindingV1>>()
        );
        for invalid in [
            0,
            HEADER - 1,
            MAX_KERNEL_IR_CONTRACT_CATALOG_BYTES_V1 + 1,
            usize::MAX,
        ] {
            assert_eq!(
                InertCanonicalKernelIrContractCatalogV1::decode_quote_for_length_v1(invalid),
                Err(CatalogError::Invalid("catalog length"))
            );
        }
        let maximum = InertCanonicalKernelIrContractCatalogV1::decode_quote_for_length_v1(
            MAX_KERNEL_IR_CONTRACT_CATALOG_BYTES_V1,
        )
        .unwrap();
        assert!(maximum.work() > quote.work());
        assert!(maximum.additional_storage() > maximum.retained_storage());
        assert!(maximum.additional_storage() < 256 * 1024 * 1024);
    }
}
