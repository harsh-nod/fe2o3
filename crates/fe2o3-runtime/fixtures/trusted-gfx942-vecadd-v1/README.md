# gfx942 Vecadd Qualification Fixture V1

This directory owns the exact kernel artifact used by the runtime's bounded
KFD and HSA hardware qualification lanes. `vecadd.ll` is the reviewable source,
`policy-v1.txt` is the complete address-free ABI and effect premise, and
`build-and-verify.sh` requires the recorded ROCm 7.2.4 clang version string and
a byte-identical rebuild of `vecadd.hsaco`. The version string is an
environment check, not compiler-lineage authentication.

Run the recipe on the pinned ROCm host:

```sh
crates/fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/build-and-verify.sh
```

The Rust qualification module independently checks the source, policy, and
artifact SHA-256 values, then validates the COV6 envelope, target, selected
symbol, kernarg layout, workgroup shape, and metadata effects through the
ordinary loader. The feature-gated KFD qualification constructor additionally
admits only the exact invocation described by `policy-v1.txt`; it does not
implement the production launch-authority trait.

This fixture is production-quality test custody for one repository-owned
kernel. It is not Worker V3 authentication, general compiler-lineage authority,
or permission to launch another artifact or invocation.
