const $=id=>document.getElementById(id),token=document.querySelector('meta[name=scriptsync-token]').content;
const player=$('audio'),playerId=crypto.randomUUID();player.volume=.5;
let state=null,dirty=false,locked=false,applied=-1,audioKey='',polling=false,owned=false;
const secs=value=>`${Math.floor(value/60)}:${String(Math.floor(value%60)).padStart(2,'0')}`;
function message(text,error=false){$('message').textContent=text;$('message').classList.toggle('error',error);}
async function api(path,data){const r=await fetch('/api/'+path,{method:data?'POST':'GET',headers:{'X-ScriptSync-Token':token,'Content-Type':'application/json'},body:data?JSON.stringify(data):undefined});const value=await r.json();if(!r.ok)throw new Error(value.error||'通信に失敗しました');return value;}
function dirtyState(value){dirty=value;$('dirty').textContent=value?'未反映の変更があります':'反映済み';$('dirty').classList.toggle('changed',value);}
function mustBeSaved(){if(dirty)throw new Error('先に「設定を反映」または「編集を取り消す」を押してください');}
function populate(){
  if(!state)return;const cue=state.plan.cues[state.index];
  $('cue-title').value=cue.title;$('bullets').value=cue.bullets.join('\n');$('slide-id').value=cue.slide_id??'';$('slide-choice').value=String(cue.slide_id??'');$('scene').value=cue.scene;
  $('ppt-path').value=state.plan.powerpoint_path;$('ppt-enabled').checked=state.plan.powerpoint_enabled;$('obs-enabled').checked=state.plan.obs_enabled;$('deck-title').value=state.plan.title;$('mode').value=state.plan.mode;
}
function render(){
  const {map,plan,index,status}=state,cue=plan.cues[index];
  $('count').textContent=`${map.blocks.length} ブロック`;$('source-info').textContent=map.project_id==='demo'?'デモ：各4秒の短い確認音です。読み上げ音声ではありません。':`出力 ${map.export_id} ／ ${map.blocks.length} 音声（SHA-256照合済み）`;
  $('blocks').replaceChildren(...plan.cues.map((c,i)=>{const b=document.createElement('button');b.className='block'+(i===index?' selected':'');b.setAttribute('aria-pressed',String(i===index));const n=document.createElement('b');n.textContent=String(i+1).padStart(2,'0');const title=document.createElement('span');title.textContent=c.title;const details=document.createElement('small');details.textContent=`${secs(map.blocks[i].duration_seconds)} · ${c.scene||'シーン未指定'}`;b.append(n,title,details);b.onclick=()=>transport('seek',{index:i});return b;}));
  $('slide-title').textContent=cue.title;$('slide-bullets').replaceChildren(...cue.bullets.map(text=>{const li=document.createElement('li');li.textContent=text;return li;}));$('slide-kicker').textContent=`SCRIPT / ${String(index+1).padStart(2,'0')}`;$('slide-page').textContent=`${index+1} / ${map.blocks.length}`;
  $('script').textContent=map.blocks[index].text;$('cue-info').textContent=`PowerPoint：${plan.powerpoint_enabled?(cue.slide_id??'未指定'):'オフ'}　／　OBS：${plan.obs_enabled?(cue.scene||'未指定'):'オフ'}`;
  $('playing-status').textContent={playing:'再生中',paused:'一時停止',stopped:'停止'}[status];$('previous').disabled=index===0;$('next').disabled=index===map.blocks.length-1;$('seek').max=String(Math.max(0,map.blocks[index].duration_seconds-.05));
  $('obs-status').textContent=state.obs_connected?'OBSに接続済み':'未接続（同じPCのOBS WebSocket v5）';
  $('log').textContent=state.log.map(e=>`${new Date(e.at).toLocaleTimeString()}　${e.message}`).join('\n');
  if(!dirty)populate();
}
async function accept(next){
  if(state&&next.revision<state.revision)return;const changed=!state||next.revision!==state.revision;state=next;
  if(changed){render();if(state.error)message(state.error,true);}
  if(!changed||applied===next.revision)return;
  applied=next.revision;const currentRevision=applied;player.pause();
  const key=`${next.epoch}/${next.index}`;
  if(audioKey!==key){audioKey=key;player.src=`/media/${next.index}?token=${token}&epoch=${next.epoch}`;}
  const seek=()=>{if(applied===currentRevision)player.currentTime=next.time;};
  if(player.readyState>=1)seek();else player.addEventListener('loadedmetadata',seek,{once:true});
  if(next.status==='playing'&&owned){try{await player.play();if(applied!==currentRevision)player.pause();}catch{if(applied===currentRevision){await api('transport',{action:'failure',player_id:playerId,revision:currentRevision});message('再生を開始できません。再生ボタンを押してください。',true);}}}
}
async function run(work){if(locked)return;locked=true;document.body.setAttribute('aria-busy','true');try{await work();}catch(e){message(e.message,true);try{await accept(await api('state'));}catch{player.pause();}}finally{locked=false;document.body.removeAttribute('aria-busy');}}
async function claim(){await api('progress',{player_id:playerId,revision:applied,index:state?.index??0,time:player.currentTime||0});owned=true;}
function transport(action,extra={}){return run(async()=>{if(!['pause','stop'].includes(action))mustBeSaved();await claim();player.pause();await accept(await api('transport',{action,...extra}));if(!state.error)message('再生位置と切り替えを更新しました');});}
function editedPlan(){const p=structuredClone(state.plan),c=p.cues[state.index];c.title=$('cue-title').value;c.bullets=$('bullets').value.split(/\r?\n/).filter(Boolean);c.slide_id=$('slide-id').value?Number($('slide-id').value):null;c.scene=$('scene').value;p.title=$('deck-title').value;p.powerpoint_path=$('ppt-path').value;p.powerpoint_enabled=$('ppt-enabled').checked;p.obs_enabled=$('obs-enabled').checked;p.mode=$('mode').value;return p;}
async function save(){const next=await api('plan',{plan:editedPlan()});dirtyState(false);await accept(next);message('設定を反映しました。対応表を保存すると次回も使えます。');}
for(const id of ['cue-title','bullets','slide-id','scene','ppt-path','ppt-enabled','obs-enabled','deck-title','mode'])$(id).addEventListener('input',()=>dirtyState(true));
$('slide-choice').onchange=()=>{$('slide-id').value=$('slide-choice').value;dirtyState(true);};
$('apply').onclick=()=>run(save);$('discard').onclick=()=>{dirtyState(false);populate();message('未反映の編集を取り消しました');};
for(const action of ['play','pause','stop','previous','next'])$(action).onclick=()=>transport(action);
$('seek').onchange=()=>transport('seek',{index:state.index,time:Number($('seek').value)});
$('volume').oninput=()=>{player.volume=Number($('volume').value);};
$('load').onclick=()=>run(async()=>{mustBeSaved();player.pause();await api('transport',{action:'stop'});await accept(await api('load',{path:$('map-path').value.trim().replace(/^"|"$/g,'')}));message('音声出力を読み込みました');});
$('demo').onclick=()=>run(async()=>{mustBeSaved();player.pause();await api('transport',{action:'stop'});await accept(await api('demo',{}));message('デモを読み込みました。まず再生を試してください。');});
$('ppt-open').onclick=()=>run(async()=>{const result=await api('powerpoint/open',{path:$('ppt-path').value});$('ppt-path').value=result.path;const empty=document.createElement('option');empty.value='';empty.textContent='未指定';$('slide-choice').replaceChildren(empty,...result.slides.map(s=>{const o=document.createElement('option');o.value=String(s.id);o.textContent=`${s.index}: ${s.title}`;return o;}));dirtyState(true);message(`${result.slides.length}枚のスライドを取得しました。対応を選んで設定を反映してください。`);});
$('obs-connect').onclick=()=>run(async()=>{try{const result=await api('obs/connect',{url:$('obs-url').value,password:$('obs-password').value});$('scene-list').replaceChildren(...result.scenes.map(s=>{const o=document.createElement('option');o.value=s.sceneName;return o;}));$('obs-status').textContent='OBSに接続済み';message('OBSへ接続しました。シーンを選んで設定を反映してください。');}finally{$('obs-password').value='';}});
$('obs-disconnect').onclick=()=>run(async()=>{await accept(await api('obs/disconnect',{}));$('obs-status').textContent='OBSを切断しました';});
function download(data,name,type){const url=URL.createObjectURL(new Blob([data],{type})),a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),10000);}
$('plan-export').onclick=()=>run(async()=>{mustBeSaved();download(JSON.stringify(state.plan,null,2),'presentation-plan.json','application/json');message('対応表を保存しました');});
$('plan-import').onchange=()=>run(async()=>{mustBeSaved();const file=$('plan-import').files[0];if(!file)return;if(file.size>2*1024*1024)throw new Error('対応表が大きすぎます');const plan=JSON.parse(await file.text());plan.powerpoint_enabled=false;plan.obs_enabled=false;await accept(await api('plan',{plan}));$('plan-import').value='';message('対応表を取り込みました。外部連携はオフで読み込みました。');});
$('deck').onclick=()=>run(async()=>{mustBeSaved();const response=await fetch('/api/deck',{headers:{'X-ScriptSync-Token':token}});if(!response.ok)throw new Error('PPTXを生成できません');download(await response.arrayBuffer(),'ScriptSync-slides.pptx','application/vnd.openxmlformats-officedocument.presentationml.presentation');message('表紙と各ブロックのPPTXを生成しました');});
$('api-token').textContent=token;
player.ontimeupdate=()=>{if(!state)return;$('time').textContent=`${secs(player.currentTime)} / ${secs(state.map.blocks[state.index].duration_seconds)}`;if(document.activeElement!==$('seek'))$('seek').value=String(player.currentTime);};
player.onended=()=>run(async()=>{if(!state||state.status!=='playing')return;await accept(await api('transport',{action:'ended',player_id:playerId,revision:applied}));});
player.onerror=()=>{if(state?.status==='playing')void run(async()=>{await accept(await api('transport',{action:'failure',player_id:playerId,revision:applied}));message('音声を読み込めません。出力ファイルを確認してください。',true);});};
window.addEventListener('beforeunload',event=>{if(dirty){event.preventDefault();event.returnValue='';}});
async function poll(){if(polling||locked)return;polling=true;try{await claim();await accept(await api('state'));}catch(e){owned=false;player.pause();message(e.message,true);}finally{polling=false;}}
await poll();if(state)message('デモを用意しました。再生、次へ、自動進行を試せます。');setInterval(poll,500);
