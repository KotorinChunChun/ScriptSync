export type Difference={kind:"same"|"added"|"removed";text:string};
export function scriptDifferences(before:string,after:string):Difference[]{
 const a=before?before.replace(/\r\n/g,"\n").split("\n").filter(x=>x.trim()):[],b=after?after.replace(/\r\n/g,"\n").split("\n").filter(x=>x.trim()):[];
 let prefix=0;while(prefix<a.length&&prefix<b.length&&a[prefix]===b[prefix])prefix++;
 let endA=a.length,endB=b.length;while(endA>prefix&&endB>prefix&&a[endA-1]===b[endB-1]){endA--;endB--;}
 const rows:Difference[]=a.slice(0,prefix).map(text=>({kind:"same",text}));
 const n=endA-prefix,m=endB-prefix;
 if(n*m>1_000_000){rows.push(...a.slice(prefix,endA).map(text=>({kind:"removed" as const,text})),...b.slice(prefix,endB).map(text=>({kind:"added" as const,text})));}
 else{const dp=Array.from({length:n+1},()=>new Uint32Array(m+1));
 for(let i=n-1;i>=0;i--)for(let j=m-1;j>=0;j--)dp[i][j]=a[prefix+i]===b[prefix+j]?dp[i+1][j+1]+1:Math.max(dp[i+1][j],dp[i][j+1]);
 let i=0,j=0;while(i<n||j<m){if(i<n&&j<m&&a[prefix+i]===b[prefix+j]){rows.push({kind:"same",text:a[prefix+i++]});j++;}else if(i<n&&(j===m||dp[i+1][j]>=dp[i][j+1]))rows.push({kind:"removed",text:a[prefix+i++]});else rows.push({kind:"added",text:b[prefix+j++]});}}
 rows.push(...a.slice(endA).map(text=>({kind:"same" as const,text})));return rows;
}
