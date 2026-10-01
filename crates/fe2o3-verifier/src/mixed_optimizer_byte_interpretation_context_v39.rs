//! A checked target interpretation, not source validity or paired admission.
//! One closed registry is emitted per inventory by the paired program builder;
//! functions borrow it and keep its exact owner/width/custody through emission.
use super::super::target_view_contracts_v38::TargetByteViewContractsV38 as Contracts;
use super::{FormalIndexWidth, Owner, Result, Writer};
use std::{fmt::Write as _, mem::size_of};

#[derive(Clone, Copy)]
enum Views<'a, 'owner> {
    Native,
    Classified {
        contracts: &'a Contracts<'a, 'owner>,
        namespace: usize,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct ByteInterpretationContextV39<'a, 'owner> {
    pub(super) width: FormalIndexWidth,
    views: Views<'a, 'owner>,
}

impl<'a, 'owner> ByteInterpretationContextV39<'a, 'owner> {
    pub(crate) const fn native(width: FormalIndexWidth) -> Self {
        Self {
            width,
            views: Views::Native,
        }
    }

    // Constructing this descriptor grants nothing: derive and every body query
    // check the retained classifier before any registry-dependent work.
    pub(in super::super) const fn classified(
        width: FormalIndexWidth,
        contracts: &'a Contracts<'a, 'owner>,
        namespace: usize,
    ) -> Self {
        Self {
            width,
            views: Views::Classified {
                contracts,
                namespace,
            },
        }
    }

    pub(super) fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()> {
        match self.views {
            Views::Native => Ok(()),
            Views::Classified { contracts, .. } => {
                contracts.check_owner_width_v39(owner, self.width, out)
            }
        }
    }

    pub(super) fn emit_state_predicate(
        &self,
        memory: &str,
        values: &str,
        little_endian: Option<&str>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let result = match (self.views, little_endian) {
            (Views::Native, _) => write!(out, "byte_native_view_inputs_v38({memory}, {values})"),
            (Views::Classified { namespace, .. }, Some(endian)) => write!(
                out,
                "byte_target_view_contracts_match_{namespace}_v38({memory}, {endian})"
            ),
            // Control transfer does not access memory or change the registry.
            // The next memory step again checks the actual explicit endianness.
            (Views::Classified { namespace, .. }, None) => write!(
                out,
                "(byte_target_view_contracts_match_{namespace}_v38({memory}, true) || byte_target_view_contracts_match_{namespace}_v38({memory}, false))"
            ),
        };
        result.map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    size_of::<ByteInterpretationContextV39<'_, '_>>()
        + size_of::<Views<'_, '_>>()
        + size_of::<(&Contracts<'_, '_>, &Owner, &mut Writer<'_, '_>)>()
        + size_of::<([&str; 3], Option<&str>, usize, std::fmt::Result, Result<()>)>()
}
