//! Reuse source-stable slice metadata across intrinsic and ordinary bounds projection.
//! Ranked parameters are analysis values, not additional physical ABI arguments.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1;
use ranked_projection_source_v1::resource;
use std::mem::size_of;

type Error = ProductionRankedProjectionErrorV1;

pub(super) struct Scratch {
    pub(super) origins: Vec<Option<u32>>,
    pub(super) arguments: Vec<Option<u32>>,
    pub(super) definitions: Vec<u8>,
    pub(super) escaped: Vec<bool>,
}

pub(super) struct Scope<'a> {
    facts: &'a mut dyn ProjectedAssertionFactsV1,
    count: usize,
    retained: usize,
}

fn storage(counts: [usize; 5]) -> Result<usize, Error> {
    let widths = [
        size_of::<Option<u32>>(),
        size_of::<Option<u32>>(),
        size_of::<u8>(),
        size_of::<bool>(),
        size_of::<BoundsLocalDefinitionV1<'_>>(),
    ];
    counts.into_iter().zip(widths).try_fold(
        size_of::<Scratch>()
            + size_of::<Scope<'_>>()
            + size_of::<Vec<BoundsLocalDefinitionV1<'_>>>(),
        |total, (count, width)| {
            count
                .checked_mul(width)
                .and_then(|bytes| total.checked_add(bytes))
                .ok_or_else(|| resource(Resource::Arithmetic))
        },
    )
}

/// The producer's existing tables are prepaid through their extra retained
/// lifetime. The callback must dispose of them before returning. Other live
/// projection scopes (notably induction storage) keep their own reservations.
pub(super) fn with_scope<T>(
    count: usize,
    facts: &mut dyn ProjectedAssertionFactsV1,
    action: impl FnOnce(&mut Scope<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    facts.charge_private_array_work(4)?;
    let requested = storage([count; 5])?;
    let floor = facts.scalar_private_storage_v1()?;
    let mut scope = Scope {
        facts,
        count,
        retained: 0,
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scope.facts.reserve_scalar_private_storage_v1(requested)?;
        scope.retained = requested;
        action(&mut scope)
    }));
    let minimum = floor
        .checked_add(scope.retained)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    if scope.facts.scalar_private_storage_v1()? < minimum {
        return Err(resource(Resource::Accounting));
    }
    scope
        .facts
        .release_scalar_private_storage_v1(scope.retained)?;
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

impl Scope<'_> {
    pub(super) fn facts(&mut self) -> &mut dyn ProjectedAssertionFactsV1 {
        self.facts
    }

    pub(super) fn retain(&mut self, scratch: &Scratch) -> Result<(), Error> {
        self.facts.charge_private_array_work(8)?;
        if [
            scratch.origins.len(),
            scratch.arguments.len(),
            scratch.definitions.len(),
            scratch.escaped.len(),
        ] != [self.count; 4]
        {
            return Err(resource(Resource::Accounting));
        }
        let actual = storage([
            scratch.origins.capacity(),
            scratch.arguments.capacity(),
            scratch.definitions.capacity(),
            scratch.escaped.capacity(),
            self.count,
        ])?;
        let extra = actual
            .checked_sub(self.retained)
            .ok_or_else(|| resource(Resource::Accounting))?;
        self.facts.reserve_scalar_private_storage_v1(extra)?;
        self.retained = actual;
        Ok(())
    }
}

pub(super) struct Context<'a> {
    pub(super) scratch: &'a mut Scratch,
    pub(super) facts: &'a mut dyn ProjectedAssertionFactsV1,
}

