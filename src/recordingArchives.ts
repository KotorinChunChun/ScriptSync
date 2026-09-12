import {escapeHtml as esc} from "./html";
type ArchiveSegment={id:string;text:string;kana?:string;source_id?:string;takes:{id:string;file:string;hash:string;status:string}[]};
type ArchiveSource={id:string;name:string;confirmed_text:string;draft_text?:string};
type ArchiveProject={id?:string;generation?:number;archived_segments?:ArchiveSegment[];script_sources?:ArchiveSource[];archived_sources?:ArchiveSource[];deletion_records?:{segment_id?:string;source_id?:string;status:string}[]};
type Context={project:ArchiveProject;getProject?:()=>ArchiveProject;rpc:(action:string,payload:unknown)=>Promise<unknown>;play:(file:string,hash:string)=>Promise<void>;stop?:()=>void;refresh:()=>Promise<void>;notice:(text:string,error?:boolean)=>void};
function trap(event:KeyboardEvent,element:HTMLElement){
 if(event.key!=='Tab')return;
 const buttons=[...element.querySelectorAll<HTMLElement>('button:not(:disabled),[tabindex="0"]')];
 const index=buttons.indexOf(document.activeElement as HTMLElement);
 if(!buttons.length)return;
 if(event.shiftKey&&index<=0){event.preventDefault();buttons.at(-1)!.focus();}
 else if(!event.shiftKey&&index===buttons.length-1){event.preventDefault();buttons[0].focus();}
}
async function confirmArchive(label:string):Promise<boolean>{
 return new Promise(resolve=>{
  const overlay=document.createElement('div');overlay.className='delete-modal';overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');overlay.setAttribute('aria-label','完全消去の確認');
  overlay.innerHTML=`<section><h2>アーカイブから完全消去しますか？</h2><p>${esc(label)}</p><p>本プロジェクト内の原稿・読み・履歴と専用音声を消去します。元ファイルは残します。他の発話も使う音声は保持します。</p><div class="toolbar"><button data-cancel>キャンセル</button><button data-confirm class="danger">完全消去</button></div></section>`;
  const previous=document.activeElement as HTMLElement|null;
  const siblings=[...document.body.children] as HTMLElement[];const states=siblings.map(n=>n.inert);
  siblings.forEach(n=>n.inert=true);document.body.append(overlay);
  const close=(value:boolean)=>{overlay.remove();siblings.forEach((n,i)=>n.inert=states[i]);previous?.focus();resolve(value);};
  overlay.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=()=>close(false);
  overlay.querySelector<HTMLButtonElement>('[data-confirm]')!.onclick=()=>close(true);
  overlay.onkeydown=event=>{if(event.key==='Escape'){event.preventDefault();close(false);}trap(event,overlay);};
  overlay.querySelector<HTMLButtonElement>('[data-cancel]')!.focus();
 });
}
export function openArchiveManager(context:Context){
 if(document.getElementById('archive-manager'))return;
 const overlay=document.createElement('div');overlay.id='archive-manager';overlay.className='archive-manager';overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');overlay.setAttribute('aria-label','アーカイブ管理');
 const previous=document.activeElement as HTMLElement|null;
 const siblings=[...document.body.children] as HTMLElement[];const states=siblings.map(n=>n.inert);
 siblings.forEach(n=>n.inert=true);document.body.append(overlay);
 let running=false,message='';
 const close=()=>{if(running)return;context.stop?.();overlay.remove();siblings.forEach((n,i)=>n.inert=states[i]);previous?.focus();};
 const draw=()=>{
  const p=context.getProject?.()??context.project,rows=p.archived_segments??[],sources=p.archived_sources??[];
  const allSources=[...(p.script_sources??[]),...sources];
  const groups=[...new Set([...sources.map(s=>s.id),...rows.map(s=>s.source_id??'')])];
  const pending=(p.deletion_records??[]).filter(r=>r.status==='pending');
  overlay.innerHTML=`<section class="archive-dialog"><div class="section-head"><h2>アーカイブ管理</h2><button data-close>閉じる</button></div><p>現在の読み上げ対象につながっていない原稿・行です。完全消去すると元に戻せません。</p><p role="status">${esc(message)}</p><div class="archive-content">${groups.map(sourceId=>{
   const source=allSources.find(s=>s.id===sourceId),deleted=sources.some(s=>s.id===sourceId);
   return `<section class="archive-group"><div class="section-head"><h3>${esc(source?.name??'以前の原稿')}${deleted?'（削除済み）':''}</h3>${deleted?`<button class="danger" data-purge-source="${esc(sourceId)}">原稿全体を完全消去</button>`:''}</div>${deleted&&source?`<pre>${esc(source.draft_text??source.confirmed_text)}</pre>`:''}${rows.filter(s=>(s.source_id??'')===sourceId).map(s=>`<article><p>${esc(s.text||'空の発話')}</p>${s.kana?`<p class="muted">読み：${esc(s.kana)}</p>`:''}<div class="toolbar">${s.takes.filter(t=>t.status==='ready').map((t,i)=>`<button data-archive-play="${esc(t.id)}">テイク ${i+1} を試聴</button>`).join('')}<button class="danger" data-purge-segment="${esc(s.id)}">この行を完全消去</button></div></article>`).join('')}</section>`;
  }).join('')||'<p>アーカイブはありません。</p>'}${pending.map(r=>`<p>完全消去が未完了です。<button data-retry-id="${esc(r.source_id??r.segment_id??'')}" data-retry-source="${!!r.source_id}">消去を再実行</button></p>`).join('')}</div><button data-stop>試聴を停止</button></section>`;
  overlay.querySelector<HTMLButtonElement>('[data-close]')!.onclick=close;
  overlay.querySelector<HTMLButtonElement>('[data-stop]')!.onclick=()=>context.stop?.();
  overlay.querySelectorAll<HTMLButtonElement>('[data-archive-play]').forEach(button=>button.onclick=()=>{const take=rows.flatMap(s=>s.takes).find(t=>t.id===button.dataset.archivePlay);if(take)void context.play(take.file,take.hash).catch(error=>{message=String(error);draw();});});
  const purge=async(id:string,isSource:boolean,retry=false)=>{
   if(running)return;
   const label=isSource?allSources.find(s=>s.id===id)?.name:rows.find(s=>s.id===id)?.text;
   if(!retry&&!await confirmArchive(label??'消去未完了のデータ'))return;
   running=true;overlay.querySelectorAll<HTMLButtonElement>('button').forEach(button=>button.disabled=true);
   try{
    context.stop?.();
    const result=await context.rpc('archive_purge',{project_id:p.id,base_generation:p.generation,...(isSource?{source_id:id}:{segment:id}),confirmed:true}) as {shared_files?:number}|null;
    message=`完全消去しました。${result?.shared_files?`共有音声等 ${result.shared_files}件は保持しました。`:''}`;
   }catch(error){message=String(error);}
   finally{try{await context.refresh();}catch(error){message+=' '+String(error);}running=false;draw();overlay.querySelector<HTMLButtonElement>('[data-close]')?.focus();}
  };
  overlay.querySelectorAll<HTMLButtonElement>('[data-purge-source]').forEach(b=>b.onclick=()=>void purge(b.dataset.purgeSource!,true));
  overlay.querySelectorAll<HTMLButtonElement>('[data-purge-segment]').forEach(b=>b.onclick=()=>void purge(b.dataset.purgeSegment!,false));
  overlay.querySelectorAll<HTMLButtonElement>('[data-retry-id]').forEach(b=>b.onclick=()=>void purge(b.dataset.retryId!,b.dataset.retrySource==='true',true));
 };
 overlay.onkeydown=event=>{if(event.key==='Escape'){event.preventDefault();close();}trap(event,overlay);};
 draw();overlay.querySelector<HTMLButtonElement>('[data-close]')!.focus();
}
export function bindRecordingArchives(context:Context){
 document.getElementById('recording-archives')?.remove();
 const button=document.createElement('button');button.id='recording-archives';button.textContent='アーカイブ管理';
 button.onclick=()=>openArchiveManager(context);
 document.getElementById('stage-extras')?.append(button);
}
