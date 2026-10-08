//! Copied-result correspondence while original native backing remains retained.

use crate::{
    KfdRuntimeBackendErrorV1, RuntimeErrorV1, RuntimeGeneratedResultDomainV1,
    RuntimeGfx942GeneratedCarrierV1, RuntimeGfx942GeneratedCompletionViewV1,
    RuntimeGfx942ReadbackErrorV1,
};

/// Trusted correspondence for an independently copied registry result.
/// This is deliberately not the scalar completion trait: decoded output does
/// not establish native DATA retirement, Context completion or a graph version.
///
/// # Safety
///
/// Before decoding, the domain and lending contract are exactly those of
/// `RuntimeGfx942GeneratedCompletionCarrierV1`. Only the original installed
/// readback and decoder may commit the original result gate. Decode must be
/// one-shot and keep the original immutable source, authority, account retention
/// and all credits for retained storage in `self`, including on error/unwind.
/// No source/currentness owner may be replaced or disposed by this transition.
/// Subsequent `source()` calls must still return the same original source and
/// revalidate the actual retained authority. No new readback or control may be
/// installed after decoding begins. A distinct repeat profile must therefore
/// retain its second frame before the first decode. No mapped native view is
/// exposed.
///
/// Runtime invokes decoding only after exact original native completion,
/// signal recycle, copied readback validation and closing currentness. Runtime
/// separately retains common native backing through actual registry teardown;
/// this trait cannot manufacture that teardown or an ordinary completion receipt.
#[doc(hidden)]
pub unsafe trait RuntimeGfx942RegistryCompletionCarrierV1:
    RuntimeGfx942GeneratedCarrierV1
{
    fn registry_completion_domain_v1(
        &self,
    ) -> Result<RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1>;

    fn with_registry_completion_view_v1(
        &mut self,
        callback: impl for<'a> FnOnce(
            RuntimeGfx942GeneratedCompletionViewV1<'a, Self::CurrentnessError>,
        ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>;

    fn decode_registry_readback_retaining_source_v1(
        &mut self,
    ) -> Result<(), RuntimeGfx942ReadbackErrorV1>;

    /// Only the distinct two-cycle implementation can supply a second domain.
    fn registry_second_completion_domain_v1(
        &self,
    ) -> Result<RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
        Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
    }

    fn with_registry_cycle_completion_view_v1(
        &mut self,
        cycle: u8,
        callback: impl for<'a> FnOnce(
            RuntimeGfx942GeneratedCompletionViewV1<'a, Self::CurrentnessError>,
        ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        if cycle != 0 {
            return Err(RuntimeErrorV1::Validation(
                crate::RuntimeValidationErrorV1::InvalidBackendDescription,
            ));
        }
        self.with_registry_completion_view_v1(callback)
    }

    fn decode_registry_cycle_retaining_source_v1(
        &mut self,
        cycle: u8,
    ) -> Result<(), RuntimeGfx942ReadbackErrorV1> {
        if cycle != 0 {
            return Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage);
        }
        self.decode_registry_readback_retaining_source_v1()
    }
}

/// Trusted extension for two separately prepaid copied-result frames.
///
/// # Safety
/// Both domains, decoders and readback destinations must be disjoint and fully
/// reserved before native adoption. Cycle zero obeys the original trait exactly;
/// cycle one uses only the second original frame and cannot reset the first gate
/// or take its credits, even when an old result/alias remains live. Both decode
/// paths retain the same original immutable source, actual authority and loan
/// through actual common native destruction. Neither result establishes that
/// destruction. All other cycle values must refuse before calling the callback.
#[doc(hidden)]
pub unsafe trait RuntimeGfx942RegistryRepeat2CarrierV1:
    RuntimeGfx942RegistryCompletionCarrierV1
{
}
