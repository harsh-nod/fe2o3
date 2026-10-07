# Native Copy Consumer Capture Fixture

This separate source fixture requests one rank-one, workgroup-64 `copy_u32`
kernel. It reads a shared input slice and writes the same coordinate of a
write-only disjoint output. Missing input or output coordinates are unchanged.
The safe Rust point reference is independent of generated code.

This is source for a future actual protected V5 capture, not a recognized or
proved consumer. No kernarg offsets, machine instruction offsets, decoded
machine profile, consumer proof boundary, or GPU authority are asserted here.
In particular, it must not be admitted through the existing index-fill family.

The public native route is selected explicitly with `authority release
--native build`. The existing launcher, original V4 contract, root compiler
service, retained native profile, pinned compiler/runtime closure, and original
account must all succeed. A source fixture or a command line cannot construct
those owners. Run CPU fixture tests and generate its lockfile from the reviewed
offline dependency closure before using a frozen protected build.

Required before execution: a measured native Cargo/backend/toolchain closure;
the independently approved production build configuration and its expected
identity, containing this exact crate, source and working directory; installed
V3 compiler profile/images/policies/signing custody; and a standalone native
compiler coordinator under its existing whole-domain root custodian. The
separate native application manager and GPU queues are not needed for this
build-only capture. A Verus runtime installation by itself does not provide
any of these compiler-service owners. No environment override or fixture
manifest may replace them.

Once that deployment is independently available, the build-only capture shape
is:

```sh
FE2O3_TARGET=gfx942 "$CARGO_FE2O3" authority release --native build \
  --manifest-path "$SOURCE/crates/cargo-fe2o3/tests/fixtures/native-copy-consumer/Cargo.toml" \
  --lib --offline --frozen --target-dir "$OWNED_TARGET" \
  --export-native-policy-inputs fe2o3_native_copy_consumer src/lib.rs "$NEW_EXPORT"
```

`NEW_EXPORT` must not exist and its parent must be owned by the invoking user
without group/other write access. The export retains source packet and policy
roster observations; it is not a proof or a substitute for the original
published artifact/readiness owner. Preserve the actual first refusal. Capture
the actual descriptor, final F module and authenticated machine analysis before
adding any consumer-specific ABI or machine recognizer. This command has not
yet been qualified against an installed protected deployment.
