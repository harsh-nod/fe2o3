//! Numeric packing from the exact native closed ABI, never a V1 descriptor cast.
use super::*;
use crate::{CheckedNativeConditionalFillAbiV1 as Abi, NativeConditionalFillAbiErrorV1 as Error};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

const STORAGE: usize = size_of::<GeneratedArgumentPackingPlanV1>()
    + size_of::<GeneratedArgumentPackingPlanSealV1>()
    + 4 * size_of::<usize>()
    + size_of::<AbiField>()
    + 4
    + 2 * size_of::<GeneratedPackingComponentV1>();

pub(crate) fn prepare(
    abi: &Abi<'_>,
    generated: &CompilerGeneratedArgumentLayoutV1,
    budget: &mut Budget<'_>,
) -> Result<(GeneratedArgumentPackingPlanV1, usize), Error> {
    let owner = abi.owner();
    let floor=owner.storage().retained_storage().checked_add(owner.handoff().backing_capacity())
        .and_then(|n|n.checked_add(fe2o3_compiler_ffi::INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5))
        .and_then(|n|n.checked_add(size_of::<Abi<'_>>() + size_of::<CompilerGeneratedArgumentLayoutV1>()
            + size_of::<AbiField>() + 4 + size_of::<Option<RustDisjointIndexSpaceV1>>()))
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(floor, 8, 4096, 16 * 1024, |_| {
        check(generated)?;
        Ok((
            packing_plan_from_layout(KernelId::from_bytes(*abi.kernel_id()), &generated.layout),
            STORAGE,
        ))
    })
}

fn check(generated: &CompilerGeneratedArgumentLayoutV1) -> Result<(), Error> {
    let layout = &generated.layout;
    let [field] = layout.fields() else {
        return Err(Error::Mismatch("native generated output cardinality"));
    };
    if layout.size() != 16
        || layout.alignment() != 8
        || layout.pointer_width() != PointerWidth::Bits64
        || generated.disjoint_index_spaces.as_ref() != [Some(RustDisjointIndexSpaceV1::Index1D)]
        || field.name().as_str() != "arg0"
        || field.offset() != 0
        || field.size() != 16
        || field.alignment() != 8
        || field.kind()
            != (AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            })
        || field.mutability() != Mutability::Mutable
        || field.access() != Access::WriteOnly
        || field.address_space() != AddressSpace::Global
        || field.type_identity()
            != u32::disjoint_slice_type_identity_for_index_space_v1(
                PointerWidth::Bits64,
                RustDisjointIndexSpaceV1::Index1D,
            )
        || field.ownership() != ArgumentOwnership::UniqueBorrow
        || field.alias_class() != AliasClass::Exclusive
    {
        return Err(Error::Mismatch(
            "exact native generated u32/Index1D physical layout",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifacts::Name;

    fn layout(mutation: u8) -> CompilerGeneratedArgumentLayoutV1 {
        let field = AbiField::new(
            Name::new(if mutation == 1 { "other" } else { "arg0" }).unwrap(),
            0,
            16,
            8,
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            },
            Mutability::Mutable,
            if mutation == 2 {
                Access::ReadWrite
            } else {
                Access::WriteOnly
            },
            AddressSpace::Global,
            if mutation == 3 {
                i32::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
            } else {
                u32::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
            },
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
        )
        .unwrap();
        CompilerGeneratedArgumentLayoutV1::new_with_disjoint_index_spaces_v1(
            if mutation == 4 { 24 } else { 16 },
            8,
            PointerWidth::Bits64,
            vec![field],
            vec![if mutation == 5 {
                None
            } else {
                Some(RustDisjointIndexSpaceV1::Index1D)
            }],
        )
        .unwrap()
    }

    #[test]
    fn native_closed_packing_accepts_only_exact_inert_layout() {
        check(&layout(0)).unwrap();
        for mutation in 1..=5 {
            assert!(check(&layout(mutation)).is_err(), "mutation {mutation}");
        }
        let generated = layout(0);
        // Only this physical layout helper is tested. No native V5/source owner
        // or execution authority is fabricated by an inert layout fixture.
        let plan = packing_plan_from_layout(KernelId::from_bytes([7; 32]), &generated.layout);
        assert_eq!(plan.kernarg_size(), 16);
        assert_eq!(plan.kernarg_alignment(), 8);
        assert_eq!(plan.argument_count(), 1);
        assert_eq!(plan.component_count(), 2);
        assert!(
            STORAGE
                >= size_of_val(&plan)
                    + size_of::<AbiField>()
                    + 2 * size_of::<GeneratedPackingComponentV1>()
        );
    }
}
