use std::fmt;

use crate::generated_argument_plan::GeneratedArgumentPackError;
use crate::generated_kfd_arguments::{GeneratedKfdArgumentError, GeneratedKfdPrepareError};

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedRuntimeArgumentErrorV1 {
    Prepare(GeneratedKfdPrepareError),
    Pack(GeneratedArgumentPackError),
    Binding(GeneratedKfdArgumentError),
    ByteLength,
    PayloadLimit,
    BindingMismatch,
    StaleOrAliasedOutput,
    OutputUnavailable,
    Allocation,
    Custody,
    ResultCredit(fe2o3_resource_accounting::ResourceCreditErrorV1),
}

impl fmt::Display for GeneratedRuntimeArgumentErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prepare(error) => write!(f, "owned generated preparation failed: {error}"),
            Self::Pack(error) => write!(f, "owned generated layout binding failed: {error}"),
            Self::Binding(error) => write!(f, "owned generated packing failed: {error}"),
            Self::ByteLength => f.write_str("owned generated storage byte length overflows"),
            Self::PayloadLimit => {
                f.write_str("owned generated storage exceeds its byte or binding limit")
            }
            Self::BindingMismatch => {
                f.write_str("owned generated binding type, shape or budget differs")
            }
            Self::StaleOrAliasedOutput => {
                f.write_str("owned generated output custody is stale or aliased")
            }
            Self::OutputUnavailable => {
                f.write_str("owned generated output was consumed or discarded")
            }
            Self::Allocation => f.write_str("owned generated storage allocation failed"),
            Self::Custody => f.write_str("owned generated output custody lock is poisoned"),
            Self::ResultCredit(error) => {
                write!(f, "owned generated result reservation failed: {error}")
            }
        }
    }
}

impl std::error::Error for GeneratedRuntimeArgumentErrorV1 {}
