//! Complete original rustc descriptor projection for source-owned preparation.
//! These rows remain inert until the lowerer captures/replays the full source ABI.
use super::*;
use crate::production_pipeline::source_owned_v29::{Budget, Error, Resource, paid_vec};
use fe2o3_kernel_descriptor::{AliasSemantics, PhysicalAbiComponentKind, SourceTypeDescriptorV3};
use fe2o3_lower_mir_kernel::{
    ProductionKernelArgumentAbiArgumentV18 as Argument, ProductionKernelArgumentAbiKindV18 as Kind,
    ProductionKernelArgumentAbiRootV18 as Root,
};
use std::mem::{align_of, size_of};

#[path = "compiler_descriptor_source_mixed_v28.rs"]
pub(crate) mod mixed_v28;

// One private descriptor component contains exactly these six public field
// types. Paying each field's size plus alignment bounds all aggregate padding
// without exposing or relying on the private component's Rust layout.
const COMPONENT_BYTES: usize = size_of::<PhysicalAbiComponentKind>()
    + align_of::<PhysicalAbiComponentKind>()
    + size_of::<u32>()
    + align_of::<u32>()
    + 2 * (size_of::<u16>() + align_of::<u16>())
    + size_of::<AccessMode>()
    + align_of::<AccessMode>()
    + size_of::<AliasSemantics>()
    + align_of::<AliasSemantics>();

struct CapturedRoot<'a> {
    original: &'a TypedDescriptorRootV1,
    binding: [u8; 32],
    arguments: Vec<Argument>,
}

pub(crate) struct SourceAbi<'a> {
    roots: Vec<CapturedRoot<'a>>,
}

impl<'a> SourceAbi<'a> {
    pub(crate) fn capture(
        original: &'a [TypedDescriptorRootV1],
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        budget.reserve_storage(size_of::<Self>())?;
        let mut roots = paid_vec(original.len(), budget)?;
        for root in original {
            budget.charge_work(8)?;
            let mut arguments = paid_vec(root.arguments.as_slice().len(), budget)?;
            for (index, argument) in root.arguments.as_slice().iter().enumerate() {
                budget.charge_work(32)?;
                let kind = match argument.kind {
                    DescriptorArgumentKindV1::CompilerLaidOutByValue => {
                        if argument.access != AccessMode::ByValue {
                            return Err(Error::Unsupported("by-value descriptor access"));
                        }
                        Kind::CompilerLaidOutByValue {
                            offset: argument.offset,
                        }
                    }
                    kind => {
                        let (record_kind, source_kind, components) = descriptor_kind(kind);
                        // Descriptor constructors allocate exactly one or two
                        // components. Pay their complete backing and owned name.
                        let bytes = argument
                            .name
                            .len()
                            .checked_add(
                                COMPONENT_BYTES
                                    .checked_mul(components)
                                    .ok_or(Resource::Arithmetic)?,
                            )
                            .ok_or(Resource::Arithmetic)?;
                        budget.charge_work(bytes)?;
                        budget.reserve_storage(bytes)?;
                        let mut name = String::new();
                        name.try_reserve_exact(argument.name.len())
                            .map_err(|_| Resource::Allocation)?;
                        budget.reserve_storage(
                            name.capacity()
                                .checked_sub(argument.name.len())
                                .ok_or(Resource::Accounting)?,
                        )?;
                        name.push_str(&argument.name);
                        let (source, device) = descriptor_records(record_kind);
                        let index = u16::try_from(index).map_err(|_| Resource::Arithmetic)?;
                        let name =
                            ValidName::new(name).map_err(CompilerDescriptorError::Validation)?;
                        let logical = match record_kind {
                            DescriptorArgumentKindV1::Scalar(_) => LogicalArgumentV1::scalar(
                                index,
                                name,
                                &source,
                                &device,
                                argument.offset,
                            ),
                            DescriptorArgumentKindV1::SharedSlice(_) => {
                                LogicalArgumentV1::shared_slice(
                                    index,
                                    name,
                                    &source,
                                    &device,
                                    argument.offset,
                                )
                            }
                            DescriptorArgumentKindV1::DisjointSlice(_) => {
                                LogicalArgumentV1::disjoint_slice(
                                    index,
                                    name,
                                    &source,
                                    &device,
                                    argument.access,
                                    argument.offset,
                                )
                            }
                            DescriptorArgumentKindV1::GlobalMutPointer(_) => {
                                LogicalArgumentV1::global_mut_pointer(
                                    index,
                                    name,
                                    &source,
                                    &device,
                                    argument.offset,
                                )
                            }
                            _ => unreachable!("descriptor_kind returns a physical record kind"),
                        }
                        .map_err(CompilerDescriptorError::Validation)?;
                        if logical.access() != argument.access {
                            return Err(Error::Unsupported("original descriptor access changed"));
                        }
                        Kind::Descriptor {
                            source: source_kind,
                            argument: logical,
                        }
                    }
                };
                arguments.push(Argument {
                    semantic_type_identity: argument.semantic_type_identity,
                    kind,
                });
            }
            roots.push(CapturedRoot {
                original: root,
                binding: root.kernel_binding_bytes(),
                arguments,
            });
        }
        Ok(Self { roots })
    }

