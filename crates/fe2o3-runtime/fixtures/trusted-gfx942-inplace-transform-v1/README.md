# gfx942 In-Place Transform Qualification Fixture V1

This directory owns the exact one-buffer kernel artifact for the runtime's
persistent-compute qualification lane. `inplace_transform.ll` is the reviewable
source, `policy-v1.txt` is the complete address-free ABI, effect, extent, and
validation policy, and `build-and-verify.sh` requires its recorded ROCm clang
version and a byte-identical rebuild of `inplace_transform.hsaco`.

Run the recipe on a host with the pinned ROCm compiler:

```sh
crates/fe2o3-runtime/fixtures/trusted-gfx942-inplace-transform-v1/build-and-verify.sh
```

The Rust qualification module independently hashes all three files, validates
the COV6 envelope and selected kernel through the ordinary loader, admits only
one exact 1 MiB DeviceLocal ReadWrite binding, and compares every output byte
against the deterministic expected image. No prefix, sample, checksum-only, or
probabilistic result validation is accepted.

This fixture is bounded test custody for one repository-owned kernel. It is not
general compiler-lineage authority, Worker V3 authentication, or permission to
launch another artifact, allocation shape, or invocation.
