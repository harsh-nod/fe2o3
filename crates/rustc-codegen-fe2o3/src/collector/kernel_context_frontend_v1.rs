//! Bounded, inert declarations for logical context entries.

use super::*;
use fe2o3_rustc_front::{
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1, KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1,
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1,
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1, KernelContextFrontendContractV1,
    MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1, MAX_KERNEL_CONTEXT_GENERATED_ITEM_NAME_BYTES_V1,
    decode_kernel_context_frontend_contract_v1,
};

#[derive(Clone, Debug)]
pub(super) struct BoundContextEntryV1 {
    pub(super) registration_path: String,
    pub(super) canonical_bytes: Vec<u8>,
    pub(super) contract: KernelContextFrontendContractV1,
    /// Same-session producer evidence, set only after MIR flow and ABI checks.
    pub(super) authenticated_items: Option<[DefId; 4]>,
}

#[derive(Clone, Debug)]
pub(super) struct DeclaredContextEntryV1<'tcx> {
    pub(super) target: Instance<'tcx>,
    pub(super) logical_name: String,
    pub(super) bound: BoundContextEntryV1,
}

pub(super) fn decode_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<Vec<DeclaredContextEntryV1<'tcx>>, RegistrationError> {
    let root_count = count_production_roots_before_monomorphization_v1(tcx);
    let mut candidates = Vec::new();
    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        let def_id = item.owner_id.def_id;
        let path = tcx.def_path_str(def_id.to_def_id());
        if !final_path_segment(&path).starts_with(KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1) {
            continue;
        }
        if candidates.len() >= root_count.min(fe2o3_rustc_front::MAX_FUNCTIONS_V1) {
            return Err(RegistrationError::new(
                path,
                "kernel-context declaration count exceeds registered kernel roots",
            ));
        }
        candidates.push((path, def_id, item));
    }
    candidates.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    let mut records = Vec::with_capacity(candidates.len());
    for (path, def_id, item) in candidates {
        let error = |reason: &str| RegistrationError::new(&path, reason);
        if !matches!(item.kind, ItemKind::Static(..)) || tcx.is_mutable_static(def_id.to_def_id()) {
            return Err(error(
                "kernel-context declaration must be an immutable static",
            ));
        }
        if !tcx
            .codegen_fn_attrs(def_id)
            .flags
            .intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER)
        {
            return Err(error("kernel-context declaration must carry #[used]"));
        }
        let ty = tcx.type_of(def_id).instantiate_identity();
        let TyKind::Tuple(types) = ty.kind() else {
            return Err(error(
                "kernel-context declaration requires the exact V1 tuple type",
            ));
        };
        if types.len() != 6
            || types[0] != tcx.types.u64
            || types[1] != tcx.types.u16
            || types[2] != tcx.types.u16
            || !is_shared_str(types[3])
            || !is_shared_u8_slice(types[4])
            || !matches!(types[5].kind(), TyKind::FnPtr(..))
        {
            return Err(error(
                "kernel-context declaration requires (u64, u16, u16, &str, &[u8], fn pointer)",
            ));
        }
        let body = tcx.mir_for_ctfe(def_id);
        let fields = registration_tuple_fields(body, 6, &path)?;
        for (index, ty, name, expected) in [
            (
                0,
                tcx.types.u64,
                "magic",
                u128::from(KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1),
            ),
            (
                1,
                tcx.types.u16,
                "version",
                u128::from(KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1),
            ),
            (
                2,
                tcx.types.u16,
                "kind",
                u128::from(KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1),
            ),
        ] {
            if registration_integer(tcx, fields[index], ty, name, &path)? != expected {
                return Err(error("kernel-context declaration header does not match V1"));
            }
        }
        let name_bytes = bounded_registration_bytes(
            tcx,
            fields[3],
            true,
            MAX_KERNEL_CONTEXT_GENERATED_ITEM_NAME_BYTES_V1,
            &path,
        )?;
        let name =
            std::str::from_utf8(&name_bytes).map_err(|_| error("logical name is not UTF-8"))?;
        if name.is_empty()
            || final_path_segment(&path)
                != format!("{KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1}{name}")
        {
            return Err(error(
                "kernel-context declaration item and logical name differ",
            ));
        }
        let canonical_bytes = bounded_registration_bytes(
            tcx,
            fields[4],
            false,
            MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1,
            &path,
        )?;
        let contract =
            decode_kernel_context_frontend_contract_v1(&canonical_bytes).map_err(|err| {
                RegistrationError::new(&path, format!("invalid kernel-context contract: {err}"))
            })?;
        let target = registration_target(tcx, body, fields[5], &path)?;
        records.push(DeclaredContextEntryV1 {
            target,
            logical_name: name.to_owned(),
            bound: BoundContextEntryV1 {
                registration_path: path,
                canonical_bytes,
                contract,
                authenticated_items: None,
            },
        });
    }
    Ok(records)
}

