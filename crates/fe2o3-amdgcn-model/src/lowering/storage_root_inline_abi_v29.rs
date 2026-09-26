//! Inert target roles, not source, host-pointer or initialization authority.

use super::storage_native_v18::StorageEmissionContextV18 as Context;
use super::*;
use fe2o3_kernel_ir::StorageLayoutIdV1;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RootParameterRoleV29 {
    Existing,
    InlineObject(StorageLayoutIdV1),
}

pub(crate) struct RootKernelRolesV29<'a> {
    pub(crate) kernel: &'a KernelId,
    pub(crate) entry: &'a FunctionId,
    pub(crate) entry_function_ordinal: usize,
    pub(crate) parameters: &'a [RootParameterRoleV29],
}

pub(crate) struct RootRolesV29<'a> {
    pub(crate) roots: &'a [RootKernelRolesV29<'a>],
}

fn compare(context: &Context<'_>, left: &str, right: &str) -> Result<Ordering, LoweringErrors> {
    let work = left
        .len()
        .checked_add(right.len())
        .and_then(|bytes| bytes.checked_add(1))
        .ok_or_else(|| context.reject("root role id comparison overflow"))?;
    context.charge(work)?;
    Ok(left.cmp(right))
}

fn check_role(
    context: &Context<'_>,
    role: RootParameterRoleV29,
    ty: &Type,
) -> Result<(), LoweringErrors> {
    context.charge(1)?;
    match role {
        RootParameterRoleV29::Existing => {
            if matches!(ty, Type::Pointer(pointer) if pointer.address_space != KernelAddressSpace::Global)
                || matches!(ty, Type::Slice(slice) if slice.address_space != KernelAddressSpace::Global)
            {
                return Err(
                    context.reject("existing root role requires the original global pointer ABI")
                );
            }
        }
        RootParameterRoleV29::InlineObject(id) => {
            let Type::Pointer(pointer) = ty else {
                return Err(context.reject("inline root role requires an object pointer"));
            };
            if pointer.address_space != KernelAddressSpace::Constant
                || pointer.access != AccessMode::ReadOnly
                || pointer.pointee.as_ref() != &Type::StorageObject(id)
            {
                return Err(
                    context.reject("inline root role differs from its Constant read-only object")
                );
            }
            let row = context.row(id)?;
            if row.size == 0 || row.size > u64::from(u32::MAX) || !row.alignment.is_power_of_two() {
                return Err(
                    context.reject("inline root object has no exact addressable ABI extent")
                );
            }
        }
    }
    Ok(())
}

impl RootRolesV29<'_> {
    fn lookup(
        &self,
        context: &Context<'_>,
        kernel: &KernelId,
    ) -> Result<Option<&RootKernelRolesV29<'_>>, LoweringErrors> {
        context.charge(1)?;
        let mut first = 0usize;
        let mut end = self.roots.len();
        while first < end {
            let middle = first + (end - first) / 2;
            let row = &self.roots[middle];
            match compare(context, row.kernel.as_str(), kernel.as_str())? {
                Ordering::Less => first = middle + 1,
                Ordering::Greater => end = middle,
                Ordering::Equal => return Ok(Some(row)),
            }
        }
        Ok(None)
    }

    fn entry<'a>(
        &self,
        context: &Context<'a>,
        kernel: &Kernel,
        row: &RootKernelRolesV29<'_>,
    ) -> Result<&'a Function, LoweringErrors> {
        context.charge(1)?;
        let function = context
            .owner
            .module()
            .functions
            .get(row.entry_function_ordinal)
            .ok_or_else(|| {
                context.reject("root role function ordinal is outside its actual owner")
            })?;
        if compare(context, row.entry.as_str(), kernel.entry.as_str())? != Ordering::Equal
            || compare(context, function.id.as_str(), row.entry.as_str())? != Ordering::Equal
            || function.role != FunctionRole::KernelEntry
            || function
                .body
                .as_ref()
                .is_none_or(|body| body.parameters.len() != function.signature.parameters.len())
            || row.parameters.len() != function.signature.parameters.len()
        {
            return Err(context.reject("root role entry id, ordinal or parameter census differs"));
        }
        Ok(function)
    }

    pub(super) fn validate(&self, context: &Context<'_>) -> Result<(), LoweringErrors> {
        context.charge(1)?;
        let module = context.owner.module();
        if self.roots.len() != module.kernels.len() {
            return Err(context.reject("root role roster is not complete"));
        }
        for (index, row) in self.roots.iter().enumerate() {
            context.charge(1)?;
            if index > 0
                && compare(
                    context,
                    self.roots[index - 1].kernel.as_str(),
                    row.kernel.as_str(),
                )? != Ordering::Less
            {
                return Err(context.reject("root roles require unique ordered kernel identities"));
            }
        }
        // Only the borrowed role index is sorted; actual owner vectors are not.
        for kernel in &module.kernels {
            context.charge(1)?;
            let row = self
                .lookup(context, &kernel.id)?
                .ok_or_else(|| context.reject("actual kernel is absent from root role roster"))?;
            let function = self.entry(context, kernel, row)?;
            for (&role, ty) in row.parameters.iter().zip(&function.signature.parameters) {
                check_role(context, role, ty)?;
            }
        }
        Ok(())
    }

    pub(super) fn parameter(
        &self,
        context: &Context<'_>,
        kernel: &Kernel,
        function: &Function,
        ordinal: usize,
    ) -> Result<RootParameterRoleV29, LoweringErrors> {
        let row = self
            .lookup(context, &kernel.id)?
            .ok_or_else(|| context.reject("actual kernel is absent from root role roster"))?;
        let actual = self.entry(context, kernel, row)?;
        context.charge(1)?;
        if !std::ptr::eq(actual, function) {
            return Err(context.reject("root role query requires its original actual function"));
        }
        let role = *row
            .parameters
            .get(ordinal)
            .ok_or_else(|| context.reject("root role parameter ordinal is absent"))?;
        let ty = actual
            .signature
            .parameters
            .get(ordinal)
            .ok_or_else(|| context.reject("actual root parameter ordinal is absent"))?;
        check_role(context, role, ty)?;
        Ok(role)
    }
}

#[cfg(test)]
#[path = "storage_root_inline_abi_v29_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "storage_root_inline_abi_resources_v29_tests.rs"]
mod resources;
