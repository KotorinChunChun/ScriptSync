import { escapeHtml as esc } from "./html";
type ArchiveSegment={id:string;text:string;source_id?:string;takes:{id:string;file:string;hash:string;status:string}[]};
type Context={project:{archived_segments?:ArchiveSegment[];deletion_records?:{segment_id:string;status:string}[]};rpc:(action:string,payload:unknown)=>Promise<unknown>;play:(file:string,hash:string)=>Promise<void>;refresh:()=>Promise<void>;notice:(text:string,error?:boolean)=>void};
function confirmArchive(segment:ArchiveSegment):Promise<boolean>{return new Promise(resolve=>{
 const overlay=document.createElement('div');overlay.className='delete-modal';overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');overlay.innerHTML=`<section><h2>廃棄済みの発話を完全消去しますか？</h2><p>${esc(segment.text||'空の発話')}</p><p>この発話の履歴と専用の音声を消去します。他の発話が使う音声は保持します。</p><div class="toolbar"><button data-cancel>キャンセル</button><button data-confirm class="danger">完全消去</button></div></section>`;
 const siblings=[...document.body.children] as HTMLElement[];siblings.forEach(n=>n.inert=true);document.body.append(overlay);
 const cancel=overlay.querySelector<HTMLButtonElement>('[data-cancel]')!,confirm=overlay.querySelector<HTMLButtonElement>('[data-confirm]')!;
 const close=(value:boolean)=>{overlay.remove();siblings.forEach(n=>n.inert=false);resolve(value);};cancel.onclick=()=>close(false);confirm.onclick=()=>close(true);overlay.onkeydown=event=>{if(event.key==='Escape'){event.preventDefault();close(false);}if(event.key==='Tab'){event.preventDefault();(document.activeElement===cancel?confirm:cancel).focus();}};cancel.focus();
});}
export function bindRecordingArchives(context:Context){
 document.getElementById('recording-archives')?.remove();
 const rows=context.project.archived_segments??[],pending=(context.project.deletion_records??[]).filter(r=>r.status==='pending');
 if(!rows.length&&!pending.length)return;
 const panel=document.createElement('details');panel.id='recording-archives';panel.className='panel';panel.innerHTML=`<summary>廃棄された行を表示（${rows.length}件）</summary>${rows.map(s=>`<article class="take"><p>${esc(s.text||'空の発話')}</p><div>${s.takes.filter(t=>t.status==='ready').map((t,i)=>`<button data-archive-play="${esc(t.id)}">旧テイク ${i+1} を試聴</button>`).join('')}<button class="danger" data-archive-purge="${esc(s.id)}">完全消去</button></div></article>`).join('')}${pending.map(r=>`<p>完全消去が未完了です。<button data-archive-retry="${esc(r.segment_id)}">消去を再実行</button></p>`).join('')}`;
 document.getElementById('stage-extras')?.append(panel);
 panel.querySelectorAll<HTMLButtonElement>('[data-archive-play]').forEach(b=>b.onclick=()=>{const take=rows.flatMap(s=>s.takes).find(t=>t.id===b.dataset.archivePlay);if(take)void context.play(take.file,take.hash).catch(e=>context.notice(String(e),true));});
 const purge=async(id:string)=>{try{await context.rpc('archive_purge',{segment:id,confirmed:true});await context.refresh();context.notice('廃棄済みの発話を完全消去しました');}catch(e){await context.refresh();context.notice(String(e),true);}};
 panel.querySelectorAll<HTMLButtonElement>('[data-archive-purge]').forEach(b=>b.onclick=()=>{const s=rows.find(s=>s.id===b.dataset.archivePurge);if(s)void confirmArchive(s).then(yes=>{if(yes)void purge(s.id);});});
 panel.querySelectorAll<HTMLButtonElement>('[data-archive-retry]').forEach(b=>b.onclick=()=>void purge(b.dataset.archiveRetry!));
}
