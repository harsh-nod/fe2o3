//! Sealed layout observations for SDK view representation fields, not allocation authority.

use super::*;
use crate::rust_type_layout_general::{
    AdtKind, BackendRepresentationFacts, PointerKind, ScalarPrimitiveFacts, SourceScalarKind,
};
use crate::rustc_semantic_adapter_v1::rustc_type_identity_v1;
use crate::trusted_device_items::{self, TrustedDeviceItem};
use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Step {
    Pointee,
    Element,
    Tuple(usize),
    Field(u32, usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ViewPointer {
    path: Vec<Step>,
    view_type: SemanticTypeIdentityV1,
    provider: TrustedDeviceItem,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SdkCaptureObservationV1 {
    source_index: usize,
    capture_type: SemanticTypeIdentityV1,
    physical: Option<Box<TypeLayoutFacts>>,
    pointers: Vec<ViewPointer>,
}

impl SdkCaptureObservationV1 {
    pub(super) fn observe<'tcx>(
        tcx: TyCtxt<'tcx>,
        source_index: usize,
        capture_ty: Ty<'tcx>,
        facts: &TypeLayoutFacts,
        work: &mut SourceClosureWorkV1,
    ) -> Result<Self, ClosureProfileErrorV1> {
        let mut pointers = Vec::new();
        discover(tcx, capture_ty, facts, &mut Vec::new(), &mut pointers, work)?;
        let physical = if pointers.is_empty() {
            None
        } else {
            charge_layout(facts, work)?;
            Some(Box::new(facts.clone()))
        };
        Ok(Self {
            source_index,
            capture_type: rustc_type_identity_v1(tcx, capture_ty),
            physical,
            pointers,
        })
    }

    pub(super) fn validate(
        &self,
        source_index: usize,
        capture_type: SemanticTypeIdentityV1,
        facts: &TypeLayoutFacts,
        origin: ClosureOriginV1,
        work: &mut SourceClosureWorkV1,
    ) -> Result<(), ClosureProfileErrorV1> {
        charge_work(work, 1)?;
        if source_index != self.source_index || capture_type != self.capture_type {
            return Err(error("SDK capture source index or compiler type changed"));
        }
        match (&self.physical, self.pointers.is_empty()) {
            (None, true) => {}
            (Some(expected), false) => {
                if origin != ClosureOriginV1::DeviceInternal {
                    return Err(error(
                        "host SDK view captures require allocation/completion authority",
                    ));
                }
                charge_layout(facts, work)?;
                if **expected != *facts {
                    return Err(error("SDK capture physical layout changed"));
                }
            }
            _ => return Err(error("SDK capture observation roster changed")),
        }
        charge_work(work, self.pointers.len())?;
        let mut consumed = vec![false; self.pointers.len()];
        validate_node(
            facts,
            origin,
            &mut Vec::new(),
            &self.pointers,
            &mut consumed,
            work,
        )?;
        if consumed.iter().any(|used| !used) {
            return Err(error("SDK representation pointer path was not consumed"));
        }
        Ok(())
    }
}

fn error(reason: &'static str) -> ClosureProfileErrorV1 {
    ClosureProfileErrorV1::new(reason)
}

fn descend<T>(path: &mut Vec<Step>, step: Step, f: impl FnOnce(&mut Vec<Step>) -> T) -> T {
    path.push(step);
    let result = f(path);
    path.pop();
    result
}

fn discover<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    facts: &TypeLayoutFacts,
    path: &mut Vec<Step>,
    pointers: &mut Vec<ViewPointer>,
    work: &mut SourceClosureWorkV1,
) -> Result<(), ClosureProfileErrorV1> {
    charge_work(work, 1)?;
    match (ty.kind(), &facts.kind) {
        (TyKind::Ref(_, pointee, _), TypeLayoutKind::Pointer(pointer)) => {
            descend(path, Step::Pointee, |path| {
                discover(tcx, *pointee, &pointer.pointee, path, pointers, work)
            })?;
        }
        (TyKind::Array(element, _), TypeLayoutKind::Array(array)) => {
            descend(path, Step::Element, |path| {
                discover(tcx, *element, &array.element, path, pointers, work)
            })?;
        }
        (TyKind::Tuple(types), TypeLayoutKind::Tuple(fields)) => {
            if types.len() != fields.len() {
                return Err(error("capture tuple layout/type cardinality changed"));
            }
            for field in fields {
                let ty = types
                    .get(field.source_index)
                    .copied()
                    .ok_or_else(|| error("capture tuple source field is absent"))?;
                descend(path, Step::Tuple(field.source_index), |path| {
                    discover(tcx, ty, &field.layout, path, pointers, work)
                })?;
            }
        }
        (TyKind::Adt(definition, arguments), TypeLayoutKind::Adt(adt)) => {
            if let Some(
                provider @ (TrustedDeviceItem::DisjointSlice
                | TrustedDeviceItem::WriteOnlyDisjointSlice),
            ) = trusted_device_items::classify(tcx, definition.did())
            {
                check_view(tcx, *definition, arguments, facts)?;
                charge_work(work, path.len().saturating_add(1))?;
                let mut pointer_path = path.clone();
                pointer_path.push(Step::Field(0, 0));
                charge_work(
                    work,
                    pointers
                        .len()
                        .saturating_mul(pointer_path.len().saturating_add(1)),
                )?;
                if pointers.iter().any(|pointer| pointer.path == pointer_path) {
                    return Err(error("duplicate SDK representation pointer observation"));
                }
                pointers.push(ViewPointer {
                    path: pointer_path,
                    view_type: rustc_type_identity_v1(tcx, ty),
                    provider,
                });
            }
            if definition.variants().len() != adt.variants.len() {
                return Err(error("capture ADT layout/type variant count changed"));
            }
            for variant in &adt.variants {
                charge_work(work, definition.variants().len())?;
                let raw = definition
                    .variants()
                    .iter_enumerated()
                    .find(|(index, _)| index.as_u32() == variant.source_index)
                    .map(|(_, variant)| variant)
                    .ok_or_else(|| error("capture ADT source variant is absent"))?;
                if raw.fields.len() != variant.fields.len() {
                    return Err(error("capture ADT layout/type field count changed"));
                }
                for field in &variant.fields {
                    charge_work(work, raw.fields.len())?;
                    let declaration = raw
                        .fields
                        .iter_enumerated()
                        .find(|(index, _)| index.as_usize() == field.source_index)
                        .map(|(_, field)| field)
                        .ok_or_else(|| error("capture ADT source field is absent"))?;
                    let field_ty = tcx
                        .try_normalize_erasing_regions(
                            TypingEnv::fully_monomorphized(),
                            declaration.ty(tcx, arguments),
                        )
                        .map_err(|_| error("capture ADT field normalization failed"))?;
                    descend(
                        path,
                        Step::Field(variant.source_index, field.source_index),
                        |path| discover(tcx, field_ty, &field.layout, path, pointers, work),
                    )?;
                }
            }
        }
        // Raw pointees never introduce additional representation permissions.
        // The validation walk below still checks their complete element layouts.
        _ => {}
    }
    Ok(())
}

fn check_view<'tcx>(
    tcx: TyCtxt<'tcx>,
    definition: rustc_middle::ty::AdtDef<'tcx>,
    arguments: rustc_middle::ty::GenericArgsRef<'tcx>,
    facts: &TypeLayoutFacts,
) -> Result<(), ClosureProfileErrorV1> {
    let bad = || error("authenticated SDK view has unsupported physical capture layout");
    if !definition.is_struct() || arguments.len() != 2 {
        return Err(bad());
    }
    let element = arguments[0].as_type().ok_or_else(bad)?;
    arguments[1].as_type().ok_or_else(bad)?;
    let fields = &definition.non_enum_variant().fields;
    if fields.len() != 3 {
        return Err(bad());
    }
    let mut fields = fields.iter();
    let types = [
        fields.next().ok_or_else(bad)?.ty(tcx, arguments),
        fields.next().ok_or_else(bad)?.ty(tcx, arguments),
        fields.next().ok_or_else(bad)?.ty(tcx, arguments),
    ];
    if !matches!(types[0].kind(), TyKind::RawPtr(pointee, Mutability::Mut) if *pointee == element)
        || types[1] != tcx.types.usize
        || !matches!(types[2].kind(), TyKind::Adt(marker, _) if marker.is_phantom_data())
    {
        return Err(bad());
    }
    check_view_layout(facts)
}

