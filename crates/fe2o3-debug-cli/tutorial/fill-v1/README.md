# Fill V1 debugger transcript

This fixture reuses the exact canonical KIR and strict request in
`crates/fe2o3-kir-sim-cli/tutorial/fill-v1/`. Generate the transcript from the
workspace root:

```sh
cargo run -p fe2o3-debug-cli --bin fe2o3-debug -- \
  sim \
  --kir-v7 crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir \
  --request crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json \
  --protocol jsonl \
  < crates/fe2o3-debug-cli/tutorial/fill-v1/requests.jsonl \
  > crates/fe2o3-debug-cli/tutorial/fill-v1/responses.jsonl
```

`responses.jsonl` is an exact golden, not hand-authored example. The website
projection is deterministic and lossless for every field it displays:

- provenance comes from `session.{backend,execution_kind,simulated,hardware_observed,performance_prediction}`;
- cursor and revision come from `session.cursor` and `session.revision`;
- hierarchy rows come from `result=scopes` and retain
  `interpretation=logical_visualization`;
- the selected lane/site/SSA/pointer state comes from the first captured
  `result=control` snapshot and the following `result=values` response;
- memory bytes and initialization state come from `result=memory`;
- timeline rows come from `result=events` without changing sequence, scope,
  site, category, or provenance;
- source/register limitations come from the two typed `unavailable` responses;
- replay completeness and identity come from `result=trace`.

The checked-in `workbench-projection.json` contains only those selected wire
objects plus fixture metadata. The integration test regenerates it from
`responses.jsonl`, so tutorial UI data cannot drift into synthetic debugger
claims.
