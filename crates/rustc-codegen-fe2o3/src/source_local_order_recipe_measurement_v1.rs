//! Read-only observations of the same returned attempt and optional live stage.
//! No active compiler/source/graph owner is reconstructed or retained here.

use super::{
    SourceLocalOrderRecipeAttemptV1 as Attempt, SourceLocalOrderRecipeFailureV1 as Failure,
    SourceLocalOrderRecipeOutputV1 as Output, SourceLocalOrderRecipeRequestV1 as Request,
};
use fe2o3_kernel_ir::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error,
    LogicalStorageLimitsV1 as Limits,
};
use std::{mem::size_of, time::Duration};

/// Exact logical extent of one returned Attempt owner, not whole-action storage.
/// The enclosing header contains request/result headers, fixed evidence, callback
/// counters and optional Duration once. No canonical/source/rustc owner survives
/// in this Attempt. Caller argv/retained recipe/file paths and all dropped stage
/// owners, temporary overlap, allocator metadata, peak heap and RSS are excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocalOrderRecipeRetainedStorageV1 {
    pub inline_bytes: usize,
    pub request_owned_bytes: usize,
    pub llvm_owned_bytes: usize,
    pub created_recipe_owned_bytes: usize,
    pub failure_diagnostic_owned_bytes: usize,
    pub total_bytes: usize,
    pub visited_items: usize,
}
fn part(
    c: &mut Counter,
    charge: impl FnOnce(&mut Counter) -> Result<(), Error>,
) -> Result<usize, Error> {
    let before = c.bytes();
    charge(c)?;
    c.bytes().checked_sub(before).ok_or(Error::Arithmetic)
}
impl Attempt {
    /// Charges the actual immutable owner returned by this driver invocation.
    /// None/explicit observer limits only; this imposes no new constructor cap
    /// and does not compare the contract's aggregate128MiB target.
    pub fn retained_logical_storage_v1(
        &self,
        limits: Limits,
    ) -> Result<SourceLocalOrderRecipeRetainedStorageV1, Error> {
        let Self {
            request,
            result,
            callback_count: _,
            compiler_callback_count: _,
            callback_stage_elapsed: _,
        } = self;
        let mut c = Counter::new(limits);
        let inline_bytes = size_of::<Self>();
        c.charge(inline_bytes, 1)?;
        // The existing immutable request constructor records exactly its own
        // header plus actual source String/replay Vec capacities. Its decoded
        // Recipe has only fixed inline closed variants/arrays, no heap owners.
        // Subtract that header because it already lives in enclosing Attempt.
        let request_owned_bytes = request
            .retained_input_storage()
            .checked_sub(size_of::<Request>())
            .ok_or(Error::Arithmetic)?;
        c.charge(request_owned_bytes, 1)?;
        let mut llvm_owned_bytes = 0;
        let mut created_recipe_owned_bytes = 0;
        let mut failure_diagnostic_owned_bytes = 0;
        match result {
            Ok(Output {
                llvm,
                created_recipe,
                evidence: _,
            }) => {
                llvm_owned_bytes = part(&mut c, |c| c.string(llvm))?;
                if let Some(bytes) = created_recipe {
                    created_recipe_owned_bytes = part(&mut c, |c| c.vector(bytes))?;
                }
            }
            Err(Failure {
                phase: _,
                diagnostic,
                compiler_fatal: _,
                cancellation: _, // fixed inline field already included in size_of::<Attempt>()
            }) => {
                failure_diagnostic_owned_bytes = part(&mut c, |c| c.string(diagnostic))?;
            }
        }
        Ok(SourceLocalOrderRecipeRetainedStorageV1 {
            inline_bytes,
            request_owned_bytes,
            llvm_owned_bytes,
            created_recipe_owned_bytes,
            failure_diagnostic_owned_bytes,
            total_bytes: c.bytes(),
            visited_items: c.items(),
        })
    }

    /// Present only for the opt-in measured driver when one unique callback
    /// returns its original stage Result without a rustc fatal. A stage refusal
    /// can have a duration: inspect result() separately. Not a warm replay clock,
    /// process/front-end duration, performance receipt or launch authority.
    pub const fn callback_stage_elapsed_v1(&self) -> Option<Duration> {
        self.callback_stage_elapsed
    }

    pub(crate) fn with_callback_stage_elapsed_v1(
        mut self,
        elapsed: Option<Duration>,
        fatal: bool,
    ) -> Self {
        self.callback_stage_elapsed =
            if self.callback_count == 1 && self.compiler_callback_count == 1 && !fatal {
                elapsed
            } else {
                None
            };
        self
    }
}
