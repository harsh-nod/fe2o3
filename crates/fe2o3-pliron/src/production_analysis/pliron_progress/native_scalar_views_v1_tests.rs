use super::*;
use dialect_gpu::optimization_v1::{
    BinaryKindAttr, BinaryOp, CastKindAttr, CastOp, ConstantOp, IndexAttr,
};
use pliron::{
    builtin::{
        attributes::{FPSingleAttr, IntegerAttr},
        types::{IntegerType, Signedness},
    },
    r#type::TypeHandle,
    utils::apint::{APInt, bw},
    value::Value,
};

fn context() -> Context {
    let mut context = Context::new();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

fn literal(context: &mut Context, width: u32, signed: bool, bits: u128) -> Value {
    let ty = IntegerType::get(
        context,
        width,
        if signed {
            Signedness::Signed
        } else {
            Signedness::Unsigned
        },
    );
    let attribute = IntegerAttr::new(ty, APInt::from_u128(bits, bw(width as usize)));
    ConstantOp::new(context, Box::new(attribute)).result(context)
}

fn argument(context: &mut Context, ty: TypeHandle) -> Value {
    BasicBlock::new(context, None, vec![ty])
        .deref(context)
        .get_argument(0)
}

#[test]
fn native_progress_literals_preserve_width_sign_and_order_independently() {
    let mut context = context();
    for width in [8, 16, 32, 64, 128] {
        let high = 1_u128 << (width - 1);
        let maximum = if width == 128 {
            u128::MAX
        } else {
            (1_u128 << width) - 1
        };
        for signed in [false, true] {
            for bits in [0, 1, high - 1, high, maximum] {
                let value = literal(&mut context, width, signed, bits);
                let decoded = native_progress_literal_v1(&context, value).unwrap();
                assert_eq!(
                    decoded.domain,
                    NativeProgressDomainV1::Fixed { width, signed }
                );
                assert_eq!(decoded.bits, bits);
                let expected = if signed {
                    if bits < high {
                        bits + high
                    } else {
                        bits - high
                    }
                } else {
                    bits
                };
                assert_eq!(decoded.ordered(), Some(expected));
            }
        }
    }
}

#[test]
fn native_progress_u8_no_wrap_matches_exhaustive_mathematical_update() {
    let mut context = context();
    for signed in [false, true] {
        let domain = NativeProgressDomainV1::Fixed { width: 8, signed };
        for bound_bits in [0_u128, 1, 2, 125, 126, 127, 128, 254, 255] {
            let bound = literal(&mut context, 8, signed, bound_bits);
            for step in [1_u64, 2, 3, 7, 127] {
                let numeric_bound = if signed {
                    i32::from(bound_bits as u8 as i8)
                } else {
                    bound_bits as i32
                };
                let (minimum, maximum) = if signed { (-128, 127) } else { (0, 255) };
                let independently_safe = (minimum..=maximum)
                    .filter(|initial| *initial < numeric_bound)
                    .all(|initial| initial + step as i32 <= maximum);
                assert_eq!(
                    native_progress_no_wrap_v1(&context, bound, domain, step),
                    independently_safe,
                    "signed={signed}, bound={bound_bits}, step={step}"
                );
            }
        }
    }
}

#[test]
fn native_progress_step_is_actual_add_result_zero_and_exact_u64() {
    let mut context = context();
    for width in [8, 16, 32, 64, 128] {
        for signed in [false, true] {
            let one = literal(&mut context, width, signed, 1);
            let ty = one.get_type(&context);
            let base = argument(&mut context, ty);
            let domain = NativeProgressDomainV1::Fixed { width, signed };
            for checked in [false, true] {
                for commuted in [false, true] {
                    let (lhs, rhs) = if commuted { (one, base) } else { (base, one) };
                    let op = BinaryOp::new(
                        &mut context,
                        if checked {
                            BinaryKindAttr::CheckedAdd
                        } else {
                            BinaryKindAttr::Add
                        },
                        lhs,
                        rhs,
                    );
                    assert_eq!(
                        progress_induction_offset_v1(&context, op.result(&context), base, domain),
                        Some(1)
                    );
                    if let Some(flag) = op.overflow(&context) {
                        assert_eq!(
                            progress_induction_offset_v1(&context, flag, base, domain),
                            None
                        );
                    }
                }
            }
            let minus_one = literal(
                &mut context,
                width,
                signed,
                if width == 128 {
                    u128::MAX
                } else {
                    (1_u128 << width) - 1
                },
            );
            let add = BinaryOp::new(&mut context, BinaryKindAttr::Add, base, minus_one);
            if signed || width == 128 {
                assert_eq!(
                    progress_induction_offset_v1(&context, add.result(&context), base, domain),
                    None
                );
            }
            let subtract = BinaryOp::new(&mut context, BinaryKindAttr::Subtract, base, one);
            assert_eq!(
                progress_induction_offset_v1(&context, subtract.result(&context), base, domain),
                None
            );
        }
    }
    let large = literal(&mut context, 128, false, u128::from(u64::MAX) + 1);
    let ty = large.get_type(&context);
    let base = argument(&mut context, ty);
    let add = BinaryOp::new(&mut context, BinaryKindAttr::Add, base, large);
    assert_eq!(
        progress_induction_offset_v1(
            &context,
            add.result(&context),
            base,
            NativeProgressDomainV1::Fixed {
                width: 128,
                signed: false
            }
        ),
        None
    );
}

#[test]
fn native_progress_widened_literals_and_dynamic_bounds_use_actual_cast_semantics() {
    let mut context = context();
    for signed in [false, true] {
        let mut one = literal(&mut context, 8, signed, 1);
        let mut negative = literal(&mut context, 8, signed, 255);
        for width in [16, 32, 64, 128] {
            let ty = IntegerType::get(
                &context,
                width,
                if signed {
                    Signedness::Signed
                } else {
                    Signedness::Unsigned
                },
            )
            .into();
            let kind = if signed {
                CastKindAttr::SignExtend
            } else {
                CastKindAttr::ZeroExtend
            };
            one = CastOp::new(&mut context, kind, one, ty).result(&context);
            negative = CastOp::new(&mut context, kind, negative, ty).result(&context);
            assert_eq!(native_progress_literal_v1(&context, one).unwrap().bits, 1);
            assert_eq!(
                native_progress_literal_v1(&context, negative).unwrap().bits,
                if signed {
                    if width == 128 {
                        u128::MAX
                    } else {
                        (1_u128 << width) - 1
                    }
                } else {
                    255
                }
            );
        }
        let ty = one.get_type(&context);
        let base = argument(&mut context, ty);
        let add = BinaryOp::new(&mut context, BinaryKindAttr::CheckedAdd, base, one);
        assert_eq!(
            progress_induction_offset_v1(
                &context,
                add.result(&context),
                base,
                NativeProgressDomainV1::Fixed { width: 128, signed }
            ),
            Some(1)
        );
    }
    let source_type = IntegerType::get(&context, 8, Signedness::Unsigned).into();
    let source = argument(&mut context, source_type);
    let target_type = IntegerType::get(&context, 16, Signedness::Unsigned).into();
    let bound =
        CastOp::new(&mut context, CastKindAttr::ZeroExtend, source, target_type).result(&context);
    let domain = NativeProgressDomainV1::Fixed {
        width: 16,
        signed: false,
    };
    assert_eq!(
        native_progress_bound_upper_v1(&context, bound, domain),
        Some(255)
    );
    assert!(native_progress_no_wrap_v1(&context, bound, domain, 257));
    let wide_type = IntegerType::get(&context, 32, Signedness::Unsigned).into();
    let wide =
        CastOp::new(&mut context, CastKindAttr::ZeroExtend, bound, wide_type).result(&context);
    let wide_domain = NativeProgressDomainV1::Fixed {
        width: 32,
        signed: false,
    };
    assert_eq!(
        native_progress_bound_upper_v1(&context, wide, wide_domain),
        Some(255)
    );
    assert!(native_progress_no_wrap_v1(
        &context,
        wide,
        wide_domain,
        u64::from(u32::MAX) - 254
    ));
    assert!(!native_progress_no_wrap_v1(
        &context,
        wide,
        wide_domain,
        u64::from(u32::MAX) - 253
    ));
    let wrong =
        CastOp::new(&mut context, CastKindAttr::SignExtend, source, target_type).result(&context);
    assert!(native_progress_cast_v1(&context, wrong).is_none());
    let narrower =
        CastOp::new(&mut context, CastKindAttr::Truncate, bound, source_type).result(&context);
    assert!(native_progress_literal_v1(&context, narrower).is_none());
    assert!(!native_progress_no_wrap_v1(
        &context,
        narrower,
        NativeProgressDomainV1::Fixed {
            width: 8,
            signed: false
        },
        2
    ));
}

#[test]
fn native_progress_index_unit_proof_has_no_assumed_target_width() {
    let mut context = context();
    let one = ConstantOp::new(&mut context, Box::new(IndexAttr(1))).result(&context);
    let two = ConstantOp::new(&mut context, Box::new(IndexAttr(2))).result(&context);
    let ty = one.get_type(&context);
    let bound = argument(&mut context, ty);
    assert_eq!(
        native_progress_domain_v1(&context, bound),
        Some(NativeProgressDomainV1::IndexUnknown)
    );
    assert!(native_progress_no_wrap_v1(
        &context,
        bound,
        NativeProgressDomainV1::IndexUnknown,
        1
    ));
    assert!(!native_progress_no_wrap_v1(
        &context,
        two,
        NativeProgressDomainV1::IndexUnknown,
        2
    ));
    assert!(!native_progress_no_wrap_v1(
        &context,
        bound,
        NativeProgressDomainV1::IndexUnknown,
        2
    ));
    assert_eq!(
        native_progress_literal_v1(&context, two).unwrap().ordered(),
        None
    );
}

#[test]
fn native_progress_noninteger_and_malformed_literal_reads_fail_before_copy() {
    let mut context = context();
    let element = IntegerType::get(&context, 32, Signedness::Unsigned).into();
    let vector = dialect_gpu::vector_v12::FixedVectorTypeV12::try_get(
        &context,
        element,
        4,
        dialect_gpu::vector_v12::VectorLayoutAttrV12::CONTIGUOUS,
    )
    .unwrap()
    .into();
    let pointer = dialect_gpu::optimization_v1::PointerType::get(
        &context,
        element,
        dialect_gpu::AddressSpaceAttr::Global,
        dialect_gpu::optimization_v1::AccessModeAttr::ReadOnly,
    )
    .into();
    for ty in [vector, pointer] {
        let value = argument(&mut context, ty);
        assert!(native_progress_domain_v1(&context, value).is_none());
    }
    for float in [f32::NAN, -0.0, 1.0] {
        let constant = ConstantOp::new(&mut context, Box::new(FPSingleAttr::from(float)));
        assert!(native_progress_domain_v1(&context, constant.result(&context)).is_none());
        assert!(native_progress_literal_v1(&context, constant.result(&context)).is_none());
    }
    for (width, sign) in [
        (1, Signedness::Signless),
        (7, Signedness::Unsigned),
        (256, Signedness::Signed),
    ] {
        let ty = IntegerType::get(&context, width, sign);
        let attr = IntegerAttr::new(ty, APInt::zero(bw(width as usize)));
        let value = ConstantOp::new(&mut context, Box::new(attr)).result(&context);
        assert!(native_progress_literal_v1(&context, value).is_none());
    }
    let ty = IntegerType::get(&context, 8, Signedness::Unsigned);
    let malformed = IntegerAttr::new(ty, APInt::zero(bw(1024)));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ConstantOp::new(&mut context, Box::new(malformed.clone()))
        }))
        .is_err()
    );
    // The typed setter refuses this attribute. Raw mutation independently
    // exercises the reader's rejection before copying its oversized payload.
    let value = literal(&mut context, 8, false, 1);
    value
        .defining_op()
        .unwrap()
        .deref_mut(&context)
        .attributes
        .set("gpu_constant_value".try_into().unwrap(), malformed);
    native_progress_scalar_observation_v1::reset();
    native_progress_scalar_observation_v1::panic_after_literal();
    assert!(native_progress_literal_v1(&context, value).is_none());
    assert_eq!(native_progress_scalar_observation_v1::copy_attempts(), 0);
    native_progress_scalar_observation_v1::reset();
    let value = literal(&mut context, 8, false, 1);
    assert!(native_progress_literal_v1(&context, value).is_some());
    assert_eq!(native_progress_scalar_observation_v1::copy_attempts(), 1);
    native_progress_scalar_observation_v1::reset();
    let signed = IntegerType::get(&context, 8, Signedness::Signed).into();
    value.set_type(&context, signed);
    assert!(native_progress_literal_v1(&context, value).is_none());
    assert_eq!(native_progress_scalar_observation_v1::copy_attempts(), 0);
}

