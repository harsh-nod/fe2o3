//! Compile-time observation policy; submission and custody share one driver.

use super::*;

pub(super) trait OperationProgressV1<B: RuntimeBackendV1, A>: 'static {
    fn observe(
        context: &mut RuntimeContextV1<B>,
        submission: &mut RuntimeSubmissionV1<A>,
    ) -> Result<crate::RuntimePollV1, RuntimeErrorV1<B::Error>>;

    fn flush_stream(stream: RuntimeStreamIdV1) -> Option<RuntimeStreamIdV1>;
}

pub(super) struct ObservedProgressV1;

impl<B: RuntimeBackendV1, A> OperationProgressV1<B, A> for ObservedProgressV1 {
    fn observe(
        context: &mut RuntimeContextV1<B>,
        submission: &mut RuntimeSubmissionV1<A>,
    ) -> Result<crate::RuntimePollV1, RuntimeErrorV1<B::Error>> {
        context.poll(submission)
    }

    fn flush_stream(stream: RuntimeStreamIdV1) -> Option<RuntimeStreamIdV1> {
        Some(stream)
    }
}

pub(super) struct DirectedPeerProgressV1;

impl<B: RuntimeDirectedScalarPeerCopyBackendV1>
    OperationProgressV1<B, RuntimeDirectedScalarPeerCopyV1> for DirectedPeerProgressV1
{
    fn observe(
        context: &mut RuntimeContextV1<B>,
        submission: &mut RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>,
    ) -> Result<crate::RuntimePollV1, RuntimeErrorV1<B::Error>> {
        context.progress_directed_peer_copy_v1(submission)
    }

    fn flush_stream(_: RuntimeStreamIdV1) -> Option<RuntimeStreamIdV1> {
        None
    }
}
