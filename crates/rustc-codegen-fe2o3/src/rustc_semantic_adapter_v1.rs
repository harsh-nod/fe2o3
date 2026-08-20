//! Neutral rustc-derived identity primitives for canonical semantic MIR.

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticConstGenericArgumentsIdentityV1, SemanticFunctionIdentityV1,
    SemanticGenericTypeArgumentsIdentityV1, SemanticItemDefinitionIdentityV1,
    SemanticLayoutIdentityV1, SemanticMonomorphizationIdentityV1, SemanticTargetDataLayoutV1,
};
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::ty::{GenericArgKind, Instance, TyCtxt};
use sha2::{Digest as _, Sha256};

use crate::semantic_layout_bridge::SemanticLayoutTargetV1;

const TARGET_LAYOUT_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-target-layout/v1";
const FUNCTION_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/function/v1";
const ITEM_DEFINITION_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/item-definition/v1";
const MONOMORPHIZATION_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/monomorphization/v1";
const TYPE_ARGUMENTS_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/type-arguments/v1";
const CONST_ARGUMENTS_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/const-arguments/v1";

macro_rules! stable_fingerprint {
    ($tcx:expr, $value:expr) => {{
        let fingerprint: Fingerprint = $tcx.with_stable_hashing_context(|mut context| {
            let mut hasher = StableHasher::new();
            ($value).hash_stable(&mut context, &mut hasher);
            hasher.finish()
        });
        fingerprint.to_le_bytes()
    }};
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CanonicalFunctionIdentitiesV1 {
    function: SemanticFunctionIdentityV1,
    item_definition: SemanticItemDefinitionIdentityV1,
    monomorphization: SemanticMonomorphizationIdentityV1,
    generic_type_arguments: SemanticGenericTypeArgumentsIdentityV1,
    const_generic_arguments: SemanticConstGenericArgumentsIdentityV1,
}

pub(crate) struct SemanticIdentityDigestV1 {
    digest: Sha256,
}

impl SemanticIdentityDigestV1 {
    pub(crate) fn new(domain: &[u8]) -> Self {
        let mut digest = Sha256::new();
        append_field(&mut digest, domain);
        Self { digest }
    }

    pub(crate) fn field(&mut self, field: &[u8]) {
        append_field(&mut self.digest, field);
    }

    pub(crate) fn finish(self) -> [u8; 32] {
        self.digest.finalize().into()
    }
}

impl CanonicalFunctionIdentitiesV1 {
    pub(crate) const fn function(self) -> SemanticFunctionIdentityV1 {
        self.function
    }

    pub(crate) const fn item_definition(self) -> SemanticItemDefinitionIdentityV1 {
        self.item_definition
    }

    pub(crate) const fn monomorphization(self) -> SemanticMonomorphizationIdentityV1 {
        self.monomorphization
    }

    pub(crate) const fn generic_type_arguments(self) -> SemanticGenericTypeArgumentsIdentityV1 {
        self.generic_type_arguments
    }

    pub(crate) const fn const_generic_arguments(self) -> SemanticConstGenericArgumentsIdentityV1 {
        self.const_generic_arguments
    }
}

/// Derives every canonical function identity axis from one live rustc instance.
///
/// Diagnostic symbols and paths are deliberately absent from these preimages.
pub(crate) fn canonical_function_identities_v1(
    tcx: TyCtxt<'_>,
    instance: Instance<'_>,
) -> CanonicalFunctionIdentitiesV1 {
    let instance_fingerprint = stable_fingerprint!(tcx, instance);
    let definition_fingerprint = tcx.def_path_hash(instance.def_id()).0.to_le_bytes();
    let mut type_arguments = SemanticIdentityDigestV1::new(TYPE_ARGUMENTS_DOMAIN_V1);
    let mut const_arguments = SemanticIdentityDigestV1::new(CONST_ARGUMENTS_DOMAIN_V1);
    for argument in instance.args {
        match argument.kind() {
            GenericArgKind::Type(ty) => {
                type_arguments.field(&stable_fingerprint!(tcx, ty));
            }
            GenericArgKind::Const(value) => {
                const_arguments.field(&stable_fingerprint!(tcx, value));
            }
            GenericArgKind::Lifetime(_) => {}
        }
    }
    CanonicalFunctionIdentitiesV1 {
        function: SemanticFunctionIdentityV1::from_sha256(domain_digest(
            FUNCTION_DOMAIN_V1,
            &[&instance_fingerprint],
        )),
        item_definition: SemanticItemDefinitionIdentityV1::from_sha256(domain_digest(
            ITEM_DEFINITION_DOMAIN_V1,
            &[&definition_fingerprint],
        )),
        monomorphization: SemanticMonomorphizationIdentityV1::from_sha256(domain_digest(
            MONOMORPHIZATION_DOMAIN_V1,
            &[&instance_fingerprint],
        )),
        generic_type_arguments: SemanticGenericTypeArgumentsIdentityV1::from_sha256(
            type_arguments.finish(),
        ),
        const_generic_arguments: SemanticConstGenericArgumentsIdentityV1::from_sha256(
            const_arguments.finish(),
        ),
    }
}

/// Derives the canonical target-layout identity from exact, already observed
/// rustc target facts. Authentication remains the importer's responsibility.
pub(crate) fn canonical_target_layout_v1(
    target: &SemanticLayoutTargetV1,
) -> SemanticTargetDataLayoutV1 {
    let pointer_width = target.default_pointer_width_bits().to_le_bytes();
    let cpu = target.active_cpu().unwrap_or_default();
    let features = target.active_features().unwrap_or_default();
    SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(domain_digest(
        TARGET_LAYOUT_DOMAIN_V1,
        &[
            target.llvm_target().as_bytes(),
            target.data_layout().as_bytes(),
            &pointer_width,
            cpu.as_bytes(),
            features.as_bytes(),
        ],
    )))
}

pub(crate) fn domain_digest(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = SemanticIdentityDigestV1::new(domain);
    for field in fields {
        digest.field(field);
    }
    digest.finish()
}

fn append_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_le_bytes());
    digest.update(field);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(features: &str) -> SemanticLayoutTargetV1 {
        SemanticLayoutTargetV1::new_with_codegen_profile(
            "amdgcn-amd-amdhsa",
            "e-p:64:64",
            64,
            "gfx942",
            "",
            features,
        )
        .unwrap()
    }

    #[test]
    fn framed_identity_preimages_reject_concatenation_and_domain_substitution() {
        assert_ne!(
            domain_digest(b"domain-a", &[b"ab", b"c"]),
            domain_digest(b"domain-a", &[b"a", b"bc"])
        );
        assert_ne!(
            domain_digest(b"domain-a", &[b"same"]),
            domain_digest(b"domain-b", &[b"same"])
        );
    }

    #[test]
    fn target_layout_identity_binds_every_codegen_profile_axis() {
        let exact = canonical_target_layout_v1(&target("-xnack,+wavefrontsize64"));
        let substituted = canonical_target_layout_v1(&target("+xnack,+wavefrontsize64"));
        assert_ne!(exact.identity(), substituted.identity());

        let reordered = canonical_target_layout_v1(&target("+wavefrontsize64,-xnack"));
        assert_eq!(exact.identity(), reordered.identity());
    }
}
