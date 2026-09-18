//! All-or-nothing collection under the original generated completion receipt.

use super::*;
use fe2o3_runtime::{
    RuntimeAsyncEngineCallErrorV1, RuntimeAsyncGeneratedCompletionResultV1,
    RuntimeAsyncGeneratedCompletionV1, RuntimeGeneratedCompletionReceiptV1,
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// Tuple coverage must track the admitted ABI, not a smaller convenience limit.
const _: () = assert!(MAX_ABI_FIELDS == 64);

mod private {
    use super::*;

    pub trait Sealed {
        fn check_binding(
            &self,
            completion: &RuntimeAsyncGeneratedCompletionV1,
        ) -> Result<(), GeneratedRuntimeTypedBindErrorV1>;

        fn take_completed(
            &mut self,
            receipt: &RuntimeGeneratedCompletionReceiptV1,
        ) -> Result<Self::Results, GeneratedRuntimeTypedOutputErrorV1>
        where
            Self: GeneratedRuntimeTypedOutputBundleV1;
    }
}

/// A sealed tuple of 2 through 64 move-only output observers from one invocation.
/// The result tuple has the same order and scalar types, with each element
/// retaining its original storage and individual result-peak credit.
///
/// ```compile_fail,E0277
/// use fe2o3_host::{GeneratedRuntimeChargedResultV1, GeneratedRuntimeTypedOutputBundleV1};
/// type O = GeneratedRuntimeChargedResultV1<u8>;
/// type TooMany = (
///     O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
///     O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
///     O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
///     O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O, O,
/// );
/// fn require_bundle<B: GeneratedRuntimeTypedOutputBundleV1>() {}
/// fn reject() { require_bundle::<TooMany>(); }
/// ```
///
/// ```compile_fail,E0599
/// use fe2o3_host::{GeneratedRuntimeChargedResultV1, GeneratedRuntimeTypedOutputBundleV1};
/// fn fabricate(mut outputs: (GeneratedRuntimeChargedResultV1<u32>, GeneratedRuntimeChargedResultV1<u16>)) {
///     outputs.take_completed_matching_v1(|_| true);
/// }
/// ```
///
/// ```compile_fail,E0277
/// use fe2o3_host::GeneratedRuntimeTypedOutputBundleV1;
/// struct Forged;
/// impl GeneratedRuntimeTypedOutputBundleV1 for Forged { type Results = (); }
/// ```
pub trait GeneratedRuntimeTypedOutputBundleV1: private::Sealed + Sized + Send + Unpin {
    type Results: Send;

    /// Binds every member to the original completion, without allocating another
    /// reply or copying storage. Busy, poisoned, foreign, duplicate or stale
    /// slots return the whole tuple and completion unchanged. Already-ready
    /// outputs may bind; readiness alone never substitutes for the completion.
    fn bind_completion_bundle_v1(
        self,
        completion: RuntimeAsyncGeneratedCompletionV1,
    ) -> Result<
        GeneratedRuntimeTypedBundleCompletionV1<Self>,
        GeneratedRuntimeTypedBundleBindFailureV1<Self>,
    > {
        match self.check_binding(&completion) {
            Ok(()) => Ok(GeneratedRuntimeTypedBundleCompletionV1 {
                completion: Some(completion),
                outputs: Some(self),
            }),
            Err(error) => Err(GeneratedRuntimeTypedBundleBindFailureV1 {
                outputs: self,
                completion,
                error,
            }),
        }
    }
}

pub struct GeneratedRuntimeTypedBundleBindFailureV1<B: GeneratedRuntimeTypedOutputBundleV1> {
    pub outputs: B,
    pub completion: RuntimeAsyncGeneratedCompletionV1,
    pub error: GeneratedRuntimeTypedBindErrorV1,
}

/// Original charged results plus the one original completion receipt.
///
/// ```compile_fail,E0599
/// use fe2o3_host::{ChargedTypedResultV1, GeneratedRuntimeCompletedBundleV1};
/// fn duplicate(value: GeneratedRuntimeCompletedBundleV1<(ChargedTypedResultV1<u32>, ChargedTypedResultV1<u16>)>) {
///     value.clone();
/// }
/// ```
#[must_use = "each typed result retains its credit until disposal"]
pub struct GeneratedRuntimeCompletedBundleV1<R> {
    pub results: R,
    pub receipt: RuntimeGeneratedCompletionReceiptV1,
}

/// Extraction failure preserves the complete observer tuple and receipt;
/// engine/readback failure preserves the tuple without manufacturing a receipt.
pub struct GeneratedRuntimeTypedBundleFailureV1<B: GeneratedRuntimeTypedOutputBundleV1> {
    pub outputs: B,
    pub receipt: Option<RuntimeGeneratedCompletionReceiptV1>,
    pub error: GeneratedRuntimeTypedCompletionErrorV1,
}

pub type GeneratedRuntimeTypedBundleOutcomeV1<B> = Result<
    GeneratedRuntimeCompletedBundleV1<<B as GeneratedRuntimeTypedOutputBundleV1>::Results>,
    GeneratedRuntimeTypedBundleFailureV1<B>,
>;

/// One executor-neutral completion observer for an all-or-nothing output bundle.
/// Only the original future returns `Pending`. Dropping this future loses
/// observation, not producer custody, and does not cancel the invocation.
/// Collection adds no decoder, gate, reply, storage allocation or Context command.
///
/// ```no_run
/// use fe2o3_host::{GeneratedRuntimeChargedResultV1, GeneratedRuntimeTypedOutputBundleV1};
/// use fe2o3_runtime::RuntimeAsyncGeneratedCompletionV1;
/// async fn collect(
///     completion: RuntimeAsyncGeneratedCompletionV1,
///     words: GeneratedRuntimeChargedResultV1<u32>,
///     halves: GeneratedRuntimeChargedResultV1<u16>,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let completed = (words, halves).bind_completion_bundle_v1(completion)?.await?;
///     let (words, halves) = completed.results;
///     let _ = (words.as_slice(), halves.as_slice());
///     Ok(())
/// }
/// fn collect_blocking(
///     completion: RuntimeAsyncGeneratedCompletionV1,
///     words: GeneratedRuntimeChargedResultV1<u32>,
///     halves: GeneratedRuntimeChargedResultV1<u16>,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let completed = (words, halves).bind_completion_bundle_v1(completion)?.try_join()??;
///     let _ = (completed.results.0.as_slice(), completed.results.1.as_slice());
///     Ok(())
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fe2o3_host::{GeneratedRuntimeChargedResultV1, GeneratedRuntimeTypedBundleCompletionV1};
/// type Outputs = (GeneratedRuntimeChargedResultV1<u32>, GeneratedRuntimeChargedResultV1<u16>);
/// fn duplicate(value: GeneratedRuntimeTypedBundleCompletionV1<Outputs>) { value.clone(); }
/// ```
#[must_use = "dropping this observer does not cancel the invocation"]
pub struct GeneratedRuntimeTypedBundleCompletionV1<B: GeneratedRuntimeTypedOutputBundleV1> {
    completion: Option<RuntimeAsyncGeneratedCompletionV1>,
    outputs: Option<B>,
}

pub struct GeneratedRuntimeTypedBundleJoinFailureV1<B: GeneratedRuntimeTypedOutputBundleV1> {
    pub completion: GeneratedRuntimeTypedBundleCompletionV1<B>,
    pub error: RuntimeAsyncEngineCallErrorV1,
}

macro_rules! failure_display {
    ($name:ident) => {
        impl<B: GeneratedRuntimeTypedOutputBundleV1> fmt::Debug for $name<B> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("error", &self.error)
                    .finish_non_exhaustive()
            }
        }
        impl<B: GeneratedRuntimeTypedOutputBundleV1> fmt::Display for $name<B> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.error.fmt(f)
            }
        }
        impl<B: GeneratedRuntimeTypedOutputBundleV1> std::error::Error for $name<B> {}
    };
}
failure_display!(GeneratedRuntimeTypedBundleBindFailureV1);
failure_display!(GeneratedRuntimeTypedBundleFailureV1);
failure_display!(GeneratedRuntimeTypedBundleJoinFailureV1);

