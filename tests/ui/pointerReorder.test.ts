import {afterEach,expect,it,vi} from 'vitest';
import {bindPointerReorder} from '../../src/pointerReorder';
afterEach(()=>{vi.restoreAllMocks();vi.useRealTimers();});
const pointer=(target:EventTarget,type:string,x:number,y:number)=>target.dispatchEvent(new MouseEvent(type,{bubbles:true,button:0,clientX:x,clientY:y,cancelable:true}));
it('ドラッグだけを並び替えとし、直後のクリックを抑制する',()=>{
 vi.useFakeTimers();document.body.innerHTML='<div id="list"><button data-id="a">甲</button><button data-id="b">乙</button></div>';
 const list=document.querySelector<HTMLElement>('#list')!,a=list.children[0] as HTMLElement,b=list.children[1] as HTMLElement,move=vi.fn(),click=vi.fn();
 a.onclick=click;Object.defineProperty(document,'elementFromPoint',{value:()=>b,configurable:true});
 bindPointerReorder(list,'button',el=>el.dataset.id!,move);
 pointer(a,'pointerdown',10,10);pointer(document,'pointermove',11,11);pointer(document,'pointerup',11,11);a.click();expect(move).not.toHaveBeenCalled();expect(click).toHaveBeenCalledTimes(1);
 pointer(a,'pointerdown',10,10);pointer(document,'pointermove',10,60);expect(b.classList.contains('reorder-after')).toBe(true);
 pointer(document,'pointerup',10,60);a.click();expect(move).toHaveBeenCalledWith('a','b');expect(click).toHaveBeenCalledTimes(1);
 vi.runAllTimers();a.click();expect(click).toHaveBeenCalledTimes(2);
});
