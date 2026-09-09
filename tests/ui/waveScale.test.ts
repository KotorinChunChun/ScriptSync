import {expect,it} from 'vitest';
import {waveAmplitude} from '../../src/waveScale';
it('微小信号を拡大せず固定フルスケールで描く',()=>{
 expect(waveAmplitude(0)).toBe(0);expect(waveAmplitude(0.0001)).toBe(0);
 expect(waveAmplitude(0.01)).toBeCloseTo(0.1);expect(waveAmplitude(1)).toBe(1);
 expect(waveAmplitude(2)).toBe(1);expect(waveAmplitude(NaN)).toBe(0);
});
