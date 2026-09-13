import {escapeHtml as esc} from './html';
export type Replacement={from:string;to:string};
type Token={text:string;word:boolean};
const tokenize=(text:string):Token[]=>Array.from(new Intl.Segmenter('ja',{granularity:'word'}).segment(text),part=>({text:part.segment,word:!!part.isWordLike}));
export function improvementDiff(before:string,after:string){
 const a=tokenize(before),b=tokenize(after);const left:string[]=[],right:string[]=[],candidates:Replacement[]=[];
 if(a.length*b.length>500000)return {left:esc(before),right:esc(after),candidates};
 const dp=Array.from({length:a.length+1},()=>new Uint32Array(b.length+1));
 for(let i=a.length-1;i>=0;i--)for(let j=b.length-1;j>=0;j--)dp[i][j]=a[i].text===b[j].text?dp[i+1][j+1]+1:Math.max(dp[i+1][j],dp[i][j+1]);
 let i=0,j=0,removed:Token[]=[],added:Token[]=[];
 const flush=()=>{if(!removed.length&&!added.length)return;
  const from=removed.map(t=>t.text).join(''),to=added.map(t=>t.text).join('');left.push(`<del>${esc(from)}</del>`);right.push(`<ins>${esc(to)}</ins>`);
  if(removed.length===1&&added.length===1&&removed[0].word&&added[0].word&&from!==to&&from.length<=80&&to.length<=80&&!candidates.some(x=>x.from===from&&x.to===to))candidates.push({from,to});
  removed=[];added=[];
 };
 while(i<a.length||j<b.length){if(i<a.length&&j<b.length&&a[i].text===b[j].text){flush();left.push(esc(a[i++].text));right.push(esc(b[j++].text));}else if(i<a.length&&(j===b.length||dp[i+1][j]>=dp[i][j+1]))removed.push(a[i++]);else added.push(b[j++]);}flush();
 return {left:left.join(''),right:right.join(''),candidates};
}
export type AiReview={key:string;before:string;after:string;prepared:string;editVersion:number;viewVersion:number;candidates:Replacement[];checked:number[]};
export function aiReviewHtml(review:AiReview){
 const diff=improvementDiff(review.before,review.after);
 return `<section class="ai-review" aria-label="AI改善結果の比較"><div class="ai-diff-columns"><div><h4>改善前の修正案</h4><div class="ai-diff-text" tabindex="0">${diff.left}</div></div><div><h4>AI改善結果</h4><div class="ai-diff-text" tabindex="0">${diff.right}</div></div></div>${review.prepared!==review.before?'<p>登録済み辞書の置換を行ってからAIに送信しました。</p>':''}${review.candidates.length?`<fieldset><legend>今後の置換辞書に登録する単語</legend>${review.candidates.map((pair,index)=>`<label><input type="checkbox" data-ai-word="${index}" ${review.checked.includes(index)?'checked':''}>${esc(pair.from)} → ${esc(pair.to)}</label>`).join('')}</fieldset>`:'<p>登録候補となる単語の置換はありません。</p>'}<div class="toolbar"><button data-action="discard_ai">取り消す</button><button class="primary" data-action="approve_ai">承認して反映</button></div></section>`;
}
