//! Test-only observer activated INSIDE the genuine rustc callback thread.
//! No callback or borrowed owner appears in the normal public interface.
use super::{
    SourceLocalOrderRecipeEvidenceV1 as Evidence, SourceLocalOrderRecipeFailurePhaseV1 as Phase,
    SourceLocalOrderRecipeFailureV1 as Failure, SourceLocalOrderRecipeOutputV1 as Output,
};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 as Owner;
use std::cell::RefCell;

pub(crate) type Observer = Box<dyn FnOnce(&Owner, &Evidence) -> Result<(), String> + Send>;
thread_local! { static CURRENT: RefCell<Option<Observer>> = RefCell::new(None); }

pub(crate) fn with_observer(
    observer: Option<Observer>,
    run: impl FnOnce() -> Result<Output, Failure>,
) -> Result<Output, Failure> {
    CURRENT.with(|cell| {
        let mut current = cell
            .try_borrow_mut()
            .map_err(|_| Failure::new(Phase::Observation, "recipe test observer reentry".into()))?;
        if current.is_some() {
            return Err(Failure::new(
                Phase::Observation,
                "recipe test observer already active".into(),
            ));
        }
        *current = observer;
        Ok(())
    })?;
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            CURRENT.with(|cell| {
                cell.borrow_mut().take();
            });
        }
    }
    let _clear = Clear;
    run()
}

pub(crate) fn observe_current(output: &Owner, evidence: &Evidence) -> Result<(), String> {
    let observer = CURRENT.with(|cell| {
        cell.try_borrow_mut()
            .map(|mut current| current.take())
            .map_err(|_| "recipe test observer reentry".to_string())
    })?;
    match observer {
        Some(observer) => observer(output, evidence),
        None => Ok(()),
    }
}
