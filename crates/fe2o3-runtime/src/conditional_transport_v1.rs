//! Bounded, inert invocation transport on the existing runtime/KFD path.
//!
//! No descriptor decoder or admission constructor lives here. Family coordinates
//! are compared with the separately retained unsafe Worker authority; neither a
//! matching hash nor successful request preparation establishes that authority.

use fe2o3_kfd::{ConditionalDispatchPremisesV1, Gfx942KfdDispatchRequestV1};
use sha2::{Digest, Sha256};

use crate::{Gfx942RuntimeDispatchInputsV1, Gfx942RuntimePreparationErrorV1};

const CONDITIONAL_DISPATCH_DOMAIN_V1: &[u8] =
    b"FE2O3/RUNTIME/GFX942/CONDITIONAL-NOMINAL-V4-DISPATCH/V1\0";
const CONDITIONAL_NOMINAL_V4_TAG: u16 = 4;

/// Descriptive coordinates, never an execution or descriptor-admission token.
///
/// The authority's value must come from retained authenticated admission, not be
/// copied from a caller's prepared request. Existing V1 authority reports Ordinary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942RuntimeInvocationBindingV1 {
    OrdinaryV1,
    ConditionalNominalV4 {
        contract_identity: [u8; 32],
        premise_identity: [u8; 32],
    },
}

/// A conditional variant always owns its complete bounded payload. No fallback
/// or conversion drops it; only the existing request transition consumes it.
pub(crate) enum RuntimeInvocationPremisesV1 {
    OrdinaryV1,
    ConditionalNominalV4(ConditionalDispatchPremisesV1),
}

impl RuntimeInvocationPremisesV1 {
    pub(crate) fn binding(&self) -> Gfx942RuntimeInvocationBindingV1 {
        match self {
            Self::OrdinaryV1 => Gfx942RuntimeInvocationBindingV1::OrdinaryV1,
            Self::ConditionalNominalV4(premises) => conditional_binding(premises),
        }
    }

    pub(crate) fn attach(
        self,
        request: Gfx942KfdDispatchRequestV1,
    ) -> Result<Gfx942KfdDispatchRequestV1, Gfx942RuntimePreparationErrorV1> {
        match self {
            Self::OrdinaryV1 => Ok(request),
            Self::ConditionalNominalV4(premises) => {
                Ok(request.with_conditional_premises_v1(premises)?)
            }
        }
    }
}

impl Gfx942RuntimeDispatchInputsV1 {
    /// Attaches inert numerical obligations, not nominal V4 admission. A raw
    /// contract cannot mint Worker authority or promote this input to safe launch.
    /// The move reuses the already bounded allocation and performs no new decode,
    /// allocation, proof execution, or ledger creation.
    ///
    /// ```compile_fail,E0277
    /// use fe2o3_runtime::{Gfx942RuntimeDispatchInputsV1, WorkerV3Gfx942ExecutionAuthorityV1};
    /// fn needs_authority<T: WorkerV3Gfx942ExecutionAuthorityV1>(_: T) {}
    /// fn cannot_promote(input: Gfx942RuntimeDispatchInputsV1) { needs_authority(input); }
    /// ```
    /// ```compile_fail,E0382
    /// use fe2o3_kfd::ConditionalDispatchPremisesV1;
    /// use fe2o3_runtime::Gfx942RuntimeDispatchInputsV1;
    /// fn cannot_reuse(input: Gfx942RuntimeDispatchInputsV1, p: ConditionalDispatchPremisesV1) {
    ///     let bound = input.with_conditional_premises_v1(p);
    ///     drop(input);
    ///     drop(bound);
    /// }
    /// ```
    pub fn with_conditional_premises_v1(
        mut self,
        premises: ConditionalDispatchPremisesV1,
    ) -> Result<Self, Gfx942RuntimePreparationErrorV1> {
        if !matches!(self.invocation, RuntimeInvocationPremisesV1::OrdinaryV1) {
            return Err(Gfx942RuntimePreparationErrorV1::ConditionalPremisesAlreadyBound);
        }
        self.invocation = RuntimeInvocationPremisesV1::ConditionalNominalV4(premises);
        Ok(self)
    }

    /// Inert transport inspection. This cannot select or construct authority.
    pub fn invocation_binding(&self) -> Gfx942RuntimeInvocationBindingV1 {
        self.invocation.binding()
    }
}

fn conditional_binding(
    premises: &ConditionalDispatchPremisesV1,
) -> Gfx942RuntimeInvocationBindingV1 {
    Gfx942RuntimeInvocationBindingV1::ConditionalNominalV4 {
        contract_identity: *premises.contract_identity(),
        premise_identity: *premises.identity(),
    }
}

pub(crate) fn request_binding(
    request: &Gfx942KfdDispatchRequestV1,
) -> Gfx942RuntimeInvocationBindingV1 {
    match request.conditional_premises_v1() {
        Some(premises) => conditional_binding(premises),
        None => Gfx942RuntimeInvocationBindingV1::OrdinaryV1,
    }
}

pub(crate) fn dispatch_identity(
    ordinary_identity: [u8; 32],
    binding: Gfx942RuntimeInvocationBindingV1,
) -> [u8; 32] {
    match binding {
        // Do not prepend even an ordinary tag: published ordinary hashes stay exact.
        Gfx942RuntimeInvocationBindingV1::OrdinaryV1 => ordinary_identity,
        Gfx942RuntimeInvocationBindingV1::ConditionalNominalV4 {
            contract_identity,
            premise_identity,
        } => {
            let mut hash = Sha256::new();
            hash.update(CONDITIONAL_DISPATCH_DOMAIN_V1);
            hash.update(CONDITIONAL_NOMINAL_V4_TAG.to_le_bytes());
            hash.update(ordinary_identity);
            hash.update(contract_identity);
            hash.update(premise_identity);
            hash.finalize().into()
        }
    }
}

#[cfg(test)]
#[path = "conditional_transport_v1_tests.rs"]
mod tests;