impl<B: GeneratedRuntimeTypedOutputBundleV1> GeneratedRuntimeTypedBundleCompletionV1<B> {
    /// Blocks on the original completion. Owner-thread rejection returns the
    /// unchanged bundle future. Like scalar join, this has no timeout and may
    /// remain pending indefinitely when runtime custody is uncertain.
    pub fn try_join(
        mut self,
    ) -> Result<GeneratedRuntimeTypedBundleOutcomeV1<B>, GeneratedRuntimeTypedBundleJoinFailureV1<B>>
    {
        let completion = self
            .completion
            .take()
            .expect("unconsumed bundle completion");
        let outputs = self.outputs.take().expect("owned output bundle");
        match typed_completion::join_with(
            completion,
            outputs,
            |completion| {
                completion
                    .try_join()
                    .map_err(|failure| (failure.completion, failure.error))
            },
            finish,
        ) {
            Ok(outcome) => Ok(outcome),
            Err((completion, outputs, error)) => {
                self.completion = Some(completion);
                self.outputs = Some(outputs);
                Err(GeneratedRuntimeTypedBundleJoinFailureV1 {
                    completion: self,
                    error,
                })
            }
        }
    }
}

impl<B: GeneratedRuntimeTypedOutputBundleV1> Future for GeneratedRuntimeTypedBundleCompletionV1<B> {
    type Output = GeneratedRuntimeTypedBundleOutcomeV1<B>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        typed_completion::poll_with(&mut this.completion, &mut this.outputs, cx, finish)
    }
}

