//! Same-session reference inputs retained by the authenticated collector closure.
//! These are not independent native-source replay or proof authority.

use super::{CollectedFunction, CollectedFunctionRole};
use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingV1, AuthenticatedReferenceEffectBindingsV1,
    ReferenceBindingErrorV1, authenticate_reference_binding_v1, equivalent_bindings_v1,
    instantiated_signature,
};
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1;
use rustc_middle::ty::{FnSig, Instance, TyCtxt};

struct ReferenceInputV1<'tcx> {
    function: usize,
    kernel: Instance<'tcx>,
    reference: Instance<'tcx>,
    kernel_signature: FnSig<'tcx>,
    reference_signature: FnSig<'tcx>,
    registration_path: String,
    logical_kernel_name: String,
}

pub(super) struct RetainedReferenceInputsV1<'tcx> {
    function_count: usize,
    inputs: Box<[ReferenceInputV1<'tcx>]>,
}

fn error(reason: &'static str) -> ReferenceBindingErrorV1 {
    ReferenceBindingErrorV1::new(reason)
}

fn charge(work: &mut SourceClosureWorkV1, amount: usize) -> Result<(), ReferenceBindingErrorV1> {
    work.charge(amount)
        .map_err(|failure| ReferenceBindingErrorV1::new(failure.to_string()))
}

fn charge_storage<T>(
    work: &mut SourceClosureWorkV1,
    count: usize,
) -> Result<(), ReferenceBindingErrorV1> {
    charge(
        work,
        count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| error("reference input storage overflow"))?,
    )
}

fn charge_names(
    work: &mut SourceClosureWorkV1,
    registration: &str,
    name: &str,
) -> Result<(), ReferenceBindingErrorV1> {
    charge(
        work,
        registration
            .len()
            .checked_add(name.len())
            .ok_or_else(|| error("reference name work overflow"))?,
    )
}

