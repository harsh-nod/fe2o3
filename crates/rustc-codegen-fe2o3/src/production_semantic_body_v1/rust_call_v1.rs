//! Reconstruct the source tuple consumed by rustc's flattened closure-body arguments.

use super::*;
use crate::rustc_semantic_adapter_v1::SemanticIdentityDigestV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1;

struct ClosureArgumentV1 {
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
}

pub(super) struct ClosureArgumentsV1 {
    tuple_type: SemanticTypeIdV1,
    tuple_identity: SemanticLocalIdentityV1,
    source: SemanticSourceProvenanceV1,
    fields: Vec<ClosureArgumentV1>,
}

/// Inserting one identity preserves the relative order of every source local.
#[derive(Clone, Copy, Debug)]
pub(super) struct InsertedLocalOrderV1 {
    pub(super) inserted_local: SemanticLocalIdV1,
    original_count: u32,
}

impl InsertedLocalOrderV1 {
    pub(super) fn new(
        locals: &[SemanticLocalDeclV1],
        inserted_identity: SemanticLocalIdentityV1,
        owner: &mut ProductionSemanticBodyRequestOwnerV1<'_>,
    ) -> Result<Self, ProductionSemanticBodyErrorV1> {
        let original_count = u32::try_from(locals.len())
            .ok()
            .filter(|count| *count < u32::MAX)
            .ok_or_else(|| table("closure RustCall local cardinality"))?;
        let mut tuple_index = original_count;
        let mut previous = None;
        for (index, local) in locals.iter().enumerate() {
            owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
            let identity = local.identity();
            if previous.is_some_and(|previous| previous >= identity)
                || identity == inserted_identity
            {
                return Err(table("closure RustCall local identity order"));
            }
            if tuple_index == original_count && identity > inserted_identity {
                tuple_index = index as u32;
            }
            previous = Some(identity);
        }
        Ok(Self {
            inserted_local: SemanticLocalIdV1::from_index(tuple_index),
            original_count,
        })
    }

    pub(super) fn remap(
        self,
        local: SemanticLocalIdV1,
    ) -> Result<SemanticLocalIdV1, ProductionSemanticBodyErrorV1> {
        let index = local.index();
        if index >= self.original_count {
            return Err(table("closure RustCall original local"));
        }
        Ok(SemanticLocalIdV1::from_index(
            index + u32::from(index >= self.inserted_local.index()),
        ))
    }
}

#[derive(Debug)]
pub(super) struct NormalizedClosureArgumentsV1 {
    pub(super) local_order: InsertedLocalOrderV1,
    statements: Vec<SemanticStatementV1>,
}

impl BodyProducerV1<'_, '_, '_> {
    pub(super) fn closure_arguments_v1(
        &mut self,
        input: &ProductionSemanticBodyInputV1<'_, '_>,
    ) -> Result<Option<ClosureArgumentsV1>, ProductionSemanticBodyErrorV1> {
        let typing_env = TypingEnv::fully_monomorphized();
        let TyKind::Closure(_, arguments) = *self.instance.ty(self.tcx, typing_env).kind() else {
            return Ok(None);
        };
        // Unlike an Fn-trait shim, a closure's own MIR takes each tuple field
        // as a separate local. Neither ABI arguments nor raw MIR are rewritten.
        if input.abi.extern_abi() != SemanticExternAbiV1::RustCall
            || input.body.spread_arg.is_some()
            || input.abi.source_input_types().len() != 2
        {
            return Err(table("closure RustCall source signature"));
        }
        let signature = self.tcx.normalize_erasing_regions(
            typing_env,
            self.tcx
                .instantiate_bound_regions_with_erased(arguments.as_closure().sig()),
        );
        let [tuple] = signature.inputs() else {
            return Err(table("closure RustCall source tuple"));
        };
        let TyKind::Tuple(field_types) = tuple.kind() else {
            return Err(table("closure RustCall source tuple"));
        };
        let tuple_type = self.type_id(*tuple, None, None)?;
        if input.abi.source_input_types()[1] != tuple_type
            || input.body.arg_count != field_types.len().saturating_add(1)
        {
            return Err(table("closure RustCall argument cardinality"));
        }
        let mut fields = try_vec_v1(field_types.len(), SemanticMirResourceV1::CallArguments)?;
        for (field, ty) in field_types.iter().enumerate() {
            self.work()?;
            fields.push(ClosureArgumentV1 {
                local: self.local_id(field + 2)?,
                ty: self.type_id(ty, None, None)?,
            });
        }
        let mut identity =
            SemanticIdentityDigestV1::new(b"fe2o3/semantic-mir/closure-source-tuple/v1");
        identity.field(input.identities.identity.as_bytes());
        identity.field(input.abi.identity().as_bytes());
        Ok(Some(ClosureArgumentsV1 {
            tuple_type,
            tuple_identity: SemanticLocalIdentityV1::from_sha256(identity.finish()),
            source: input.source,
            fields,
        }))
    }
}

