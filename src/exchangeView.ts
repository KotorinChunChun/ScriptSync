import {escapeHtml as esc} from './html';
import type {Segment} from './models';
import {bindBackdropCancel} from './popupDialog';
export type ExchangePreview={token:string;manifest:{source:string;sourceFileName?:string;lines:{order:number;text:string;selected:string|null;takes:{id:string}[]}[]}};
export async function chooseExchange(rpc:<T>(action:string,payload?:unknown)=>Promise<T>):Promise<ExchangePreview|null>{
 const password=await passwordDialog();if(password===null)return null;
 return rpc<ExchangePreview|null>('exchange_inspect',{password:password||null});
}
function passwordDialog():Promise<string|null>{return new Promise(resolve=>{
 const dialog=document.createElement('dialog');dialog.className='action-confirm';dialog.innerHTML='<h2>WebのZIPを取り込む</h2><label>暗号化キー（非暗号化なら空欄）<input type="password" autocomplete="off"></label><div class="toolbar popup-actions"><button data-cancel>キャンセル</button><button data-open class="primary">ZIPを選択</button></div>';
 const finish=(value:string|null)=>{dialog.remove();resolve(value);};dialog.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=()=>finish(null);dialog.querySelector<HTMLButtonElement>('[data-open]')!.onclick=()=>finish(dialog.querySelector('input')!.value);dialog.oncancel=e=>{e.preventDefault();finish(null);};bindBackdropCancel(dialog,()=>finish(null));document.body.append(dialog);dialog.showModal();
});}
export function mapExchange(preview:ExchangePreview,segments:Segment[]):Promise<{order:number;segment_id:string}[]|null>{return new Promise(resolve=>{
 const rows=preview.manifest.lines.filter(l=>l.takes.length);if(!rows.length){resolve([]);return;}
 const dialog=document.createElement('dialog');dialog.className='action-confirm exchange-map';dialog.style.maxWidth='850px';
 const proposed=new Map<number,string>();const used=new Set<string>();
 for(const row of rows){const matches=segments.filter(s=>s.text.trim()===row.text.trim());if(matches.length===1&&!used.has(matches[0].id)){proposed.set(row.order,matches[0].id);used.add(matches[0].id);}}
 dialog.innerHTML=`<h2>録音を追加する発話を確認</h2><p>既存の採用は保持します。同じ音声は重複追加しません。</p><div style="max-height:55vh;overflow:auto">${rows.map(row=>`<label style="display:block;margin:12px 0">${row.order}. ${esc(row.text)}（${row.takes.length}テイク${row.selected?'・Web採用あり':''}）<select data-order="${row.order}"><option value="">対応先を選択</option>${segments.map((s,i)=>`<option value="${esc(s.id)}" ${proposed.get(row.order)===s.id?'selected':''}>${i+1}. ${esc(s.text)}</option>`).join('')}</select></label>`).join('')}</div><p data-error role="alert"></p><div class="toolbar popup-actions"><button data-cancel>後で追加</button><button data-merge class="primary">候補へ追加</button></div>`;
 const finish=(value:{order:number;segment_id:string}[]|null)=>{dialog.remove();resolve(value);};dialog.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=()=>finish(null);dialog.oncancel=e=>{e.preventDefault();finish(null);};bindBackdropCancel(dialog,()=>finish(null));
 dialog.querySelector<HTMLButtonElement>('[data-merge]')!.onclick=()=>{const values=[...dialog.querySelectorAll<HTMLSelectElement>('select')].map(s=>({order:Number(s.dataset.order),segment_id:s.value}));if(values.some(v=>!v.segment_id)){dialog.querySelector('[data-error]')!.textContent='すべての録音行の対応先を指定してください。';return;}finish(values);};document.body.append(dialog);dialog.showModal();
});}
