# gfx942 Active Checkpoint Qualification Fixture V1

This directory owns the finite kernel artifact used only to qualify non-empty
direct-KFD stopped checkpoints. The checked kernel runs one Wave64 workgroup
for a fixed one billion loop iterations, writes one result per lane, and then
terminates normally. It neither waits on host input nor requires a reset to
finish. A harmless volatile read through LLVM's public implicit-argument
intrinsic keeps the complete COV6 implicit block visible to the ordinary
loader after optimization.

`active-checkpoint.ll` is the reviewable source, `policy-v1.txt` is the exact
address-free ABI and launch policy, and `build-and-verify.sh` requires the
recorded ROCm 7.2.4 clang identity and a byte-identical rebuild. Run the recipe
on the pinned ROCm host:

```sh
crates/fe2o3-runtime/fixtures/trusted-gfx942-active-checkpoint-v1/build-and-verify.sh
```

The ignored live test independently checks all three digests and passes the
object through the ordinary COV6 loader before publication. Its unsafe test
authority admits only the complete pinned invocation. This is bounded
qualification evidence, not Worker V3 production authority and not a general
permission to execute unverified artifacts. The target's runtime publication
record is a sequential observation naming the fixture and native queue; it is
not independent authentication of the bytes physically loaded on that queue.