#[test]
fn native_progress_header_requires_actual_comparison_and_exact_graph_membership() {
    use super::native_loop_v1_tests::{parse, source};
    let (context, function) = parse(source("dynamic"));
    verify_operation(function.get_operation(), &context).unwrap();
    let inventory = bounded_structural_inventory(&context, &function).unwrap();
    let pointer = inventory.root_blocks[1]
        .deref(&context)
        .get_terminator(&context)
        .unwrap();
    let terminator = Operation::get_op_dyn(pointer, &context);
    assert!(
        progress_header_view_v1(&context, &terminator, &inventory.root_operation_blocks).is_some()
    );
    assert!(progress_header_view_v1(&context, &terminator, &HashMap::new()).is_none());
    let (foreign_context, foreign) = parse(source("dynamic"));
    assert_ne!(foreign.get_operation(), function.get_operation());
    assert!(matches!(
        run_pliron_progress_check_v1(&foreign_context, &function).findings(),
        [PlironProgressFindingV1::StructuralPrerequisiteRejected { .. }]
    ));
    let mut incomplete = inventory.root_operation_blocks.clone();
    let compare = terminator
        .get_operation()
        .deref(&context)
        .get_operand(0)
        .defining_op()
        .unwrap();
    incomplete.remove(&compare);
    assert!(progress_header_view_v1(&context, &terminator, &incomplete).is_none());
    let lhs = compare.deref(&context).get_operand(0);
    Operation::replace_operand(pointer, &context, 0, lhs);
    assert!(
        progress_header_view_v1(
            &context,
            &Operation::get_op_dyn(pointer, &context),
            &inventory.root_operation_blocks
        )
        .is_none()
    );
}

