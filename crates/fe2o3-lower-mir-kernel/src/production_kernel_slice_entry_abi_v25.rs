// This is the original argument's conditional ABI contract, not a runtime
// allocation, initialized-region, non-aliasing, or concurrency certificate.
pub(super) struct SourceSliceEntryAbiV25<'a> {
    profile: &'a CapturedKernelArgumentAbiV18,
    root: &'a Root,
    argument: &'a Argument,
    ordinal: u32,
    scalar: DescriptorScalar,
    exclusive: bool,
}

impl SourceSliceEntryAbiV25<'_> {
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
    pub(super) fn is_exclusive_contract(&self) -> bool {
        self.exclusive
    }

    pub(super) fn components_v36(&self) -> [(u32, u16, u16); 2] {
        self.argument.components.map(|component| {
            let component = component.expect("checked source slice ABI component");
            (component.offset, component.size, component.alignment)
        })
    }

    pub(super) fn allows_reads(&self) -> bool {
        matches!(
            self.argument.access,
            DescriptorAccess::ReadOnly | DescriptorAccess::ReadWrite
        )
    }

    pub(super) fn allows_writes(&self) -> bool {
        matches!(
            self.argument.access,
            DescriptorAccess::WriteOnly | DescriptorAccess::ReadWrite
        )
    }
}

impl<'a> SourceDescriptorRootAbiV29<'a> {
    pub(super) fn argument_count_v25(
        self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let root = self
            .profile
            .roots
            .get(self.original_root)
            .ok_or_else(mismatch)?;
        root.arguments
            .end
            .checked_sub(root.arguments.start)
            .ok_or_else(mismatch)
    }

    // Caller pays slice_entry_abi_headers_v25 before entering this allocation-
    // free query. The profile itself was authenticated against the actual SSA
    // source, descriptor components, ownership and nominal registration.
    pub(super) fn slice_entry_v25(
        self,
        argument: u32,
        ty: SemanticTypeIdV1,
        physical: &Type,
        writing: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceSliceEntryAbiV25<'a>>, ProductionSemanticKirErrorV1> {
        self.slice_parameter_contract_v26(argument, ty, physical, Some(writing), budget)
    }

    // A declaration has an ABI even when it has no memory-access occurrence.
    // None below requests no read/write authority, including for WriteOnly.
    pub(super) fn slice_parameter_v26(
        self,
        argument: u32,
        ty: SemanticTypeIdV1,
        physical: &Type,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceSliceEntryAbiV25<'a>>, ProductionSemanticKirErrorV1> {
        self.slice_parameter_contract_v26(argument, ty, physical, None, budget)
    }

    pub(super) fn slice_parameter_type_v26(
        self,
        argument: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SemanticTypeIdV1>, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
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
        Ok(match row.kind {
            Kind::Descriptor(
                SourceTypeDescriptorV3::SharedSlice(_) | SourceTypeDescriptorV3::DisjointSlice(_),
            ) => Some(row.ty),
            _ => None,
        })
    }

    fn slice_parameter_contract_v26(
        self,
        argument: u32,
        ty: SemanticTypeIdV1,
        physical: &Type,
        writing: Option<bool>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceSliceEntryAbiV25<'a>>, ProductionSemanticKirErrorV1> {
        budget.charge_work(24)?;
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
        let (scalar, exclusive) = match row.kind {
            Kind::Descriptor(SourceTypeDescriptorV3::SharedSlice(scalar)) => (scalar, false),
            Kind::Descriptor(SourceTypeDescriptorV3::DisjointSlice(scalar)) => (scalar, true),
            // In particular, GlobalMutPointer has no authenticated extent.
            _ => return Ok(None),
        };
        let expected_ownership = if exclusive {
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner
        } else {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        };
        let expected_alias = if exclusive {
            AliasSemantics::Exclusive
        } else {
            AliasSemantics::SharedReadOnly
        };
        let access_permits = match (row.access, writing) {
            (_, None) | (DescriptorAccess::ReadWrite, _) => true,
            (DescriptorAccess::ReadOnly, Some(false))
            | (DescriptorAccess::WriteOnly, Some(true)) => true,
            _ => false,
        };
        let Type::Slice(slice) = physical else {
            return Err(mismatch());
        };
        if row.ownership != expected_ownership
            || row.alias != expected_alias
            || (!exclusive && (writing == Some(true) || row.access != DescriptorAccess::ReadOnly))
            || !access_permits
            || slice.address_space != AddressSpace::Global
            || slice.element.as_ref() != &Type::Scalar(descriptor_scalar(scalar))
            || !access_matches(slice.access, row.access)
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
                    }),
                ]
            )
        {
            return Err(mismatch());
        }
        Ok(Some(SourceSliceEntryAbiV25 {
            profile: self.profile,
            root,
            argument: row,
            ordinal: argument,
            scalar,
            exclusive,
        }))
    }
}

