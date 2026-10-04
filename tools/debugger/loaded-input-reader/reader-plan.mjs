// Pure bridge from complete qualified historical Buffers to an inactive reader plan.
import {createHash} from 'node:crypto';
import {isDeepStrictEqual as equal} from 'node:util';
import {planLoadedSelection} from '../loaded-profile/loaded-selection.mjs';
import {EXACT_ALIASES} from './reader-aliases.mjs';
import {deriveReadBudget} from './reader-protocol.mjs';
const freeze=v=>{if(v&&typeof v==='object'){for(const x of Object.values(v))freeze(x);Object.freeze(v);}return v;};
const check=(v,m)=>{if(!v)throw Error('historical reader bridge: '+m);};
export function buildHistoricalReadProtocol(input){
 const historical_plan=planLoadedSelection(input);
 check(historical_plan.qualified_retained_input_bytes===true&&historical_plan.entries.length===1173,'complete historical selection');
 check(equal(historical_plan.named_alias_roles.map(r=>({pin:r.pin,resolved:r.resolved})),EXACT_ALIASES.map(r=>({pin:r.pin,resolved:r.resolved}))),'three exact named aliases');
 const entries=historical_plan.entries.map(r=>{
  let ownership=null;
  for(const index of r.loaded_named_roles){
   const f=historical_plan.loaded_named_roles[index].observation;
   if(f.exists){const owner={uid:String(f.uid),gid:String(f.gid)};check(ownership===null||equal(ownership,owner),'all loaded ownership labels');ownership=owner;}
  }
  const labels={prior:r.prior_input_roles.map(i=>'prior:'+i),loaded:r.loaded_named_roles.map(i=>'loaded:'+i),duties:r.duty_targets.map(x=>'duty:'+x.original_index+':target:'+x.target_index),extras:r.extra_roles.map(x=>'extra:'+x.index)};
  const inherited=r.duty_targets.map(x=>x.cap).concat(r.extra_roles.map(x=>x.cap));
  const cap=r.kind==='readable'?Math.min(512*1024*1024,...inherited,...(r.loaded_named_roles.length?[256*1024*1024]:[])):0;
  return{path:r.path,kind:r.kind,pin:r.pin,resolved:r.resolved,identity:r.identity,ownership,labels,cap};
 });
 const aliases=structuredClone(EXACT_ALIASES),budget=deriveReadBudget(entries,aliases,2);
 check(budget.one_pass.readable_paths===1150&&budget.one_pass.absence_observations===23,'closed readable/absence census');
 check(budget.one_pass.payload_bytes===1817924086&&budget.one_pass.reserved_bytes===1817925236&&budget.one_pass.content_calls===29748,'unchanged historical data-plane arithmetic');
 const selection_digest=createHash('sha256').update(JSON.stringify(historical_plan)).digest('hex');
 const protocol=freeze({schema:'fe2o3-loaded-read-protocol-v1',entries,aliases,passes:2,budget,selection_digest,custody:{qualified_historical_input_bytes:true,operational_roster_complete:false,root_cap_change_approved:false,execution_authority:false}});
 return freeze({protocol,historical_plan,source_authority:'none',complete_operational_count:null,complete_operational_budget:null,cap_change_approved:false,live_activation:false});
}