pub(super) fn finish<B: GeneratedRuntimeTypedOutputBundleV1>(
    mut outputs: B,
    outcome: RuntimeAsyncGeneratedCompletionResultV1,
) -> GeneratedRuntimeTypedBundleOutcomeV1<B> {
    let receipt = match outcome {
        Ok(Ok(receipt)) => receipt,
        Ok(Err(error)) => {
            return Err(GeneratedRuntimeTypedBundleFailureV1 {
                outputs,
                receipt: None,
                error: GeneratedRuntimeTypedCompletionErrorV1::Readback(error),
            });
        }
        Err(error) => {
            return Err(GeneratedRuntimeTypedBundleFailureV1 {
                outputs,
                receipt: None,
                error: GeneratedRuntimeTypedCompletionErrorV1::Engine(error),
            });
        }
    };
    match outputs.take_completed(&receipt) {
        Ok(results) => Ok(GeneratedRuntimeCompletedBundleV1 { results, receipt }),
        Err(error) => Err(GeneratedRuntimeTypedBundleFailureV1 {
            outputs,
            receipt: Some(receipt),
            error: GeneratedRuntimeTypedCompletionErrorV1::Output(error),
        }),
    }
}

// The matching-driver interface is private: external callers can supply only
// original runtime completion/receipt owners through the sealed adapter above.
pub(super) trait BundleStateV1: GeneratedRuntimeTypedOutputBundleV1 {
    fn check_binding_matching_v1(
        &self,
        matches: impl Fn(&Arc<ResultReadyGateV1>) -> bool,
    ) -> Result<(), GeneratedRuntimeTypedBindErrorV1>;

    fn take_completed_matching_v1(
        &mut self,
        matches: impl Fn(&Arc<ResultReadyGateV1>) -> bool,
    ) -> Result<Self::Results, GeneratedRuntimeTypedOutputErrorV1>;
}

fn distinct_slots(slots: &[*const ()]) -> Result<(), GeneratedRuntimeTypedOutputErrorV1> {
    for (index, slot) in slots.iter().enumerate() {
        if slots[..index].contains(slot) {
            return Err(GeneratedRuntimeTypedOutputErrorV1::BindingMismatch);
        }
    }
    Ok(())
}

