use std::fmt;

use crate::{
    AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, Mutability,
    PointerWidth,
};

/// A semantically validated view of the ABI subset supported by typed host launch.
///
/// Construction checks only properties already declared by an [`AbiLayout`]. It
/// does not establish where those declarations came from, bind them to a code
/// object, inspect a payload, match a compiler-generated host descriptor, select
/// a device, or authorize loading or launch. A runtime must establish those
/// independent facts before treating an ABI as authoritative.
///
/// The current subset permits scalar values and 64-bit global or generic
/// references represented by ordinary shared or unique Rust borrows. Atomic
/// references need operation, width, ordering, scope, and host-coherence
/// contracts that manifest v1 does not carry. Constant, workgroup, and private
/// address spaces need address-space-specific physical ABI and provenance rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostLaunchAbi<'layout> {
    layout: &'layout AbiLayout,
}

impl<'layout> HostLaunchAbi<'layout> {
    /// Semantically validates `layout` against the current host-launch subset.
    ///
    /// Success is deliberately not artifact authority. It means only that the
    /// supplied, already-valid model uses field contracts the future typed host
    /// launcher knows how to represent conservatively.
    pub fn validate(layout: &'layout AbiLayout) -> Result<Self, HostLaunchAbiError> {
        if layout.pointer_width() != PointerWidth::Bits64 {
            return Err(HostLaunchAbiError::UnsupportedPointerWidth(
                layout.pointer_width(),
            ));
        }

        for (field_index, field) in layout.fields().iter().enumerate() {
            match field.kind() {
                AbiKind::Scalar(_) => continue,
                AbiKind::Pointer { .. } | AbiKind::Slice { .. } => {}
            }

            let address_space = field.address_space();
            if !matches!(address_space, AddressSpace::Global | AddressSpace::Generic) {
                return Err(HostLaunchAbiError::UnsupportedAddressSpace {
                    field_index,
                    address_space,
                });
            }

            let supported_contract = match (
                field.mutability(),
                field.access(),
                field.ownership(),
                field.alias_class(),
            ) {
                (
                    Mutability::Immutable,
                    Access::ReadOnly,
                    ArgumentOwnership::SharedBorrow,
                    AliasClass::SharedReadOnly,
                ) => true,
                (
                    Mutability::Mutable,
                    access,
                    ArgumentOwnership::UniqueBorrow,
                    AliasClass::Exclusive,
                ) => matches!(
                    access,
                    Access::ReadOnly | Access::WriteOnly | Access::ReadWrite
                ),
                _ => false,
            };

            if !supported_contract {
                return Err(HostLaunchAbiError::UnsupportedReferenceContract {
                    field_index,
                    mutability: field.mutability(),
                    access: field.access(),
                    ownership: field.ownership(),
                    alias_class: field.alias_class(),
                });
            }
        }

        Ok(Self { layout })
    }

    /// Returns the structurally validated layout represented by this view.
    pub const fn layout(self) -> &'layout AbiLayout {
        self.layout
    }
}

/// Why an [`AbiLayout`] is outside the currently supported host-launch subset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HostLaunchAbiError {
    /// Typed host launch currently requires 64-bit device pointers.
    UnsupportedPointerWidth(PointerWidth),
    /// A reference uses an address space without a supported host representation.
    UnsupportedAddressSpace {
        field_index: usize,
        address_space: AddressSpace,
    },
    /// A reference does not have one of the supported borrow and alias contracts.
    UnsupportedReferenceContract {
        field_index: usize,
        mutability: Mutability,
        access: Access,
        ownership: ArgumentOwnership,
        alias_class: AliasClass,
    },
}

impl fmt::Display for HostLaunchAbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPointerWidth(width) => {
                write!(f, "host launch does not support {width:?} pointers")
            }
            Self::UnsupportedAddressSpace {
                field_index,
                address_space,
            } => write!(
                f,
                "ABI field {field_index} uses unsupported host-launch address space {address_space:?}"
            ),
            Self::UnsupportedReferenceContract {
                field_index,
                mutability,
                access,
                ownership,
                alias_class,
            } => write!(
                f,
                "ABI field {field_index} has unsupported host-launch reference contract \
                 ({mutability:?}, {access:?}, {ownership:?}, {alias_class:?})"
            ),
        }
    }
}

impl std::error::Error for HostLaunchAbiError {}
