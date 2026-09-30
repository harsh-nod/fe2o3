//! Reuse source-stable slice metadata across intrinsic and ordinary bounds projection.
//! Ranked parameters are analysis values, not additional physical ABI arguments.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use ranked_projection_source_v1::resource;
use std::mem::size_of;

type Error = ProductionRankedProjectionErrorV1;

pub(super) struct Scratch {
    pub(super) origins: Vec<Option<u32>>,
    pub(super) arguments: Vec<Option<u32>>,
    pub(super) definitions: Vec<u8>,
    pub(super) escaped: Vec<bool>,
}

pub(super) struct Scope<'a> {
    facts: &'a mut dyn ProjectedAssertionFactsV1,
    count: usize,
    retained: usize,
}

fn storage(counts: [usize; 5]) -> Result<usize, Error> {
    let widths = [
        size_of::<Option<u32>>(),
        size_of::<Option<u32>>(),
        size_of::<u8>(),
        size_of::<bool>(),
        size_of::<BoundsLocalDefinitionV1<'_>>(),
    ];
    counts.into_iter().zip(widths).try_fold(
        size_of::<Scratch>()
            + size_of::<Scope<'_>>()
            + size_of::<Vec<BoundsLocalDefinitionV1<'_>>>(),
        |total, (count, width)| {
            count
                .checked_mul(width)
                .and_then(|bytes| total.checked_add(bytes))
                .ok_or_else(|| resource(Resource::Arithmetic))
        },
    )
}

/// The producer's existing tables are prepaid through their extra retained
/// lifetime. The callback must dispose of them before returning. Other live
/// projection scopes (notably induction storage) keep their own reservations.
pub(super) fn with_scope<T>(
    count: usize,
    facts: &mut dyn ProjectedAssertionFactsV1,
    action: impl FnOnce(&mut Scope<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    facts.charge_private_array_work(4)?;
    let requested = storage([count; 5])?;
    let floor = facts.scalar_private_storage_v1()?;
    let mut scope = Scope {
        facts,
        count,
        retained: 0,
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scope.facts.reserve_scalar_private_storage_v1(requested)?;
        scope.retained = requested;
        action(&mut scope)
    }));
    let minimum = floor
        .checked_add(scope.retained)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    if scope.facts.scalar_private_storage_v1()? < minimum {
        return Err(resource(Resource::Accounting));
    }
    scope
        .facts
        .release_scalar_private_storage_v1(scope.retained)?;
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

impl Scope<'_> {
    pub(super) fn facts(&mut self) -> &mut dyn ProjectedAssertionFactsV1 {
        self.facts
    }

    pub(super) fn retain(&mut self, scratch: &Scratch) -> Result<(), Error> {
        self.facts.charge_private_array_work(8)?;
        if [
            scratch.origins.len(),
            scratch.arguments.len(),
            scratch.definitions.len(),
            scratch.escaped.len(),
        ] != [self.count; 4]
        {
            return Err(resource(Resource::Accounting));
        }
        let actual = storage([
            scratch.origins.capacity(),
            scratch.arguments.capacity(),
            scratch.definitions.capacity(),
            scratch.escaped.capacity(),
            self.count,
        ])?;
        let extra = actual
            .checked_sub(self.retained)
            .ok_or_else(|| resource(Resource::Accounting))?;
        self.facts.reserve_scalar_private_storage_v1(extra)?;
        self.retained = actual;
        Ok(())
    }
}

#[path = "retained_slice_scope_prefix_v1.rs"]
mod retained_scope_prefix;
pub(super) use retained_scope_prefix::{
    RetainedSliceScopePrefixV1, retained_slice_prefix_frame_v1,
};

pub(super) struct Context<'a> {
    pub(super) scratch: &'a mut Scratch,
    pub(super) facts: &'a mut dyn ProjectedAssertionFactsV1,
}

impl Context<'_> {
    pub(super) fn extent(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        length: SemanticLocalIdV1,
        definitions: &[BoundsLocalDefinitionV1<'_>],
        next_argument: &mut usize,
    ) -> Result<Option<ProductionRankedValueV1>, Error> {
        super::root_bounds_extent_preparation_v1::extent_legacy_v1(
            self.scratch,
            self.facts,
            types,
            function,
            length,
            definitions,
            next_argument,
        )
    }
}
