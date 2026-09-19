import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import {makePlan,validatePlan,validateMap,demo,demoWave} from './model.mjs';
import {createDeck} from './pptx.mjs';
import {startServer} from './server.mjs';
import {ObsClient} from './adapters.mjs';

test('対応表は同じ出力・発話を要求し、不正な入力を拒否する',()=>{
  const map=demo(),plan=makePlan(map);assert.deepEqual(validatePlan(plan,map),plan);
  assert.throws(()=>validatePlan({...plan,export_id:'other'},map),/別の出力/);
  const bad=structuredClone(plan);bad.cues[0].segment_ids=['wrong'];assert.throws(()=>validatePlan(bad,map),/発話/);
  bad.cues[0]=plan.cues[0];bad.cues[0].slide_id=-1;assert.throws(()=>validatePlan(bad,map),/スライド/);
  for(const name of ['../secret.wav','C:\\secret.wav','x/secret.wav','a.wav:stream']){const m=demo();m.blocks[0].audio_file=name;assert.throws(()=>validateMap(m),/音声名/);}
});

test('PPTXは表紙・各ブロック・全文ノート・日本語を含むOOXML ZIPを生成する',()=>{
  const map=demo(),plan=makePlan(map);plan.title='説明 <日本語> & 資料';
  const bytes=createDeck(plan,map),files=new Map();let p=0;
  while(bytes.readUInt32LE(p)===0x04034b50){const size=bytes.readUInt32LE(p+18),n=bytes.readUInt16LE(p+26),name=bytes.subarray(p+30,p+30+n).toString(),start=p+30+n;files.set(name,bytes.subarray(start,start+size).toString());p=start+size;}
  assert.equal([...files.keys()].filter(n=>/^ppt\/slides\/slide\d+\.xml$/.test(n)).length,4);
  assert.match(files.get('ppt/slides/slide1.xml'),/説明 &lt;日本語&gt; &amp; 資料/);
  assert.match(files.get('ppt/notesSlides/notesSlide2.xml'),/台本と音声から/);
  assert.match(files.get('ppt/notesSlides/notesSlide2.xml'),/ph type="body"/);
  assert.match(files.get('[Content_Types].xml'),/presentation.main/);
  assert.equal(bytes.readUInt32LE(bytes.length-22),0x06054b50);
});

test('HTTP APIで手動・自動進行、停止、対応表と外部連携を統合する',async t=>{
  const calls=[],obs={connected:true,disconnect(){},async scene(name){calls.push(['obs',name]);}};
  const app=await startServer({port:0,ppt:async(...args)=>{calls.push(args);return {};},obs});t.after(()=>app.server.close());
  const api=async(path,body)=>{const r=await fetch(app.url+path,{method:body?'POST':'GET',headers:{'x-scriptsync-token':app.token},body:body?JSON.stringify(body):undefined});const data=await r.json();return {status:r.status,data};};
  assert.equal((await fetch(app.url+'/api/state')).status,400);
  assert.equal((await fetch(app.url+'/api/state',{headers:{'x-scriptsync-token':app.token,Origin:'https://other.test'}})).status,400);
  let state=(await api('/api/state')).data,plan=state.plan;plan.mode='auto';plan.powerpoint_path='C:\\demo.pptx';plan.powerpoint_enabled=true;plan.obs_enabled=true;plan.cues[0].slide_id=256;plan.cues[0].scene='カメラ';
  await api('/api/plan',{plan});await api('/api/progress',{player_id:'test',revision:state.revision,index:0,time:0});
  state=(await api('/api/transport',{action:'play'})).data;assert.equal(state.status,'playing');assert.deepEqual(calls,[['goto','C:\\demo.pptx',256],['obs','カメラ']]);
  assert.equal((await api('/api/plan',{plan})).status,400);
  const stale=state.revision;state=(await api('/api/transport',{action:'pause'})).data;
  state=(await api('/api/transport',{action:'ended',player_id:'test',revision:stale})).data;assert.equal(state.index,0);assert.equal(state.status,'paused');
  state=(await api('/api/transport',{action:'play'})).data;
  state=(await api('/api/transport',{action:'ended',player_id:'test',revision:state.revision})).data;assert.equal(state.index,1);assert.equal(state.status,'playing');
  state=(await api('/api/transport',{action:'seek',index:2,time:1})).data;assert.equal(state.time,1);
  state=(await api('/api/transport',{action:'ended',player_id:'test',revision:state.revision})).data;assert.equal(state.status,'paused');assert.equal(state.index,2);
  assert.equal((await api('/api/transport',{action:'seek',index:900})).status,400);
  await api('/api/transport',{action:'stop'});assert.equal(app.snapshot().time,0);
  const r=await fetch(`${app.url}/media/0?token=${app.token}&epoch=0`,{headers:{Range:'bytes=0-43'}});assert.equal(r.status,206);assert.equal((await r.arrayBuffer()).byteLength,44);
});

