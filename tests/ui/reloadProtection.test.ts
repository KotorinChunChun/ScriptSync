import {it,expect} from 'vitest';
import {isReloadKey,installReloadProtection} from '../../src/reloadProtection';
it('修飾キーを含む再読み込みだけを識別する',()=>{for(const key of ['F5','r','R'])expect(isReloadKey({key,ctrlKey:true,metaKey:false})).toBe(true);expect(isReloadKey({key:'F4',ctrlKey:false,metaKey:false})).toBe(false);expect(isReloadKey({key:'r',ctrlKey:false,metaKey:false})).toBe(false);});
it('入力欄でもF5をブラウザーの既定動作へ渡さない',()=>{let blocked=0;installReloadProtection(()=>blocked++);const input=document.createElement('input');document.body.append(input);const event=new KeyboardEvent('keydown',{key:'F5',bubbles:true,cancelable:true});input.dispatchEvent(event);expect(event.defaultPrevented).toBe(true);expect(blocked).toBe(1);});
