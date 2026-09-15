//! Mutable carriers are inert until the complete root/context source audit.
//! In particular, the atomic and exclusive roles have the same physical ABI.

use super::*;
use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingV1, ReferenceArgumentRelationV1,
    ReferenceFunctionIdentityV1, ReferenceScalarTypeV1,
};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub(super) enum RootReferenceBindingV1<T> {
    Authenticated(AuthenticatedReferenceEffectBindingV1),
    Pending(ReferenceBindingRegistrationRecord<T>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExclusiveOutputSourceV1 {
    root_function: [u8; 32],
    root_body: [u8; 32],
    argument: u32,
    element: ReferenceScalarTypeV1,
    authentication: [u8; 32],
}

impl ExclusiveOutputSourceV1 {
    // IR-only fixtures cannot authenticate a rustc source. Never compiled into
    // the producer; actual ownership tests use the production collector below.
    #[cfg(test)]
    pub(crate) fn test_only_v1(root: u8, argument: u32, element: ReferenceScalarTypeV1) -> Self {
        Self {
            root_function: [root; 32],
            root_body: [root.wrapping_add(1); 32],
            argument,
            element,
            authentication: [root.wrapping_add(2); 32],
        }
    }

    pub(crate) fn canonical_sha256_v1(&self) -> &[u8; 32] {
        &self.authentication
    }

    pub(crate) fn matches_binding_v1(
        &self,
        root: &ReferenceFunctionIdentityV1,
        argument: u32,
        element: ReferenceScalarTypeV1,
    ) -> bool {
        self.root_function == root.function_sha256
            && self.root_body == root.rustc_mir_body_sha256
            && self.argument == argument
            && self.element == element
    }
}

pub(crate) struct ExclusiveReferenceSourcesV1<'tcx> {
    kernel: Instance<'tcx>,
    arguments: Vec<Option<(Ty<'tcx>, ExclusiveOutputSourceV1)>>,
}

impl<'tcx> ExclusiveReferenceSourcesV1<'tcx> {
    pub(crate) fn for_argument_v1(
        &self,
        kernel: Instance<'tcx>,
        argument: u32,
        physical: Ty<'tcx>,
    ) -> Option<ExclusiveOutputSourceV1> {
        let (expected, source) = self.arguments.get(argument as usize)?.as_ref()?;
        (kernel == self.kernel && physical == *expected).then_some(*source)
    }
}

fn signature<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> rustc_middle::ty::FnSig<'tcx> {
    tcx.normalize_erasing_regions(
        TypingEnv::fully_monomorphized(),
        tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(instance.def_id())
                .instantiate(tcx, instance.args),
        ),
    )
}

fn primitive_mutable_slice(ty: Ty<'_>) -> Option<ReferenceScalarTypeV1> {
    let TyKind::Ref(_, pointee, Mutability::Mut) = ty.kind() else {
        return None;
    };
    let TyKind::Slice(element) = pointee.kind() else {
        return None;
    };
    crate::reference_effect_v1::scalar_type_v1(*element)
}

pub(super) fn requires_context_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    kernel: Instance<'tcx>,
) -> Result<bool, String> {
    let signature = signature(tcx, kernel);
    if signature.inputs().len() > MAX_ARGUMENTS_PER_KERNEL {
        return Err(
            "exclusive reference physical argument inventory exceeds its kernel bound".into(),
        );
    }
    Ok(signature
        .inputs()
        .iter()
        .any(|ty| primitive_mutable_slice(*ty).is_some()))
}

fn fail(detail: &str) -> CollectError {
    CollectError {
        message: format!("exclusive reference source authentication rejected: {detail}"),
    }
}

