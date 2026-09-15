# Original Core Option::zip

`authenticate_reviewed_safe_core_option_zip_helper_v1(tcx, instance) -> bool`
observes only the exact inherent core Option method. It neither supplies a
terminal nor replaces MIR. The collector still admits the complete original
call closure and independently rejects nontrivial Drop edges.

The nominal contract binds the current core crate through its lang items,
Option's exact variants, inherent impl ownership, both raw generic parameter
owners/indices, safe Rust ABI, and raw and instantiated input/output types.
A contract is tied to its exact instance and original source definition.

The body checker bounds locals (32), blocks (32), statements (128), scopes (8),
debug records (32), mentioned drops (3), and shared path work (2048). A closed
shape audit rejects calls, assertions, pointer operations, arbitrary effects,
and malformed typed places even in unreachable blocks. Finite interpretation
checks all four input-variant cases, following actual discriminants and drop
flags. Separate left/right ownership tokens must either become Some((T,U)) in
order or be dropped exactly once on the None route. Generic optimized-MIR Copy
operands in aggregates are treated as last-use ownership transfers, never as
permission to duplicate non-Copy payloads. Cleanup paths are checked separately;
unwind-unreachable is accepted only in a panic-abort session.

Pinned-rustc standalone tests live in `standalone.rs`. Link with
`-Cprefer-dynamic -Crpath -L native=<sysroot>/lib`; use the same sysroot's rustc
on PATH and its lib directory in LD_LIBRARY_PATH. No Cargo is required.

Filters:
- `option_zip_actual_host_`: nominal, generic ownership, unwind and mutations.
- `option_zip_actual_amdgpu_`: ignored actual cached-core callbacks, default MIR
  and mir-opt-level=0, plus production collection retention, rejected caller
  unsafe/panic source and left/right nontrivial payload destructors.
- `option_zip_host_collection_`: exact rejection of the installed host core's
  retained cleanup resume, even with a panic-abort caller. The intended
  production collector guarantees run against the original AMDGPU core body;
  no test removes cleanup or substitutes a hand-written helper body.

AMDGPU tests require existing `FE2O3_WRAPPING_AMDGPU_CORE` and
`FE2O3_WRAPPING_AMDGPU_BUILTINS` metadata. The standalone source tests optionally
take `FE2O3_WRAPPING_TARGET_CPU=gfx942|gfx950`; they do not build a sysroot.
Production collector tests are mounted under `collector::production_importer_v1`
and are not included in the standalone binary.
