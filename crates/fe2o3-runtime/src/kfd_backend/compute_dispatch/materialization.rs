//! Runtime custody around lower native initialization and borrowed overwrites.

#![forbid(unsafe_code)]

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) struct MaterializationCustodyV1<S, T, I> {
    pub(super) specs: Vec<S>,
    pub(super) data: Vec<T>,
    initializer: Option<I>,
}

pub(super) fn materialize_with_custody_v1<S, T, I>(
    specs: Vec<S>,
    capacity_error: &'static str,
    initialize: I,
    retain: impl FnOnce(MaterializationCustodyV1<S, T, I>),
) -> Result<Vec<T>, String>
where
    I: FnMut(usize, &S) -> Result<T, String>,
{
    let mut custody = MaterializationCustodyV1 {
        specs,
        data: Vec::new(),
        initializer: Some(initialize),
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        custody
            .data
            .try_reserve_exact(custody.specs.len())
            .map_err(|_| capacity_error.to_owned())?;
        for (index, spec) in custody.specs.iter().enumerate() {
            // The lower call retains anything it cannot return; we root each return.
            let item = (custody.initializer.as_mut().expect("rooted initializer"))(index, spec)?;
            custody.data.push(item);
        }
        // Complete capture/metadata destruction while every native owner is rooted.
        drop(custody.initializer.take());
        drop(core::mem::take(&mut custody.specs));
        Ok(())
    }));
    match result {
        Ok(Ok(())) => Ok(custody.data),
        Ok(Err(error)) => {
            retain(custody);
            Err(error)
        }
        Err(payload) => {
            retain(custody);
            resume_unwind(payload)
        }
    }
}

/// The callback retains any native outputs it cannot return. The session stays
/// installed until the complete output is ready for the consuming constructor.
pub(super) fn materialize_in_retained_session_v1<M, T>(
    session: &mut Option<M>,
    initialize: impl FnOnce(&mut M) -> Result<T, String>,
) -> Result<(M, T), String> {
    let data = initialize(
        session
            .as_mut()
            .ok_or_else(|| "KFD materialization session is missing".to_owned())?,
    )?;
    Ok((
        session.take().expect("borrowed session remains installed"),
        data,
    ))
}

pub(super) struct ResidentOverwriteCustodyV1<D, T, I> {
    pub(super) descriptors: Vec<D>,
    pub(super) data: Vec<T>,
    overwrite: Option<I>,
}

pub(super) fn overwrite_with_custody_v1<D, T, I>(
    descriptors: Vec<D>,
    data: Vec<T>,
    overwrite: I,
    retain: impl FnOnce(ResidentOverwriteCustodyV1<D, T, I>),
) -> Result<Vec<T>, String>
where
    I: FnMut(usize, &D, &mut T) -> Result<(), String>,
{
    let mut custody = ResidentOverwriteCustodyV1 {
        descriptors,
        data,
        overwrite: Some(overwrite),
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        if custody.descriptors.len() != custody.data.len() {
            return Err("KFD resident-data overwrite roster mismatch".to_owned());
        }
        for (index, (descriptor, data)) in custody
            .descriptors
            .iter()
            .zip(&mut custody.data)
            .enumerate()
        {
            (custody.overwrite.as_mut().expect("rooted overwrite"))(index, descriptor, data)?;
        }
        drop(custody.overwrite.take());
        drop(core::mem::take(&mut custody.descriptors));
        Ok(())
    }));
    match result {
        Ok(Ok(())) => Ok(custody.data),
        Ok(Err(error)) => {
            retain(custody);
            Err(error)
        }
        Err(payload) => {
            retain(custody);
            resume_unwind(payload)
        }
    }
}

#[cfg(test)]
mod tests;
