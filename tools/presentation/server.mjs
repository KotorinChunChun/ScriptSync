import http from 'node:http';
import { readFile, realpath, stat } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { dirname, resolve, basename } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { randomBytes, createHash } from 'node:crypto';
import { ensure, validateMap, makePlan, validatePlan, demo, demoWave } from './model.mjs';
import { createDeck } from './pptx.mjs';
import { ObsClient, powerpoint } from './adapters.mjs';

const root=dirname(fileURLToPath(import.meta.url));
async function body(req) {let bytes=0,chunks=[];for await(const chunk of req){bytes+=chunk.length;ensure(bytes<=2*1024*1024,'リクエストが大きすぎます');chunks.push(chunk);}return JSON.parse(Buffer.concat(chunks).toString()||'{}');}
async function hashFile(path){const h=createHash('sha256');for await(const chunk of createReadStream(path))h.update(chunk);return h.digest('hex');}
export async function startServer({port=4317,host='127.0.0.1',ppt=powerpoint,obs=new ObsClient()}={}) {
  ensure(host==='127.0.0.1','ローカル接続専用です');
  const token=randomBytes(24).toString('hex');
  let map=demo(),plan=makePlan(map),media=null,epoch=0,revision=0,index=0,time=0,status='stopped',error='',log=[],busy=false,lastProgress=0,playerId=null;
  const snapshot=()=>({epoch,revision,index,time,status,error,busy,obs_connected:obs.connected,plan,map,log});
  const record=message=>{log.push({at:new Date().toISOString(),message});log=log.slice(-30);};
  const reset=()=>{index=0;time=0;status='stopped';error='';epoch++;revision++;};
  async function applyCue(i) {
    const cue=plan.cues[i];
    if(plan.powerpoint_enabled&&cue.slide_id!==null){ensure(plan.powerpoint_path,'PowerPointファイルを指定してください');await ppt('goto',plan.powerpoint_path,cue.slide_id);record(`PowerPoint: スライドID ${cue.slide_id}`);}
    if(plan.obs_enabled&&cue.scene){await obs.scene(cue.scene);record(`OBS: ${cue.scene}`);}
    record(`ブロック ${i+1}: ${cue.title}`);
  }
  const server=http.createServer(async(req,res)=>{
    res.setHeader('Cache-Control','no-store');res.setHeader('X-Content-Type-Options','nosniff');res.setHeader('Referrer-Policy','no-referrer');
    res.setHeader('Content-Security-Policy',"default-src 'self'; script-src 'self'; style-src 'self'; media-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'");
    const json=(data,code=200)=>{res.writeHead(code,{'Content-Type':'application/json; charset=utf-8'});res.end(JSON.stringify(data));};
    let ownsMutation=false;
    try {
      const expected=`127.0.0.1:${server.address().port}`;
      ensure(req.headers.host===expected,'接続先Hostが不正です');
      ensure(!req.headers.origin||req.headers.origin===`http://${expected}`,'別サイトからの操作は許可しません');
      ensure(!req.headers['sec-fetch-site']||['same-origin','none'].includes(req.headers['sec-fetch-site']),'別サイトからのアクセスは許可しません');
      const url=new URL(req.url,`http://${expected}`),path=url.pathname;
      if(req.method==='GET'&&['/','/app.mjs','/style.css'].includes(path)) {
        const file=path==='/'?'index.html':path.slice(1);let data=await readFile(resolve(root,'web',file));
        if(path==='/')data=Buffer.from(data.toString().replace('__TOKEN__',token));
        res.writeHead(200,{'Content-Type':path==='/'?'text/html; charset=utf-8':path.endsWith('css')?'text/css':'text/javascript'});res.end(data);return;
      }
      ensure(req.headers['x-scriptsync-token']===token || (path.startsWith('/media/')&&url.searchParams.get('token')===token),'セッショントークンが必要です');
      if(req.method==='GET'&&path==='/api/state'){json(snapshot());return;}
      if(req.method==='GET'&&path==='/api/deck'){
        const data=createDeck(plan,map);res.writeHead(200,{'Content-Type':'application/vnd.openxmlformats-officedocument.presentationml.presentation','Content-Disposition':'attachment; filename="ScriptSync-slides.pptx"'});res.end(data);return;
      }
      if(req.method==='GET'&&path.startsWith('/media/')) {
        const n=Number(path.slice(7));ensure(Number.isInteger(n)&&n>=0&&n<map.blocks.length,'音声がありません');
        ensure(Number(url.searchParams.get('epoch'))===epoch,'出力が変更されました');
        const data=media?null:demoWave(n),file=media?.[n];
        if(file){const current=await realpath(file.path);ensure(current===file.path,'音声参照先が変わりました');const info=await stat(current);ensure(info.size===file.size&&info.mtimeMs===file.mtime,'音声ファイルが変更されました。再取込してください');}
        const size=data?.length??file.size;let start=0,end=size-1;
        if(req.headers.range){const match=/^bytes=(\d+)-(\d*)$/.exec(req.headers.range);ensure(match,'未対応のRangeです');start=Number(match[1]);end=match[2]?Math.min(Number(match[2]),end):end;ensure(start<=end&&start>=0&&start<size,'音声範囲が不正です');res.statusCode=206;res.setHeader('Content-Range',`bytes ${start}-${end}/${size}`);}
        res.setHeader('Accept-Ranges','bytes');res.setHeader('Content-Type',map.blocks[n].audio_file.endsWith('.flac')?'audio/flac':'audio/wav');res.setHeader('Content-Length',end-start+1);
        if(data)res.end(data.subarray(start,end+1));else{const stream=createReadStream(file.path,{start,end});stream.on('error',()=>res.destroy());stream.pipe(res);}return;
      }
      ensure(req.method==='POST','未対応のAPIです');
      const request=await body(req);
      if(path==='/api/progress') {
        ensure(typeof request.player_id==='string'&&request.player_id.length<=100,'再生画面IDが必要です');
        if(playerId && playerId!==request.player_id && Date.now()-lastProgress<3000){json({error:'別の画面が再生を担当しています'},409);return;}
        playerId=request.player_id;lastProgress=Date.now();
        if(request.revision===revision&&request.index===index&&Number.isFinite(request.time)&&request.time>=0)time=Math.min(request.time,map.blocks[index].duration_seconds);
        json({ok:true});return;
      }
      ensure(!busy,'前の操作が完了するまでお待ちください');busy=true;ownsMutation=true;
      if(path==='/api/demo'){ensure(status!=='playing','停止してからデモを読み込んでください');map=demo();plan=makePlan(map);media=null;reset();record('デモを読み込みました');}
      else if(path==='/api/load') {
        ensure(status!=='playing','停止してから音声出力を読み込んでください');
        ensure(typeof request.path==='string'&&basename(request.path)==='block-map.json','block-map.jsonの絶対パスを指定してください');
        const mapPath=await realpath(request.path),dir=dirname(mapPath),info=await stat(mapPath);ensure(info.size<=2*1024*1024,'マップが大きすぎます');
        const next=validateMap(JSON.parse(await readFile(mapPath,'utf8'))),files=[];
        for(const block of next.blocks){const path=await realpath(resolve(dir,block.audio_file));ensure(dirname(path)===dir,'出力フォルダー外の音声は読めません');const s=await stat(path);ensure(s.isFile(),'音声ファイルではありません');ensure(await hashFile(path)===block.audio_hash,`${block.audio_file}のハッシュが一致しません`);files.push({path,size:s.size,mtime:s.mtimeMs});}
        map=next;media=files;plan=makePlan(map);reset();record('音声出力を読み込みました');
      }
      else if(path==='/api/plan'){ensure(status!=='playing','一時停止してから対応表を変更してください');plan=validatePlan(request.plan,map);revision++;record('対応表を反映しました');}
      else if(path==='/api/powerpoint/open'){ensure(status!=='playing','停止してから開いてください');const result=await ppt('open',request.path);busy=false;json(result);return;}
      else if(path==='/api/obs/connect'){ensure(status!=='playing','停止してから接続してください');const result=await obs.connect(request.url,request.password);record('OBSへ接続しました');busy=false;json(result);return;}
      else if(path==='/api/obs/disconnect'){obs.disconnect();record('OBSを切断しました');}
      else if(path==='/api/transport') {
        const action=request.action;ensure(['play','pause','stop','next','previous','seek','ended','failure'].includes(action),'未対応の再生操作です');
        if(action==='ended'||action==='failure'){ensure(request.player_id===playerId,'再生担当の画面から操作してください');if(request.revision!==revision){busy=false;json(snapshot());return;}}
        if(action==='pause'){status='paused';revision++;}
        else if(action==='stop'){status='stopped';time=0;revision++;}
        else if(action==='failure'){status='paused';error='音声を再生できません。画面の再生ボタンから再試行してください';revision++;}
        else if(action==='ended'&&(plan.mode==='manual'||index===map.blocks.length-1)){status='paused';time=0;revision++;}
        else {
          const target=action==='next'||action==='ended'?Math.min(index+1,map.blocks.length-1):action==='previous'?Math.max(0,index-1):action==='seek'?request.index:index;
          ensure(Number.isInteger(target)&&target>=0&&target<map.blocks.length,'ブロック番号が不正です');
          const position=action==='play'?time:action==='seek'?(request.time??0):0;
          ensure(Number.isFinite(position)&&position>=0&&position<map.blocks[target].duration_seconds,'再生位置が不正です');
          // 移動先を確定してから再生する。外部側で失敗したら自動進行を止める。
          await applyCue(target);index=target;time=position;status=action==='play'||action==='ended'||status==='playing'?'playing':'paused';error='';revision++;
        }
      } else throw new Error('未対応のAPIです');
      busy=false;json(snapshot());
    } catch(e) {
      if(ownsMutation){busy=false;if(req.url?.startsWith('/api/transport')){status='paused';revision++;}error=e.message;record(e.message);}
      if(!res.headersSent)json({error:e.message},400);else res.destroy();
    }
  });
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,host,resolve);});
  const watchdog=setInterval(()=>{if(status==='playing'&&Date.now()-lastProgress>3000){status='paused';revision++;error='再生画面との通信が途切れたため停止しました';record(error);}},1000);watchdog.unref();
  server.on('close',()=>{clearInterval(watchdog);obs.disconnect();});
  return {server,token,url:`http://${host}:${server.address().port}`,snapshot};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const {url}=await startServer({port:Number(process.env.SCRIPTSYNC_PRESENTATION_PORT||4317)});
  console.log(`ScriptSync プレゼン連携試作: ${url}\n終了: Ctrl+C。ブラウザーで開いてください。`);
}
