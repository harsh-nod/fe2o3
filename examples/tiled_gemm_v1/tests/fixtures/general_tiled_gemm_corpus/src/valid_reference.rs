use fe2o3_device::kernel;

use crate::support::KernelContext;

#[kernel(control_flow(loop_bounds(268435456, 4)))]
pub fn valid_general_tiled_gemm(context: &mut KernelContext<'_>) {
    if context.lane >= 64 {
        return;
    }

    let lane_row = context.lane % 16;
    let lane_column = context.lane % 16;
    let depth_base = 4 * (context.lane / 16);
    let mut accumulator = [0.0_f32; 4];
    let phase_count = context.k / 16 + usize::from(!context.k.is_multiple_of(16));
    let mut phase = 0;
    while phase < phase_count {
        let mut component = 0;
        while component < 4 {
            let row = context.group_y * 16 + lane_row;
            let column = context.group_x * 16 + lane_column;
            let depth = phase * 16 + depth_base + component;
            let a = if row < context.m && depth < context.k {
                context.load_a(row, depth)
            } else {
                0
            };
            let b = if depth < context.k && column < context.n {
                context.load_b(depth, column)
            } else {
                0
            };
            let tile_depth = depth_base + component;
            let a_slot = 16 * lane_row + (tile_depth ^ (4 * (lane_row % 4)));
            let b_slot = 256 + 16 * lane_column + (tile_depth ^ (4 * (lane_column % 4)));
            context.stage(a_slot, phase as u32, a);
            context.stage(b_slot, phase as u32, b);
            component += 1;
        }

        context.publish_barrier();
        let slot0 = depth_base ^ (4 * (lane_row % 4));
        let slot1 = (depth_base + 1) ^ (4 * (lane_row % 4));
        let slot2 = (depth_base + 2) ^ (4 * (lane_row % 4));
        let slot3 = (depth_base + 3) ^ (4 * (lane_row % 4));
        let lhs = [
            context.read_stage(16 * lane_row + slot0, phase as u32),
            context.read_stage(16 * lane_row + slot1, phase as u32),
            context.read_stage(16 * lane_row + slot2, phase as u32),
            context.read_stage(16 * lane_row + slot3, phase as u32),
        ];
        let rhs = [
            context.read_stage(256 + 16 * lane_column + slot0, phase as u32),
            context.read_stage(256 + 16 * lane_column + slot1, phase as u32),
            context.read_stage(256 + 16 * lane_column + slot2, phase as u32),
            context.read_stage(256 + 16 * lane_column + slot3, phase as u32),
        ];
        accumulator = context.mfma_fragment(lhs, rhs, accumulator);
        context.reuse_barrier();
        phase += 1;
    }

    let column = context.group_x * 16 + lane_column;
    let row_base = context.group_y * 16 + 4 * (context.lane / 16);
    if row_base < context.m && column < context.n {
        let initial = context.load_c(row_base, column);
        context.store_c(
            row_base,
            column,
            context.alpha * accumulator[0] + context.beta * initial,
        );
    }
    if row_base + 1 < context.m && column < context.n {
        let initial = context.load_c(row_base + 1, column);
        context.store_c(
            row_base + 1,
            column,
            context.alpha * accumulator[1] + context.beta * initial,
        );
    }
    if row_base + 2 < context.m && column < context.n {
        let initial = context.load_c(row_base + 2, column);
        context.store_c(
            row_base + 2,
            column,
            context.alpha * accumulator[2] + context.beta * initial,
        );
    }
    if row_base + 3 < context.m && column < context.n {
        let initial = context.load_c(row_base + 3, column);
        context.store_c(
            row_base + 3,
            column,
            context.alpha * accumulator[3] + context.beta * initial,
        );
    }
}
