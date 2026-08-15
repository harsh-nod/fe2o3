#![cfg(target_os = "linux")]

pub use fe2o3_hsaco_finalize::*;

#[allow(dead_code)]
#[path = "../src/lds_gemm_finalizer.rs"]
mod lds_gemm_finalizer;

mod registry_fixture {
    include!("lds_gemm_profile_registry.rs");

    pub(super) fn exact_import_and_handoff() -> (
        fe2o3_hsaco_finalize::InspectedExactLdsGemmCompilerImportV1,
        fe2o3_compiler_ffi::CompilerModuleHandoffV2,
    ) {
        let fixture = Slice1Fixture::canonical();
        let handoff = fixture.handoff();
        let import = fe2o3_hsaco_finalize::inspect_exact_lds_gemm_compiler_import_v1(
            fixture.pins(),
            fixture.handoff(),
        )
        .expect("canonical exact import");
        (import, handoff)
    }

    pub(super) fn mismatched_handoff() -> fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        let fixture = Slice1Fixture::canonical();
        let mut module = fixture.canonical_module();
        module.push(b'\n');
        fixture.handoff_for_module(&module)
    }
}

#[allow(dead_code)]
mod hsaco_fixture {
    include!("fixtures/worker_v2_hsaco_test_support.rs");

    pub(super) fn exact_symbol_fixture(include_extra: bool, hidden_extra: bool) -> Vec<u8> {
        let mut options = FixtureOptions::valid();
        options.target = "gfx942";
        options.code_object_version = 6;
        options.entry = "tiled_gemm_lds_v1";
        options.descriptor = "tiled_gemm_lds_v1.kd";
        options.required_workgroup_size = [64, 1, 1];
        options.max_flat_workgroup_size = 64;
        options.include_export = include_extra;
        let mut artifact = fixture(options).bytes;
        if include_extra && hidden_extra {
            let section_table = u64::from_le_bytes(artifact[40..48].try_into().unwrap()) as usize;
            let symtab_header = section_table + SYMTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
            let symtab = u64::from_le_bytes(
                artifact[symtab_header + 24..symtab_header + 32]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let extra = symtab + 3 * 24;
            artifact[extra + 4] = 0x02;
            artifact[extra + 5] = 0x02;
        }
        artifact
    }

    pub(super) fn undefined_extra_symbol_fixture() -> Vec<u8> {
        let mut artifact = exact_symbol_fixture(true, false);
        let section_table = u64::from_le_bytes(artifact[40..48].try_into().unwrap()) as usize;
        let symtab_header = section_table + SYMTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
        let symtab = u64::from_le_bytes(
            artifact[symtab_header + 24..symtab_header + 32]
                .try_into()
                .unwrap(),
        ) as usize;
        let extra = symtab + 3 * 24;
        artifact[extra + 6..extra + 8].fill(0);
        artifact
    }
}

use std::{
    env,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_hsaco::{
    ArgumentAccess, CodeObjectVersion as InspectedCodeObjectVersion, ExplicitValueType,
};
use lds_gemm_finalizer::{
    ExactLdsGemmFinalizationErrorV1 as LocalFinalizationError, ObservedArtifactShapeV1,
    exact_observed_artifact_shape_for_test,
    finalize_exact_lds_gemm_compiler_import_v1 as finalize_local_import,
    validate_elf_safety_for_test, validate_exact_symbol_closure_for_test,
    validate_observed_artifact_shape, validate_transactional_handoff_for_test,
};

fn minimal_elf(section_type: u32, dynamic_tag: Option<u64>) -> Vec<u8> {
    const HEADER: usize = 64;
    const SECTION: usize = 64;
    let payload = dynamic_tag.map_or(0, |_| 16);
    let section_table = HEADER + payload;
    let mut bytes = vec![0; section_table + 2 * SECTION];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 40, section_table as u64);
    write_u16(&mut bytes, 52, HEADER as u16);
    write_u16(&mut bytes, 58, SECTION as u16);
    write_u16(&mut bytes, 60, 2);
    let second = section_table + SECTION;
    write_u32(&mut bytes, second + 4, section_type);
    if let Some(tag) = dynamic_tag {
        write_u64(&mut bytes, HEADER, tag);
        write_u64(&mut bytes, second + 24, HEADER as u64);
        write_u64(&mut bytes, second + 32, 16);
        write_u64(&mut bytes, second + 56, 16);
    }
    bytes
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn hex_digest(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing a digest to String cannot fail");
    }
    output
}

#[test]
fn exact_slice1_artifact_shape_is_the_only_admitted_shape() {
    let exact = exact_observed_artifact_shape_for_test();
    validate_observed_artifact_shape(&exact).expect("exact artifact shape");

    let mut mutations = Vec::new();
    let mut target = exact.clone();
    target.target = "gfx942:xnack+".to_owned();
    mutations.push(target);
    let mut cov = exact.clone();
    cov.code_object_version = InspectedCodeObjectVersion::V5;
    mutations.push(cov);
    let mut entry = exact.clone();
    entry.entry.push_str("_substitute");
    mutations.push(entry);
    let mut workgroup = exact.clone();
    workgroup.required_workgroup_size = Some([256, 1, 1]);
    mutations.push(workgroup);
    let mut descriptor = exact.clone();
    descriptor.descriptor.push_str("_substitute");
    mutations.push(descriptor);
    let mut wave = exact.clone();
    wave.wavefront_size = 32;
    mutations.push(wave);
    let mut lds = exact.clone();
    lds.group_segment_fixed_size = 0;
    mutations.push(lds);
    let mut scratch = exact.clone();
    scratch.private_segment_fixed_size = 4;
    mutations.push(scratch);
    let mut kernarg = exact.clone();
    kernarg.kernarg_segment_size = 320;
    mutations.push(kernarg);
    let mut spill = exact.clone();
    spill.vgpr_spill_count = Some(1);
    mutations.push(spill);
    let mut stack = exact.clone();
    stack.uses_dynamic_stack = true;
    mutations.push(stack);
    let mut argument = exact.clone();
    argument.explicit_arguments[4].offset = 16;
    mutations.push(argument);
    let mut access = exact;
    access.explicit_arguments[0].actual_access = Some(fe2o3_hsaco::ArgumentAccess::ReadWrite);
    mutations.push(access);

    for mutation in mutations {
        assert!(matches!(
            validate_observed_artifact_shape(&mutation),
            Err(LocalFinalizationError::ArtifactShape(_))
        ));
    }
}

fn assert_shape_rejected(label: &str, mutate: impl FnOnce(&mut ObservedArtifactShapeV1)) {
    let mut observed = exact_observed_artifact_shape_for_test();
    mutate(&mut observed);
    assert!(
        matches!(
            validate_observed_artifact_shape(&observed),
            Err(LocalFinalizationError::ArtifactShape(_))
        ),
        "hostile exact-profile shape was admitted: {label}"
    );
}

fn assert_shape_admitted(label: &str, mutate: impl FnOnce(&mut ObservedArtifactShapeV1)) {
    let mut observed = exact_observed_artifact_shape_for_test();
    mutate(&mut observed);
    if let Err(error) = validate_observed_artifact_shape(&observed) {
        panic!("compatible exact-profile shape was rejected ({label}): {error}");
    }
}

#[test]
fn exact_slice1_required_and_guaranteed_emitted_metadata_rejects_absence() {
    assert_shape_rejected("required workgroup size", |observed| {
        observed.required_workgroup_size = None;
    });
    assert_shape_rejected("implicit argument offset", |observed| {
        observed.implicit_argument_offset = None;
    });
    assert_shape_rejected("SGPR spill count", |observed| {
        observed.sgpr_spill_count = None;
    });
    assert_shape_rejected("VGPR spill count", |observed| {
        observed.vgpr_spill_count = None;
    });

    for role in 0..3 {
        let pointer = role * 2;
        assert_shape_rejected(&format!("pointer {role} name"), |observed| {
            observed.explicit_arguments[pointer].name = None;
        });
        assert_shape_rejected(&format!("pointer {role} type name"), |observed| {
            observed.explicit_arguments[pointer].type_name = None;
        });
        assert_shape_rejected(&format!("pointer {role} address space"), |observed| {
            observed.explicit_arguments[pointer].address_space = None;
        });
        assert_shape_rejected(&format!("pointer {role} declared access"), |observed| {
            observed.explicit_arguments[pointer].access = None;
        });
        if role < 2 {
            assert_shape_rejected(&format!("pointer {role} actual access"), |observed| {
                observed.explicit_arguments[pointer].actual_access = None;
            });
            assert_shape_rejected(&format!("pointer {role} const qualifier"), |observed| {
                observed.explicit_arguments[pointer].is_const = None;
            });
        } else {
            assert_shape_rejected("pointer 2 restrict qualifier", |observed| {
                observed.explicit_arguments[pointer].is_restrict = None;
            });
        }

        let length = pointer + 1;
        assert_shape_rejected(&format!("length {role} name"), |observed| {
            observed.explicit_arguments[length].name = None;
        });
        assert_shape_rejected(&format!("length {role} type name"), |observed| {
            observed.explicit_arguments[length].type_name = None;
        });
    }
}

#[test]
fn exact_slice1_upstream_optional_argument_metadata_may_be_absent() {
    for role in 0..3 {
        let pointer = role * 2;
        assert_shape_admitted(&format!("pointer {role} alignment absent"), |observed| {
            observed.explicit_arguments[pointer].alignment = None;
        });
        assert_shape_admitted(&format!("pointer {role} value type absent"), |observed| {
            observed.explicit_arguments[pointer].value_type = None;
        });
        if role == 2 {
            assert_shape_admitted("pointer 2 actual access absent", |observed| {
                observed.explicit_arguments[pointer].actual_access = None;
            });
        }
        assert_shape_admitted(
            &format!("pointer {role} pointee alignment absent"),
            |observed| {
                observed.explicit_arguments[pointer].pointee_alignment = None;
            },
        );
        if role == 2 {
            assert_shape_admitted("pointer 2 const qualifier absent", |observed| {
                observed.explicit_arguments[pointer].is_const = None;
            });
        } else {
            assert_shape_admitted(
                &format!("pointer {role} restrict qualifier absent"),
                |observed| {
                    observed.explicit_arguments[pointer].is_restrict = None;
                },
            );
        }

        let length = pointer + 1;
        assert_shape_admitted(&format!("length {role} alignment absent"), |observed| {
            observed.explicit_arguments[length].alignment = None;
        });
        assert_shape_admitted(&format!("length {role} value type absent"), |observed| {
            observed.explicit_arguments[length].value_type = None;
        });
    }
}

#[test]
fn exact_slice1_upstream_optional_hostile_some_values_are_rejected() {
    const ACCESS_VARIANTS: [ArgumentAccess; 3] = [
        ArgumentAccess::ReadOnly,
        ArgumentAccess::WriteOnly,
        ArgumentAccess::ReadWrite,
    ];

    for role in 0..3 {
        let pointer = role * 2;
        let expected_access = if role < 2 {
            ArgumentAccess::ReadOnly
        } else {
            ArgumentAccess::ReadWrite
        };
        let hostile_type = if role < 2 {
            ExplicitValueType::F32
        } else {
            ExplicitValueType::U16
        };
        let hostile_pointee_alignment = if role < 2 { 4 } else { 2 };

        assert_shape_rejected(&format!("pointer {role} alignment"), |observed| {
            observed.explicit_arguments[pointer].alignment = Some(16);
        });
        assert_shape_rejected(&format!("pointer {role} type name"), |observed| {
            observed.explicit_arguments[pointer].type_name = Some("int*".to_owned());
        });
        assert_shape_rejected(&format!("pointer {role} value type"), |observed| {
            observed.explicit_arguments[pointer].value_type = Some(hostile_type);
        });
        for access in ACCESS_VARIANTS {
            if access != expected_access {
                assert_shape_rejected(
                    &format!("pointer {role} contract access {access:?}"),
                    |observed| {
                        observed.explicit_arguments[pointer].access = Some(access);
                    },
                );
            }
        }
        for actual_access in ACCESS_VARIANTS {
            if role < 2 && actual_access != ArgumentAccess::ReadOnly {
                assert_shape_rejected(
                    &format!("pointer {role} actual access {actual_access:?}"),
                    |observed| {
                        observed.explicit_arguments[pointer].actual_access = Some(actual_access);
                    },
                );
            } else {
                assert_shape_admitted(
                    &format!("pointer {role} narrowed actual access {actual_access:?}"),
                    |observed| {
                        observed.explicit_arguments[pointer].actual_access = Some(actual_access);
                    },
                );
            }
        }
        assert_shape_rejected(&format!("pointer {role} pointee alignment"), |observed| {
            observed.explicit_arguments[pointer].pointee_alignment =
                Some(hostile_pointee_alignment);
        });
        assert_shape_rejected(&format!("pointer {role} const qualifier"), |observed| {
            observed.explicit_arguments[pointer].is_const = Some(role >= 2);
        });
        assert_shape_rejected(&format!("pointer {role} restrict qualifier"), |observed| {
            observed.explicit_arguments[pointer].is_restrict = Some(role < 2);
        });

        let length = pointer + 1;
        assert_shape_rejected(&format!("length {role} alignment"), |observed| {
            observed.explicit_arguments[length].alignment = Some(4);
        });
        assert_shape_rejected(&format!("length {role} type name"), |observed| {
            observed.explicit_arguments[length].type_name = Some("uint".to_owned());
        });
        assert_shape_rejected(&format!("length {role} value type"), |observed| {
            observed.explicit_arguments[length].value_type = Some(ExplicitValueType::I64);
        });
        for access in ACCESS_VARIANTS {
            assert_shape_rejected(&format!("length {role} access {access:?}"), |observed| {
                observed.explicit_arguments[length].access = Some(access);
            });
            assert_shape_rejected(
                &format!("length {role} actual access {access:?}"),
                |observed| {
                    observed.explicit_arguments[length].actual_access = Some(access);
                },
            );
        }
        assert_shape_rejected(&format!("length {role} pointee alignment"), |observed| {
            observed.explicit_arguments[length].pointee_alignment = Some(8);
        });
        assert_shape_rejected(&format!("length {role} const qualifier"), |observed| {
            observed.explicit_arguments[length].is_const = Some(true);
        });
        assert_shape_rejected(&format!("length {role} restrict qualifier"), |observed| {
            observed.explicit_arguments[length].is_restrict = Some(true);
        });
        assert_shape_admitted(
            &format!("length {role} explicit false const qualifier"),
            |observed| {
                observed.explicit_arguments[length].is_const = Some(false);
            },
        );
        assert_shape_admitted(
            &format!("length {role} explicit false restrict qualifier"),
            |observed| {
                observed.explicit_arguments[length].is_restrict = Some(false);
            },
        );
    }
}

#[test]
fn linked_elf_rejects_rel_rela_and_needed_dependencies() {
    validate_elf_safety_for_test(&minimal_elf(1, None)).expect("relocation-free ELF");
    for section_type in [4, 9] {
        assert!(matches!(
            validate_elf_safety_for_test(&minimal_elf(section_type, None)),
            Err(LocalFinalizationError::ElfPolicy(
                "a residual relocation section"
            ))
        ));
    }
    assert!(matches!(
        validate_elf_safety_for_test(&minimal_elf(6, Some(1))),
        Err(LocalFinalizationError::ElfPolicy("a DT_NEEDED dependency"))
    ));
    validate_elf_safety_for_test(&minimal_elf(6, Some(0)))
        .expect("terminated dependency-free dynamic table");
}

#[test]
fn exact_symbol_closure_includes_static_and_dynamic_tables() {
    validate_exact_symbol_closure_for_test(&hsaco_fixture::exact_symbol_fixture(false, false))
        .expect("exact entry and descriptor definitions");
    for hostile in [
        hsaco_fixture::exact_symbol_fixture(true, false),
        hsaco_fixture::exact_symbol_fixture(true, true),
        hsaco_fixture::undefined_extra_symbol_fixture(),
    ] {
        assert!(matches!(
            validate_exact_symbol_closure_for_test(&hostile),
            Err(LocalFinalizationError::ElfPolicy(_))
        ));
    }
}

#[test]
fn transactional_handoff_substitution_is_rejected_before_worker_execution() {
    let directory = TestDirectory::new();
    let (import, _) = registry_fixture::exact_import_and_handoff();
    let hostile = registry_fixture::mismatched_handoff();
    let consumed = consumed_handoff(&directory, &hostile);
    assert!(matches!(
        validate_transactional_handoff_for_test(&import, &consumed),
        Err(LocalFinalizationError::TransactionalHandoffMismatch)
    ));
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-lds-gemm-finalizer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create exact finalizer transaction directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn consumed_handoff(
    directory: &TestDirectory,
    handoff: &fe2o3_compiler_ffi::CompilerModuleHandoffV2,
) -> fe2o3_artifact_transaction::ConsumedCompilerModuleHandoffV1 {
    let producer = ProducerIdentity::from_codegen(
        "lds_gemm_finalizer",
        Some(Path::new("tests/lds_gemm_finalizer.rs")),
    )
    .expect("exact finalizer test producer");
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x97; 32]),
        BuildSession::from_bytes([0x42; 16]),
    )
    .expect("begin exact finalizer attempt");
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, handoff.canonical_bytes())
        .expect("publish exact finalizer handoff");
    consume_compiler_module_handoff_v1(&directory.0, &producer, attempt)
        .expect("consume exact finalizer handoff")
}