pub(super) fn bind_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions_by_symbol: &BTreeMap<String, Vec<Instance<'tcx>>>,
    roots: &mut [KernelRoot<Instance<'tcx>>],
    declarations: &[DeclaredContextEntryV1<'tcx>],
) -> Result<(), RegistrationError> {
    for declaration in declarations {
        let target = declaration.target;
        let bound = &declaration.bound;
        let error = |reason: &str| RegistrationError::new(&bound.registration_path, reason);
        let symbol = tcx.symbol_name(target).name;
        if functions_by_symbol.get(symbol).map(Vec::as_slice) != Some(&[target][..]) {
            return Err(error(
                "kernel-context target is not one exact monomorphized function",
            ));
        }
        let Some(root) = roots
            .iter_mut()
            .find(|root| root.logical_name == declaration.logical_name)
        else {
            return Err(error(
                "orphan kernel-context declaration has no registered root",
            ));
        };
        if root.target != target
            || tcx.item_name(target.def_id()).as_str()
                != bound.contract.physical_kernel_root().name()
        {
            return Err(error(
                "kernel-context target differs from its registered physical root",
            ));
        }
        if root.kernel_context_contract.is_some() {
            return Err(error("duplicate kernel-context declaration"));
        }
        root.kernel_context_contract = Some(bound.clone());
    }
    Ok(())
}

fn bounded_registration_bytes<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    string: bool,
    maximum: usize,
    path: &str,
) -> Result<Vec<u8>, RegistrationError> {
    let error = |reason: &str| RegistrationError::new(path, reason);
    let Operand::Constant(constant) = operand else {
        return Err(error("kernel-context declaration payload must be constant"));
    };
    let ty = constant.const_.ty();
    if !(if string {
        is_shared_str(ty)
    } else {
        is_shared_u8_slice(ty)
    }) {
        return Err(error(
            "kernel-context declaration payload has the wrong type",
        ));
    }
    let value = constant
        .const_
        .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
        .map_err(|_| error("kernel-context declaration payload could not be evaluated"))?;
    let bytes = value
        .try_get_slice_bytes_for_diagnostics(tcx)
        .ok_or_else(|| error("kernel-context declaration payload has no slice bytes"))?;
    bounded_copy(bytes, maximum).map_err(error)
}

fn bounded_copy(bytes: &[u8], maximum: usize) -> Result<Vec<u8>, &'static str> {
    if bytes.len() > maximum {
        return Err("kernel-context declaration payload exceeds its byte limit");
    }
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_payload_is_bounded_before_copying() {
        let maximum = MAX_KERNEL_CONTEXT_FRONTEND_CONTRACT_BYTES_V1;
        assert_eq!(
            bounded_copy(&vec![7; maximum], maximum).unwrap().len(),
            maximum
        );
        assert_eq!(
            bounded_copy(&vec![7; maximum + 1], maximum),
            Err("kernel-context declaration payload exceeds its byte limit")
        );
        assert!(bounded_copy(&[], 0).unwrap().is_empty());
    }
}