    pub(crate) fn roots(&self, budget: &mut Budget<'_>) -> Result<Vec<Root<'_>>, Error> {
        let mut roots = paid_vec(self.roots.len(), budget)?;
        for root in &self.roots {
            budget.charge_work(6)?;
            roots.push(Root {
                kernel_binding: &root.binding,
                export: root.original.entry_symbol(),
                arguments: &root.arguments,
                explicit_argument_bytes: root.original.explicit_argument_bytes,
                kernarg_alignment_bytes: root.original.kernarg_alignment_bytes,
            });
        }
        Ok(roots)
    }
}

fn descriptor_kind(
    kind: DescriptorArgumentKindV1,
) -> (DescriptorArgumentKindV1, SourceTypeDescriptorV3, usize) {
    use DescriptorArgumentKindV1 as Original;
    use SourceTypeDescriptorV3 as Source;
    match kind {
        Original::Scalar(scalar) => (kind, Source::Scalar(scalar), 1),
        Original::SharedSlice(scalar) => (kind, Source::SharedSlice(scalar), 2),
        Original::DisjointSlice(scalar) => (kind, Source::DisjointSlice(scalar), 2),
        Original::GlobalMutPointer(scalar) => (kind, Source::GlobalMutPointer(scalar), 1),
        // Physical integer records do not erase the original nominal source kind.
        Original::CompilerLaidOutUsize => (Original::Scalar(ScalarTypeV1::U64), Source::Usize, 1),
        Original::CompilerLaidOutIsize => (Original::Scalar(ScalarTypeV1::I64), Source::Isize, 1),
        Original::CompilerLaidOutByValue => {
            unreachable!("by-value layout has no descriptor components")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    fn fixture() -> TypedDescriptorRootV1 {
        let kinds = [
            (
                DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32),
                AccessMode::ByValue,
            ),
            (
                DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
                AccessMode::ReadOnly,
            ),
            (
                DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::U32),
                AccessMode::WriteOnly,
            ),
            (
                DescriptorArgumentKindV1::GlobalMutPointer(ScalarTypeV1::U32),
                AccessMode::ReadWrite,
            ),
            (
                DescriptorArgumentKindV1::CompilerLaidOutByValue,
                AccessMode::ByValue,
            ),
            (
                DescriptorArgumentKindV1::CompilerLaidOutUsize,
                AccessMode::ByValue,
            ),
            (
                DescriptorArgumentKindV1::CompilerLaidOutIsize,
                AccessMode::ByValue,
            ),
        ];
        let arguments = kinds
            .into_iter()
            .enumerate()
            .map(|(index, (kind, access))| TypedDescriptorArgumentV1 {
                name: format!("arg{index}"),
                kind,
                access,
                offset: index as u32 * 16,
                layout: None,
                source_size: 16,
                source_alignment: 8,
                rustc_abi_class: RustcAbiClassV1::ScalarPair,
                semantic_type_identity: SemanticTypeIdentityV1::from_sha256([index as u8; 32]),
                semantic_layout_identity:
                    fe2o3_mir_model::semantic_mir_v1::SemanticLayoutIdentityV1::from_sha256(
                        [index as u8; 32],
                    ),
            })
            .collect();
        TypedDescriptorRootV1 {
            logical_name: "projection".to_owned(),
            export_name: "kernel_projection".to_owned(),
            kernel_binding: KernelBindingIdV1::from_bytes([17; 32]),
            arguments: TypedArgumentListV1::new(arguments).unwrap(),
            explicit_argument_bytes: 112,
            kernarg_alignment_bytes: 8,
            source_launch: None,
        }
    }

    #[test]
    fn full_original_abi_projection_preserves_types_order_access_and_physical_components() {
        let original = [fixture()];
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        let captured = SourceAbi::capture(&original, &mut budget).unwrap();
        let roots = captured.roots(&mut budget).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(*roots[0].kernel_binding, [17; 32]);
        assert_eq!(roots[0].export, "kernel_projection");
        assert_eq!(
            (
                roots[0].explicit_argument_bytes,
                roots[0].kernarg_alignment_bytes
            ),
            (112, 8)
        );
        let expected = [
            Some(SourceTypeDescriptorV3::Scalar(ScalarTypeV1::U32)),
            Some(SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::U32)),
            Some(SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32)),
            Some(SourceTypeDescriptorV3::GlobalMutPointer(ScalarTypeV1::U32)),
            None,
            Some(SourceTypeDescriptorV3::Usize),
            Some(SourceTypeDescriptorV3::Isize),
        ];
        for (index, ((argument, original), expected)) in roots[0]
            .arguments
            .iter()
            .zip(original[0].arguments.as_slice())
            .zip(expected)
            .enumerate()
        {
            assert_eq!(
                argument.semantic_type_identity,
                original.semantic_type_identity
            );
            match (&argument.kind, expected) {
                (Kind::CompilerLaidOutByValue { offset }, None) => {
                    assert_eq!(*offset, 64);
                    assert_ne!(original.source_size, 0, "not only a ZST fixture");
                }
                (Kind::Descriptor { source, argument }, Some(expected)) => {
                    assert_eq!(*source, expected);
                    assert_eq!(argument.source_index(), index as u16);
                    assert_eq!(argument.name().as_str(), original.name);
                    assert_eq!(argument.access(), original.access);
                    let components: Vec<_> = argument.physical_components().collect();
                    assert_eq!(
                        components.len(),
                        if index == 1 || index == 2 { 2 } else { 1 }
                    );
                    assert_eq!(components[0].1, original.offset);
                    if components.len() == 2 {
                        assert_eq!(components[1].1, original.offset + 8);
                    }
                    assert_eq!(
                        argument.alias(),
                        match index {
                            1 => AliasSemantics::SharedReadOnly,
                            2 | 3 => AliasSemantics::Exclusive,
                            _ => AliasSemantics::Value,
                        }
                    );
                }
                _ => panic!("projection changed an original kind"),
            }
        }
    }

    #[test]
    fn full_original_abi_projection_refuses_access_substitution() {
        for index in [0, 1, 3, 4, 5, 6] {
            let mut original = fixture();
            let mut arguments = original.arguments.as_slice().to_vec();
            arguments[index].access = if index == 3 {
                AccessMode::ReadOnly
            } else {
                AccessMode::ReadWrite
            };
            original.arguments = TypedArgumentListV1::new(arguments).unwrap();
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 100_000);
            let original = [original];
            let refused = SourceAbi::capture(&original, &mut budget);
            let expected = if index == 4 {
                "by-value descriptor access"
            } else {
                "original descriptor access changed"
            };
            assert!(matches!(refused, Err(Error::Unsupported(actual)) if actual == expected));
        }
    }

    #[test]
    fn full_original_abi_projection_exact_and_short_storage_have_an_independent_oracle() {
        let original = [fixture()];
        let component = size_of::<PhysicalAbiComponentKind>()
            + align_of::<PhysicalAbiComponentKind>()
            + size_of::<u32>()
            + align_of::<u32>()
            + 2 * (size_of::<u16>() + align_of::<u16>())
            + size_of::<AccessMode>()
            + align_of::<AccessMode>()
            + size_of::<AliasSemantics>()
            + align_of::<AliasSemantics>();
        // Six descriptor names, eight physical components, one non-ZST by-value row.
        let capture_bytes = size_of::<SourceAbi<'_>>()
            + size_of::<CapturedRoot<'_>>()
            + 7 * size_of::<Argument>()
            + 6 * 4
            + 8 * component;
        let total = capture_bytes + size_of::<Root<'_>>();
        for limit in [total - 1, total] {
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, limit);
            let captured = SourceAbi::capture(&original, &mut budget).unwrap();
            assert_eq!(budget.storage(), capture_bytes);
            let roots = captured.roots(&mut budget);
            if limit == total {
                assert_eq!(roots.unwrap().len(), 1);
                assert_eq!(budget.storage(), total);
            } else {
                assert!(
                    matches!(roots, Err(Error::Resource(Resource::Storage(error))) if error.actual() == total && error.limit() == limit)
                );
                assert_eq!(budget.storage(), capture_bytes);
            }
        }
    }
}
