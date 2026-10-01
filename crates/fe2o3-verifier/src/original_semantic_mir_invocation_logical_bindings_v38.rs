//! Dynamic witness obligations paired with exact owner-backed SSA locators.

use super::super::source_bytes::descriptor_loans::Recipe as DescriptorRecipe;
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionSourceReferenceCarrierV38 as Carrier, ProductionSourceReferenceEndpointV38,
    ProductionSourceSsaEndpointV36,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum LogicalBinding {
    Plain,
    Witness {
        source_type: u32,
    },
    DescriptorReference(DescriptorRecipe),
    Reference {
        source_type: u32,
        origin: usize,
        generation: u32,
        instance: usize,
        block: u32,
        statement: usize,
    },
}

impl LogicalBinding {
    pub(super) fn derive(
        slots: &SourceSlots<'_, '_>,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        endpoint: &ProductionSourceSsaEndpointV36<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.charge_work(3)?;
        let ty = endpoint.source_type(out.budget)?;
        if slots.witness_class(ty, out)?.is_some() {
            return Ok(Self::Witness {
                source_type: ty.index(),
            });
        }
        let Some(reference) = endpoint.reference(out.budget)? else {
            return Ok(Self::Plain);
        };
        match reference.carrier(out.budget)? {
            Carrier::MemoryPointer => return Ok(Self::Plain),
            Carrier::DescriptorSlice => {
                return Ok(Self::DescriptorReference(DescriptorRecipe::derive(
                    slots, plan, root, endpoint, out,
                )?));
            }
            Carrier::StableScalar => (),
        }
        let origin_type = reference.origin_type(out.budget)?;
        if slots.witness_class(origin_type, out)?.is_none() {
            return Err(Error::Statement(
                "original MIR stable scalar referent has no intrinsic witness semantics",
            ));
        }
        let origin_instance = reference.origin_instance(out.budget)?;
        let origin_row = plan.instance(root, origin_instance, out)?;
        let origin_local = reference.origin_local(out.budget)?.index() as usize;
        out.budget.charge_work(5)?;
        if origin_row.function != reference.origin_function(out.budget)?
            || origin_local >= origin_row.locals.len()
        {
            return Err(mismatch());
        }
        let (instance, block, statement) = reference.borrow_site(out.budget)?;
        Ok(Self::Reference {
            source_type: origin_type.index(),
            origin: add(origin_row.locals.start, origin_local)?,
            generation: reference.origin_generation(out.budget)?,
            instance,
            block: block.index(),
            statement: statement.ok_or_else(mismatch)?,
        })
    }
}

pub(super) fn headers() -> usize {
    size_of::<LogicalBinding>()
        + 2 * size_of::<Result<LogicalBinding>>()
        + size_of::<Option<ProductionSourceReferenceEndpointV38<'_, '_>>>()
        + size_of::<Result<Option<ProductionSourceReferenceEndpointV38<'_, '_>>>>()
        + 8 * size_of::<usize>()
        + 4 * size_of::<&()>()
}
