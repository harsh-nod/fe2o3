fn value_type_cases_v57(width: FormalIndexWidth) -> Vec<(Type, String, String)> {
    let index_bytes = if width == FormalIndexWidth::Bits32 {
        4
    } else {
        8
    };
    let mut cases = Vec::new();
    for (scalar, bytes) in [
        (ScalarType::Bool, 0),
        (ScalarType::U8, 1),
        (ScalarType::I8, 1),
        (ScalarType::U16, 2),
        (ScalarType::I16, 2),
        (ScalarType::U32, 4),
        (ScalarType::I32, 4),
        (ScalarType::U64, 8),
        (ScalarType::I64, 8),
        (ScalarType::F32, 4),
        (ScalarType::F64, 8),
        (ScalarType::Index, index_bytes),
    ] {
        let bound = if scalar == ScalarType::Bool {
            "2".to_owned()
        } else {
            format!("memory_value_modulus_v30({bytes})")
        };
        cases.push((
            Type::Scalar(scalar),
            format!("byte_scalar_type_v57(value, {bound})"),
            format!("match value {{ MemoryValueV30::Scalar(v) => 0 <= v < {bound}, _ => false }}"),
        ));
    }
    for (space, encoded) in [
        (AddressSpace::Private, 0),
        (AddressSpace::Global, 1),
        (AddressSpace::Generic, 2),
    ] {
        cases.push((
            Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadOnly),
            format!("byte_pointer_value_type_v57(value, {encoded}, {index_bytes})"),
            format!("match value {{ MemoryValueV30::Pointer(p) => byte_pointer_type_v30(p, {encoded}, {index_bytes}), _ => false }}"),
        ));
        cases.push((
            Type::slice(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite),
            format!("byte_slice_type_v57(value, {encoded}, {index_bytes})"),
            format!("match value {{ MemoryValueV30::Slice(slice) => 0 <= slice.length < memory_value_modulus_v30({index_bytes}) && byte_pointer_type_v30(slice.pointer, {encoded}, {index_bytes}), _ => false }}"),
        ));
    }
    cases.push((
        Type::Vector(fe2o3_kernel_ir::FixedVectorTypeV12::new(
            ScalarType::U16,
            4,
            fe2o3_kernel_ir::VectorLayoutV12::Interleaved { factor: 2 },
        )),
        "byte_vector_type_v57(value, 4, 2)".to_owned(),
        "match value { MemoryValueV30::Vector(v) => v.len() == 4 && (forall|lane: int| 0 <= lane < v.len() ==> 0 <= v[lane] < memory_value_modulus_v30(2)), _ => false }".to_owned(),
    ));
    cases
}

#[test]
fn byte_value_type_shared_predicates_keep_every_shape_width_and_address_space() {
    for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
        for (ty, expected, _) in value_type_cases_v57(width) {
            let actual = run(37, LIMIT, LIMIT, |out| {
                emit_value_type(&ty, width, "value", out)
            });
            assert_eq!(actual.0.unwrap(), expected);
            if let Type::Pointer(pointer) = ty {
                assert_eq!(
                    run(37, LIMIT, LIMIT, |out| {
                        emit_pointer_value_type(pointer.address_space, width, "value", out)
                    })
                    .0
                    .unwrap(),
                    expected,
                );
            }
        }
        assert_eq!(
            run(37, LIMIT, LIMIT, |out| emit_value_type(
                &Type::Unit,
                width,
                "value",
                out
            ))
            .0
            .unwrap(),
            "matches!(value, MemoryValueV30::Unit)",
        );
        assert!(matches!(
            run(37, LIMIT, LIMIT, |out| emit_value_type(
                &Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(0)),
                width,
                "value",
                out,
            ))
            .0,
            Err(Error::Statement("actual byte value type is not modeled")),
        ));
    }
}

fn value_type_census_v57(
    shared: bool,
    work: usize,
    storage: usize,
) -> (Result<String>, usize, usize) {
    let width = FormalIndexWidth::Bits64;
    let cases = value_type_cases_v57(width);
    run(37, work, storage, |out| {
        emit!(
            out,
            "use vstd::prelude::*;\nverus! {{\n{}",
            super::super::byte_memory_v30::BYTE_MEMORY_V30
        );
        for segment in 0..32 {
            emit!(
                out,
                "open spec fn value_type_segment_{segment}_v57(values: Seq<MemoryValueV30>) -> bool {{ values.len() == 64"
            );
            for definition in 0..64 {
                let (ty, _, old) = &cases[definition % cases.len()];
                emit!(out, "\n && ({{ let value = values[{definition}]; ");
                if shared {
                    emit_value_type(ty, width, "value", out)?;
                } else {
                    emit!(out, "{old}");
                }
                emit!(out, " }})");
            }
            emit!(out, " }}\n");
        }
        emit!(out, "}}\n");
        Ok(())
    })
}