macro_rules! bundle_impl {
    ($($scalar:ident : $index:tt),+) => {
        impl<$($scalar: GeneratedDeviceScalarV1),+> GeneratedRuntimeTypedOutputBundleV1
            for ($(GeneratedRuntimeChargedResultV1<$scalar>,)+)
        {
            type Results = ($(ChargedTypedResultV1<$scalar>,)+);
        }

        impl<$($scalar: GeneratedDeviceScalarV1),+> private::Sealed
            for ($(GeneratedRuntimeChargedResultV1<$scalar>,)+)
        {
            fn check_binding(&self, completion: &RuntimeAsyncGeneratedCompletionV1)
                -> Result<(), GeneratedRuntimeTypedBindErrorV1>
            {
                self.check_binding_matching_v1(|gate| completion.matches_owner(gate))
            }

            fn take_completed(&mut self, receipt: &RuntimeGeneratedCompletionReceiptV1)
                -> Result<<Self as GeneratedRuntimeTypedOutputBundleV1>::Results, GeneratedRuntimeTypedOutputErrorV1>
            {
                self.take_completed_matching_v1(|gate| receipt.matches_owner(gate))
            }
        }

        impl<$($scalar: GeneratedDeviceScalarV1),+> BundleStateV1
            for ($(GeneratedRuntimeChargedResultV1<$scalar>,)+)
        {
            fn check_binding_matching_v1(&self, matches: impl Fn(&Arc<ResultReadyGateV1>) -> bool)
                -> Result<(), GeneratedRuntimeTypedBindErrorV1>
            {
                distinct_slots(&[$(Arc::as_ptr(&self.$index.slot).cast(),)+])
                    .map_err(GeneratedRuntimeTypedBindErrorV1::Output)?;
                $(self.$index.check_completion_binding_v1(&matches)?;)+
                Ok(())
            }

            fn take_completed_matching_v1(&mut self, matches: impl Fn(&Arc<ResultReadyGateV1>) -> bool)
                -> Result<Self::Results, GeneratedRuntimeTypedOutputErrorV1>
            {
                distinct_slots(&[$(Arc::as_ptr(&self.$index.slot).cast(),)+])?;
                // C4 publishes the receipt after the decoder releases its locks.
                // Keep every guard until the entire tuple is validated; only the
                // subsequent infallible move phase may consume a slot.
                let mut states = ($(self.$index.slot.state.lock()
                    .map_err(|_| GeneratedRuntimeTypedOutputErrorV1::Custody)?,)+);
                $(GeneratedRuntimeChargedResultV1::<$scalar>::validate_completed_state_v1(
                    &states.$index, &matches,
                )?;)+
                Ok(($(GeneratedRuntimeChargedResultV1::<$scalar>::take_validated_state_v1(
                    &mut states.$index,
                ),)+))
            }
        }
    };
}

macro_rules! bundle_prefixes {
    ([$($scalar:ident : $index:tt),+];) => { bundle_impl!($($scalar : $index),+); };
    ([$($scalar:ident : $index:tt),+]; $next:ident : $next_index:tt $(, $rest:ident : $rest_index:tt)*) => {
        bundle_impl!($($scalar : $index),+);
        bundle_prefixes!([$($scalar : $index),+, $next : $next_index]; $($rest : $rest_index),*);
    };
}

bundle_prefixes!([T0: 0, T1: 1]; T2: 2, T3: 3, T4: 4, T5: 5, T6: 6, T7: 7,
    T8: 8, T9: 9, T10: 10, T11: 11, T12: 12, T13: 13, T14: 14, T15: 15,
    T16: 16, T17: 17, T18: 18, T19: 19, T20: 20, T21: 21, T22: 22, T23: 23,
    T24: 24, T25: 25, T26: 26, T27: 27, T28: 28, T29: 29, T30: 30, T31: 31,
    T32: 32, T33: 33, T34: 34, T35: 35, T36: 36, T37: 37, T38: 38, T39: 39,
    T40: 40, T41: 41, T42: 42, T43: 43, T44: 44, T45: 45, T46: 46, T47: 47,
    T48: 48, T49: 49, T50: 50, T51: 51, T52: 52, T53: 53, T54: 54, T55: 55,
    T56: 56, T57: 57, T58: 58, T59: 59, T60: 60, T61: 61, T62: 62, T63: 63);
