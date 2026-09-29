// Resource proposal only: exact reader arithmetic is not a global IO or allocation attestation.
import {compileOperationalGraph,checkedAdd} from './loaded-operational-graph.mjs';
const fail=m=>{throw Error('loaded operational policy: '+m);};
const check=(v,m)=>{if(!v)fail(m);};
const freeze=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
function times(a,b){check(Number.isSafeInteger(a)&&a>=0&&Number.isSafeInteger(b)&&b>=0&&Number.isSafeInteger(a*b),'checked multiplication');return a*b;}
export function exactReadEnvelope(entries,aliases,passes){
 check(Array.isArray(entries)&&Array.isArray(aliases)&&Number.isInteger(passes)&&passes>0&&passes<=4,'finite read phase');
 const aliasNames=new Set(aliases.map(a=>a.pin.path));let payload=0,readable=0,absence=0,calls=0,metadata=0;
 for(const r of entries){
  if(r.kind==='readable'){
   check(Number.isSafeInteger(r.pin?.bytes)&&r.pin.bytes>=0&&r.pin.bytes<=512*1024*1024,'member bound');
   readable++;payload=checkedAdd(payload,r.pin.bytes);calls=checkedAdd(calls,Math.ceil(r.pin.bytes/65536)+1);metadata=checkedAdd(metadata,aliasNames.has(r.path)?19:10);
  }else{
   check(r.kind==='absence-observation'&&r.pin===null&&typeof r.path==='string'&&r.path.startsWith('/'),'absence shape');
   absence++;const prefixes=r.path.split('/').length;check(prefixes<=129,'absence component bound');metadata=checkedAdd(metadata,2*(2*prefixes+1));
  }
 }
 const one={readable,absence,payload_bytes:payload,reserved_bytes:checkedAdd(payload,readable),content_calls:calls,metadata_provider_calls:metadata};
 return freeze({passes,one_pass:one,total:Object.fromEntries(Object.entries(one).map(([k,v])=>[k,times(v,passes)])),chunk_bytes:65536,short_read:'refuse without retry',eof_probe_bytes_per_readable:1});
}
export function proposeResourcePolicy(raw,{fixture_manifest_bytes,historical_paths}){
 const graph=compileOperationalGraph(raw);
 check(Number.isSafeInteger(fixture_manifest_bytes)&&fixture_manifest_bytes>0&&fixture_manifest_bytes<=65536,'explicit bounded fixture manifest size');
 check(Array.isArray(historical_paths)&&historical_paths.length===1173&&new Set(historical_paths).size===1173,'exact historical name count');
 const byName=new Map(graph.entries.map(r=>[r.path,r])),historical=historical_paths.map(p=>{check(byName.has(p),'historical name missing');return byName.get(p);});
 const driver=exactReadEnvelope(historical,graph.aliases,2);
 check(driver.one_pass.readable===1150&&driver.one_pass.absence===23&&driver.one_pass.payload_bytes===1817924086&&driver.one_pass.content_calls===29748,'fixed historical arithmetic');
 const outer=exactReadEnvelope(graph.entries,graph.aliases,2);
 const fixture={files:77,manifest_bytes:fixture_manifest_bytes,payload_bytes:checkedAdd(13854356,fixture_manifest_bytes),reserved_bytes:checkedAdd(13854432,fixture_manifest_bytes+1),content_calls:checkedAdd(347,Math.ceil(fixture_manifest_bytes/65536)+1),limit_bytes:33554432,limit_calls:1024};
 check(fixture.reserved_bytes<=fixture.limit_bytes&&fixture.content_calls<=fixture.limit_calls,'fixture aggregate');
 let outputBytes=0;for(const o of graph.outputs)outputBytes=checkedAdd(outputBytes,o.cap_bytes);
 const required=graph.counts.named;
 const moduleNames=[...new Set(graph.imports.flatMap(e=>e.specifier.startsWith('node:')?[e.from]:[e.from,e.to]))];
 const moduleEnvelope=exactReadEnvelope(moduleNames.map(p=>byName.get(p)),[],1).one_pass;
 return freeze({schema:'fe2o3-inactive-loaded-operational-policy-v1',
 graph_input_sha256:graph.input_sha256,
 known_named_input_cap_required:required,existing_generic_cap:1024,known_input_cap_shortfall:Math.max(0,required-1024),approved_input_cap:null,
 phases:{external_fixture_reader:fixture,proposed_supervisor_before_and_after:outer,inner_historical_reader:driver,
  request_before_and_after:{member_cap_bytes:1048576,reserved_bytes_cap:2097154,content_calls_cap:34,exact_chunk_and_eof_required:true},
  module_loader:{logical_module_sources:moduleNames,logical_module_count:moduleNames.length,logical_payload_bytes:moduleEnvelope.payload_bytes,logical_inclusive_bytes:moduleEnvelope.reserved_bytes,logical_exact_chunk_or_refuse_calls:moduleEnvelope.content_calls,actual_loader_io_metered:false,not_part_of_controlled_read_subtotal:true},
  existing_generic_supervisor:{exact_chunk_contract:false,eof_probe:false,aggregate_call_budget:false,absence_obligations:false,all_repository_paths_individually_pinned:false,not_covered_by_proposed_envelope:true}},
 controlled_read_phases_subtotal:{reserved_bytes:checkedAdd(checkedAdd(outer.total.reserved_bytes,driver.total.reserved_bytes),fixture.reserved_bytes),content_calls:checkedAdd(checkedAdd(outer.total.content_calls,driver.total.content_calls),fixture.content_calls),
  scope:'Only proposed two graph passes, two historical passes and one fixture read; excludes request, module loader, external resource probes and any extra source scan.'},
 output:{roles:graph.outputs,total_cap_bytes:outputBytes,all_paths_bound:graph.outputs.every(o=>o.path!==null),atomic_new_file_writer_required:true,oversize_result:'refuse, never silently truncate accepted evidence'},
 memory:{reader_scratch_bytes:65537,fixture_payload_bytes:13854356,planner_private_buffers_bytes:13854356,profile_private_buffers_bytes:11419340,graph_json_cap_bytes:33554432,process_rss_bound_proved:false},
 scheduling:{inner_reader_milliseconds:240000,outer_process_deadline_required:true,blocking_syscall_preemption:false,scope_end:null,coordination_evidence:null},
 metadata:{provider_counts_are_not_kernel_syscall_counts:true,fixture_metadata_metered:false,resource_probe_io_metered:false},
 requirements:{all_sources_and_import_edges_individually_pinned:true,all_role_labels_preserved:true,all_23_absences_retained:true,all_three_alias_policies_exact:true,root_must_bind_fresh_identity_and_currentness:true,root_must_qualify_new_supervisor:true},
 complete_operational_budget:false,cap_change_approved:false,execution_authority:false,native_authority:false});
}
