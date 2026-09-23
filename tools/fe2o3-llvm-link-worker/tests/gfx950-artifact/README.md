# Diagnostic-only gfx950 artifact fixture

This fixed fixture emits one actual gfx950:xnack-/Wave64 COV6 HSACO through the
existing LLVM/LLD worker. It supplies input for the read-only checked-artifact
observer. It does not authenticate Rust source, publish a protected artifact,
load code, open KFD, dispatch or prove algorithmic correctness. The worker
request's custody identity fields are explicitly synthetic test values.

Use the worker's existing reviewed SDK/build-identity setup documented in
[MEASUREMENT.md](../../MEASUREMENT.md). Do not invent a build identity to make
configuration pass. An example configuration, with absolute locally reviewed
paths substituted, is:

~~~sh
cmake -S tools/fe2o3-llvm-link-worker/tests/gfx950-artifact \
  -B /absolute/fresh-gfx950-fixture-build \
  -DCMAKE_BUILD_TYPE=Release \
  -DFE2O3_WORKER_SOURCE=/absolute/fe2o3/tools/fe2o3-llvm-link-worker \
  -DLLVM_DIR=/absolute/reviewed-sdk/lib/cmake/llvm \
  -DLLD_DIR=/absolute/reviewed-sdk/lib/cmake/lld \
  -DFE2O3_PINNED_LLVM_VERSION=22.0.0git \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID=REVIEWED_BUILD_ID \
  -DFE2O3_LLVM_BUILD_ID_FILE=/absolute/reviewed-build-id.txt
cmake --build /absolute/fresh-gfx950-fixture-build \
  --target gfx950-artifact-fixture --parallel 2
/absolute/fresh-gfx950-fixture-build/gfx950-artifact-fixture \
  /absolute/fresh-output/fixture.hsaco
~~~

The output directory must already exist. The artifact file must not exist.
Output creation is exclusive and no-follow; a short/failed write is not a
qualified artifact. LLVM input is fixed and at most 4 KiB; artifact/report are
each at most 64 KiB. Each successful report includes exact input/artifact hashes
and recomputed worker derivation identity, plus ordinary publication inspection
facts. This fixture does not manufacture source or runtime authority from them.

The [qualification record](../../../../docs/evidence/gfx950-checked-artifact-20260923.md)
names actual tools, receipts, errors and observation boundaries. Root-owned
supervision supplied time/stream/resource guards; the fixture alone does not
guarantee elapsed-time or RSS bounds.
