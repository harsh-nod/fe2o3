#!/usr/bin/env node
// Five Cargo actions over one fixed fixture. No rustc argv reconstruction.
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {Work,saveNew,cargoChild} from './bf16-source-workflow/io.mjs';
import {check,sha,LIMITS,FIXTURE,FIXTURE_SHA,STAGES,packageManifest,cargoArgs,cargoEnv,
  inspect,request,publication,admission} from './bf16-source-workflow/protocol.mjs';

export function options(argv) {
  const names=['repo','extractor','cargo','rustc','work','deadline'],o={};
  check(argv.length===names.length*2,'required: --repo --extractor --cargo --rustc --work --deadline');
  for(let i=0;i<argv.length;i+=2) {
    const key=argv[i].slice(2);
    check(argv[i].startsWith('--')&&names.includes(key)&&!Object.hasOwn(o,key),'closed unique option');
    check(typeof argv[i+1]==='string'&&argv[i+1].length>0&&!argv[i+1].includes('\0'),'option value');
    o[key]=argv[i+1];
  }
  for(const key of names.filter(k=>k!=='deadline'))check(path.isAbsolute(o[key])&&Buffer.byteLength(o[key])<=4096,'absolute bounded '+key);
  return o;
}
function mkdir(p){fs.mkdirSync(p,{mode:0o700});}
function parseReport(work,dir) {
  const file=path.join(dir,'observation.json'),r=work.read(file,LIMITS.report);
  const text=new TextDecoder('utf-8',{fatal:true}).decode(r.bytes);
  return {report:JSON.parse(text),pin:{path:file,bytes:r.bytes.length,sha256:sha(r.bytes)}};
}
function sameBytes(work,file,expected,cap) {
  const actual=work.read(file,cap);check(actual.bytes.equals(expected),'immutable input changed: '+file);
  return actual;
}
export async function main(argv, inherited=process.env) {
  const o=options(argv);
  check(path.normalize(o.work)===o.work&&path.basename(o.work)!=='.'&&o.work!=='/','fresh work spelling');
  check(fs.realpathSync(path.dirname(o.work))===path.dirname(o.work),'work parent must be canonical');
  const repo=fs.realpathSync(o.repo);
  check(repo===o.repo,'repo path must be canonical');
  const work=new Work(o.work,o.deadline),results=[];
  let created=false;
  try {
    // No existing output tree, fallback, deletion or resume path.
    mkdir(o.work);created=true;
    const tools={};
    for(const key of ['cargo','rustc','extractor'])tools[key]=work.tool(o[key]);
    const fixture=path.join(repo,FIXTURE);
    const source=work.read(path.join(fixture,'src/lib.rs'),LIMITS.source).bytes;
    check(source.length===3950&&sha(source)===FIXTURE_SHA,'this workflow supports only the unchanged direct BF16 fixture');
    const template=work.read(path.join(fixture,'Cargo.toml'),8192).bytes;
    const lock=work.read(path.join(fixture,'Cargo.lock'),1024*1024).bytes;
    const manifest=packageManifest(new TextDecoder('utf-8',{fatal:true}).decode(template),repo);
    const original=path.join(o.work,'original');
    mkdir(original);mkdir(path.join(original,'src'));
    mkdir(path.join(o.work,'target'));
    const packageDirs=[original,path.join(original,'identity'),path.join(original,'swap01')];
    for(const dir of packageDirs) {
      if(dir!==original){mkdir(dir);mkdir(path.join(dir,'src'));}
      saveNew(path.join(dir,'Cargo.toml'),manifest);saveNew(path.join(dir,'Cargo.lock'),lock);
    }
    saveNew(path.join(original,'src/lib.rs'),source);
    const immutable=[
      ...packageDirs.flatMap(dir=>[[path.join(dir,'Cargo.toml'),manifest,8192],[path.join(dir,'Cargo.lock'),lock,1024*1024]]),
      [path.join(original,'src/lib.rs'),source,LIMITS.source],
    ];
    work.save('plan.json',{schema:'fe2o3-public-cargo-bf16-workflow-plan-v1',
      target:'gfx942:xnack-',fixture_source_sha256:sha(source),manifest_sha256:sha(manifest),
      lock_sha256:sha(lock),tools,stages:STAGES,limits:LIMITS,deadline:o.deadline,
      direct_cargo_calls:5,total_descendant_processes:'not fixed; Cargo-managed under owned process groups',
      report_claim:'diagnostic source admission and unchanged normal-ranked refusal only',
      constructor_context_memory_bound:false,numerical_or_hardware_qualification:false});
    const actualTools={cargo:tools.cargo.path,rustc:tools.rustc.path,extractor:tools.extractor.path};
    let selected=null;
    const published=new Map(),requests=new Map();
    for(const stage of STAGES) {
      for(const [file,bytes,cap]of immutable)sameBytes(work,file,bytes,cap);
      const cwd=path.join(o.work,stage.package),out=path.join(o.work,stage.name);
      let requestPath;
      if(stage.mode==='publish') {
        const req=request(selected,stage.order);
        requestPath=work.save(stage.order+'.request.json',req);requests.set(stage.order,req);
        check(!fs.existsSync(path.join(original,stage.order,'src/lib.rs')),'candidate must be absent before publication');
      }
      if(stage.mode==='admit') {
        const p=published.get(stage.order);
        check(p,'prior successful publication required');
        const r=sameBytes(work,path.join(cwd,'src/lib.rs'),p.bytes,LIMITS.source);
        check(String(r.stat.dev)===p.fact.device&&String(r.stat.ino)===p.fact.inode,'published inode changed before admission');
      }
      check(!fs.existsSync(out),'new report directory must be absent');
      const terminal=await cargoChild(work,stage.name,actualTools.cargo,
        cargoArgs(path.join(cwd,'Cargo.toml'),path.join(o.work,'target')),cwd,
        cargoEnv(inherited,actualTools,stage,out,requestPath));
      const parsed=parseReport(work,out);
      // Raw files and terminal are already durable before any interpretation.
      const row={name:stage.name,terminal,report:parsed.pin,qualified:false};
      results.push(row);
      if(stage.mode==='inspect')selected=inspect(parsed.report,source);
      if(stage.mode==='publish') {
        const r=work.read(path.join(original,stage.order,'src/lib.rs'),LIMITS.source);
        const fact=publication(parsed.report,selected,requests.get(stage.order),stage.order,r.bytes,r.stat);
        published.set(stage.order,{bytes:r.bytes,fact});
        immutable.push([path.join(original,stage.order,'src/lib.rs'),r.bytes,LIMITS.source]);
      }
      if(stage.mode==='admit') {
        const r=work.read(path.join(cwd,'src/lib.rs'),LIMITS.source),p=published.get(stage.order);
        row.admission=admission(parsed.report,stage.order,r.bytes,p.fact,r.stat);
      }
      row.qualified=true;work.save(stage.name+'.accepted.json',row);
      for(const [file,bytes,cap]of immutable)sameBytes(work,file,bytes,cap);
    }
    check(work.steps===5&&results.length===5&&results.every(r=>r.qualified),'complete five-action roster');
    for(const key of ['cargo','rustc','extractor'])check(JSON.stringify(work.tool(tools[key].path))===JSON.stringify(tools[key]),'selected executable changed');
    sameBytes(work,path.join(fixture,'src/lib.rs'),source,LIMITS.source);
    sameBytes(work,path.join(fixture,'Cargo.toml'),template,8192);
    sameBytes(work,path.join(fixture,'Cargo.lock'),lock,1024*1024);
    const report={schema:'fe2o3-public-cargo-bf16-workflow-v1',status:'passed',results,
      resources:work.census(),read_bytes:work.readBytes,source_publications:2,fresh_nominal_admissions:2,
      normal_ranked_admissions:0,artifacts:0,simulation:false,native_execution:false,
      all_milestones_complete:false,
      scope:'Real Cargo public wrapper source actions. No complete dependency/runtime input census, whole-memory bound, numerical proof, or hardware claim.'};
    work.save('PASSED.json',report);
    // A deadline/output failure after PASSED leaves that record historical only.
    work.guard();work.census();
    process.stdout.write('BF16 Cargo source workflow passed: '+o.work+'\n');
    return report;
  } catch(error) {
    if(created) {
      const record={schema:'fe2o3-public-cargo-bf16-workflow-failure-v1',status:'failed',
        error:String(error?.message??error).slice(0,4096),completed_results:results,
        retained_outputs:true,qualified_workflow:false,
        note:'Failed report defaults are not evidence that earlier stages or publication effects did not occur. PASSED, if present before a final postflight refusal, is historical only.'};
      try{saveNew(path.join(o.work,'FAILED.json'),Buffer.from(JSON.stringify(record,null,2)+'\n'));}
      catch(e){process.stderr.write('Could not retain FAILED.json: '+String(e.message).slice(0,256)+'\n');}
    }
    throw error;
  }
}
if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href) {
  main(process.argv.slice(2)).catch(error=>{
    process.stderr.write('BF16 Cargo workflow refused: '+String(error.message).slice(0,4096)+'\n');
    // Do not throw/exit here: referenced failed-child SIGKILL timers must fire.
    process.exitCode=1;
  });
}