fn check_view_layout(facts: &TypeLayoutFacts) -> Result<(), ClosureProfileErrorV1> {
    let bad = || error("authenticated SDK view has unsupported physical capture layout");
    let TypeLayoutKind::Adt(adt) = &facts.kind else {
        return Err(bad());
    };
    let BackendRepresentationFacts::ScalarPair {
        first,
        second,
        second_offset_bytes,
    } = &facts.backend_representation
    else {
        return Err(bad());
    };
    if facts.size_bytes != 16
        || facts.abi_alignment_bytes != 8
        || adt.kind != AdtKind::Struct
        || !adt.representation.c
        || adt.representation.packed_alignment_bytes.is_some()
        || adt.variants.len() != 1
        || adt.variants[0].source_index != 0
        || adt.variants[0].fields.len() != 3
        || first.primitive != (ScalarPrimitiveFacts::Pointer { address_space: 0 })
        || first.size_bytes != 8
        || first.abi_alignment_bytes != 8
        || !first.initialized
        || second.primitive
            != (ScalarPrimitiveFacts::Integer {
                bits: 64,
                signed: false,
            })
        || second.size_bytes != 8
        || second.abi_alignment_bytes != 8
        || !second.initialized
        || *second_offset_bytes != 8
    {
        return Err(bad());
    }
    let fields = &adt.variants[0].fields;
    if fields
        .iter()
        .enumerate()
        .any(|(index, field)| field.source_index != index || field.memory_index != index)
        || fields[0].offset_bytes != 0
        || fields[1].offset_bytes != 8
        || fields[2].offset_bytes != 16
        || fields[2].layout.size_bytes != 0
        || fields[0].layout.size_bytes != 8
        || fields[0].layout.abi_alignment_bytes != 8
        || fields[1].layout.size_bytes != 8
        || fields[1].layout.abi_alignment_bytes != 8
        || !matches!(fields[0].layout.kind, TypeLayoutKind::Pointer(ref pointer)
            if pointer.kind == PointerKind::MutRaw && pointer.address_space == 0)
        || !matches!(
            fields[1].layout.kind,
            TypeLayoutKind::Scalar(SourceScalarKind::PointerSizedUnsignedInteger { bits: 64 })
        )
    {
        return Err(bad());
    }
    Ok(())
}