impl ClosureArgumentsV1 {
    pub(super) fn normalize_locals(
        self,
        locals: &mut Vec<SemanticLocalDeclV1>,
        owner: &mut ProductionSemanticBodyRequestOwnerV1<'_>,
    ) -> Result<NormalizedClosureArgumentsV1, ProductionSemanticBodyErrorV1> {
        // Check all raw field roles and types before changing any records.
        for (field, binding) in self.fields.iter().enumerate() {
            owner.charge(SemanticMirResourceV1::ValidationWork, 1)?;
            let local = locals
                .get(binding.local.index() as usize)
                .ok_or_else(|| table("closure RustCall field local"))?;
            if local.role() != SemanticLocalRoleV1::Argument(field as u32 + 1)
                || local.ty() != binding.ty
            {
                return Err(table("closure RustCall field role or type"));
            }
        }
        owner.charge(SemanticMirResourceV1::Locals, 1)?;
        owner.charge(SemanticMirResourceV1::Statements, self.fields.len())?;
        owner.charge(SemanticMirResourceV1::Projections, self.fields.len())?;
        owner.charge(SemanticMirResourceV1::Operands, self.fields.len())?;
        let local_order = InsertedLocalOrderV1::new(locals, self.tuple_identity, owner)?;
        let mut statements = try_vec_v1(self.fields.len(), SemanticMirResourceV1::Statements)?;
        locals
            .try_reserve(1)
            .map_err(|_| allocation(SemanticMirResourceV1::Locals))?;
        for (field, binding) in self.fields.iter().enumerate() {
            let local = &locals[binding.local.index() as usize];
            statements.push(SemanticStatementV1::new(
                local.source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(local_order.remap(binding.local)?, vec![], binding.ty)?,
                    SemanticRvalueV1::new(
                        binding.ty,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(SemanticPlaceV1::new(
                            local_order.inserted_local,
                            vec![SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Field(field as u32),
                                binding.ty,
                            )?],
                            binding.ty,
                        )?)),
                    ),
                )),
            ));
        }
        for binding in &self.fields {
            let local = &mut locals[binding.local.index() as usize];
            *local = SemanticLocalDeclV1::new(
                local.identity(),
                local.ty(),
                SemanticLocalRoleV1::Temporary,
                local.source(),
            );
        }
        locals.insert(
            local_order.inserted_local.index() as usize,
            SemanticLocalDeclV1::new(
                self.tuple_identity,
                self.tuple_type,
                SemanticLocalRoleV1::Argument(1),
                self.source,
            ),
        );
        Ok(NormalizedClosureArgumentsV1 {
            local_order,
            statements,
        })
    }
}

impl NormalizedClosureArgumentsV1 {
    pub(super) fn initialize_entry(
        mut self,
        blocks: &mut [SemanticBasicBlockV1],
        entry: SemanticBlockIdV1,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        let block = blocks
            .get_mut(entry.index() as usize)
            .ok_or_else(|| table("closure RustCall entry block"))?;
        self.statements
            .try_reserve(block.statements().len())
            .map_err(|_| allocation(SemanticMirResourceV1::Statements))?;
        self.statements.extend_from_slice(block.statements());
        *block = SemanticBasicBlockV1::new(
            block.identity(),
            block.source(),
            self.statements,
            block.terminator().clone(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(index: u8, ty: u32, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([index; 32]),
            SemanticTypeIdV1::from_index(ty),
            role,
            SemanticSourceProvenanceV1::unavailable(),
        )
    }

    fn fixture() -> (
        ClosureArgumentsV1,
        Vec<SemanticLocalDeclV1>,
        Vec<SemanticBasicBlockV1>,
    ) {
        let source = SemanticSourceProvenanceV1::unavailable();
        let arguments = ClosureArgumentsV1 {
            tuple_type: SemanticTypeIdV1::from_index(3),
            tuple_identity: SemanticLocalIdentityV1::from_sha256([99; 32]),
            source,
            fields: vec![
                ClosureArgumentV1 {
                    local: SemanticLocalIdV1::from_index(3),
                    ty: SemanticTypeIdV1::from_index(1),
                },
                ClosureArgumentV1 {
                    local: SemanticLocalIdV1::from_index(2),
                    ty: SemanticTypeIdV1::from_index(2),
                },
            ],
        };
        let locals = vec![
            local(1, 0, SemanticLocalRoleV1::Return),
            local(2, 0, SemanticLocalRoleV1::Argument(0)),
            local(3, 2, SemanticLocalRoleV1::Argument(2)),
            local(4, 1, SemanticLocalRoleV1::Argument(1)),
        ];
        let block = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([1; 32]),
            source,
            vec![SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Nop,
            )],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap();
        (arguments, locals, vec![block.clone(), block])
    }

    fn owner(limits: SemanticMirLimitsV1) -> ProductionSemanticBodyRequestOwnerV1<'static> {
        ProductionSemanticBodyRequestOwnerV1 {
            limits,
            totals: ConstructionTotalsV1::default(),
            callables: HashMap::new(),
        }
    }

