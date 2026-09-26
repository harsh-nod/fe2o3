use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn ty(kind: SemanticRustTypeKindV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    )
    .with_rust_type_kind(kind)
}

#[test]
fn source_census_encoding_has_exact_sibling_membership_not_numeric_maximum() {
    let cases = [
        (
            0,
            Version::V28,
            vec![Version::V5, Version::V15, Version::V28],
        ),
        (EXECUTION, Version::V29, vec![Version::V29]),
        (INTEGER, Version::V33, vec![Version::V30, Version::V33]),
        (ORDERED_REGION, Version::V31, vec![Version::V31]),
        (ORDERED_PROGRAM, Version::V32, vec![Version::V32]),
        (INLINE, Version::V34, vec![Version::V34]),
        (POINTER_SIZED, Version::V35, vec![Version::V35]),
        (ORDERED_REGION | INLINE, Version::V31, vec![Version::V31]),
        (ORDERED_PROGRAM | INLINE, Version::V32, vec![Version::V32]),
    ];
    let profiles = [
        Version::V2,
        Version::V3,
        Version::V4,
        Version::V5,
        Version::V15,
        Version::V28,
        Version::V29,
        Version::V30,
        Version::V31,
        Version::V32,
        Version::V33,
        Version::V34,
        Version::V35,
    ];
    for (bits, encoded, accepted) in cases {
        let encoding = ProductionSourceCensusEncodingV1::from_families(bits).unwrap();
        assert_eq!(encoding.wire_version(), encoded);
        for profile in profiles {
            assert_eq!(
                encoding.accepts_profile(profile),
                accepted.contains(&profile),
                "features {bits}, profile {profile:?}"
            );
        }
    }
    for bits in 0..64 {
        assert_eq!(
            ProductionSourceCensusEncodingV1::from_families(bits).is_ok(),
            matches!(
                bits,
                0 | EXECUTION | INTEGER | ORDERED_REGION | ORDERED_PROGRAM | INLINE | POINTER_SIZED
            ) || bits == ORDERED_REGION | INLINE
                || bits == ORDERED_PROGRAM | INLINE
        );
    }
}

#[test]
fn source_census_encoding_reads_actual_types_and_all_callable_rows() {
    for kind in [
        SemanticRustTypeKindV1::Ordinary,
        SemanticRustTypeKindV1::Str,
        SemanticRustTypeKindV1::Usize,
        SemanticRustTypeKindV1::Isize,
    ] {
        let types = [ty(kind)];
        let callables = [
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ];
        let mut work = 0;
        let encoding = ProductionSourceCensusEncodingV1::select(&types, &callables, |amount| {
            work += amount;
            Ok(())
        })
        .unwrap();
        assert_eq!(work, types.len() + callables.len() + 1);
        assert_eq!(
            encoding.wire_version(),
            if matches!(
                kind,
                SemanticRustTypeKindV1::Usize | SemanticRustTypeKindV1::Isize
            ) {
                Version::V35
            } else {
                Version::V28
            }
        );
    }
}

#[test]
fn source_census_encoding_selection_exact_and_one_short_work() {
    let types = [ty(SemanticRustTypeKindV1::Ordinary)];
    let callables = [SemanticCallableDeclV1::defined(
        SemanticFunctionIdV1::from_index(0),
    )];
    let required = types.len() + callables.len() + 1;
    for maximum in [required - 1, required] {
        let mut work = 0;
        let result = ProductionSourceCensusEncodingV1::select(&types, &callables, |amount| {
            work += amount;
            if work > maximum {
                Err(table("test work exhausted"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.is_ok(), maximum == required);
        assert_eq!(work, required);
    }
}

fn terminal(operation: SemanticCompilerIntrinsicOperationV1) -> SemanticCallableDeclV1 {
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([4; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([5; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([6; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([7; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([8; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([9; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([10; 32]),
    }
}

#[test]
fn source_census_encoding_actual_intrinsic_rows_select_complete_shared_encoder() {
    // These are inert encoder rows, not admitted intrinsic ABI/source fixtures.
    let id = SemanticTypeIdV1::from_index(0);
    let types = [ty(SemanticRustTypeKindV1::Ordinary)];
    let inline = SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
        SemanticGfx942InlineU32V30::new(
            SemanticGfx942InlineInstructionV30::VXorB32,
            SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS,
        )
        .unwrap(),
    );
    let region = SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(
        SemanticGfx942OrderedRegionProfileV31::XorAddU32E32,
    );
    let program = SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(
        SemanticGfx942U32ProgramV32::from_instructions(&[
            SemanticGfx942ProgramInstructionV32::Move {
                destination: SemanticGfx942ProgramDestinationV32::Output,
                source: SemanticGfx942ProgramRoleV32::Input0,
            },
        ])
        .unwrap(),
    );
    for (operation, version) in [
        (
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::ContextIssue { context: id },
            ),
            Version::V29,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::SaturatingInteger(
                SemanticSaturatingIntegerOpV1::Add,
            ),
            Version::V33,
        ),
        (
            SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex {
                context: id,
                element: id,
            },
            Version::V33,
        ),
        (inline, Version::V34),
        (region, Version::V31),
        (program, Version::V32),
    ] {
        let callables = [terminal(operation)];
        let encoding =
            ProductionSourceCensusEncodingV1::select(&types, &callables, |_| Ok(())).unwrap();
        assert_eq!(encoding.wire_version(), version);
        let commitment = canonical_declaration_tables_commitment_v1(
            &types,
            &callables,
            version,
            SemanticMirLimitsV1::default(),
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(commitment.wire_version(), version);
        assert!(
            canonical_declaration_tables_commitment_v1(
                &types,
                &callables,
                Version::V28,
                SemanticMirLimitsV1::default(),
                &mut |_| Ok(())
            )
            .is_err()
        );
    }
    for operation in [region, program] {
        let callables = [terminal(operation), terminal(inline)];
        let encoding =
            ProductionSourceCensusEncodingV1::select(&types, &callables, |_| Ok(())).unwrap();
        assert!(
            canonical_declaration_tables_commitment_v1(
                &types,
                &callables,
                encoding.wire_version(),
                SemanticMirLimitsV1::default(),
                &mut |_| Ok(())
            )
            .is_ok()
        );
    }
    for operation in [
        SemanticCompilerIntrinsicOperationV1::SaturatingInteger(SemanticSaturatingIntegerOpV1::Add),
        SemanticCompilerIntrinsicOperationV1::Execution(
            SemanticExecutionOperationV29::ContextIssue { context: id },
        ),
        region,
    ] {
        let callables = [terminal(operation), terminal(program)];
        assert!(ProductionSourceCensusEncodingV1::select(&types, &callables, |_| Ok(())).is_err());
    }
}
