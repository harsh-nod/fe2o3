//! Same-owner V3 middle-end content. Decoding never constructs an executed owner.

use super::*;
use fe2o3_compiler_lineage::{
    MIXED_MIDDLE_END_WORKING_STORAGE_V50 as SCRATCH, MixedMiddleEndErrorV50 as CodecError,
    MixedMiddleEndIdentityV50 as Identity, MixedMiddleEndInputV50 as Input,
    MixedMiddleEndLayoutV50 as Layout, MixedMiddleEndRefV50 as Content,
    OrderedInertSemanticLineageReceiptsV3 as Receipts, encode_mixed_middle_end_v50 as encode,
    read_mixed_middle_end_v50 as read,
};

fn codec(error: CodecError<Resource>) -> Error {
    match error {
        CodecError::Charge(error) => error.into(),
        CodecError::Arithmetic => Resource::Arithmetic.into(),
        _ => refusal("exact Policy11 mixed middle-end content"),
    }
}
fn frames<R, F, S>() -> Result<usize> {
    [
        SCRATCH,
        size_of::<F>(),
        align_of::<F>(),
        size_of::<AssertUnwindSafe<F>>(),
        size_of::<(&S, F)>(),
        size_of::<&mut Budget<'_>>(),
        size_of::<AssertUnwindSafe<(&S, F)>>(),
        size_of::<Result<R>>(),
        size_of::<std::thread::Result<Result<R>>>(),
        size_of::<Result<Input<'_>>>(),
        size_of::<std::result::Result<Layout, CodecError<Resource>>>(),
        size_of::<std::result::Result<Identity, CodecError<Resource>>>(),
        size_of::<std::result::Result<Content<'_>, CodecError<Resource>>>(),
        2 * size_of::<std::result::Result<(), CodecError<Resource>>>(),
        2 * size_of::<CodecError<Resource>>(),
        2 * size_of::<&mut Budget<'_>>(),
        size_of::<Layout>(),
        size_of::<Identity>(),
        size_of::<[usize; 5]>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic.into())
    })
}

