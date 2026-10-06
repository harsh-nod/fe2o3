// Borrowed entry requirements only. The retained profile was checked against
// the original SSA owner before SourceDescriptorRootAbiV29 was lent.
pub(super) struct SourceSharedEntryAbiV18<'a> {
    profile: &'a CapturedKernelArgumentAbiV18,
    root: &'a Root,
    argument: &'a Argument,
    ordinal: u32,
    scalar: DescriptorScalar,
}

impl SourceSharedEntryAbiV18<'_> {
    pub(super) fn function(&self) -> SemanticFunctionIdV1 {
        self.root.function
    }
    pub(super) fn argument(&self) -> u32 {
        self.ordinal
    }
    pub(super) fn ty(&self) -> SemanticTypeIdV1 {
        self.argument.ty
    }
    pub(super) fn identity(&self) -> SemanticTypeIdentityV1 {
        self.argument.identity
    }
    pub(super) fn source(&self) -> &[u8; 32] {
        &self.profile.source
    }
    pub(super) fn binding(&self) -> &[u8; 32] {
        &self.root.binding
    }
    pub(super) fn scalar(&self) -> ScalarType {
        descriptor_scalar(self.scalar)
    }
}

impl<'a> SourceDescriptorRootAbiV29<'a> {
    // The enclosing shared-entry scope prepays shared_entry_abi_headers_v18.
    // No new allocation or type/layout traversal occurs in this borrowed query.
    pub(super) fn shared_entry_v18(
        self,
        argument: u32,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceSharedEntryAbiV18<'a>>, ProductionSemanticKirErrorV1> {
        budget.charge_work(12)?;
        let root = self
            .profile
            .roots
            .get(self.original_root)
            .ok_or_else(mismatch)?;
        let ordinal = usize::try_from(argument).map_err(|_| mismatch())?;
        let index = root
            .arguments
            .start
            .checked_add(ordinal)
            .ok_or_else(mismatch)?;
        if index >= root.arguments.end {
            return Err(mismatch());
        }
        let row = self.profile.arguments.get(index).ok_or_else(mismatch)?;
        if row.ty != ty {
            return Err(mismatch());
        }
        let Kind::Descriptor(SourceTypeDescriptorV3::SharedSlice(scalar)) = row.kind else {
            return Ok(None);
        };
        if row.ownership != SemanticSourceArgumentOwnershipV1::SharedBorrow
            || row.access != DescriptorAccess::ReadOnly
            || row.alias != AliasSemantics::SharedReadOnly
            || !matches!(
                row.components,
                [
                    Some(Component {
                        kind: PhysicalAbiComponentKind::GlobalPointer,
                        size: 8,
                        alignment: 8,
                        ..
                    }),
                    Some(Component {
                        kind: PhysicalAbiComponentKind::SliceLengthU64,
                        size: 8,
                        alignment: 8,
                        ..
                    })
                ]
            )
        {
            return Err(mismatch());
        }
        Ok(Some(SourceSharedEntryAbiV18 {
            profile: self.profile,
            root,
            argument: row,
            ordinal: argument,
            scalar,
        }))
    }
}

pub(super) fn shared_entry_abi_headers_v18() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<(
            SourceDescriptorRootAbiV29<'_>,
            u32,
            SemanticTypeIdV1,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<SourceSharedEntryAbiV18<'_>>()?,
        h::<Option<SourceSharedEntryAbiV18<'_>>>()?,
        h::<&Root>()?,
        h::<Option<&Root>>()?,
        h::<&Argument>()?,
        h::<Option<&Argument>>()?,
        h::<[usize; 2]>()?,
        h::<Option<usize>>()?,
        h::<Result<usize, std::num::TryFromIntError>>()?,
        h::<DescriptorScalar>()?,
        h::<Kind>()?,
        h::<[Option<Component>; 2]>()?,
        h::<bool>()?,
        h::<SourceDescriptorRootAbiV29<'_>>()?,
        h::<&SourceSharedEntryAbiV18<'_>>()?,
        h::<ScalarType>()?,
        h::<SemanticTypeIdentityV1>()?,
        h::<SemanticFunctionIdV1>()?,
        h::<SemanticTypeIdV1>()?,
        h::<u32>()?,
        h::<&[u8; 32]>()?,
        h::<AliasSemantics>()?,
        h::<DescriptorAccess>()?,
        h::<SemanticSourceArgumentOwnershipV1>()?,
        h::<Result<(), ArgumentResourceV1>>()?,
    ])
}

#[cfg(test)]
#[test]
fn shared_entry_abi_fixed_frame_equation_is_independent() {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let expected = [
        h::<(
            SourceDescriptorRootAbiV29<'_>,
            u32,
            SemanticTypeIdV1,
            &mut ArgumentBudgetV1<'_>,
        )>(),
        h::<SourceSharedEntryAbiV18<'_>>(),
        h::<Option<SourceSharedEntryAbiV18<'_>>>(),
        h::<&Root>(),
        h::<Option<&Root>>(),
        h::<&Argument>(),
        h::<Option<&Argument>>(),
        h::<[usize; 2]>(),
        h::<Option<usize>>(),
        h::<Result<usize, std::num::TryFromIntError>>(),
        h::<DescriptorScalar>(),
        h::<Kind>(),
        h::<[Option<Component>; 2]>(),
        h::<bool>(),
        h::<SourceDescriptorRootAbiV29<'_>>(),
        h::<&SourceSharedEntryAbiV18<'_>>(),
        h::<ScalarType>(),
        h::<SemanticTypeIdentityV1>(),
        h::<SemanticFunctionIdV1>(),
        h::<SemanticTypeIdV1>(),
        h::<u32>(),
        h::<&[u8; 32]>(),
        h::<AliasSemantics>(),
        h::<DescriptorAccess>(),
        h::<SemanticSourceArgumentOwnershipV1>(),
        h::<Result<(), ArgumentResourceV1>>(),
    ]
    .into_iter()
    .sum::<usize>();
    assert_eq!(shared_entry_abi_headers_v18().unwrap(), expected);
}
