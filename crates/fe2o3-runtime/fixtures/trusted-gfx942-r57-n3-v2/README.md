# R57 N3 Qualification V2

This is a new qualification identity, not a revision of V1 hardware evidence.
The shared vecadd source/object, exact R/R/W ABI, initial images, two ordered
equations and cleanup requirements are unchanged.

V1 required an uninitialized-C rejection before final authority. DeviceLocal
allocations now begin with initialized zeros; initialization coverage is distinct
from this fixture's exact initial-content requirement. The current negative is
therefore a mixed-memory A/B/HostVisible-upload roster, rejected synchronously
with InvalidLaunch and the pinned exact reason, before C/D H2D and with zero
authority consultations. The final authority still verifies exact initial A/B/C
bytes and digests, then initial B/D with C's digest cleared after its first write.

V2 has its own policy digest, typed-argument signature and backend gate. V1
policy bytes, digest, public entry point and historical evidence remain intact;
neither authority accepts the other's request signature. The private two-phase
implementation is shared. Direct malformed authority consultations still count,
even though they do not advance the phase.

The existing `gfx942-runtime-r57-n3-qualification` example now runs V2 and reports
its profile and policy digest after explicit cleanup:

```sh
cargo run -p fe2o3-runtime --release --features hardware-qualification \
  --example gfx942-runtime-r57-n3-qualification -- DEVICE_UNIQUE_ID
```

Select an available device by its KFD unique ID on shared hosts. CPU tests and
the source-shape gate do not establish native qualification, formal verification
or HIP/HSA performance parity. V2 requires fresh hardware evidence; old V1
receipts must not be relabeled.
