// SPDX-License-Identifier: GPL-3.0-or-later
// Explicit --reliability only. All timing is observation, never musical scheduling.
export class Timing {
 constructor(){this.bins=new Uint32Array(10001);this.n=0;this.worst=0}
 record(ms){if(!Number.isFinite(ms)||ms<0)return;this.n++;this.worst=Math.max(this.worst,ms);this.bins[Math.min(10000,Math.ceil(ms*10))]++}
 summary(){const q=p=>{let n=0;for(let i=0;i<this.bins.length;i++){n+=this.bins[i];if(n>=Math.ceil(p*this.n))return i/10}return 0};return {n:this.n,median:q(.5),p95:q(.95),worst:this.worst}}
}
export async function runReliability({t,getState,invoke,skinManager,options}){
 const sleep=ms=>new Promise(r=>setTimeout(r,ms)),paint=()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
 const command=new Timing(),frames=new Timing(),display=new Timing(),snapshot=new Timing();let running=true,last=0,count=0,theme=0;
 const frame=now=>{if(last)frames.record(now-last);last=now;if(running)requestAnimationFrame(frame)};requestAnimationFrame(frame);
 const call=async c=>{const start=performance.now();const s=await t.call(c);command.record(performance.now()-start);return s};
 try{
  const outputs=await t.call({kind:'devices'}),matches=outputs.filter(d=>d.name.toLowerCase().includes(options.output.toLowerCase()));
  if(!options.output.trim()||matches.length!==1)throw Error('Required test output missing or ambiguous: '+options.output);
  console.info('RELIABILITY_OUTPUT',matches[0].name);
  await call({kind:'start',device:matches[0].id,cue:false,phrase_bars:8});
  await call({kind:'quantize',grid:'Immediate'});await call({kind:'launch',deck:0,slot:0});await call({kind:'launch',deck:1,slot:3});
  const startupCommand=command.summary();command.bins.fill(0);command.n=0;command.worst=0;
  // Compare continuous playback in both frontends, excluding device-start frames.
  frames.bins.fill(0);frames.n=0;frames.worst=0;last=0;
  const start=performance.now();let nextTheme=10000;
  while(performance.now()-start<options.seconds*1000){
   const tick=performance.now(),x=count%100/100;
   for(const c of [{kind:'crossfader',value:x},{kind:'gain',value:.3+x*.5},{kind:'cutoff',hz:200+x*10000}])await call(c);
   const stamp=performance.now();await paint();display.record(performance.now()-stamp);
   const poll=performance.now();await t.call({kind:'snapshot'});snapshot.record(performance.now()-poll);
   if(!getState().online||getState().failed)throw Error('Audio failed during soak');
   if(performance.now()-start>=nextTheme){const id=theme%2?await skinManager.importText(await (await fetch('skins/soft-grid.dawjskin.json')).text()):'bundled:psychic-genesis';theme++;await skinManager.preview(id);await skinManager.cancel();nextTheme+=10000}
   count++;await sleep(Math.max(0,50-(performance.now()-tick)));
  }
  const s=getState();if(s.decks[0][0].active==null||s.decks[1][3].active==null)throw Error('Both decks did not remain active');
  const metrics={frontend:'tauri',passed:true,seconds:(performance.now()-start)/1000,changes:count,command_ms:command.summary(),frame_ms:frames.summary(),receipt_to_frame_ms:display.summary(),snapshot_roundtrip_ms:snapshot.summary(),themes:theme,peaks:s.peaks,startup_command_ms:startupCommand};
  running=false;await t.call({kind:'stop'});await invoke('report',{metrics});
 }catch(e){running=false;await t.call({kind:'stop'}).catch(()=>{});await invoke('report',{metrics:{error:String(e),frontend:'tauri',passed:false}})}
}
