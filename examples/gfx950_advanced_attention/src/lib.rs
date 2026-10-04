#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! Fixed-shape Rust gfx950 advanced-attention kernels.
//!
//! The attributed functions are ordinary Rust source with independent safe CPU
//! references. AMDGPU builds select exactly one production kernel root.

#[cfg(all(
    target_arch = "amdgpu",
    not(any(
        feature = "kernel-kda-decode",
        feature = "kernel-kda-prefill",
        feature = "kernel-content-sparse-attention",
        feature = "kernel-deepseek-sparse-attention",
        feature = "kernel-compressed-hybrid-attention",
        feature = "kernel-attnres-aggregate",
        feature = "kernel-four-branch-residual",
        feature = "kernel-mhc-sinkhorn-mix",
    ))
))]
compile_error!("an AMDGPU build must select exactly one advanced-attention kernel feature");

#[cfg(all(
    target_arch = "amdgpu",
    any(
        all(feature = "kernel-kda-decode", feature = "kernel-kda-prefill"),
        all(
            feature = "kernel-kda-decode",
            feature = "kernel-content-sparse-attention"
        ),
        all(
            feature = "kernel-kda-decode",
            feature = "kernel-deepseek-sparse-attention"
        ),
        all(
            feature = "kernel-kda-decode",
            feature = "kernel-compressed-hybrid-attention"
        ),
        all(feature = "kernel-kda-decode", feature = "kernel-attnres-aggregate"),
        all(feature = "kernel-kda-decode", feature = "kernel-four-branch-residual"),
        all(feature = "kernel-kda-decode", feature = "kernel-mhc-sinkhorn-mix"),
        all(
            feature = "kernel-kda-prefill",
            feature = "kernel-content-sparse-attention"
        ),
        all(
            feature = "kernel-kda-prefill",
            feature = "kernel-deepseek-sparse-attention"
        ),
        all(
            feature = "kernel-kda-prefill",
            feature = "kernel-compressed-hybrid-attention"
        ),
        all(feature = "kernel-kda-prefill", feature = "kernel-attnres-aggregate"),
        all(
            feature = "kernel-kda-prefill",
            feature = "kernel-four-branch-residual"
        ),
        all(feature = "kernel-kda-prefill", feature = "kernel-mhc-sinkhorn-mix"),
        all(
            feature = "kernel-content-sparse-attention",
            feature = "kernel-deepseek-sparse-attention"
        ),
        all(
            feature = "kernel-content-sparse-attention",
            feature = "kernel-compressed-hybrid-attention"
        ),
        all(
            feature = "kernel-content-sparse-attention",
            feature = "kernel-attnres-aggregate"
        ),
        all(
            feature = "kernel-content-sparse-attention",
            feature = "kernel-four-branch-residual"
        ),
        all(
            feature = "kernel-content-sparse-attention",
            feature = "kernel-mhc-sinkhorn-mix"
        ),
        all(
            feature = "kernel-deepseek-sparse-attention",
            feature = "kernel-compressed-hybrid-attention"
        ),
        all(
            feature = "kernel-deepseek-sparse-attention",
            feature = "kernel-attnres-aggregate"
        ),
        all(
            feature = "kernel-deepseek-sparse-attention",
            feature = "kernel-four-branch-residual"
        ),
        all(
            feature = "kernel-deepseek-sparse-attention",
            feature = "kernel-mhc-sinkhorn-mix"
        ),
        all(
            feature = "kernel-compressed-hybrid-attention",
            feature = "kernel-attnres-aggregate"
        ),
        all(
            feature = "kernel-compressed-hybrid-attention",
            feature = "kernel-four-branch-residual"
        ),
        all(
            feature = "kernel-compressed-hybrid-attention",
            feature = "kernel-mhc-sinkhorn-mix"
        ),
        all(
            feature = "kernel-attnres-aggregate",
            feature = "kernel-four-branch-residual"
        ),
        all(
            feature = "kernel-attnres-aggregate",
            feature = "kernel-mhc-sinkhorn-mix"
        ),
        all(
            feature = "kernel-four-branch-residual",
            feature = "kernel-mhc-sinkhorn-mix"
        ),
    )
))]
compile_error!("an AMDGPU build must not select more than one advanced-attention kernel feature");

#[cfg(all(
    target_arch = "amdgpu",
    any(
        feature = "kernel-content-sparse-attention-reciprocal-reuse-v1",
        feature = "kernel-compressed-hybrid-attention-division-baseline-v1",
        feature = "kernel-attnres-aggregate-explicit-reuse-v1",
        feature = "kernel-four-branch-residual-explicit-v1",
        feature = "kernel-mhc-sinkhorn-mix-scalar-v1",
    )
))]
mod ablation;

#[cfg(all(
    target_arch = "amdgpu",
    any(
        feature = "kernel-kda-decode-baseline-v1",
        feature = "kernel-kda-prefill-baseline-v1"
    )
))]
mod kda_baseline;

pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod reference;

mod dimensions;
pub use dimensions::*;
