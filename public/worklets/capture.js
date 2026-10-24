class Capture extends AudioWorkletProcessor{
 constructor(){super();this.stopping=false;this.port.onmessage=e=>{if(e.data==='stop')this.stopping=true;};}
 process(inputs){if(this.stopping){this.port.postMessage({done:true});return false;}const input=inputs[0]?.[0];if(input?.length){const samples=input.slice();this.port.postMessage({samples},[samples.buffer]);}return true;}
}
registerProcessor('scriptsync-capture',Capture);
