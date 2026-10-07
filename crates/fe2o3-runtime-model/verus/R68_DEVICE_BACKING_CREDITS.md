# R68 Device-Backing Credit Projection

`r68_device_backing_credits_v1.rs` checks four obligations for the N2
device-backing cost projection. Its executable function accepts an established
backing extent in `1..=206158430208` bytes and produces the existing R67
nineteen-coordinate vector: `ResidentDeviceAllocationBytes` equals that extent,
`AllocationRecords` equals one, and every other coordinate is zero.

The remaining obligations state exact bytes/record cardinality, absence of
other charges, and algebraic conservation when R67's reserve/release
postconditions are instantiated with this projection. That last statement
assumes those arithmetic postconditions; it does not prove that disposal took
place or compose the whole native engine.

## Production Correspondence

The native adapter in `fe2o3-kfd/src/shared_memory/resource_accounting.rs`
rechecks the actual private `Gfx942DeviceMemoryLayoutV1` through the existing
`device_memory_layout` function, then calls the production-used
`r68_device_backing_charge_v1` helper. It does not substitute requested logical
bytes for padded backing. The shared `fe2o3-resource-accounting` engine applies
the existing R67 admission and release decisions.

The Rust helper and Verus executable use the same bounds and coordinates;
their source correspondence is reviewed, not a machine-checked cross-language
refinement. Native layout extraction, session/device/VM and allocation identity,
record ownership, locking, unwind custody, driver currentness, actual backing
disposal and hardware behavior remain adapter/composition obligations. R68
does not establish root/global budgets, account bootstrap costs, GTT or queue
residency, pool-path qualification, or complete MEM-2 through MEM-5 coverage.

## Deliberate Negatives

Five pinned standalone mutations fail named postconditions:

- `zero_accepted`: removes the positive-extent requirement.
- `oversized_accepted`: removes the native profile upper bound.
- `logical_bytes_substitution`: charges logical bytes while backing includes padding.
- `allocation_record_omitted`: omits the allocation-record coordinate.
- `host_double_charge`: invents an additional host-backing charge for the N2 object.

Each must produce exactly `0 verified, 1 errors` at its named proof function.
Direct pinned-Verus checks passed for the positive and rejected all five
mutations. The complete authenticated runner registers them separately; a
focused direct run alone is not evidence that the full release gate passed.
The subsequent [authenticated R68 gate](../../../docs/evidence/local-r68-native-backing-2026-09-10/README.md)
passed all 57 positive sources, 1,334 obligations and 645 expected negatives,
including source/inventory, transcript and pre/post pinned closure checks.