impl<'r, 'h, 'n, 'p, 'v, 's> ExecutedTypedSourceTailV50<'r, 'h, 'n, 'p, 'v, 's> {
    fn lineage_input<'a>(&'a self, budget: &mut Budget<'_>) -> Result<Input<'a>> {
        self.check(budget)?;
        let request = self.prepared.request;
        let source = request.source;
        let native = request.handoff;
        let prefix = native.relocation(budget)?.prefix(budget)?.output(budget)?;
        Ok(Input {
            semantic_mir: source.source_semantic(budget)?.canonical_encoding(),
            source_ssa_identity: &self.prepared.binding.source[1],
            original: prefix.input_audit_bytes(),
            prefix: prefix.owner().canonical_bytes(),
            licm: native
                .relocation(budget)?
                .tail(budget)?
                .output()
                .canonical_bytes(),
            forwarded: native.output(budget)?.canonical_bytes(),
            prefix_witness: prefix.execution().canonical_bytes(),
            generated_source: request.generated_source(budget)?,
            execution_receipt: &self.wire,
        })
    }
    fn with_lineage<R, F>(
        &self,
        external_bytes: usize,
        budget: &mut Budget<'_>,
        visit: F,
    ) -> Result<R>
    where
        F: FnOnce(Input<'_>, &mut Budget<'_>) -> Result<R>,
    {
        self.check(budget)?;
        let source = self.prepared.request.source;
        let floor = self
            .prepared
            .required
            .checked_add(external_bytes)
            .ok_or(Resource::Arithmetic)?;
        let scratch = frames::<R, F, Self>()?;
        let capture = (self, visit);
        let operation = move |budget: &mut Budget<'_>| {
            let (owner, visit) = std::convert::identity(capture);
            owner.replay_signed_receipt(budget)?;
            visit(owner.lineage_input(budget)?, budget)
        };
        #[cfg(test)]
        {
            assert_eq!(std::mem::size_of_val(&operation), size_of::<(&Self, F)>());
            assert_eq!(std::mem::align_of_val(&operation), align_of::<(&Self, F)>());
        }
        let result = budget.with_prepaid_scope(floor, 0, 0, scratch, operation);
        result.map_err(|error| match error {
            Error::Resource(error) => source.retain_query_resource_error_v18(error).into(),
            other => other,
        })
    }
    /// Requires the actual original composed request, not equal serialized owners.
    /// This revalidates the retained runtime but creates no new authority.
    pub fn check_lineage_request_v50(
        &self,
        request: &Request<'h, 'n, 'p, 'v, 's>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.check(budget)?;
        if !std::ptr::eq(self.prepared.request, request) {
            return Err(refusal("mixed lineage must retain the original request"));
        }
        self.replay_signed_receipt(budget)
    }
    /// Computes the exact bounded middle-end extent from the retained chain.
    /// This does not allocate or return an execution receipt replacement.
    pub fn lineage_layout_v50(&self, budget: &mut Budget<'_>) -> Result<Layout> {
        self.with_lineage(0, budget, |input, _| Layout::new(input).map_err(codec))
    }
    /// Writes the distinct typed V50 preimage for the frozen V3 middle-end slot.
    /// Caller prepays the complete output backing in this same ledger for its
    /// whole lifetime. This method reserves only additional transient frames.
    pub fn encode_lineage_content_v50(
        &self,
        bytes: &mut [u8],
        budget: &mut Budget<'_>,
    ) -> Result<Identity> {
        self.with_lineage(bytes.len(), budget, |input, budget| {
            encode(input, bytes, SCRATCH, |n| budget.charge_work(n)).map_err(codec)
        })
    }
    /// Strictly reads then byte-matches every field to this actual executed owner.
    /// Caller prepays complete immutable input backing separately; no decode
    /// path can construct source custody or an executed owner.
    pub fn replay_lineage_content_v50(
        &self,
        bytes: &[u8],
        budget: &mut Budget<'_>,
    ) -> Result<Identity> {
        self.with_lineage(bytes.len(), budget, |input, budget| {
            let decoded = read(bytes, SCRATCH, |n| budget.charge_work(n)).map_err(codec)?;
            decoded
                .check_exact(input, |n| budget.charge_work(n))
                .map_err(codec)?;
            Ok(decoded.identity())
        })
    }
    /// Joins the same executed chain to the existing capsule's mandatory stage
    /// receipts. Remaining MIR, memory, target, compiler and currentness proof
    /// inputs remain mandatory and are not manufactured by this content check.
    pub fn replay_lineage_capsule_v50(
        &self,
        receipts: &Receipts,
        budget: &mut Budget<'_>,
    ) -> Result<Identity> {
        let bytes = receipts.middle_end().canonical_preimage();
        self.with_lineage(bytes.len(), budget, |input, budget| {
            let decoded = read(bytes, SCRATCH, |n| budget.charge_work(n)).map_err(codec)?;
            decoded
                .check_exact(input, |n| budget.charge_work(n))
                .map_err(codec)?;
            decoded
                .check_capsule(receipts, |n| budget.charge_work(n))
                .map_err(codec)?;
            Ok(decoded.identity())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of_val, size_of_val};
    #[test]
    fn typed_source_lineage_frames_include_input_result_capture_and_both_returns() {
        type R = Identity;
        type F = fn(Input<'_>, &mut Budget<'_>) -> Result<R>;
        type S = ExecutedTypedSourceTailV50<'static, 'static, 'static, 'static, 'static, 'static>;
        let expected = SCRATCH
            + size_of::<F>()
            + align_of::<F>()
            + size_of::<AssertUnwindSafe<F>>()
            + size_of::<(&S, F)>()
            + size_of::<&mut Budget<'_>>()
            + size_of::<AssertUnwindSafe<(&S, F)>>()
            + size_of::<Result<R>>()
            + size_of::<std::thread::Result<Result<R>>>()
            + size_of::<Result<Input<'_>>>()
            + size_of::<std::result::Result<Layout, CodecError<Resource>>>()
            + size_of::<std::result::Result<Identity, CodecError<Resource>>>()
            + size_of::<std::result::Result<Content<'_>, CodecError<Resource>>>()
            + 2 * size_of::<std::result::Result<(), CodecError<Resource>>>()
            + 2 * size_of::<CodecError<Resource>>()
            + 2 * size_of::<&mut Budget<'_>>()
            + size_of::<Layout>()
            + size_of::<Identity>()
            + 5 * size_of::<usize>();
        assert_eq!(frames::<R, F, S>().unwrap(), expected);
        // This pointer-only capture does not construct a runtime or executed owner.
        let captured = (0usize as *const S, 7u64);
        let closure = move || std::convert::identity(captured);
        assert_eq!(size_of_val(&closure), size_of_val(&captured));
        assert_eq!(align_of_val(&closure), align_of_val(&captured));
    }
}