    #[test]
    fn closure_rust_call_normalizes_exact_field_moves_at_mapped_entry() {
        let (arguments, mut locals, mut blocks) = fixture();
        let original = blocks.clone();
        let mut owner = owner(SemanticMirLimitsV1::default());
        arguments
            .normalize_locals(&mut locals, &mut owner)
            .unwrap()
            .initialize_entry(&mut blocks, SemanticBlockIdV1::from_index(1))
            .unwrap();
        assert_eq!(locals[0].role(), SemanticLocalRoleV1::Return);
        assert_eq!(locals[1].role(), SemanticLocalRoleV1::Argument(0));
        assert_eq!(locals[2].role(), SemanticLocalRoleV1::Temporary);
        assert_eq!(locals[3].role(), SemanticLocalRoleV1::Temporary);
        assert_eq!(locals[4].role(), SemanticLocalRoleV1::Argument(1));
        assert_eq!(locals[4].ty(), SemanticTypeIdV1::from_index(3));
        assert_eq!(blocks[0], original[0]);
        assert_eq!(&blocks[1].statements()[2..], original[1].statements());
        assert_eq!(blocks[1].terminator(), original[1].terminator());
        for (field, (destination, ty)) in [(3, 1), (2, 2)].into_iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = blocks[1].statements()[field].kind()
            else {
                panic!("expected field assignment")
            };
            assert_eq!(assignment.destination().local().index(), destination);
            assert_eq!(assignment.value().result_type().index(), ty);
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) =
                assignment.value().kind()
            else {
                panic!("tuple field must move, not copy")
            };
            assert_eq!(place.local().index(), 4);
            assert_eq!(place.ty().index(), ty);
            assert_eq!(
                place.projections(),
                &[SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Field(field as u32),
                    SemanticTypeIdV1::from_index(ty)
                )
                .unwrap()]
            );
        }
        assert_eq!(owner.totals.locals, 1);
        assert_eq!(owner.totals.statements, 2);
        assert_eq!(owner.totals.projections, 2);
        assert_eq!(owner.totals.operands, 2);
    }

    #[test]
    fn closure_rust_call_rejects_missing_duplicate_or_mistyped_field_roles() {
        for (role, ty) in [
            (SemanticLocalRoleV1::Temporary, 1),
            (SemanticLocalRoleV1::Argument(0), 1),
            (SemanticLocalRoleV1::Argument(2), 1),
            (SemanticLocalRoleV1::Argument(1), 2),
            (SemanticLocalRoleV1::Return, 1),
        ] {
            let (arguments, mut locals, blocks) = fixture();
            locals[3] = local(4, ty, role);
            let before = (locals.clone(), blocks.clone());
            let error = arguments
                .normalize_locals(&mut locals, &mut owner(SemanticMirLimitsV1::default()))
                .unwrap_err();
            assert!(matches!(
                error,
                ProductionSemanticBodyErrorV1::IdentityTableMismatch {
                    table: "closure RustCall field role or type"
                }
            ));
            assert_eq!((locals, blocks), before);
        }
    }

    #[test]
    fn closure_rust_call_empty_tuple_retains_the_required_source_argument() {
        let (mut arguments, mut locals, mut blocks) = fixture();
        arguments.fields.clear();
        locals.truncate(2);
        let before = blocks.clone();
        arguments
            .normalize_locals(&mut locals, &mut owner(SemanticMirLimitsV1::default()))
            .unwrap()
            .initialize_entry(&mut blocks, SemanticBlockIdV1::from_index(1))
            .unwrap();
        assert_eq!(locals[2].role(), SemanticLocalRoleV1::Argument(1));
        assert_eq!(blocks, before);
    }

    #[test]
    fn closure_rust_call_inserts_by_identity_and_preserves_source_local_bindings() {
        for (insertion, identity) in [10, 30, 50, 70, 90].into_iter().enumerate() {
            let (mut arguments, mut locals, mut blocks) = fixture();
            for (index, declaration) in locals.iter_mut().enumerate() {
                *declaration = local(
                    (index as u8 + 1) * 20,
                    declaration.ty().index(),
                    declaration.role(),
                );
            }
            arguments.tuple_identity = SemanticLocalIdentityV1::from_sha256([identity; 32]);
            let tuple_identity = arguments.tuple_identity;
            let original = locals.clone();
            let normalized = arguments
                .normalize_locals(&mut locals, &mut owner(SemanticMirLimitsV1::default()))
                .unwrap();
            let order = normalized.local_order;
            assert_eq!(order.inserted_local.index() as usize, insertion);
            assert!(
                locals
                    .windows(2)
                    .all(|pair| pair[0].identity() < pair[1].identity())
            );
            assert_eq!(locals[insertion].identity(), tuple_identity);
            assert_eq!(locals[insertion].role(), SemanticLocalRoleV1::Argument(1));
            for (old, declaration) in original.iter().enumerate() {
                let mapped = order
                    .remap(SemanticLocalIdV1::from_index(old as u32))
                    .unwrap();
                let expected = old + usize::from(old >= insertion);
                assert_eq!(mapped.index() as usize, expected);
                assert_eq!(locals[expected].identity(), declaration.identity());
                assert_eq!(locals[expected].ty(), declaration.ty());
                assert_eq!(locals[expected].source(), declaration.source());
                assert_eq!(
                    locals[expected].role(),
                    if old >= 2 {
                        SemanticLocalRoleV1::Temporary
                    } else {
                        declaration.role()
                    },
                );
            }
            assert!(
                order
                    .remap(SemanticLocalIdV1::from_index(original.len() as u32))
                    .is_err()
            );
            assert!(
                order
                    .remap(SemanticLocalIdV1::from_index(u32::MAX))
                    .is_err()
            );
            normalized
                .initialize_entry(&mut blocks, SemanticBlockIdV1::from_index(1))
                .unwrap();
            for (field, original_local) in [3, 2].into_iter().enumerate() {
                let SemanticStatementKindV1::Assign(assignment) =
                    blocks[1].statements()[field].kind()
                else {
                    panic!("expected tuple-field initialization")
                };
                assert_eq!(
                    assignment.destination().local(),
                    order
                        .remap(SemanticLocalIdV1::from_index(original_local))
                        .unwrap(),
                );
                let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) =
                    assignment.value().kind()
                else {
                    panic!("expected tuple-field move")
                };
                assert_eq!(place.local(), order.inserted_local);
                assert_eq!(
                    place.projections()[0].kind(),
                    SemanticProjectionKindV1::Field(field as u32)
                );
            }
        }
    }

    #[test]
    fn closure_rust_call_rejects_noncanonical_or_colliding_local_identities() {
        for mutation in 0..3 {
            let (mut arguments, mut locals, _) = fixture();
            match mutation {
                0 => arguments.tuple_identity = locals[2].identity(),
                1 => locals[1] = local(1, 0, SemanticLocalRoleV1::Argument(0)),
                2 => locals[0] = local(5, 0, SemanticLocalRoleV1::Return),
                _ => unreachable!(),
            }
            let original = locals.clone();
            assert!(matches!(
                arguments.normalize_locals(&mut locals, &mut owner(SemanticMirLimitsV1::default())),
                Err(ProductionSemanticBodyErrorV1::IdentityTableMismatch {
                    table: "closure RustCall local identity order"
                })
            ));
            assert_eq!(locals, original);
        }
    }

    #[test]
    fn closure_rust_call_normalization_charges_cumulative_resource_limits() {
        for resource in [
            SemanticMirResourceV1::Locals,
            SemanticMirResourceV1::Statements,
            SemanticMirResourceV1::Projections,
            SemanticMirResourceV1::Operands,
            SemanticMirResourceV1::ValidationWork,
        ] {
            let (arguments, mut locals, blocks) = fixture();
            let before = (locals.clone(), blocks.clone());
            let mut owner = owner(
                SemanticMirLimitsV1::default()
                    .with_limit(resource, 1)
                    .unwrap(),
            );
            owner.charge(resource, 1).unwrap();
            let error = arguments
                .normalize_locals(&mut locals, &mut owner)
                .unwrap_err();
            assert!(
                matches!(error, ProductionSemanticBodyErrorV1::LimitExceeded { resource: actual, .. } if actual == resource)
            );
            assert_eq!((locals, blocks), before);
        }
    }
}
