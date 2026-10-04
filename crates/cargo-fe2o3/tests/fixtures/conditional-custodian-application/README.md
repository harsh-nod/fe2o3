# Genuine Conditional Application Fixture

This standalone package selects a freshly compiled write-only u32 index-fill
kernel. Its host entry consumes Cargo's inherited application-custodian handoff,
requests production compiler-currentness auditing and a retained conditional
proof, and revalidates the resulting remote artifact. It imports no captured
binding, compiler receipt, proof receipt, or application handoff.

Run through the `genuine` campaign in
`scripts/qualify-proof-resource-inspection.sh`, using the real installed compiler
and proof-service entrypoints. Run Cargo from this package's directory: the local
`.cargo/config.toml` supplies static GNU-host linking, and `--manifest-path` alone
does not make Cargo discover that configuration. Production supplies explicit
device and host targets, so the static flags do not affect host proc macros or
the AMDGPU compilation. The device phase selects the package library, whose
AMDGPU branch is `no_std`; the host phase selects the ordinary binary. AMDGPU
does not support a binary crate target.

Qualification is currently failing at compiler-time proof execution: protected
Cargo's inherited exec-monitoring filter conflicts with the local proof
controller's unfiltered-process requirement. Host marker projection is another
remaining gate. This fixture has not yet reached its successful host admission
message; the ignored campaign intentionally reports failure until those joins
work. See `docs/runtime-multi-gpu-critical-path.md` for the ordered work.

This is an admission fixture, not a two-GPU application or a HIP/HSA parity
claim. The remote artifact itself grants no native launch authority. The next
application stage must retain that one artifact across both devices' conditional
invocations and their completion/drain, then validate real completed bytes over
the native XGMI path.