#[test]
fn byte_value_type_sharing_bounds_complete_source_without_dropping_conditions() {
    let shared = value_type_census_v57(true, LIMIT, LIMIT).0.unwrap();
    let inline = value_type_census_v57(false, LIMIT, LIMIT).0.unwrap();
    for source in [&shared, &inline] {
        assert_eq!(
            source.matches("open spec fn value_type_segment_").count(),
            32
        );
        assert_eq!(source.matches("&& ({ let value = values[").count(), 32 * 64);
        for helper in [
            "byte_scalar_type_v57",
            "byte_pointer_value_type_v57",
            "byte_vector_type_v57",
            "byte_slice_type_v57",
        ] {
            assert_eq!(
                source.matches(&format!("open spec fn {helper}(")).count(),
                1
            );
        }
        assert!(!source.contains("assume("));
        assert!(!source.contains("external_body"));
    }
    assert!(
        shared.len() * 4 < inline.len() * 3,
        "all 2048 original checks must become smaller, not disappear"
    );
}

#[test]
fn byte_value_type_shared_source_keeps_exact_and_one_short_work_and_storage() {
    let measured = value_type_census_v57(true, LIMIT, LIMIT);
    let source = measured.0.unwrap();
    let exact = value_type_census_v57(true, measured.1, measured.2);
    assert_eq!(exact.0.unwrap(), source);
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    assert!(matches!(
        value_type_census_v57(true, measured.1 - 1, measured.2).0,
        Err(Error::Resource(Resource::Work(limit)))
            if limit.limit() == measured.1 - 1 && limit.actual() == measured.1,
    ));
    assert!(matches!(
        value_type_census_v57(true, measured.1, measured.2 - 1).0,
        Err(Error::Resource(Resource::Storage(limit)))
            if limit.limit() == measured.2 - 1 && limit.actual() == measured.2,
    ));
}

fn value_type_equivalence_program_v57() -> String {
    run(37, LIMIT, LIMIT, |out| {
        emit!(out, "use vstd::prelude::*;\nverus! {{\n{}", super::super::byte_memory_v30::BYTE_MEMORY_V30);
        for (width_id, width) in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64].into_iter().enumerate() {
            for (case, (ty, _, old)) in value_type_cases_v57(width).into_iter().enumerate() {
                emit!(out, "proof fn value_type_equivalent_{width_id}_{case}_v57(value: MemoryValueV30)\n ensures (");
                emit_value_type(&ty, width, "value", out)?;
                emit!(out, ") == ({old}),\n{{ reveal(byte_scalar_type_v57); reveal(byte_pointer_value_type_v57); reveal(byte_vector_type_v57); reveal(byte_slice_type_v57); }}\n");
            }
        }
        emit!(out, "}}\n");
        Ok(())
    }).0.unwrap()
}

#[test]
fn byte_value_type_equivalence_program_contains_every_legacy_predicate() {
    let source = value_type_equivalence_program_v57();
    assert_eq!(
        source.matches("proof fn value_type_equivalent_").count(),
        38
    );
    for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
        for (_, shared, old) in value_type_cases_v57(width) {
            assert!(source.contains(&format!("ensures ({shared}) == ({old}),")));
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_byte_value_type_predicates_equal_original_inline_expressions() {
    use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};
    use std::time::{Duration, Instant};

    let source =
        CanonicalGeneratedVerusProofInputV3::new(value_type_equivalence_program_v57().into_bytes())
            .expect("canonical complete predicate equivalence program");
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .expect("requires the actual public pinned-runtime lease");
    runtime
        .revalidate()
        .expect("revalidate before equivalence proof");
    let mut attempt = runtime
        .begin_attempt()
        .expect("acquire equivalence proof attempt");
    let output = runtime
        .execute_generated_rust_verify(
            &mut attempt,
            &source,
            Instant::now() + Duration::from_secs(120),
            16 * 1024,
        )
        .expect("execute complete sealed source through the protected runtime");
    runtime
        .revalidate()
        .expect("revalidate after equivalence proof");
    attempt
        .complete()
        .expect("complete equivalence proof attempt");
    crate::functional_refinement_receipt_v2::validate_proved_output(&output)
        .expect("every scalar/pointer/slice/vector equivalence must genuinely verify");
}
