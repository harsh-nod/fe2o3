//! Ordinary scalar SIMT evaluation of the same raw OCP FP4/FP8 expert inputs.
#![allow(missing_docs)]
#[cfg(feature="ablation-expert-serial")]
compile_error!("MoE expert SIMT mode has no MFMA serial-ablation pairing");
use crate::{ALL_EXPERTS,EXPERTS,HIDDEN,OUTPUT,SYSTEM_BATCHES,TOKENS,TOP_K};
use fe2o3_device::{Blocked,DeviceMath,DisjointSlice,Index1D,kernel,thread};
#[kernel(typed,launch(required=[256,1,1],max=[256,1,1],max_grid=[4,1,1]),control_flow(loop_bounds(128,128,128,128)))]
#[allow(clippy::too_many_arguments)]
pub fn gfx950_moe_expert_rank_fp4_fp8_v1(
    activations: &[u8], expert_weights: &[u8], top_experts: &[u32], top_weights: &[f32],
    first_expert: u32, include_shared_expert: u32,
    mut output: DisjointSlice<f32,Blocked<Index1D,64,4>>,
) {
    let index = thread::index_1d();
    let raw = index.get();
    let batch = raw / 64;
    let lane = raw & 63;
    if batch >= SYSTEM_BATCHES || first_expert as usize >= EXPERTS - 1
        || activations.len() != SYSTEM_BATCHES*TOKENS*HIDDEN
        || expert_weights.len() != SYSTEM_BATCHES*ALL_EXPERTS*HIDDEN*OUTPUT
        || top_experts.len() != SYSTEM_BATCHES*TOKENS*TOP_K
        || top_weights.len() != SYSTEM_BATCHES*TOKENS*TOP_K
        || output.len() != SYSTEM_BATCHES*TOKENS*OUTPUT { return; }
    let second_expert = first_expert + 1;
    let activation_base = batch * TOKENS * HIDDEN;
    let weights_base = batch * ALL_EXPERTS * HIDDEN * OUTPUT;
    let routes_base = batch * TOKENS * TOP_K;
    let math = DeviceMath::current();
    let Some(block) = index.checked_block::<64,4>() else { return; };
    macro_rules! component {
        ($component:literal) => {{
            let element = lane + $component * 64;
            let token = element / OUTPUT;
            let column = element & (OUTPUT - 1);
            let mut first = 0.0_f32;
            let mut second = 0.0_f32;
            let mut shared = 0.0_f32;
            let mut depth = 0_usize;
            while depth < HIDDEN {
                let a = moe_decode_fp4!(activations[activation_base + token*HIDDEN + depth]);
                let w0 = moe_decode_fp8!(expert_weights[weights_base + ((first_expert as usize)*HIDDEN + depth)*OUTPUT + column]);
                let w1 = moe_decode_fp8!(expert_weights[weights_base + ((second_expert as usize)*HIDDEN + depth)*OUTPUT + column]);
                let ws = moe_decode_fp8!(expert_weights[weights_base + ((ALL_EXPERTS-1)*HIDDEN + depth)*OUTPUT + column]);
                first += a * w0;
                second += a * w1;
                shared += a * ws;
                depth += 1;
            }
            let route = routes_base + token * TOP_K;
            let selected0 = top_experts[route];
            let selected1 = top_experts[route+1];
            let gate0 = top_weights[route];
            let gate1 = top_weights[route+1];
            let mut result = 0.0_f32;
            if selected0 == first_expert { result += gate0 * (first / (1.0+math.exp_f32(-first))); }
            else if selected0 == second_expert { result += gate0 * (second / (1.0+math.exp_f32(-second))); }
            if selected1 == first_expert { result += gate1 * (first / (1.0+math.exp_f32(-first))); }
            else if selected1 == second_expert { result += gate1 * (second / (1.0+math.exp_f32(-second))); }
            if include_shared_expert != 0 { result += 0.25 * (shared / (1.0+math.exp_f32(-shared))); }
            if let Some(slot) = output.get_block_mut(&block,$component) { *slot = result; }
        }};
    }
    component!(0);
    component!(1);
    component!(2);
    component!(3);
}