#[test]
fn native_progress_scalar_shapes_reject_extra_results_operands_and_regions() {
    let mut context = context();
    let one = literal(&mut context, 8, false, 1);
    let ty = one.get_type(&context);
    let base = argument(&mut context, ty);
    let wrong_width = literal(&mut context, 16, false, 1);
    let binary = BinaryOp::new(&mut context, BinaryKindAttr::Add, base, wrong_width);
    assert_eq!(
        progress_induction_offset_v1(
            &context,
            binary.result(&context),
            base,
            NativeProgressDomainV1::Fixed {
                width: 8,
                signed: false
            }
        ),
        None
    );
    for (operands, results, regions) in [
        (vec![base], 1, 0),
        (vec![base, one], 3, 0),
        (vec![base, one], 1, 1),
    ] {
        let operation = Operation::new(
            &mut context,
            BinaryOp::get_concrete_op_info(),
            vec![ty; results],
            operands,
            vec![],
            regions,
        );
        let binary = BinaryOp::from_operation(operation);
        binary.set_attr_gpu_binary_kind(&context, BinaryKindAttr::Add);
        assert_eq!(
            progress_induction_offset_v1(
                &context,
                binary.result(&context),
                base,
                NativeProgressDomainV1::Fixed {
                    width: 8,
                    signed: false
                }
            ),
            None
        );
    }
}

