use super::*;

struct Frame {
    ty: usize,
    variant: usize,
    field: usize,
}

enum Step {
    Child(SemanticTypeIdV1),
    NextVariant,
    Complete,
}

impl Frame {
    fn step(&mut self, types: &[SemanticTypeDeclV1]) -> Step {
        let fields = match &types[self.ty].shape {
            SemanticTypeShapeV1::Array { element, .. } | SemanticTypeShapeV1::Slice { element } => {
                std::slice::from_ref(element)
            }
            SemanticTypeShapeV1::Tuple(fields)
            | SemanticTypeShapeV1::Aggregate(fields)
            | SemanticTypeShapeV1::Union(fields) => fields.fields(),
            SemanticTypeShapeV1::Enum { variants, .. } => {
                let Some(variant) = variants.get(self.variant) else {
                    return Step::Complete;
                };
                if self.field == variant.fields.fields.len() {
                    self.variant += 1;
                    self.field = 0;
                    return Step::NextVariant;
                }
                variant.fields.fields()
            }
            // Signatures and pointees are references, not inline storage.
            SemanticTypeShapeV1::Pointer(_)
            | SemanticTypeShapeV1::FunctionPointer { .. }
            | SemanticTypeShapeV1::Unit
            | SemanticTypeShapeV1::Never
            | SemanticTypeShapeV1::Opaque
            | SemanticTypeShapeV1::Scalar(_)
            | SemanticTypeShapeV1::ValidityScalar(_) => return Step::Complete,
        };
        let Some(&child) = fields.get(self.field) else {
            return Step::Complete;
        };
        self.field += 1;
        Step::Child(child)
    }
}

pub(super) fn scratch_bytes(types: usize) -> Result<usize, SemanticMirErrorV1> {
    use std::mem::size_of;
    types
        .checked_mul(size_of::<Frame>())
        .and_then(|frames| frames.checked_add(types.checked_mul(size_of::<u8>())?))
        .and_then(|payload| payload.checked_add(size_of::<Vec<Frame>>()))
        .and_then(|payload| payload.checked_add(size_of::<Vec<u8>>()))
        .ok_or(SemanticMirErrorV1::ArithmeticOverflow {
            resource: SemanticMirResourceV1::Types,
        })
}

// Local type/reference checks run first. This second pass checks representability
// without following pointers or recursively expanding shared subobjects.
pub(super) fn validate(context: &mut ValidationContextV1<'_>) -> Result<(), SemanticMirErrorV1> {
    let types = &context.request.types;
    let count = types.len();
    scratch_bytes(count)?;
    charge_validation_work(context, count)?;
    let allocation_error = |_| SemanticMirErrorV1::AllocationFailed {
        resource: SemanticMirResourceV1::Types,
    };
    let mut colors = Vec::new();
    colors.try_reserve_exact(count).map_err(allocation_error)?;
    colors.resize(count, 0_u8);
    let mut stack = Vec::new();
    stack.try_reserve_exact(count).map_err(allocation_error)?;
    for root in 0..count {
        context.one()?;
        if colors[root] != 0 {
            continue;
        }
        context.one()?;
        colors[root] = 1;
        stack.push(Frame {
            ty: root,
            variant: 0,
            field: 0,
        });
        while let Some(frame) = stack.last_mut() {
            context.one()?;
            match frame.step(types) {
                Step::Child(child) => {
                    let child = child.index() as usize;
                    match colors.get(child) {
                        Some(0) => {
                            context.one()?;
                            colors[child] = 1;
                            stack.push(Frame {
                                ty: child,
                                variant: 0,
                                field: 0,
                            });
                        }
                        Some(1) | None => return Err(SemanticMirErrorV1::InvalidTypeLayout),
                        Some(2) => {}
                        Some(_) => return Err(SemanticMirErrorV1::InvalidTypeLayout),
                    }
                }
                Step::NextVariant => {}
                Step::Complete => {
                    colors[frame.ty] = 2;
                    stack.pop();
                }
            }
        }
    }
    Ok(())
}
