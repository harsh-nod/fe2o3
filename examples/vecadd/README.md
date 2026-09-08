# Typed vecadd M1 vertical slice

`vecadd` is an ordinary `#[kernel(typed)]` function. `KernelContext` and the three
typed `Global` capabilities are logical inputs; the generated physical ABI contains
only the `a`, `b`, and `c` fat slices.

Enter the one production compiler transaction without a kernel, source, or
workload-profile selector:

```console
FE2O3_TARGET=gfx942 cargo fe2o3 build -p fe2o3-vecadd
```

The host surface has two deliberate trust transitions:

1. `admit_protected_vecadd` consumes compiler-owned production facts and joins
   them to an independently inspected descriptor.
2. `prepare_protected_vecadd_kfd` consumes that admission plus an authenticated
   Worker V3 executable, validates shapes and launch geometry, and returns a
   move-only invocation retaining all input and output borrows through checked
   completion.

There is no digest constructor, test receipt, raw launch, or legacy fallback in
the example. The W6/W7 boundary admits the final direct-KFD transition only when
sealed Worker recovery carries the exact V5 production-result identity and its
applicable machine-refinement receipt. Generated-host admission compares that
identity with the retained production result before argument preparation; KFD and
HSA custody bind it through load and dispatch. Omission or graph, epoch, target,
launch, artifact, machine-receipt, or result substitution fails before submission.

An authority-free Bundle V8 containing the exact canonical KIR V13 graph can be
executed on the deterministic CPU simulator:

```console
cargo run -p fe2o3-vecadd -- --simulate-v8 /path/to/vecadd.bundle-v8 /path/to/request.json
```

Bundle V8 grants no compiler, proof, artifact, load, launch, or hardware authority.
Its V13 graph identity and final epoch must come from the same production
transaction; a handwritten, stale, downgraded, or cross-kernel bundle is rejected.

The focused M1 tests cover source capability forgery, unchecked access, role
substitution, Rust alias rejection, cross-kernel ABI substitution, checked vector
shape and launch arithmetic, invalid target admission, unsealed production
admission, malformed Bundle V8 custody, positive sealed production-host
admission/preparation, and hostile identity substitution before side effects.