#[test]
fn native_progress_malformed_conditional_segments_do_not_make_a_header_view() {
    use super::native_loop_v1_tests::{parse, source};
    let (context, function) = parse(&source("dynamic").replace("[1, 1, 1]", "[1, 2, 0]"));
    let inventory = bounded_structural_inventory(&context, &function).unwrap();
    let pointer = inventory.root_blocks[1]
        .deref(&context)
        .get_terminator(&context)
        .unwrap();
    assert!(
        progress_header_view_v1(
            &context,
            &Operation::get_op_dyn(pointer, &context),
            &inventory.root_operation_blocks
        )
        .is_none()
    );
    assert!(verify_operation(function.get_operation(), &context).is_err());
}

#[test]
fn native_progress_verified_signed_unsigned_cross_zero_bound_is_conservative() {
    use pliron::common_traits::Verify;
    let mut context = context();
    let source_type = IntegerType::get(&context, 8, Signedness::Signed).into();
    let source = argument(&mut context, source_type);
    let target_type = IntegerType::get(&context, 16, Signedness::Unsigned).into();
    let operation = CastOp::new(&mut context, CastKindAttr::SignExtend, source, target_type);
    operation.verify(&context).unwrap();
    let bound = operation.result(&context);
    let (actual_source, cast) = native_progress_cast_v1(&context, bound).unwrap();
    assert_eq!(actual_source, source);
    assert_eq!(cast.interval(127, 128), Some((0, u16::MAX.into())));
    assert_eq!(cast.interval(0, 255), Some((0, u16::MAX.into())));
    let domain = NativeProgressDomainV1::Fixed {
        width: 16,
        signed: false,
    };
    assert_eq!(
        native_progress_bound_upper_v1(&context, bound, domain),
        Some(u16::MAX.into())
    );
    assert!(native_progress_no_wrap_v1(&context, bound, domain, 1));
    assert!(!native_progress_no_wrap_v1(&context, bound, domain, 2));
    for value in i8::MIN..=i8::MAX {
        let expected = u128::from(value as i16 as u16);
        assert!(expected <= native_progress_bound_upper_v1(&context, bound, domain).unwrap());
    }
}