fn inventory<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    functions: &'a [CollectedFunction<'tcx>],
) -> Result<BTreeMap<[u8; 32], &'a CollectedFunction<'tcx>>, CollectError> {
    if functions.len() > fe2o3_rustc_front::MAX_FUNCTIONS_V1 {
        return Err(fail("function inventory exceeds the collection bound"));
    }
    let mut inventory = BTreeMap::new();
    for function in functions {
        let identity = *crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
            tcx,
            function.instance,
        )
        .function()
        .as_bytes();
        if inventory.insert(identity, function).is_some() {
            return Err(fail("function identity is not unique"));
        }
    }
    Ok(inventory)
}

fn hash_function(hash: &mut Sha256, identity: &ReferenceFunctionIdentityV1) {
    hash.update(identity.def_path_hash);
    hash.update(identity.function_sha256);
    hash.update(identity.item_definition_sha256);
    hash.update(identity.monomorphization_sha256);
    hash.update(identity.generic_type_arguments_sha256);
    hash.update(identity.const_generic_arguments_sha256);
    hash.update(identity.rustc_mir_body_sha256);
}

fn sources<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: &CollectedFunction<'tcx>,
    functions: &BTreeMap<[u8; 32], &CollectedFunction<'tcx>>,
) -> Result<ExclusiveReferenceSourcesV1<'tcx>, CollectError> {
    if !root.is_kernel_entry() {
        return Err(fail("owner is not a physical kernel root"));
    }
    let bound = root
        .kernel_context_contract
        .as_ref()
        .ok_or_else(|| fail("mutable output has no bound context"))?;
    let source = bound
        .authenticated_source
        .as_ref()
        .ok_or_else(|| fail("context source has not completed authentication"))?;
    let root_identity = crate::reference_effect_v1::function_identity_v1(tcx, root.instance);
    if source.root_function_identity != root_identity.function_sha256 {
        return Err(fail("context source belongs to a different physical root"));
    }
    let helper = functions
        .get(&source.logical_helper_identity)
        .ok_or_else(|| fail("authenticated logical helper is not in the complete closure"))?;
    if helper.role != CollectedFunctionRole::InternalHelper {
        return Err(fail("authenticated logical helper has a different role"));
    }
    let root_signature = signature(tcx, root.instance);
    let helper_signature = signature(tcx, helper.instance);
    let count = root_signature.inputs().len();
    if count > MAX_ARGUMENTS_PER_KERNEL
        || source.physical_argument_count as usize != count
        || source.logical_argument_count as usize != count + 1
        || helper_signature.inputs().len() != count + 1
    {
        return Err(fail("authenticated argument counts or bound changed"));
    }
    let TyKind::Adt(context, arguments) = helper_signature.inputs()[0].kind() else {
        return Err(fail("logical context type changed"));
    };
    if crate::trusted_device_items::classify(tcx, context.did())
        != Some(crate::trusted_device_items::TrustedDeviceItem::KernelContext)
        || arguments.len() != 4
        || !matches!(arguments[0].kind(), GenericArgKind::Lifetime(_))
    {
        return Err(fail("logical context nominal identity or arity changed"));
    }
    let marker = arguments[1]
        .as_type()
        .ok_or_else(|| fail("context marker is not a type"))?;
    if crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(tcx, marker).as_bytes()
        != &source.kernel_marker_identity
    {
        return Err(fail(
            "logical context marker belongs to a different source root",
        ));
    }
    let helper_identity = crate::reference_effect_v1::function_identity_v1(tcx, helper.instance);
    let binding = root
        .kernel_binding
        .ok_or_else(|| fail("source root has no kernel binding"))?;
    let frontend = root
        .frontend_contract
        .as_ref()
        .ok_or_else(|| fail("source root has no authenticated frontend contract"))?;
    let mut owner_hash = Sha256::new();
    owner_hash.update(b"fe2o3.exclusive-reference-source.v1\0");
    hash_function(&mut owner_hash, &root_identity);
    hash_function(&mut owner_hash, &helper_identity);
    owner_hash.update(binding.as_bytes());
    owner_hash.update((frontend.canonical_bytes.len() as u64).to_le_bytes());
    owner_hash.update(&frontend.canonical_bytes);
    owner_hash.update(source.kernel_marker_identity);
    owner_hash.update(source.issuance_identity);
    owner_hash.update(source.physical_argument_count.to_le_bytes());
    owner_hash.update(source.logical_argument_count.to_le_bytes());
    owner_hash.update((bound.canonical_bytes.len() as u64).to_le_bytes());
    owner_hash.update(&bound.canonical_bytes);
    let owner_hash: [u8; 32] = owner_hash.finalize().into();
    let mut arguments = Vec::with_capacity(count);
    for (ordinal, (physical, logical)) in root_signature
        .inputs()
        .iter()
        .zip(helper_signature.inputs().iter().skip(1))
        .enumerate()
    {
        let Some(element) = primitive_mutable_slice(*physical) else {
            arguments.push(None);
            continue;
        };
        authenticate_logical_physical_kernel_argument_v1(tcx, marker, *logical, *physical)
            .map_err(|error| fail(&error))?;
        // Identity carriers and atomic Global views are not exclusive sources.
        let TyKind::Adt(view, view_arguments) = logical.kind() else {
            return Err(fail(
                "mutable carrier has no exact exclusive capability source",
            ));
        };
        if crate::trusted_device_items::classify(tcx, view.did())
            != Some(crate::trusted_device_items::TrustedDeviceItem::CapabilityMemoryView)
            || view_arguments.len() != 5
        {
            return Err(fail(
                "mutable carrier has no exact exclusive capability source",
            ));
        }
        let role = view_arguments[3]
            .as_type()
            .ok_or_else(|| fail("output role is not a type"))?;
        let TyKind::Adt(role, role_arguments) = role.kind() else {
            return Err(fail("output role is not a nominal marker"));
        };
        if !role_arguments.is_empty()
            || crate::trusted_device_items::classify(tcx, role.did())
                != Some(
                    crate::trusted_device_items::TrustedDeviceItem::CapabilityExclusiveReadWrite,
                )
        {
            return Err(fail("mutable carrier is not the exact exclusive role"));
        }
        let argument = u32::try_from(ordinal).map_err(|_| fail("argument exceeds u32"))?;
        let mut hash = Sha256::new();
        hash.update(owner_hash);
        hash.update(argument.to_le_bytes());
        hash.update([element as u8]);
        hash.update(
            crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(tcx, *logical).as_bytes(),
        );
        hash.update(
            crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(tcx, *physical).as_bytes(),
        );
        arguments.push(Some((
            *physical,
            ExclusiveOutputSourceV1 {
                root_function: root_identity.function_sha256,
                root_body: root_identity.rustc_mir_body_sha256,
                argument,
                element,
                authentication: hash.finalize().into(),
            },
        )));
    }
    Ok(ExclusiveReferenceSourcesV1 {
        kernel: root.instance,
        arguments,
    })
}

