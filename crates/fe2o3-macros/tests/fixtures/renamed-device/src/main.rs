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