#[test]
#[ignore = "requires the measured upstream LLVM/LLD C++ API worker with Slice1 WG64 support"]
fn measured_worker_produces_a_deterministic_inert_slice1_cov6_receipt() {
    let worker_path = PathBuf::from(
        env::var("FE2O3_LDS_GEMM_V1_WORKER").expect("set measured Slice1 Worker V2 path"),
    );
    let worker_bytes = fs::read(&worker_path).expect("read measured Slice1 worker");
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&worker_bytes),
        env::var("FE2O3_LDS_GEMM_V1_WORKER_BUILD_ID").expect("set worker build id"),
        env::var("FE2O3_LDS_GEMM_V1_LLVM_BUILD_ID").expect("set LLVM build id"),
    )
    .expect("exact worker measurement");
    let worker = PinnedWorkerV1::open(&worker_path, measurement).expect("open measured worker");

    let mut receipts = Vec::new();
    for _ in 0..2 {
        let directory = TestDirectory::new();
        let (import, handoff) = registry_fixture::exact_import_and_handoff();
        let receipt = finalize_local_import(
            import,
            consumed_handoff(&directory, &handoff),
            &worker,
            WorkerExecutionLimitsV1::default(),
        )
        .expect("measured direct LLVM/LLD Slice1 finalization");
        assert_eq!(
            receipt.contract().profile(),
            ExactLdsGemmProfileIdV1::Slice1M16N16K16
        );
        assert!(
            receipt
                .finalized_output_identity()
                .matches(receipt.exact_finalized_bytes())
        );
        assert_ne!(receipt.canonical_descriptor_digest().as_bytes(), &[0; 32]);
        assert!(!receipt.authenticates_compiler_origin());
        assert!(!receipt.proves_llvm_to_isa_refinement());
        assert!(!receipt.proves_verus_verification());
        assert!(!receipt.grants_link_authority());
        assert!(!receipt.grants_publication_authority());
        assert!(!receipt.grants_load_authority());
        assert!(!receipt.grants_launch_authority());
        receipts.push((receipt.identity(), receipt.finalized_output_identity()));
    }
    assert_eq!(receipts[0], receipts[1]);
    println!(
        "measured Slice1 receipt identity: {}",
        hex_digest(receipts[0].0.as_bytes())
    );
    println!("measured Slice1 finalized output: {}", receipts[0].1);
}
