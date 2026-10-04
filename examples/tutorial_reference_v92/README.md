# Current Tutorial Reference Corpus

This standalone executable generates and evaluates the same request format used
by `fe2o3-kir-sim`. `list` returns the complete 45-name positive corpus;
`generate KERNEL` writes one bounded request; `execute REQUEST` reads those exact
bytes and returns independent CPU results bound to their SHA-256. The three
registered negative aggregate inputs intentionally have no positive adapter.

Most adapters call the existing examples' independent CPU reference functions.
The host executable imports those reference source files directly, without
depending on the examples' device modules or changing the typed-kernel macro
policy. Shared dimension constants and reference equations remain single-source.
The imported references' own unit tests run alongside the corpus tests. The only
device API dependency supplies the existing scalar `Bf16` representation; this
executable contains no attributed kernel or device execution route.
Component adapters express the corresponding host equations and never interpret
the original or optimized Kernel IR. Every writable buffer is returned in full,
including untouched padding. Corpus profiles fix launch and buffer extents;
values and supported scalar parameters are read from the supplied request.

`scripts/qualify-tutorial-current-simulation-v92.py` joins these observations to
the complete live V89/V90 default-Cargo census and exact captured V18 bytes. It
runs the simulator twice under the compilation target and requires identical,
complete, conflict-free observations. It also executes the pinned and current
tutorial site's structured source projection. All 64 invocations must match;
negative diagnostics and absent output artifacts are checked separately.

This route neither reinterprets the pending BundleV7/KIR12 manifest contracts nor
grants compiler, publication, launch, or hardware authority. Unsupported current
IR operations or ABI shapes fail through the simulator's existing admission
checks. A generated request is not evidence that its actual compiler graph has
passed admission or execution.
