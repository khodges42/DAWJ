// SPDX-License-Identifier: GPL-3.0-or-later
// Authoring UI only. Validation, media preparation and persistence live in Rust.
export function contentTarget(workspace,deck,slot){
  if(!Number.isInteger(deck)||deck<0||deck>1||!Number.isInteger(slot)||slot<0||slot>7)throw Error('Invalid content target');
  const composition=workspace?.project?.compositions.find(c=>c.id===workspace.project.decks[deck]);
  if(!composition)throw Error('Create or load a composition on this deck first');
  const original=composition.parts[slot],target=workspace.project.settings?.part_targets?.[deck]?.[slot];
  const part=target?.part===original.id?{...original,gain:target.gain,cutoff_hz:target.cutoff_hz,rule:target.rule}:original;
  return {deck,slot,composition:composition.id,part};
}
export function editSound(part,values){
  return {...part,name:values.name,looping:values.looping,loop_start:Number(values.start),loop_end:values.end===''?null:Number(values.end),nominal_ticks:Math.round(Number(values.beats)*960)};
}
export function sessionCommand(action){return {kind:'session',action}}
export class ContentPanel {
  constructor(t,getState,invoke,openDialog,onError){
    this.t=t;this.getState=getState;this.invoke=invoke;this.openDialog=openDialog;this.onError=onError;this.deck=0;this.slot=0;this.signature='';
    const $=id=>document.getElementById(id);this.$=$;
    for(let s=0;s<8;s++){$('jam-slot').add(new Option(String(s+1),String(s)))}
    const bind=(id,fn)=>$(id).onclick=()=>Promise.resolve().then(()=>{ $('jam-error').textContent='';return fn()}).catch(e=>this.error(e));
    this.send=action=>t.call(sessionCommand(action));
    const discard=()=>!this.getState().session?.dirty||window.confirm('Discard unsaved session changes?');
    bind('jam-new',()=>{if(discard())return this.send({kind:'new'})});
    bind('jam-demo',()=>{if(discard())return this.send({kind:'demo'})});
    bind('jam-open',async()=>{if(!discard())return;const path=await invoke('pick_local_file',{kind:'open'});if(path)return this.send({kind:'open',path})});
    const save=async as=>{let path=null;if(as||!this.getState().session?.path)path=await invoke('pick_local_file',{kind:'save'});if((as||!this.getState().session?.path)&&!path)return;return this.send({kind:'save',path})};
    bind('jam-save',()=>save(false));bind('jam-save-as',()=>save(true));
    bind('jam-rename',()=>this.send({kind:'rename',name:$('jam-name').value}));
    bind('jam-create',()=>this.send({kind:'create',name:$('jam-composition-name').value,deck:this.deck}));
    bind('jam-rename-composition',()=>this.send({kind:'rename_composition',composition:this.target().composition,name:$('jam-composition-name').value}));
    bind('jam-assign',()=>this.send({kind:'assign',deck:this.deck,composition:$('jam-composition').value===''?null:Number($('jam-composition').value)}));
    bind('jam-import',async()=>{const target=this.target();const path=await invoke('pick_local_file',{kind:'audio'});if(path)return this.import(path,target)});
    bind('jam-remove',()=>{const a=this.target();return this.send({kind:'remove',composition:a.composition,slot:a.slot})});
    bind('jam-apply',()=>{const a=this.target();const part=editSound(a.part,{name:$('jam-part-name').value,looping:$('jam-loop').checked,start:$('jam-loop-start').value,end:$('jam-loop-end').value,beats:$('jam-length').value});return this.send({kind:'edit',composition:a.composition,slot:a.slot,part})});
    bind('jam-restore-rule',()=>{const a=this.target();if(!a.part.rule)throw Error('No saved rule');return t.call({kind:'rule_install',deck:a.deck,slot:a.slot,rule:a.part.rule})});
    $('jam-deck').onchange=()=>{this.deck=Number($('jam-deck').value);this.signature='';this.update(this.getState())};
    $('jam-slot').onchange=()=>{this.slot=Number($('jam-slot').value);this.signature='';this.update(this.getState())};
  }
  error(e){this.$('jam-error').textContent=String(e);this.onError(e)}
  target(){return contentTarget(this.getState().session,this.deck,this.slot)}
  async import(path,target=this.target()){return this.send({kind:'import',composition:target.composition,slot:target.slot,path})}
  open(deck=this.deck,slot=this.slot){this.deck=deck;this.slot=slot;this.signature='';this.openDialog('content');this.update(this.getState())}
  update(s){
    const w=s.session;if(!w)return;const $=this.$;
    $('session-open').textContent='Session / sounds'+(w.dirty?' *':'');$('session-open').title=w.demo?'Supplied demo':w.project.name;
    if(!$('content').open)return;
    $('jam-path').textContent=w.demo?'Supplied demonstration':w.path??'Not saved yet';$('jam-status').textContent=(w.busy?'Preparing... ':'')+w.message;
    $('jam-deck').value=this.deck;$('jam-slot').value=this.slot;
    for(const b of $('content').querySelectorAll('button:not([data-close])'))b.disabled=w.busy;
    const current=w.project.compositions.find(c=>c.id===w.project.decks[this.deck]);
    $('jam-restore-rule').disabled=w.busy||!s.online||!current||!contentTarget(w,this.deck,this.slot).part.rule;
    const signature=JSON.stringify([w.revision,w.demo,this.deck,this.slot,w.project.name]);if(signature===this.signature)return;this.signature=signature;
    $('jam-name').value=w.project.name;
    $('jam-composition').replaceChildren(new Option('(empty deck)',''));
    for(const c of w.project.compositions)$('jam-composition').add(new Option(c.name,String(c.id)));
    $('jam-composition').value=w.project.decks[this.deck]??'';
    const c=w.project.compositions.find(c=>c.id===w.project.decks[this.deck]);$('jam-composition-name').value=c?.name??'';
    const p=c?contentTarget(w,this.deck,this.slot).part:null;$('jam-part-name').value=p?.name??'';$('jam-loop').checked=p?.looping??true;$('jam-loop-start').value=p?.loop_start??0;$('jam-loop-end').value=p?.loop_end??'';$('jam-length').value=(p?.nominal_ticks??3840)/960;
    const info=w.media.find(m=>m.deck===this.deck&&m.slot===this.slot);
    $('jam-media').textContent=info?(info.error?`${info.path}: ${info.error}`:`${info.filename} | ${info.duration.toFixed(3)} s | ${info.sample_rate} Hz | ${info.channels} ch | ${info.prepared_frames} prepared frames | ${info.path}`):(p?.media??'No sound assigned');
    $('jam-restore-rule').disabled=w.busy||!s.online||!p?.rule;
  }
}
