use gpu_device::{KernelMarkerV1, kernel};

#[kernel]
pub fn renamed_device(value: u32) -> u32 {
    value
}

#[kernel(launch(
    required = [256, 1, 1],
    max = [256, 1, 1],
    min_workgroups_per_compute_unit = 2
))]
pub fn launch_bounded(value: u32) -> u32 {
    value
}

#[kernel(unsafe_asm(
    target = "gfx942",
    operands(sgpr, immediate),
    options(nomem, pure, nostack),
    effects(none)
))]
pub unsafe fn assembly_declared(value: u32) -> u32 {
    value
}

#[kernel(control_flow(loop_bounds(8), integer_switches(u32)))]
pub fn structured_control_flow(mut value: u32) -> u32 {
    'outer: while value < 8 {
        match value {
            0 => {
                value += 1;
                continue 'outer;
            }
            1 => break 'outer,
            _ => value += 2,
        }
    }
    value
}

#[kernel(control_flow(loop_bounds(8), integer_switches(u32)))]
pub fn optional_raw_u32_control(selector: u32) -> u32 {
    let mut iteration = 0;
    match selector {
        0 => {}
        _ => {
            while iteration < 8 {
                iteration += 1;
            }
        }
    }
    iteration
}

#[kernel(control_flow(loop_bounds(4)))]
pub fn literal_for_unroll(mut value: u32) -> u32 {
    for i in 0u32..4u32 {
        if i == 1 {
            continue;
        }
        value += i;
        if i == 2 {
            break;
        }
    }
    value
}

fn assert_marker<T: KernelMarkerV1>() {}

fn main() {
    assert_marker::<__fe2o3_kernel_marker_renamed_device>();
    assert_eq!(
        <__fe2o3_kernel_marker_renamed_device as KernelMarkerV1>::LOGICAL_NAME,
        "renamed_device"
    );
    assert_eq!(
        <__fe2o3_kernel_marker_renamed_device as KernelMarkerV1>::REGISTRATION.2,
        1
    );
    assert_marker::<__fe2o3_kernel_marker_launch_bounded>();
    assert_marker::<__fe2o3_kernel_marker_assembly_declared>();
    assert_marker::<__fe2o3_kernel_marker_structured_control_flow>();
    assert_marker::<__fe2o3_kernel_marker_optional_raw_u32_control>();
    assert_eq!(optional_raw_u32_control(0), 0);
    assert_eq!(optional_raw_u32_control(4), 8);
    assert_marker::<__fe2o3_kernel_marker_literal_for_unroll>();
    assert_eq!(
        <__fe2o3_kernel_marker_literal_for_unroll as KernelMarkerV1>::FUNCTION(10),
        12
    );
    assert!(!__fe2o3_control_flow_contract_v1_literal_for_unroll.4.is_empty());

    let sidecar = __fe2o3_control_flow_contract_v1_structured_control_flow.4;
    let contract = frontend::decode_control_flow_contract_v1(sidecar).unwrap();
    assert_eq!(contract.entry().get(), 0);
    assert!(contract.nodes().len() >= 8);
    assert!(
        contract
            .nodes()
            .iter()
            .all(|node| node.span().file().ends_with("src/main.rs"))
    );
    for expected in ["loop", "break", "continue", "switch"] {
        assert!(contract.nodes().iter().any(|node| {
            matches!(
                (expected, node.kind()),
                ("loop", frontend::ControlFlowNodeKindV1::Loop { .. })
                    | ("break", frontend::ControlFlowNodeKindV1::Break { .. })
                    | ("continue", frontend::ControlFlowNodeKindV1::Continue { .. })
                    | (
                        "switch",
                        frontend::ControlFlowNodeKindV1::IntegerSwitch { .. }
                    )
            )
        }));
    }
    assert!(!contract.cfg_identity().as_bytes().is_empty());
}