pub(super) fn slice_entry_abi_headers_v25() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        SourceDescriptorRootAbiV29<'a>,
        u32,
        SemanticTypeIdV1,
        &'a Type,
        bool,
        Option<bool>,
        &'a mut ArgumentBudgetV1<'a>,
        &'a Root,
        Option<&'a Root>,
        &'a Argument,
        Option<&'a Argument>,
        [usize; 2],
        Option<usize>,
        Result<usize, std::num::TryFromIntError>,
        (DescriptorScalar, bool),
        SourceSliceEntryAbiV25<'a>,
        Option<SourceSliceEntryAbiV25<'a>>,
        DescriptorAccess,
        AliasSemantics,
        SemanticSourceArgumentOwnershipV1,
        [Option<Component>; 2],
        Type,
        ScalarType,
        &'a fe2o3_kernel_ir::SliceType,
        bool,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        argument_product_v1(
            2,
            size_of::<Result<Frame<'_>, ProductionSemanticKirErrorV1>>(),
        )?,
        size_of::<Result<(), ArgumentResourceV1>>(),
    ])
}

#[cfg(test)]
impl SourceDescriptorRootAbiV29<'_> {
    pub(super) fn check_slice_entry_shapes_v25(
        self,
        argument: u32,
        ty: SemanticTypeIdV1,
        physical: &Type,
        exclusive: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Type::Slice(slice) = physical else {
            panic!("genuine source slice parameter");
        };
        let entry = self.slice_entry_v25(argument, ty, physical, false, budget)?;
        let entry = entry.expect("captured source slice ABI");
        let parameter = self
            .slice_parameter_v26(argument, ty, physical, budget)?
            .expect("captured source slice declaration");
        assert_eq!(self.slice_parameter_type_v26(argument, budget)?, Some(ty));
        assert_eq!(parameter.argument(), entry.argument());
        assert_eq!(parameter.identity(), entry.identity());
        assert_eq!(parameter.scalar(), entry.scalar());
        assert_eq!(
            parameter.is_exclusive_contract(),
            entry.is_exclusive_contract()
        );
        assert_eq!(entry.argument(), argument);
        assert_eq!(entry.ty(), ty);
        assert_eq!(entry.is_exclusive_contract(), exclusive);
        assert!(entry.allows_reads());
        assert_eq!(entry.allows_writes(), exclusive);
        assert!(matches!(
            self.slice_entry_v25(u32::MAX, ty, physical, false, budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "kernel argument ABI profile differs from the complete original descriptor/source contract",
                ..
            })
        ));
        assert!(matches!(
            self.slice_entry_v25(
                argument,
                SemanticTypeIdV1::from_index(u32::MAX),
                physical,
                false,
                budget,
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "kernel argument ABI profile differs from the complete original descriptor/source contract",
                ..
            })
        ));
        for changed in [
            Type::Scalar(ScalarType::U32),
            Type::slice(
                (*slice.element).clone(),
                AddressSpace::Workgroup,
                slice.access,
            ),
            Type::slice(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Global,
                slice.access,
            ),
            Type::slice(
                (*slice.element).clone(),
                AddressSpace::Global,
                if exclusive {
                    AccessMode::ReadOnly
                } else {
                    AccessMode::ReadWrite
                },
            ),
        ] {
            assert!(
                self.slice_parameter_v26(argument, ty, &changed, budget)
                    .is_err()
            );
            assert!(matches!(
                self.slice_entry_v25(argument, ty, &changed, false, budget),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "kernel argument ABI profile differs from the complete original descriptor/source contract",
                    ..
                })
            ));
        }
        let write = self.slice_entry_v25(argument, ty, physical, true, budget);
        if exclusive {
            assert!(write?.is_some());
        } else {
            assert!(matches!(
                write,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "kernel argument ABI profile differs from the complete original descriptor/source contract",
                    ..
                })
            ));
        }
        Ok(())
    }
}