impl<'tcx> DeviceCollector<'tcx> {
    pub(super) fn finalize_exclusive_references_v1(&mut self) -> Result<(), CollectError> {
        if self.pending_exclusive_references.is_empty() {
            return Ok(());
        }
        if self.pending_exclusive_references.len() > fe2o3_rustc_front::MAX_FUNCTIONS_V1 {
            return Err(fail(
                "pending reference inventory exceeds the collection bound",
            ));
        }
        let functions = inventory(self.tcx, &self.result)?;
        let mut finalized = Vec::with_capacity(self.pending_exclusive_references.len());
        for record in &self.pending_exclusive_references {
            let identity =
                crate::reference_effect_v1::function_identity_v1(self.tcx, record.kernel);
            let root = functions
                .get(&identity.function_sha256)
                .ok_or_else(|| fail("pending registration lost its physical root"))?;
            if root.instance != record.kernel
                || root.logical_name.as_deref() != Some(record.logical_name.as_str())
                || root.reference_effect_binding.is_some()
            {
                return Err(fail(
                    "pending registration changed root, logical name, or binding ownership",
                ));
            }
            let sources = sources(self.tcx, root, &functions)?;
            let binding = crate::reference_effect_v1::authenticate_exclusive_reference_binding_v1(
                self.tcx,
                record.registration_path.clone(),
                record.logical_name.clone(),
                record.kernel,
                record.reference,
                &sources,
            )
            .map_err(|error| fail(&error.to_string()))?;
            finalized.push((record.kernel, binding));
        }
        // Publish only after every pending registration completes. None escape
        // into CollectionResult on a partial context/reference failure.
        for (instance, binding) in finalized {
            let root = self
                .result
                .iter_mut()
                .find(|root| root.instance == instance)
                .ok_or_else(|| fail("finalized reference lost its root"))?;
            if root.reference_effect_binding.replace(binding).is_some() {
                return Err(fail("exclusive reference was finalized more than once"));
            }
        }
        self.pending_exclusive_references.clear();
        Ok(())
    }
}