impl Context<'_> {
    pub(super) fn extent(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        length: SemanticLocalIdV1,
        definitions: &[BoundsLocalDefinitionV1<'_>],
        next_argument: &mut usize,
    ) -> Result<Option<ProductionRankedValueV1>, Error> {
        self.facts.charge_private_array_work(16)?;
        let local = length.index() as usize;
        let Some(definition) = definitions.get(local) else {
            return Ok(None);
        };
        let Some(value) = definition.value else {
            return Ok(None);
        };
        if definition.count != 1
            || self.scratch.definitions.get(local) != Some(&1)
            || self.scratch.escaped.get(local) != Some(&false)
            || function
                .locals()
                .get(local)
                .is_none_or(|local| local.ty() != value.result_type())
        {
            return Ok(None);
        }
        let receiver = match value.kind() {
            SemanticRvalueKindV1::Length(place)
                if matches!(place.projections(), [projection]
                if projection.kind() == SemanticProjectionKindV1::Dereference) =>
            {
                place.local()
            }
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand,
            } => match simple_operand_local(operand) {
                Some(local) => local,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        if !self.metadata_preserving_origin(
            types,
            function,
            receiver,
            value.result_type(),
            definitions,
        )? {
            return Ok(None);
        }
        let origin = self.scratch.origins[receiver.index() as usize]
            .ok_or_else(|| resource(Resource::Accounting))? as usize;
        let Some(slot) = self.scratch.arguments.get(origin) else {
            return Err(resource(Resource::Accounting));
        };
        if slot.is_none() && *next_argument >= fe2o3_pliron::HARD_MAX_PRODUCTION_RANKED_ARGUMENTS {
            return Err(Error::Unsupported(
                "slice metadata exceeds the ranked argument limit",
            ));
        }
        self.facts.charge_private_array_work(8)?;
        let value = project_runtime_slice_extent_argument_v1(
            receiver.index() as usize,
            &self.scratch.origins,
            &mut self.scratch.arguments,
            next_argument,
        )?;
        Ok(Some(value))
    }

    fn metadata_preserving_origin(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        receiver: SemanticLocalIdV1,
        length_type: SemanticTypeIdV1,
        definitions: &[BoundsLocalDefinitionV1<'_>],
    ) -> Result<bool, Error> {
        let Some(declaration) = function.locals().get(receiver.index() as usize) else {
            return Ok(false);
        };
        let ty = declaration.ty();
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Ok(false);
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
            || !matches!(
                types
                    .get(pointer.pointee().index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Slice { .. })
            )
        {
            return Ok(false);
        }
        // Older admitted MIR records usize structurally. The exact metadata
        // operation, not a nominal tag or any arbitrary integer, supplies length.
        if types.get(length_type.index() as usize).is_none_or(|length| {
            !matches!(
                length.rust_type_kind(),
                SemanticRustTypeKindV1::Ordinary | SemanticRustTypeKindV1::Usize
            ) || !matches!(
                length.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits })
                    if *bits == pointer.pointer_width_bits()
            )
        }) {
            return Ok(false);
        }
        let Some(origin) = self
            .scratch
            .origins
            .get(receiver.index() as usize)
            .copied()
            .flatten()
        else {
            return Ok(false);
        };
        let mut current = receiver;
        // Stable scalar provenance also follows casts. Metadata equality needs
        // the stronger whole-value, identical-type copy/move chain below.
        for _ in 0..64 {
            self.facts.charge_private_array_work(16)?;
            let index = current.index() as usize;
            let Some(local) = function.locals().get(index) else {
                return Ok(false);
            };
            let Some(definition) = definitions.get(index) else {
                return Ok(false);
            };
            if local.ty() != ty
                || self.scratch.escaped.get(index) != Some(&false)
                || self.scratch.origins.get(index) != Some(&Some(origin))
            {
                return Ok(false);
            }
            if let SemanticLocalRoleV1::Argument(argument) = local.role() {
                return Ok(argument == origin
                    && definition.count == 0
                    && self.scratch.definitions.get(index) == Some(&0)
                    && function
                        .abi()
                        .adjusted_arguments()
                        .get(argument as usize)
                        .is_some_and(|argument| argument.ty() == ty));
            }
            if definition.count != 1 || self.scratch.definitions.get(index) != Some(&1) {
                return Ok(false);
            }
            let Some(value) = definition.value else {
                return Ok(false);
            };
            let SemanticRvalueKindV1::Use(operand) = value.kind() else {
                return Ok(false);
            };
            let Some(source) = simple_operand_local(operand) else {
                return Ok(false);
            };
            if value.result_type() != ty || operand.ty() != ty {
                return Ok(false);
            }
            current = source;
        }
        Ok(false)
    }
}
