# Shared Capability SSA Mechanics

Pauli owns this child; Ram owns the Workgroup typed resolver. Neither this graph
nor a transparent reference type grants authority. The Math resolver now imports
this graph and no longer has a second implementation.

Mount `include!("production_semantic_kir_v1/capability_ssa_graph_01.rs");` once.
Exports to sibling children:

- `CapabilitySsaGraphV1::new(body, ssa, max_work)`
- `charge(usize)`, `use_value(block, local)`, `definition(SsaValueV1)`
- `incoming(target, local)`, `reaches(from, to)`
- `loan_live(CapabilityLoanV1, CapabilityDefinitionSiteV1)`
- `body` and `ssa` immutable references

`CapabilityDefinitionSiteV1 { block, statement: Option<u32>, local }` uses
`None` for the terminator. `CapabilityLoanV1 { borrow, owner_local, owner_value }`
retains the actual source storage definition. Workgroup can replace its local
Graph/Site/Loan with imports, rename `unique_use` to `use_value`, and supply a
terminator site to `loan_live` (`local` is immaterial on that consumer).

The common use/definition/incoming/reachability algorithms are the bounded
mechanics used by the Workgroup resolver. Ordered source scanning extends its
whole-block loan checks to exact statement consumers, while rejecting moves,
storage death, overwrites, deinitialization and raw/mutable exposure. Cyclic
loans reject until a storage-generation proof exists. Ambiguous multiple SSA
versions within one block still reject; this is not a new permissive origin API.

Parent/Ram: remove Workgroup's local Graph implementation when switching its
imports, not just add a forwarding wrapper around a second graph. Its closed
issuer/epoch/type validation remains unchanged. Run both typed resolver suites
and the shared graph tests centrally; no compile/test success is claimed here.