pub(super) fn validate_collected_bindings_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions: &[CollectedFunction<'tcx>],
) -> Result<(), CollectError> {
    if functions.len() > fe2o3_rustc_front::MAX_FUNCTIONS_V1 {
        return Err(fail(
            "binding replay inventory exceeds the collection bound",
        ));
    }
    let mut candidates = Vec::new();
    for root in functions {
        let Some(binding) = &root.reference_effect_binding else {
            continue;
        };
        if binding.effect_ir.relations.len()
            > MAX_ARGUMENTS_PER_KERNEL + crate::reference_effect_v1::MAX_REFERENCE_POINT_AXES_V1
        {
            return Err(fail(
                "reference relation inventory exceeds its argument bound",
            ));
        }
        let carries_exclusive = binding.effect_ir.relations.iter().any(|relation| {
            matches!(
                relation,
                ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D { .. }
            )
        });
        if requires_context_v1(tcx, root.instance).map_err(|error| fail(&error))?
            || carries_exclusive
        {
            candidates.push(root);
        }
    }
    if candidates.is_empty() {
        return Ok(());
    }
    let inventory = inventory(tcx, functions)?;
    for root in candidates {
        let expected = sources(tcx, root, &inventory)?;
        let binding = root
            .reference_effect_binding
            .as_ref()
            .expect("candidate has a binding");
        if binding.kernel != crate::reference_effect_v1::function_identity_v1(tcx, root.instance)
            || root.logical_name.as_deref() != Some(binding.logical_kernel_name.as_str())
            || binding.effect_ir_sha256 != binding.effect_ir.canonical_sha256_v1()
            || binding.observable_output_writes != binding.effect_ir.observable_output_effects
        {
            return Err(fail(
                "completed binding changed its root, body, IR, or write roster",
            ));
        }
        if binding.effect_ir.relations.len() != expected.arguments.len() + 1
            || !matches!(
                binding.effect_ir.relations.first(),
                Some(ReferenceArgumentRelationV1::PointCoordinate {
                    reference_argument: 0,
                    axis: 0
                })
            )
        {
            return Err(fail(
                "exclusive reference changed its point/physical argument roster",
            ));
        }
        for (ordinal, (relation, expected)) in binding
            .effect_ir
            .relations
            .iter()
            .skip(1)
            .zip(&expected.arguments)
            .enumerate()
        {
            match (relation, expected) {
                (
                    ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D {
                        argument,
                        element,
                        source,
                    },
                    Some((_, expected)),
                ) => {
                    if *argument as usize != ordinal {
                        return Err(fail(
                            "exclusive relation names a different physical argument",
                        ));
                    }
                    if source != expected
                        || !source.matches_binding_v1(&binding.kernel, *argument, *element)
                    {
                        return Err(fail(
                            "exclusive relation changed source, role, carrier, or argument",
                        ));
                    }
                }
                (ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D { .. }, None) => {
                    return Err(fail(
                        "exclusive relation names a different physical argument",
                    ));
                }
                (_, Some(_)) => {
                    return Err(fail(
                        "exclusive source output relation was removed or substituted",
                    ));
                }
                (_, None) => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod compiler_tests;