fn charge_layout(
    facts: &TypeLayoutFacts,
    work: &mut SourceClosureWorkV1,
) -> Result<(), ClosureProfileErrorV1> {
    charge_work(work, facts.rust_type.len().saturating_add(1))?;
    match &facts.kind {
        TypeLayoutKind::Pointer(pointer) => charge_layout(&pointer.pointee, work),
        TypeLayoutKind::SharedSliceReference { element } => charge_layout(element, work),
        TypeLayoutKind::Array(array) => charge_layout(&array.element, work),
        TypeLayoutKind::Tuple(fields) | TypeLayoutKind::Closure { fields, .. } => {
            for field in fields {
                charge_layout(&field.layout, work)?;
            }
            Ok(())
        }
        TypeLayoutKind::Adt(adt) => {
            charge_work(work, adt.definition.len())?;
            for variant in &adt.variants {
                charge_work(work, variant.name.len())?;
                for field in &variant.fields {
                    charge_work(work, field.name.as_ref().map_or(0, String::len))?;
                    charge_layout(&field.layout, work)?;
                }
            }
            Ok(())
        }
        TypeLayoutKind::Scalar(_) => Ok(()),
    }
}

fn validate_node(
    facts: &TypeLayoutFacts,
    origin: ClosureOriginV1,
    path: &mut Vec<Step>,
    pointers: &[ViewPointer],
    consumed: &mut [bool],
    work: &mut SourceClosureWorkV1,
) -> Result<(), ClosureProfileErrorV1> {
    charge_work(work, 1)?;
    let mut permitted = false;
    for (index, pointer) in pointers.iter().enumerate() {
        charge_work(work, path.len().saturating_add(1))?;
        if !matches!(
            pointer.provider,
            TrustedDeviceItem::DisjointSlice | TrustedDeviceItem::WriteOnlyDisjointSlice
        ) {
            return Err(error("SDK capture has a different trusted provider family"));
        }
        if pointer.path == *path {
            if consumed[index] || permitted {
                return Err(error("SDK representation pointer consumed more than once"));
            }
            consumed[index] = true;
            permitted = true;
        }
    }
    if permitted
        && !matches!(&facts.kind, TypeLayoutKind::Pointer(pointer) if pointer.kind == PointerKind::MutRaw)
    {
        return Err(error(
            "SDK representation path is not its mutable raw pointer field",
        ));
    }
    match &facts.kind {
        TypeLayoutKind::Closure { .. } => Err(error(
            "nested closure captures are outside the bounded profile",
        )),
        TypeLayoutKind::Scalar(_) => Ok(()),
        TypeLayoutKind::SharedSliceReference { element } => {
            if origin == ClosureOriginV1::HostArgument {
                return Err(error(
                    "host closure references require an eligible allocation/completion token; none is present in V1",
                ));
            }
            descend(path, Step::Element, |path| {
                validate_node(element, origin, path, pointers, consumed, work)
            })
        }
        TypeLayoutKind::Pointer(pointer) => {
            match pointer.kind {
                PointerKind::ConstRaw | PointerKind::MutRaw if !permitted => {
                    return Err(error("raw-pointer captures have no allocation authority"));
                }
                PointerKind::SharedReference | PointerKind::MutableReference
                    if origin == ClosureOriginV1::HostArgument =>
                {
                    return Err(error(
                        "host closure references require an eligible allocation/completion token; none is present in V1",
                    ));
                }
                _ => {}
            }
            descend(path, Step::Pointee, |path| {
                validate_node(&pointer.pointee, origin, path, pointers, consumed, work)
            })
        }
        TypeLayoutKind::Array(array) => descend(path, Step::Element, |path| {
            validate_node(&array.element, origin, path, pointers, consumed, work)
        }),
        TypeLayoutKind::Tuple(fields) => {
            for field in fields {
                descend(path, Step::Tuple(field.source_index), |path| {
                    validate_node(&field.layout, origin, path, pointers, consumed, work)
                })?;
            }
            Ok(())
        }
        TypeLayoutKind::Adt(adt) => {
            for variant in &adt.variants {
                for field in &variant.fields {
                    descend(
                        path,
                        Step::Field(variant.source_index, field.source_index),
                        |path| validate_node(&field.layout, origin, path, pointers, consumed, work),
                    )?;
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "sdk_allocation_capture_v1_tests.rs"]
mod tests;
