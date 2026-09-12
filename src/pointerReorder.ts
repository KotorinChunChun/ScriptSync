// WebViewのネイティブファイルドロップと競合しない、一覧内の並び替え。
const bindings=new WeakMap<HTMLElement,()=>void>();
export function bindPointerReorder(container:HTMLElement, selector:string, key:(item:HTMLElement)=>string, move:(from:string,to:string)=>void, enabled:()=>boolean=()=>true){
 bindings.get(container)?.();
 let activeCleanup=()=>{};
 const onDown=(event:PointerEvent)=>{
  activeCleanup();
  if(event.button!==0||!enabled()||(event.target as Element).closest('input,select,textarea'))return;
  const item=(event.target as Element).closest<HTMLElement>(selector);
  if(!item||!container.contains(item))return;
  const startX=event.clientX,startY=event.clientY,from=key(item);let dragging=false,target:HTMLElement|null=null;
  const clearMark=()=>container.querySelectorAll('.reorder-before,.reorder-after').forEach(el=>el.classList.remove('reorder-before','reorder-after'));
  const cleanup=()=>{document.removeEventListener('pointermove',onMove);document.removeEventListener('pointerup',onUp);document.removeEventListener('pointercancel',onCancel);item.classList.remove('reordering');clearMark();};
  activeCleanup=cleanup;
  const onCancel=()=>cleanup();
  const onMove=(next:PointerEvent)=>{
   if(next.pointerId!==event.pointerId)return;
   if(!container.isConnected||!enabled()){cleanup();return;}
   if(!dragging&&Math.hypot(next.clientX-startX,next.clientY-startY)<6)return;
   dragging=true;next.preventDefault();item.classList.add('reordering');
   target=document.elementFromPoint(next.clientX,next.clientY)?.closest<HTMLElement>(selector)??null;
   if(target&&!container.contains(target))target=null;
   clearMark();
   if(target&&target!==item){const items=[...container.querySelectorAll(selector)];target.classList.add(items.indexOf(target)>items.indexOf(item)?'reorder-after':'reorder-before');}
   const bounds=container.getBoundingClientRect();
   if(next.clientY<bounds.top+28)container.scrollTop-=24;
   if(next.clientY>bounds.bottom-28)container.scrollTop+=24;
  };
  const onUp=(next:PointerEvent)=>{
   if(next.pointerId!==event.pointerId)return;
   const to=target?key(target):null;cleanup();
   if(!dragging)return;
   // ドラッグ直後のclickで読み上げ対象のオン／オフが切り替わらないようにする。
   const suppress=(click:MouseEvent)=>{click.preventDefault();click.stopImmediatePropagation();};
   document.addEventListener('click',suppress,true);setTimeout(()=>document.removeEventListener('click',suppress,true),0);
   if(to!==null&&to!==from&&enabled())move(from,to);
  };
  document.addEventListener('pointermove',onMove,{passive:false});document.addEventListener('pointerup',onUp);document.addEventListener('pointercancel',onCancel);
 };
 container.addEventListener('pointerdown',onDown);
 bindings.set(container,()=>{activeCleanup();container.removeEventListener('pointerdown',onDown);});
}
