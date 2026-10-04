// Closed admission handshake for the real same-build debugger binary, not a wire qualification suite.
import { demand,same,digest,json,LIMITS } from './debug-runtime-observations-source-v1-data.mjs';
const OPERATIONS=['discover_capabilities','get_state','terminate'];
const CAPABILITIES=['hierarchy_inspection','kir_sites','source_sites','call_stack','breakpoints','watchpoints',
  'forward_step','reverse_step','pause','deterministic_replay','kir_ssa_values','source_variable_values',
  'register_values','allocation_relative_memory','semantic_trace','hardware_wave_state','kfd_dispatch_control'];
const REASONS=['absent','many_to_one','not_represented','not_captured','requires_authenticated_map','not_exposed_by_backend',
  'logical_visualization_only','read_only_backend','optimized_out','outside_capture_scope','truncated','authorization_required'];
function keys(value,expected,label) {
  demand(value!==null&&typeof value==='object'&&!Array.isArray(value),label);
  same(Object.keys(value).sort(),[...expected].sort(),label);
}
export function admissionInput() {
  return Buffer.from(OPERATIONS.map((operation,i)=>JSON.stringify({
    schema:'fe2o3-debug-request-v1',request_id:i+1,expected_revision:0,operation})).join('\n')+'\n');
}
export function admissionArguments(mode,bundle,request) {
  demand(mode==='loop'||mode==='workgroup','admission mode');
  return ['sim',mode==='loop'?'--bundle-v6':'--bundle-v5',bundle,'--request',request,
    '--runtime-observations','v1','--protocol','jsonl','--wave-width','32'];
}
export function validateAdmission(bytes) {
  demand(Buffer.isBuffer(bytes)&&bytes.length>0&&bytes.length<=LIMITS.stream,'admission stdout cap');
  const text=new TextDecoder('utf-8',{fatal:true}).decode(bytes);
  demand(text.endsWith('\n')&&!text.includes('\r'),'exact LF admission framing');
  const lines=text.slice(0,-1).split('\n');demand(lines.length===3&&lines.every(v=>v.length>0),'exactly3 correlated admission responses');
  let configuration=null,initial=null;
  const rows=lines.map((line,i)=>{
    const row=json(Buffer.from(line),262144);
    keys(row,['status','schema','request_id','operation','session','result'],'closed admission response');
    demand(row.status==='ok'&&row.schema==='fe2o3-debug-response-v1'
      &&row.request_id===i+1&&row.operation===OPERATIONS[i],'successful exact admission correlation');
    const s=row.session;
    keys(s,['backend','execution_kind','state','revision','configuration_identity','cursor','simulated',
      'hardware_observed','performance_prediction'],'closed session');
    digest(s.configuration_identity);demand(s.configuration_identity!=='0'.repeat(64),'nonzero session identity');
    if(configuration===null)configuration=s.configuration_identity;else demand(s.configuration_identity===configuration,'same admitted configuration');
    demand(s.backend==='cpu_kir_simulator'&&s.execution_kind==='cpu_kir_simulation'
      &&s.simulated===true&&s.hardware_observed===false&&s.performance_prediction===false,'CPU-only admission');
    demand(s.revision===(i===2?1:0)&&s.state===(i===2?'terminated':'stopped'),'read-only/terminate revision contract');
    keys(s.cursor,['configuration_identity','event_sequence','state_revision'],'closed cursor');
    demand(s.cursor.configuration_identity===configuration&&s.cursor.event_sequence===0
      &&s.cursor.state_revision===s.revision,'no admission navigation');
    if(i===0)initial=s;else if(i===1)same(s,initial,'get_state is read-only');
    if(i===0) {
      keys(row.result,['result','capabilities'],'closed capabilities');
      demand(row.result.result==='capabilities'&&Array.isArray(row.result.capabilities)
        &&row.result.capabilities.length===CAPABILITIES.length,'exact capability roster');
      const seen=new Set();
      for(const cap of row.result.capabilities) {
        demand(CAPABILITIES.includes(cap.name)&&!seen.has(cap.name),'closed unique capability name');seen.add(cap.name);
        if(cap.availability==='available')keys(cap,['name','availability'],'available capability');
        else {
          keys(cap,['name','availability','reason'],'unavailable capability');
          demand(['unavailable','authorization_required'].includes(cap.availability)&&REASONS.includes(cap.reason),'closed capability reason');
          if(cap.availability==='authorization_required')demand(cap.reason==='authorization_required','authorization reason');
        }
      }
      for(const name of ['register_values','hardware_wave_state','kfd_dispatch_control'])
        demand(row.result.capabilities.find(cap=>cap.name===name).availability==='unavailable','no physical hardware claim');
    } else if(i===1) {
      keys(row.result,['result','snapshot'],'closed initial state');
      demand(row.result.result==='state','state response');
      keys(row.result.snapshot,['status','reason'],'no initial invented checkpoint');
      demand(row.result.snapshot.status==='unavailable'&&row.result.snapshot.reason==='not_captured','entry snapshot not captured');
    } else {keys(row.result,['result'],'closed termination');demand(row.result.result==='terminated','terminated result');}
    return row;
  });
  return {configuration_identity:configuration,requests:3,responses:3,initial_revision:0,final_revision:1,
    cursor_event_sequence:0,terminated:rows[2].session.state==='terminated',hardware_observed:false,
    scope:'strict same-bundle request admission and bounded handshake; not output, transport or browser qualification'};
}