#[test]
fn native_progress_verified_signed_unsigned_noncrossing_intervals_are_exact() {
    use pliron::common_traits::Verify;
    let mut context = context();
    let source_type = IntegerType::get(&context, 8, Signedness::Signed).into();
    let source = argument(&mut context, source_type);
    let target_type = IntegerType::get(&context, 16, Signedness::Unsigned).into();
    let operation = CastOp::new(&mut context, CastKindAttr::SignExtend, source, target_type);
    operation.verify(&context).unwrap();
    let (_, cast) = native_progress_cast_v1(&context, operation.result(&context)).unwrap();
    for (low, high) in [(-128_i16, -1_i16), (-71, -3), (0, 127), (9, 87)] {
        let expected = (u128::from(low as u16), u128::from(high as u16));
        let actual = cast
            .interval((low + 128) as u128, (high + 128) as u128)
            .unwrap();
        assert_eq!(actual, expected);
        for value in low..=high {
            assert!((actual.0..=actual.1).contains(&u128::from(value as u16)));
        }
    }
    for value in [-128_i16, -1, 0, 127] {
        let source = literal(&mut context, 8, true, u128::from(value as u8));
        let operation = CastOp::new(&mut context, CastKindAttr::SignExtend, source, target_type);
        operation.verify(&context).unwrap();
        let bound = operation.result(&context);
        let expected = u128::from(value as u16);
        assert_eq!(
            native_progress_literal_v1(&context, bound).unwrap().bits,
            expected
        );
        assert_eq!(
            native_progress_bound_upper_v1(
                &context,
                bound,
                NativeProgressDomainV1::Fixed {
                    width: 16,
                    signed: false
                }
            ),
            Some(expected)
        );
    }
}

#[test]
fn native_progress_verified_mixed_sign_cast_chain_preserves_dynamic_interval() {
    use pliron::common_traits::Verify;
    let mut context = context();
    for signed_source in [false, true] {
        let source_type = IntegerType::get(
            &context,
            8,
            if signed_source {
                Signedness::Signed
            } else {
                Signedness::Unsigned
            },
        )
        .into();
        let source = argument(&mut context, source_type);
        let middle_type = IntegerType::get(
            &context,
            16,
            if signed_source {
                Signedness::Unsigned
            } else {
                Signedness::Signed
            },
        )
        .into();
        let first = CastOp::new(
            &mut context,
            if signed_source {
                CastKindAttr::SignExtend
            } else {
                CastKindAttr::ZeroExtend
            },
            source,
            middle_type,
        );
        first.verify(&context).unwrap();
        let target_type = IntegerType::get(
            &context,
            32,
            if signed_source {
                Signedness::Signed
            } else {
                Signedness::Unsigned
            },
        )
        .into();
        let first_value = first.result(&context);
        let second = CastOp::new(
            &mut context,
            if signed_source {
                CastKindAttr::ZeroExtend
            } else {
                CastKindAttr::SignExtend
            },
            first_value,
            target_type,
        );
        second.verify(&context).unwrap();
        let domain = NativeProgressDomainV1::Fixed {
            width: 32,
            signed: signed_source,
        };
        let bound = second.result(&context);
        let expected = if signed_source {
            (1_u128 << 31) + u128::from(u16::MAX)
        } else {
            255
        };
        assert_eq!(
            native_progress_bound_upper_v1(&context, bound, domain),
            Some(expected)
        );
        assert!(native_progress_no_wrap_v1(&context, bound, domain, 65_537));
        for bits in 0..=u8::MAX {
            let coordinate = if signed_source {
                (1_u128 << 31) + u128::from(bits as i8 as i16 as u16)
            } else {
                u128::from(bits)
            };
            assert!(coordinate <= expected);
        }
    }
}
