//! Nested original loans; every borrowed envelope remains within a () callback.
use super::*;
type NativeResult = Result<(), crate::RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>;
type LoanResult = Result<NativeResult, GeneratedNativeInputErrorV1>;
type Envelope<'a> = fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>;
type Buffers<'a> = &'a [crate::Gfx942KfdDispatchBufferV1];

pub(super) fn lend<E, const N: usize>(
    sources: [RuntimeGfx942GeneratedSourceV1<'_, E>; N],
    uid: u64,
    callback: impl for<'a> FnOnce([Envelope<'a>; N], [Buffers<'a>; N]) -> NativeResult,
) -> LoanResult {
    if N == 4 {
        return four(&sources, uid, |programs, buffers| {
            callback(exact_array(programs), exact_array(buffers))
        });
    }
    if N != 16 {
        return Err(GeneratedNativeInputErrorV1::Source(
            RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
        ));
    }
    let mut nested_error = None;
    let result = four(&sources[..4], uid, |ap, ab| {
        let result = four(&sources[4..8], uid, |bp, bb| {
            let result = four(&sources[8..12], uid, |cp, cb| {
                flatten(
                    four(&sources[12..], uid, |dp, db| {
                        callback(
                            exact_array(join(ap, bp, cp, dp)),
                            exact_array(join(ab, bb, cb, db)),
                        )
                    }),
                    &mut nested_error,
                )
            });
            flatten(result, &mut nested_error)
        });
        flatten(result, &mut nested_error)
    })?;
    match nested_error {
        Some(error) => Err(error),
        None => Ok(result),
    }
}

fn join<T>(a: [T; 4], b: [T; 4], c: [T; 4], d: [T; 4]) -> [T; 16] {
    let mut items = a.into_iter().chain(b).chain(c).chain(d);
    core::array::from_fn(|_| items.next().unwrap_or_else(|| std::process::abort()))
}

fn flatten(result: LoanResult, error: &mut Option<GeneratedNativeInputErrorV1>) -> NativeResult {
    match result {
        Ok(result) => result,
        Err(failure) => {
            *error = Some(failure);
            Ok(())
        }
    }
}

fn four<E>(
    sources: &[RuntimeGfx942GeneratedSourceV1<'_, E>],
    uid: u64,
    callback: impl for<'a> FnOnce([Envelope<'a>; 4], [Buffers<'a>; 4]) -> NativeResult,
) -> LoanResult {
    let [a, b, c, d] = sources else {
        return Err(GeneratedNativeInputErrorV1::Source(
            RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
        ));
    };
    let ar = a
        .validate(uid)
        .map_err(GeneratedNativeInputErrorV1::Source)?;
    let br = b
        .validate(uid)
        .map_err(GeneratedNativeInputErrorV1::Source)?;
    let cr = c
        .validate(uid)
        .map_err(GeneratedNativeInputErrorV1::Source)?;
    let dr = d
        .validate(uid)
        .map_err(GeneratedNativeInputErrorV1::Source)?;
    let mut nested_error = None;
    let result = a.with_native_inputs_v1(uid, &ar, |ap, ab| {
        let result = b.with_native_inputs_v1(uid, &br, |bp, bb| {
            let result = c.with_native_inputs_v1(uid, &cr, |cp, cb| {
                flatten(
                    d.with_native_inputs_v1(uid, &dr, |dp, db| {
                        callback([ap, bp, cp, dp], [ab, bb, cb, db])
                    }),
                    &mut nested_error,
                )
            });
            flatten(result, &mut nested_error)
        });
        flatten(result, &mut nested_error)
    })?;
    match nested_error {
        Some(error) => Err(error),
        None => Ok(result),
    }
}
