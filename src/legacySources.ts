type Line={line:number;text:string;enabled:boolean};
type Source={id:string;name:string;confirmed_text:string;draft_text:string};
type Segment={id:string;source_id:string;source_text:string;text:string};
type Imported={name:string;text:string;lines:Line[]};
type Context={source:Source;segments:Segment[];rpc:<T>(action:string,payload?:unknown)=>Promise<T>;flush:()=>Promise<void>;base:()=>{base_revision:number;base_generation:number};refresh:()=>Promise<void>;notice:(message:string,error?:boolean)=>void};
const escape=(value:string)=>value.replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));

/** ファイル分割の候補を利用者が確認してから、録音の所属だけを移す。 */
export function bindLegacySources(container:HTMLElement,context:Context){
 if(context.source.name!=='既存台本')return;
 const button=document.createElement('button');button.textContent='元ファイルを対応付け';container.append(button);
 button.onclick=()=>void (async()=>{
  try{
   await context.flush();
   const result=await context.rpc<{files:Imported[]}|null>('markdown_import');if(!result)return;
   const files=result.files;
   if(files.map(f=>f.text).join('\n\n')!==context.source.confirmed_text)throw new Error('元ファイルの本文の連結が既存台本と完全一致しません。確定時の元ファイルを選択してください。');
   const segments=context.segments.filter(s=>s.source_id===context.source.id);
   const candidates=files.flatMap((file,fileIndex)=>file.lines.map(line=>({fileIndex,...line})));
   const overlay=document.createElement('div');overlay.className='delete-modal';overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');
   overlay.innerHTML=`<section style="max-width:850px;max-height:85vh;overflow:auto"><h2>既存台本と元ファイルの対応</h2><p>本文は変更せず、以下のファイル順で録音の所属を移します。</p><ol>${files.map(file=>`<li>${escape(file.name)}</li>`).join('')}</ol>${segments.map(segment=>`<label style="display:block;margin:12px 0">${escape(segment.text||'空の発話')}<select data-legacy-segment="${escape(segment.id)}"><option value="">対応先を選択</option>${candidates.map((line,index)=>line.text===segment.source_text?`<option value="${index}">${escape(files[line.fileIndex].name)} ${line.line+1}行: ${escape(line.text)}</option>`:'').join('')}</select></label>`).join('')}<p data-error role="alert"></p><div class="toolbar"><button data-cancel>キャンセル</button><button data-confirm class="primary">対応を確認して登録</button></div></section>`;
   const siblings=[...document.body.children] as HTMLElement[];siblings.forEach(n=>n.inert=true);document.body.append(overlay);
   const close=()=>{overlay.remove();siblings.forEach(n=>n.inert=false);};
   const cancel=overlay.querySelector<HTMLButtonElement>('[data-cancel]')!,confirm=overlay.querySelector<HTMLButtonElement>('[data-confirm]')!;
   cancel.onclick=close;overlay.onkeydown=event=>{if(event.key==='Escape'){event.preventDefault();close();}if(event.key==='Tab'){const controls=[...overlay.querySelectorAll<HTMLElement>('button,select')];if(event.shiftKey&&document.activeElement===controls[0]){event.preventDefault();controls.at(-1)!.focus();}else if(!event.shiftKey&&document.activeElement===controls.at(-1)){event.preventDefault();controls[0].focus();}}};
   // 一意な同文行だけを候補にし、確定は利用者の確認ボタンで行う。
   overlay.querySelectorAll<HTMLSelectElement>('select').forEach(select=>{if(select.options.length===2)select.selectedIndex=1;});
   confirm.onclick=()=>void(async()=>{
    try{
     const sources=files.map(file=>({name:file.name,kind:'markdown',text:file.text,lines:[] as {line:number;enabled:boolean;previous_id:string}[]}));
     for(const select of overlay.querySelectorAll<HTMLSelectElement>('select')){
      if(select.value==='')throw new Error('全ての発話の対応先を選択してください。');
      const line=candidates[Number(select.value)];sources[line.fileIndex].lines.push({line:line.line,enabled:line.enabled,previous_id:select.dataset.legacySegment!});
     }
     confirm.disabled=true;await context.rpc('script_rebind_legacy',{...context.base(),legacy_source_id:context.source.id,sources});close();await context.refresh();context.notice('元ファイルへの対応付けを保存しました');
    }catch(error){confirm.disabled=false;overlay.querySelector<HTMLElement>('[data-error]')!.textContent=String(error);}
   })();cancel.focus();
  }catch(error){context.notice(String(error),true);}
 })();
}
