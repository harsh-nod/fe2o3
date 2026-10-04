//! Structural capability text is never checked-source admission.
use super::super::{descriptor_capabilities, descriptor_capabilities_with_complete_body_v19};
use super::*;
fn exact() -> TargetCapability {
    TargetCapability::Extension {
        namespace: fe2o3_kernel_ir::AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20.into(),
        name: fe2o3_kernel_ir::AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20.into(),
    }
}
fn refuses(capability: TargetCapability) {
    let mut module = Module::new("physical_descriptor_inert_control");
    module.required_capabilities.insert(capability);
    let original = module.clone();
    for matrix in [false, true] {
        for workgroup in [false, true] {
            assert!(matches!(
                descriptor_capabilities(&module, matrix, workgroup),
                Err(CompilerDescriptorError::UnsupportedCapability(_))
            ));
            assert!(matches!(
                descriptor_capabilities_with_complete_body_v19(&module, matrix, workgroup, true),
                Err(CompilerDescriptorError::UnsupportedCapability(_))
            ));
        }
    }
    assert_eq!(module, original);
}
#[test]
fn physical_capability_requires_retained_checked_source_and_abi_preparation() {
    refuses(exact());
}
#[test]
fn unknown_and_near_physical_capabilities_are_not_admission() {
    let TargetCapability::Extension { namespace, name } = exact() else {
        unreachable!()
    };
    refuses(TargetCapability::Extension {
        namespace: format!("{namespace}.lookalike"),
        name: name.clone(),
    });
    refuses(TargetCapability::Extension {
        namespace: namespace.clone(),
        name: format!("{name}.v2"),
    });
    refuses(TargetCapability::Extension {
        namespace,
        name: "unregistered-physical-profile".into(),
    });
}
#[test]
fn exact_target_and_wave_do_not_make_a_physical_capability_self_authenticating() {
    let mut module = Module::new("physical_descriptor_target_is_not_source");
    module.required_capabilities.extend([
        exact(),
        TargetCapability::Extension {
            namespace: fe2o3_kernel_ir::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.into(),
            name: fe2o3_kernel_ir::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.into(),
        },
        TargetCapability::WaveWidth(fe2o3_kernel_ir::WaveWidth::Wave64),
    ]);
    assert!(matches!(
        descriptor_capabilities(&module, false, false),
        Err(CompilerDescriptorError::UnsupportedCapability(_))
    ));
    assert!(matches!(
        descriptor_capabilities_with_complete_body_v19(&module, false, false, true),
        Err(CompilerDescriptorError::UnsupportedCapability(_))
    ));
}
