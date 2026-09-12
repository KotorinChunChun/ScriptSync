/** 登録済み試聴だけをWindows再生へ接続する。HTML側は無音の操作・波形時計。 */
type Rpc=<T>(action:string,payload?:unknown)=>Promise<T>;
type Source={file:string;hash:string};
type Snapshot={playing:boolean;position:number;duration:number;file:string;error?:string|null};
const sources=new Map<string,Source>();
let nativeRpc:Rpc|null=null;
let nativeNotice:((message:string,error?:boolean)=>void)|null=null;
const cueSamples=new Map<string,Promise<number[]>>();
export function hasNativeAudio(){return nativeRpc!==null;}
/** 復号のみをWeb Audioで行い、出力は独立したWASAPIプレーヤーへ渡す。 */
export async function playNativeCue(url:string,volume:number):Promise<void>{
 if(!nativeRpc)return;
 try{
  let decoded=cueSamples.get(url);
  if(!decoded){
   decoded=(async()=>{
    const response=await fetch(url);if(!response.ok)throw new Error('効果音を読み込めません');
    const context=new OfflineAudioContext(2,1,48000);
    const buffer=await context.decodeAudioData(await response.arrayBuffer());
    const left=buffer.getChannelData(0),right=buffer.getChannelData(buffer.numberOfChannels>1?1:0);
    const samples:number[]=[];for(let i=0;i<buffer.length;i++){samples.push(Math.max(-1,Math.min(1,left[i])),Math.max(-1,Math.min(1,right[i])));}
    return samples;
   })();cueSamples.set(url,decoded);
  }
  await nativeRpc('playback_cue',{samples:await decoded,volume});
  for(let i=0;i<60;i++){
   await new Promise(resolve=>setTimeout(resolve,100));
   const state=await nativeRpc<Snapshot>('playback_cue_state');
   if(state.error)throw new Error(state.error);
   if(!state.playing)return;
  }
 }catch(error){cueSamples.delete(url);nativeNotice?.(String(error),true);}
}
let installed=false;
function key(url:string){try{return new URL(url,document.baseURI).href;}catch{return url;}}
export function registerNativeAudio(url:string,file:string,hash:string){sources.set(key(url),{file,hash});}

export function installNativeAudio(rpc:Rpc,notice:(message:string,error?:boolean)=>void){
 if(installed)return;installed=true;
 nativeRpc=rpc;nativeNotice=notice;
 const originalPlay=HTMLMediaElement.prototype.play,originalPause=HTMLMediaElement.prototype.pause;
 let active:HTMLMediaElement|null=null,generation=0,queue:Promise<unknown>=Promise.resolve(),polling=false;
 const bound=new WeakSet<HTMLMediaElement>(),synchronizing=new WeakMap<HTMLMediaElement,number>();
 const run=<T>(operation:()=>Promise<T>):Promise<T>=>{const result=queue.then(operation);queue=result.catch(()=>{});return result;};
 const report=(error:unknown)=>notice(String(error),true);
 const clock=(audio:HTMLMediaElement,position:number)=>{if(Number.isFinite(position)&&Math.abs(audio.currentTime-position)>.15){synchronizing.set(audio,position);audio.currentTime=position;}};
 const bind=(audio:HTMLMediaElement)=>{
  if(bound.has(audio))return;bound.add(audio);
  audio.addEventListener("pause",()=>{
   if(active!==audio||!audio.paused||audio.ended)return;
   const version=++generation;
   void run(async()=>{if(active!==audio||version!==generation)return;const state=await rpc<Snapshot>("playback_pause");clock(audio,state.position);}).catch(report);
  });
  audio.addEventListener("seeking",()=>{
   const expected=synchronizing.get(audio);
   if(expected!==undefined){synchronizing.delete(audio);if(Math.abs(expected-audio.currentTime)<.03)return;}
   if(active!==audio)return;
   const position=audio.currentTime,version=generation;
   void run(async()=>{if(active!==audio||version!==generation)return;await rpc("playback_seek",{position});}).catch(report);
  });
  audio.addEventListener("volumechange",()=>{
   // HTML音声とWASAPI音声を二重に再生しない。
   if(!audio.muted){audio.muted=true;return;}
   if(active===audio)void run(()=>rpc("playback_volume",{volume:audio.volume})).catch(report);
  });
 };
 HTMLMediaElement.prototype.play=function():Promise<void>{
  // src切替直後のcurrentSrcには前のロード対象が残ることがある。
  const audio=this,source=sources.get(key(audio.src||audio.currentSrc));
  if(!source)return originalPlay.call(audio);
  bind(audio);audio.muted=true;
  if(active&&active!==audio)originalPause.call(active);
  active=audio;const version=++generation;
  return run(async()=>{
   if(active!==audio||version!==generation)return;
   try{
    const start=audio.ended?0:audio.currentTime||0;
    await rpc<Snapshot>("playback_play",{...source,start});
    await rpc("playback_volume",{volume:audio.volume});
    if(active!==audio||version!==generation){await rpc("playback_pause");return;}
    await originalPlay.call(audio);
   }catch(error){originalPause.call(audio);await rpc("playback_stop").catch(()=>{});throw error;}
  });
 };
 setInterval(()=>{
  if(!active||polling)return;const audio=active,version=generation;polling=true;
  void run(async()=>{
   if(active!==audio||version!==generation)return;
   const state=await rpc<Snapshot>("playback_state");
   if(active!==audio||version!==generation)return;
   if(state.error){originalPause.call(audio);active=null;report(state.error);return;}
   if(state.playing){clock(audio,state.position);return;}
   if(!audio.paused){clock(audio,state.position);originalPause.call(audio);}
  }).catch(report).finally(()=>{polling=false;});
 },100);
}