fn retained_bytes<'tcx>(
    functions: &[CollectedFunction<'tcx>],
    work: &mut SourceClosureWorkV1,
) -> Result<(usize, usize), ReferenceBindingErrorV1> {
    charge(work, functions.len())?;
    let mut count = 0_usize;
    let mut bytes = 0_usize;
    for function in functions {
        let binding = match (
            &function.reference_effect_binding,
            function.reference_instance,
        ) {
            (None, None) => continue,
            (Some(binding), Some(_)) if function.role == CollectedFunctionRole::KernelEntry => {
                binding
            }
            _ => {
                return Err(error(
                    "reference binding/instance presence or kernel role changed",
                ));
            }
        };
        count = count
            .checked_add(1)
            .ok_or_else(|| error("reference input count overflow"))?;
        bytes = bytes
            .checked_add(std::mem::size_of::<ReferenceInputV1<'tcx>>())
            .and_then(|bytes| bytes.checked_add(binding.registration_path.len()))
            .and_then(|bytes| bytes.checked_add(binding.logical_kernel_name.len()))
            .ok_or_else(|| error("reference input storage overflow"))?;
    }
    Ok((count, bytes))
}

impl<'tcx> RetainedReferenceInputsV1<'tcx> {
    pub(super) fn capture(
        tcx: TyCtxt<'tcx>,
        functions: &[CollectedFunction<'tcx>],
        work: &mut SourceClosureWorkV1,
    ) -> Result<Self, ReferenceBindingErrorV1> {
        let (count, bytes) = retained_bytes(functions, work)?;
        // SourceClosureWork is cumulative work, not the later canonical byte
        // ledger. Charging retained bytes here also bounds this new allocation.
        charge(work, bytes)?;
        let mut inputs = Vec::new();
        inputs
            .try_reserve_exact(count)
            .map_err(|_| error("reference input allocation failed"))?;
        for (function, collected) in functions.iter().enumerate() {
            charge(work, 1)?;
            let Some(reference) = collected.reference_instance else {
                continue;
            };
            let binding = collected
                .reference_effect_binding
                .as_ref()
                .ok_or_else(|| error("retained reference has no binding"))?;
            charge_names(
                work,
                &binding.registration_path,
                &binding.logical_kernel_name,
            )?;
            inputs.push(ReferenceInputV1 {
                function,
                kernel: collected.instance,
                reference,
                kernel_signature: instantiated_signature(tcx, collected.instance),
                reference_signature: instantiated_signature(tcx, reference),
                registration_path: binding.registration_path.clone(),
                logical_kernel_name: binding.logical_kernel_name.clone(),
            });
        }
        Ok(Self {
            function_count: functions.len(),
            inputs: inputs.into_boxed_slice(),
        })
    }

    pub(super) fn rederive(
        &self,
        tcx: TyCtxt<'tcx>,
        functions: &[CollectedFunction<'tcx>],
        work: &mut SourceClosureWorkV1,
    ) -> Result<AuthenticatedReferenceEffectBindingsV1, ReferenceBindingErrorV1> {
        if self.function_count != functions.len() {
            return Err(error("reference input function roster changed"));
        }
        charge(work, functions.len())?;
        let mut retained = self.inputs.iter().peekable();
        let mut bindings = Vec::new();
        charge_storage::<AuthenticatedReferenceEffectBindingV1>(work, self.inputs.len())?;
        bindings
            .try_reserve_exact(self.inputs.len())
            .map_err(|_| error("reference replay allocation failed"))?;
        for (index, function) in functions.iter().enumerate() {
            let expected = retained
                .peek()
                .filter(|input| input.function == index)
                .copied();
            let (Some(expected), Some(reference), Some(binding)) = (
                expected,
                function.reference_instance,
                function.reference_effect_binding.as_ref(),
            ) else {
                if expected.is_some()
                    || function.reference_instance.is_some()
                    || function.reference_effect_binding.is_some()
                {
                    return Err(error("reference binding or retained input roster changed"));
                }
                continue;
            };
            retained.next();
            // String equality and the later name clones each receive a debit.
            charge_names(
                work,
                &expected.registration_path,
                &expected.logical_kernel_name,
            )?;
            charge(work, expected.logical_kernel_name.len())?;
            if function.role != CollectedFunctionRole::KernelEntry
                || function.instance != expected.kernel
                || reference != expected.reference
                || function.logical_name.as_deref() != Some(expected.logical_kernel_name.as_str())
                || binding.registration_path != expected.registration_path
                || binding.logical_kernel_name != expected.logical_kernel_name
            {
                return Err(error(
                    "reference input instance, role or registration changed",
                ));
            }
            charge(work, expected.kernel_signature.inputs().len())?;
            charge(work, expected.reference_signature.inputs().len())?;
            if instantiated_signature(tcx, function.instance) != expected.kernel_signature
                || instantiated_signature(tcx, reference) != expected.reference_signature
            {
                return Err(error("reference input instantiated signature changed"));
            }
            // Charge both allocation and copying before cloning owned names.
            charge_names(
                work,
                &expected.registration_path,
                &expected.logical_kernel_name,
            )?;
            charge_names(
                work,
                &expected.registration_path,
                &expected.logical_kernel_name,
            )?;
            let fresh = authenticate_reference_binding_v1(
                tcx,
                expected.registration_path.clone(),
                expected.logical_kernel_name.clone(),
                expected.kernel,
                expected.reference,
                work,
            )?;
            if !equivalent_bindings_v1(binding, &fresh, work)? {
                return Err(error(
                    "reference binding differs from fresh compiler extraction",
                ));
            }
            bindings.push(fresh);
        }
        if retained.next().is_some() {
            return Err(error("reference input occurrence is missing"));
        }
        Ok(AuthenticatedReferenceEffectBindingsV1::new(bindings))
    }
}

#[cfg(test)]
#[path = "reference_custody_v1_tests.rs"]
mod tests;
