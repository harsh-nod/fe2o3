# Qwen3 RoPE and Paged KV Reference

This standalone CPU library retains the bounded RoPE and paged-KV foundation
from [PR #187](https://github.com/harsh-nod/fe2o3/pull/187), without its
historical GEMM package dependency. It uses only the Rust standard library.

`rope_kv` exposes split-half Qwen3 rotary-position references and a pure,
generation-bound exclusive append model. Target Qwen3-8B and draft Qwen3-0.6B
geometries, identity preimages, and finite buckets are unchanged. Absolute
positions are below 8192; context sizes are 128/1024/4096/8192 and page sizes
are 16/64/256. Contexts must be divisible by pages, including when validating
a page table directly without a candidate.

The dimension-oriented and pair-oriented RoPE functions evaluate host
`f64` trigonometry. Their differential agreement is not a BF16/FP32,
OCML, compiler, or GPU numerical-refinement claim. KV coordinates and
initialized-prefix projection do not commit memory or implement an
inference service's ownership, rollback, or retirement protocol.
The caller must supply independently expected candidate, generation, and owner
identities; these structural identities are not authenticated capabilities.
`gfx942` and Wave64 fields describe the retained model, not a running kernel.

## Production Checks

From the repository root:

```sh
bash scripts/ci-local.sh host-reference
```

This lane also runs in `test`, `generic-core`, and `generic`. It checks
formatting, strict Clippy, all test targets in debug and release, and strict
documentation for the standalone models. `standalone-locks` checks this
package's tracked lockfile. A focused test command is:

```sh
cargo test --locked --manifest-path examples/qwen3_rope_kv_v1/Cargo.toml --all-targets
```

## Structural Proof

The unchanged `verus/rope_kv_v1.rs` proves conditional integer pairing,
bounds, reconstruction, and injectivity properties. Its model and verifier
pins are retained separately from the Rust implementation; they do not
establish refinement of this implementation or a generated GPU artifact.

```sh
VERUS=/absolute/path/to/verus sh examples/qwen3_rope_kv_v1/run-verus.sh
```

The existing `row-softmax-v1.yml` production proof job runs this proof using
the same pinned Verus release. The runner reuses that example's complete
release-closure and proof-source checks, isolates verifier environment
variables, requires the proof digest and all 14 obligations, and fails if
verification fails. CPU tests
alone are not evidence that this proof was run. No artifact, load, dispatch,
or launch authority is exposed by this package.

Each numerical call handles one sequence; sequence buckets are scheduling
bounds, not batched execution. Each page-table projection handles one
descriptor. It neither establishes live ownership nor excludes aliases
between separate requests. Bounded vectors use ordinary infallible Rust
allocation, not a typed allocation-failure protocol.
