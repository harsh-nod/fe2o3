//! Send transport materializes inert drivers only after owner admission.

use super::*;

/// Materialization cannot issue native work or acquire native custody. Prepare
/// against the Context in the installed driver's first advance instead. Keep
/// the unique reply producer until the final, non-panicking transfer to a driver.
pub(in crate::async_engine) trait EngineOperationFactoryV1<B: RuntimeBackendV1>:
    Send
{
    fn materialize(&mut self) -> Box<dyn EngineOperationV1<B>>;
    /// Infallibly detach/resolve the unique reply before fallible handling.
    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1);

    /// Only handle-only drivers whose resources remain entirely in the Context
    /// may opt out. A Context-returning engine cannot carry local driver custody
    /// across its thread join or run the owned native shutdown protocol.
    fn requires_owned_shutdown(&self) -> bool {
        true
    }
}

pub(super) struct OperationFactoryV1<B: RuntimeBackendV1, A> {
    stream: RuntimeStreamIdV1,
    submit: Option<Submit<B, A>>,
    reply: Option<owned::Reply<RuntimeAsyncOperationResultV1<A, B::Error>>>,
    control: Option<RuntimeAsyncOperationControlV1>,
}

impl<B: RuntimeBackendV1, A> OperationFactoryV1<B, A> {
    pub(super) fn new(
        stream: RuntimeStreamIdV1,
        submit: Submit<B, A>,
        reply: owned::Reply<RuntimeAsyncOperationResultV1<A, B::Error>>,
        control: Option<RuntimeAsyncOperationControlV1>,
    ) -> Self {
        Self {
            stream,
            submit: Some(submit),
            reply: Some(reply),
            control,
        }
    }
}

impl<B: RuntimeBackendV1 + 'static, A: 'static> EngineOperationFactoryV1<B>
    for OperationFactoryV1<B, A>
{
    fn materialize(&mut self) -> Box<dyn EngineOperationV1<B>> {
        Box::new(Operation {
            stream: self.stream,
            submit: Some(self.submit.take().expect("factory is materialized once")),
            submission: None,
            reply: self.reply.take(),
            rejected_observations: 0,
            last_rejected_observation: None,
            control: self.control.take(),
        })
    }

    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1) {
        stop_reply(&mut self.reply, self.control.as_ref(), error);
    }

    fn requires_owned_shutdown(&self) -> bool {
        false
    }
}

impl<B: RuntimeBackendV1, A> Drop for OperationFactoryV1<B, A> {
    fn drop(&mut self) {
        stop_reply(
            &mut self.reply,
            self.control.as_ref(),
            RuntimeAsyncEngineCallErrorV1::EngineStopped,
        );
    }
}

pub(super) fn stop_reply<R>(
    reply: &mut Option<owned::Reply<R>>,
    control: Option<&RuntimeAsyncOperationControlV1>,
    error: RuntimeAsyncEngineCallErrorV1,
) {
    let error = if let Some(control) = control {
        control.stopped();
        if control.phase() == RuntimeAsyncOperationPhaseV1::CancelledBeforeSubmission {
            RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission
        } else {
            error
        }
    } else {
        error
    };
    if let Some(mut reply) = reply.take() {
        reply.complete(Err(error));
    }
}
