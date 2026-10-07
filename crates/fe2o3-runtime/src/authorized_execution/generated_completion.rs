//! Trusted host correspondence, separate from native execution authority.

use crate::{
    Gfx942RuntimeBufferAccessV1, KfdRuntimeBackendErrorV1, RuntimeErrorV1,
    RuntimeGfx942GeneratedCarrierV1, RuntimeGfx942GeneratedSourceV1, RuntimeGfx942ReadbackErrorV1,
};

/// Simultaneous borrows of the original source and reserved destinations.
/// Construction is descriptive; only runtime can inspect or fill the destinations.
///
/// ```compile_fail,E0616
/// use fe2o3_runtime::RuntimeGfx942GeneratedCompletionViewV1;
/// fn extract(value: RuntimeGfx942GeneratedCompletionViewV1<'_, ()>) {
///     let _ = value.destinations;
/// }
/// ```
///
/// A callback cannot keep the borrowed view after lending returns:
///
/// ```compile_fail,E0521
/// use fe2o3_runtime::{
///     RuntimeGfx942GeneratedCompletionCarrierV1, RuntimeGfx942GeneratedCompletionViewV1,
/// };
/// fn escape<C: RuntimeGfx942GeneratedCompletionCarrierV1>(
///     carrier: &mut C,
/// ) -> RuntimeGfx942GeneratedCompletionViewV1<'static, C::CurrentnessError> {
///     let mut escaped = None;
///     carrier.with_completion_view_v1(|view| {
///         escaped = Some(view);
///         Ok(())
///     }).unwrap();
///     escaped.unwrap()
/// }
/// ```
#[doc(hidden)]
pub struct RuntimeGfx942GeneratedCompletionViewV1<'a, E> {
    source: RuntimeGfx942GeneratedSourceV1<'a, E>,
    destinations: &'a mut [(Gfx942RuntimeBufferAccessV1, Vec<u8>)],
}

impl<'a, E> RuntimeGfx942GeneratedCompletionViewV1<'a, E> {
    pub fn new(
        source: RuntimeGfx942GeneratedSourceV1<'a, E>,
        destinations: &'a mut [(Gfx942RuntimeBufferAccessV1, Vec<u8>)],
    ) -> Self {
        Self {
            source,
            destinations,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        RuntimeGfx942GeneratedSourceV1<'a, E>,
        &'a mut [(Gfx942RuntimeBufferAccessV1, Vec<u8>)],
    ) {
        (self.source, self.destinations)
    }
}

/// Trusted correspondence between reserved host storage and its original decoder.
/// This does not grant Worker authority or authorize native publication.
///
/// The ordinary safe carrier cannot grant this correspondence:
///
/// ```compile_fail,E0200
/// use fe2o3_runtime::{
///     KfdRuntimeBackendErrorV1, RuntimeErrorV1, RuntimeGfx942GeneratedCarrierV1,
///     RuntimeGfx942GeneratedCompletionCarrierV1, RuntimeGfx942GeneratedCompletionViewV1,
///     RuntimeGfx942GeneratedSourceV1, RuntimeGfx942ReadbackErrorV1,
/// };
/// struct Wrapper<C>(C);
/// impl<C: RuntimeGfx942GeneratedCarrierV1> RuntimeGfx942GeneratedCarrierV1 for Wrapper<C> {
///     type CurrentnessError = C::CurrentnessError;
///     type Readback = C::Readback;
///     fn source(&self) -> RuntimeGfx942GeneratedSourceV1<'_, C::CurrentnessError> {
///         self.0.source()
///     }
///     fn prepare_readback(&self) -> Result<C::Readback, RuntimeGfx942ReadbackErrorV1> {
///         self.0.prepare_readback()
///     }
///     fn install_readback(&mut self, readback: C::Readback) {
///         self.0.install_readback(readback);
///     }
/// }
/// impl<C: RuntimeGfx942GeneratedCarrierV1> RuntimeGfx942GeneratedCompletionCarrierV1
///     for Wrapper<C>
/// {
///     fn completion_domain_v1(&self) -> Result<fe2o3_runtime::RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
///         unimplemented!()
///     }
///     fn with_completion_view_v1(
///         &mut self,
///         _: impl for<'a> FnOnce(RuntimeGfx942GeneratedCompletionViewV1<'a, C::CurrentnessError>)
///             -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
///     ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> { Ok(()) }
///     fn complete_readback_v1(self) -> Result<(), RuntimeGfx942ReadbackErrorV1> { Ok(()) }
/// }
/// ```
///
/// # Safety
///
/// The view must lend the exact installed readback allocations in original source
/// order, retaining their charged owner and the same uncommitted result gate.
/// The completion domain must identify that exact original gate allocation,
/// shared only by outputs of this invocation. Returning it has no reservation,
/// execution or result-publication effects. It must remain the same gate through
/// completion; a domain retains metadata, not result-credit or native custody.
/// Complete shape, gate and output-custody validation must precede callback entry.
/// The source must be the same immutable invocation that reserved those buffers.
/// Success must invoke the callback exactly once and return its result unchanged.
/// Callback errors and panics must propagate without retry or result publication.
/// After callback entry no source, destination, decoder or gate may be replaced;
/// after callback return no destination bytes or result state may be changed.
/// Completion must consume that original owner and decoder exactly once, check
/// all read-only bytes, decode every output, and settle disposed storage credits
/// before committing the original gate as its final successful action. An error
/// or unwind must leave the gate uncommitted and the result unavailable.
/// It must not fabricate a result, replace
/// the decoder or transfer completion to another invocation. Runtime calls it
/// only after native and Context settlement. This unsafe boundary transports
/// compiler/host correspondence that runtime cannot inspect, not Rust borrows.
#[doc(hidden)]
pub unsafe trait RuntimeGfx942GeneratedCompletionCarrierV1:
    RuntimeGfx942GeneratedCarrierV1
{
    fn completion_domain_v1(
        &self,
    ) -> Result<crate::RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
        Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
    }

    fn with_completion_view_v1(
        &mut self,
        callback: impl for<'a> FnOnce(
            RuntimeGfx942GeneratedCompletionViewV1<'a, Self::CurrentnessError>,
        ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>;

    fn complete_readback_v1(self) -> Result<(), RuntimeGfx942ReadbackErrorV1>
    where
        Self: Sized;
}
