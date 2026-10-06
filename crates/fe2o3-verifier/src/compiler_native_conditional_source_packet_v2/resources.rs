//! Counts the framing decoder only; nested evidence remains opaque.
use super::*;

/// Logical work and metadata overlapping the decoder callback. Borrowed packet
/// bytes and callback-owned work/storage must be funded separately. No source,
/// formula, signature, or replay admission is implied by this inert quote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalSourcePacketDecodeQuoteV2 {
    work: usize,
    callback_storage: usize,
}
impl NativeConditionalSourcePacketDecodeQuoteV2 {
    /// Bounds both successful decoding and framing refusal at this wire length.
    /// Uses the order's four-byte extent bound even before its count is joined
    /// to the root count, so a malformed large order cannot escape the quote.
    pub fn for_length(length: usize) -> Result<Self, E> {
        require(length <= MAX_BYTES, "aggregate packet limit")?;
        let roots = MAX_ROOTS.min(length / size_of::<u32>());
        let effects = length / EFFECT_BYTES;
        let root_payload = mul(
            roots,
            size_of::<u32>()
                + size_of::<u8>()
                + size_of::<decode::Root<'_>>()
                + size_of::<NativeConditionalSourceRootV2<'_>>(),
        )?;
        let effects_payload = mul(effects, size_of::<Staging>() + size_of::<Signature>())?;
        let payload = add(root_payload, effects_payload)?;
        let callback_storage = add(
            payload,
            size_of::<decode::Packet<'_>>()
                + size_of::<Vec<u8>>()
                + size_of::<Vec<NativeConditionalSourceRootV2<'_>>>(),
        )?;
        // Positive take calls consume disjoint wire bytes; only two outer and
        // six per-root blob bodies may be empty. UTF-8 revisits a wire subset.
        // Four outer vectors and two per root add their payload + one; order
        // validation adds 2 + 2R, and borrowed root-view construction adds R.
        let work = add(add(mul(length, 3)?, payload)?, add(8, mul(roots, 11)?)?)?;
        Ok(Self {
            work,
            callback_storage,
        })
    }
    pub const fn work(self) -> usize {
        self.work
    }
    /// Maximum decoder-local storage, all still live while its callback runs.
    /// Normal return drops and refunds only this metadata, never callback owners.
    pub const fn callback_storage(self) -> usize {
        self.callback_storage
    }
}
