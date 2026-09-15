use super::*;

impl Analysis<'_> {
    // Only the retained call's normal return edge initializes this value. An
    // Opaque value at a join/projection is not evidence to initialize payloads.
    // Variant activation still requires the exact discriminant-dominance proof.
    pub(super) fn call_return_value(&mut self, local: u32, flow: &Flow) -> Result<Value> {
        self.budget.charge(1)?;
        let ty = self.function().locals().get(local as usize).ok_or(())?.ty();
        let mut counts = [0; 2];
        for (variant, count) in counts.iter_mut().enumerate() {
            self.budget.charge(1)?;
            let Some(fields) = self.variant_fields(ty, variant as u32) else {
                return Ok(Value::Opaque);
            };
            *count = fields.len();
            // No recursion, reference origins, capability fields, or aggregates.
            // These scalar bits still have no numerical/effect-refinement proof.
            let supported = fields.iter().all(|ty| self.scalar(*ty));
            self.budget.charge(*count)?;
            if !supported {
                return Ok(Value::Opaque);
            }
        }
        let nodes = 3 + counts.iter().sum::<usize>();
        self.budget.charge(nodes)?;
        self.budget.check_storage(&[flow.nodes(), nodes])?;
        Ok(Value::Variants {
            ty,
            possible: 3,
            fields: counts
                .into_iter()
                .map(|count| Some(Value::Fields(vec![Some(Value::Opaque); count])))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;
    use global_enum_transport_v1::tests::scalar_enum_result_tests::scalar_result_owner;

    fn payload(function: &SemanticFunctionDeclV1) -> &SemanticPlaceV1 {
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[4].statements()[1].kind()
        else {
            panic!()
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            panic!()
        };
        place
    }

    #[test]
    fn scalar_enum_result_initialization_is_only_on_the_normal_call_edge() {
        let owner = scalar_result_owner();
        let view = owner
            .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        let mut analysis = Analysis::new(owner.source_semantic().types(), view);
        assert_eq!(
            analysis.terminator(2, &mut Flow::default()).unwrap(),
            BTreeSet::from([(3, Some(7))])
        );
        assert!(
            analysis
                .terminator(3, &mut Flow::default())
                .unwrap()
                .iter()
                .all(|(_, local)| local.is_none())
        );
        assert!(
            analysis
                .terminator(8, &mut Flow::default())
                .unwrap()
                .is_empty()
        );
        let value = analysis.call_return_value(7, &Flow::default()).unwrap();
        assert_eq!(value.nodes(), 4);
        assert!(!value.contains_references());
        let Value::Variants {
            ty,
            possible,
            fields,
        } = value
        else {
            panic!()
        };
        assert_eq!(ty, SemanticTypeIdV1::from_index(9));
        assert_eq!(possible, 3);
        assert_eq!(
            fields,
            vec![
                Some(Value::Fields(vec![])),
                Some(Value::Fields(vec![Some(Value::Opaque)]))
            ]
        );
    }

    #[test]
    fn scalar_enum_result_unknown_partial_and_wrong_variant_payloads_stay_uninitialized() {
        let owner = scalar_result_owner();
        let view = owner
            .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        let mut analysis = Analysis::new(owner.source_semantic().types(), view);
        let place = payload(view.body());
        let result = analysis.call_return_value(7, &Flow::default()).unwrap();
        let mut known = Flow::default();
        known.values.insert(7, result.clone());
        assert_eq!(
            analysis.read(place, &mut known, 4).unwrap(),
            Some(Value::Opaque)
        );
        let mut partial = result.clone();
        let Value::Variants { fields, .. } = &mut partial else {
            panic!()
        };
        fields[1] = Some(Value::Fields(vec![None]));
        for value in [None, Some(Value::Opaque), Some(partial)] {
            let mut flow = Flow::default();
            if let Some(value) = value {
                flow.values.insert(7, value);
            }
            assert_eq!(analysis.read(place, &mut flow, 4).unwrap(), None);
        }
        let mut wrong_edge = Flow::default();
        wrong_edge.values.insert(7, result);
        assert_eq!(analysis.read(place, &mut wrong_edge, 8).unwrap(), None);
        let mut unknown = Flow::default();
        unknown.values.insert(7, Value::Opaque);
        let mut joined = known.join(&unknown, &mut analysis.budget).unwrap();
        assert_eq!(analysis.read(place, &mut joined, 4).unwrap(), None);
    }

    #[test]
    fn scalar_enum_result_does_not_initialize_reference_or_aggregate_payloads() {
        let owner = scalar_result_owner();
        let view = owner
            .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        for payload in [8, 12, 16] {
            let mut types = owner.source_semantic().types().to_vec();
            let original = &types[9];
            types[9] = SemanticTypeDeclV1::new(
                original.identity(),
                original.layout_identity(),
                original.layout().clone(),
                SemanticTypeShapeV1::enum_type(
                    SemanticTypeIdV1::from_index(1),
                    vec![
                        SemanticEnumVariantV1::new(
                            0,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        ),
                        SemanticEnumVariantV1::new(
                            1,
                            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(
                                payload,
                            )])
                            .unwrap(),
                        ),
                    ],
                )
                .unwrap(),
            );
            let mut analysis = Analysis::new(&types, view);
            assert_eq!(
                analysis.call_return_value(7, &Flow::default()).unwrap(),
                Value::Opaque
            );
        }
    }

    #[test]
    fn scalar_enum_result_respects_existing_work_storage_and_dead_slot_limits() {
        let owner = scalar_result_owner();
        let view = owner
            .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        let mut analysis = Analysis::new(owner.source_semantic().types(), view);
        analysis.budget = Budget::new(0);
        assert!(analysis.call_return_value(7, &Flow::default()).is_err());
        analysis.budget = Budget::new(MAX_WORK);
        let held = analysis.budget.reserve(MAX_STORAGE).unwrap();
        assert!(analysis.call_return_value(7, &Flow::default()).is_err());
        drop(held);
        let mut dead = Flow::default();
        dead.dead.insert(7);
        let result = analysis.call_return_value(7, &dead).unwrap();
        dead.assign(7, Some(result), &mut analysis.budget).unwrap();
        assert!(!dead.values.contains_key(&7));
    }

    #[test]
    fn scalar_enum_result_keeps_two_case_and_field_count_bounds() {
        let owner = scalar_result_owner();
        let view = owner
            .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        for (cases, fields) in [(1, 1), (3, 1), (2, MAX_FIELDS + 1)] {
            let mut types = owner.source_semantic().types().to_vec();
            let original = &types[9];
            types[9] = SemanticTypeDeclV1::new(
                original.identity(),
                original.layout_identity(),
                original.layout().clone(),
                SemanticTypeShapeV1::enum_type(
                    SemanticTypeIdV1::from_index(1),
                    (0..cases)
                        .map(|case| {
                            SemanticEnumVariantV1::new(
                                case,
                                SemanticAggregateTypeV1::new(vec![
                                    SemanticTypeIdV1::from_index(7);
                                    fields
                                ])
                                .unwrap(),
                            )
                        })
                        .collect(),
                )
                .unwrap(),
            );
            let mut analysis = Analysis::new(&types, view);
            assert_eq!(
                analysis.call_return_value(7, &Flow::default()).unwrap(),
                Value::Opaque
            );
        }
    }
}