test('外部操作失敗と二重実行時に自動進行を止める',async t=>{
  let release;const app=await startServer({port:0,ppt:()=>new Promise((_,reject)=>{release=()=>reject(new Error('接続断'));})});t.after(()=>app.server.close());
  const post=async(path,body)=>{const r=await fetch(app.url+path,{method:'POST',headers:{'x-scriptsync-token':app.token},body:JSON.stringify(body)});return {code:r.status,value:await r.json()};};
  const plan=app.snapshot().plan;plan.powerpoint_enabled=true;plan.powerpoint_path='C:\\demo.pptx';plan.cues[0].slide_id=256;await post('/api/plan',{plan});
  const pending=post('/api/transport',{action:'play'});
  while(!release)await new Promise(r=>setTimeout(r,5));
  assert.equal((await post('/api/transport',{action:'next'})).code,400);release();assert.equal((await pending).code,400);assert.equal(app.snapshot().status,'paused');assert.equal(app.snapshot().index,0);
});

test('出力取込は音声hashを検証し、失敗時に現在の状態を保持する',async t=>{
  const dir=await mkdtemp(join(tmpdir(),'scriptsync-presentation-'));t.after(()=>rm(dir,{recursive:true,force:true}));
  const app=await startServer({port:0});t.after(()=>app.server.close());
  const map=demo();for(let i=0;i<3;i++){const data=demoWave(i);map.blocks[i].audio_hash=createHash('sha256').update(data).digest('hex');await writeFile(join(dir,map.blocks[i].audio_file),data);}
  await writeFile(join(dir,'block-map.json'),JSON.stringify(map));
  const load=()=>fetch(app.url+'/api/load',{method:'POST',headers:{'x-scriptsync-token':app.token},body:JSON.stringify({path:join(dir,'block-map.json')})});
  assert.equal((await load()).status,200);const epoch=app.snapshot().epoch;
  await writeFile(join(dir,'demo-0.wav'),'damaged');assert.equal((await load()).status,400);assert.equal(app.snapshot().epoch,epoch);
  assert.equal((await fetch(`${app.url}/media/0?token=${app.token}&epoch=${epoch}`)).status,400);
});

test('OBS v5の認証と要求応答を処理する',async t=>{
  const original=globalThis.WebSocket,requests=[];t.after(()=>{globalThis.WebSocket=original;});
  class FakeSocket extends EventTarget {
    readyState=1;
    constructor(){super();queueMicrotask(()=>this.receive({op:0,d:{authentication:{salt:'salt',challenge:'challenge'}}}));}
    receive(data){this.dispatchEvent(new MessageEvent('message',{data:JSON.stringify(data)}));}
    send(raw){const msg=JSON.parse(raw);requests.push(msg);queueMicrotask(()=>this.receive(msg.op===1?{op:2,d:{}}:{op:7,d:{requestId:msg.d.requestId,requestStatus:{result:true},responseData:{scenes:[{sceneName:'カメラ'}]}}}));}
    close(){this.readyState=3;}
  }
  globalThis.WebSocket=FakeSocket;const obs=new ObsClient();const result=await obs.connect('ws://127.0.0.1:4455','secret');
  const sha=s=>createHash('sha256').update(s).digest('base64');assert.equal(requests[0].d.authentication,sha(sha('secretsalt')+'challenge'));assert.equal(result.scenes[0].sceneName,'カメラ');
  await obs.scene('カメラ');assert.equal(requests.at(-1).d.requestType,'SetCurrentProgramScene');obs.disconnect();assert.equal(obs.connected,false);
  await assert.rejects(obs.connect('ws://example.com:4455'),/同じPC/);
});
