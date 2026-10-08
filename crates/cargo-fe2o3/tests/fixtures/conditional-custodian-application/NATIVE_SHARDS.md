# Native Shards

`native-conditional-fill-shards` is a separate genuine native-V5 application
entrypoint. It must be compiled through the existing `cargo fe2o3` production
binding route; ordinary Cargo checks must not bypass the typed macro guard.

Arguments are strictly ordered:

```text
--native-v5-shards --producer-source <original-source-path> --devices <uid> <uid> ...
```

The roster contains two through eight distinct nonzero UIDs, each spelled as
`0x` followed by sixteen lowercase hexadecimal digits. Every device actually
admitted into the original Context must have one original stream and one scalar
generated graph node. `AllAdmitted` is checked by the Context-created graph
group, not inferred from caller metadata. It means all devices in that Context,
not every GPU on the host. Each shard uses the existing closed fill family,
workgroup and grid of 64, and a logical output length from 32 through 39.

The existing same-thread async driver advances that single graph. The caller
keeps each healthy original typed-result gate and independently checks every
output value. A native prepublication rejection, typed preactivation cold-device
failure, or settled decoder error can produce an explicit failed shard while
unrelated admitted nodes settle. No fault is synthesized by this entrypoint.
Unknown effects, deadlines, cancellation, foreign identities, pending owners,
or other structural errors never become partial-success records.

The separate `fe2o3.native-conditional-fill-shards.v1` report is printed only
after graph retirement, settled scope/epoch closure, host currentness checks,
exact result-account refund, Context shutdown and native backend shutdown.
Successful process exit means the reported complete or partial workload was
fully disposed; consumers must inspect `settled_local_failure` and every shard,
not reinterpret exit 0 as all-shard success. Existing single-device/serial-roster
qualification report schemas and default Cargo run selection are unchanged.

This slice performs no copy and has no direct native-DATA transfer, consumer
rebind, mixed-duration, physical-overlap, native-health or refinement-proof
claim. The original SDMA-promotion bridge is not fabricated here. CPU report
controls are not native execution evidence, and this caller has not run on GPUs.
