//! Logical framing-reader costs; excludes twelve graph admissions and replay.
use super::*;
use fe2o3_kernel_ir::CanonicalKirOccurrenceRowsRefV1 as Rows;

/// One complete framing read above an existing input floor. This bound never
/// establishes syntax, graph, rewrite, source, or publication authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefinedForwardingHistoryReadQuoteV1 {
    work: usize,
    additional_storage: usize,
}
impl RefinedForwardingHistoryReadQuoteV1 {
    /// Quotes the unchanged strict reader for this wire extent. Rows are bounded
    /// by both their aggregate 4-MiB limit and the complete supplied wire length.
    pub fn for_length(length: usize) -> Result<Self, Error> {
        if !(HEADER..=MAX_REFINED_FORWARDING_HISTORY_BYTES_V1).contains(&length) {
            return Err(Error::Length);
        }
        let rows = length.min(MAX_REFINED_FORWARDING_HISTORY_ROW_BYTES_V1);
        let nested = Rows::read_work_bound_for_length_v1(rows).map_err(Error::Tail)?;
        Ok(Self {
            work: add(add(HEADER + 256 + 136, rows)?, nested)?,
            additional_storage: size_of::<Meter<'_, '_>>()
                + size_of::<InertRefinedForwardingHistoryRefV1<'_>>()
                + size_of::<rows::Reader<'_>>()
                + Rows::READ_STORAGE_V1,
        })
    }

    /// Adds the actual borrowed-wire overlap and same-account window control
    /// frame. The existing Owned-only reader still checks the original floor
    /// and unchanged <=256-MiB additional-operation ceiling.
    pub fn in_original_account_for_length(length: usize) -> Result<Self, Error> {
        let mut quote = Self::for_length(length)?;
        quote.work = add(quote.work, REFINED_FORWARDING_HISTORY_COMPOSITION_WORK_V1)?;
        quote.additional_storage = add(
            quote.additional_storage,
            add(length, REFINED_FORWARDING_HISTORY_COMPOSITION_SCRATCH_V1)?,
        )?;
        Ok(quote)
    }

    pub const fn work(self) -> usize {
        self.work
    }
    pub const fn additional_storage(self) -> usize {
        self.additional_storage
    }
    /// The reader returns this fixed view unreserved; its wire remains borrowed.
    pub const fn retained_storage(self) -> usize {
        size_of::<InertRefinedForwardingHistoryRefV1<'static>>()
    }
}
