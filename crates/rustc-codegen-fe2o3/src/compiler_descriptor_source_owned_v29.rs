//! Original rustc descriptor projection for the closed scalar source route.
//! These rows remain inert until the lowerer captures/replays the full source ABI.
use super::*;
use crate::production_pipeline::source_owned_v29::{Budget, Error, Resource, paid_vec};
use fe2o3_kernel_descriptor::{AliasSemantics, PhysicalAbiComponentKind, SourceTypeDescriptorV3};
use fe2o3_lower_mir_kernel::{
    ProductionKernelArgumentAbiArgumentV18 as Argument, ProductionKernelArgumentAbiKindV18 as Kind,
    ProductionKernelArgumentAbiRootV18 as Root,
};
use std::mem::{align_of, size_of};

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

pub(crate) struct ScalarAbi<'a> {
    roots: Vec<CapturedRoot<'a>>,
}

impl<'a> ScalarAbi<'a> {
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
                    DescriptorArgumentKindV1::Scalar(scalar) => {
                        if argument.access != AccessMode::ByValue {
                            return Err(Error::Unsupported("scalar descriptor access"));
                        }
                        // A fixed one-component descriptor owns its name and one
                        // component. Pay both requested payloads before construction.
                        let bytes = argument
                            .name
                            .len()
                            .checked_add(COMPONENT_BYTES)
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
                        let (source, device) = descriptor_records(argument.kind);
                        let logical = LogicalArgumentV1::scalar(
                            u16::try_from(index).map_err(|_| Resource::Arithmetic)?,
                            ValidName::new(name).map_err(CompilerDescriptorError::Validation)?,
                            &source,
                            &device,
                            argument.offset,
                        )
                        .map_err(CompilerDescriptorError::Validation)?;
                        Kind::Descriptor {
                            source: SourceTypeDescriptorV3::Scalar(scalar),
                            argument: logical,
                        }
                    }
                    DescriptorArgumentKindV1::CompilerLaidOutByValue
                        if argument.source_size == 0 =>
                    {
                        Kind::CompilerLaidOutByValue {
                            offset: argument.offset,
                        }
                    }
                    _ => return Err(Error::Unsupported("closed scalar descriptor kind")),
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
