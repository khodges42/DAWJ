// SPDX-License-Identifier: GPL-3.0-or-later
import {contract} from './contract.generated.mjs';
export const actionName=a=>typeof a==='string'?a:Object.keys(a)[0];
export function mappingFromForm(f){
  const target=JSON.parse(f.target),name=f.action;let action;
  if(['Launch','Stop','Cancel','Retrigger','Toggle','Gain','Cutoff','Context'].includes(name))action={[name]:target};
  else if(['SelectDeck','DeckGain','Cue'].includes(name))action={[name]:Number(f.deck)};
  else if(name==='SelectSlot')action={SelectSlot:Number(f.slot)-1};
  else if(name==='SelectPart')action={SelectPart:{deck:Number(f.deck),slot:Number(f.slot)-1}};
  else if(['Mode','Momentary'].includes(name))action={[name]:f.mode};
  else if(name==='Quantize')action={Quantize:f.grid};else action=name;
  return {input:{channel:Number(f.channel)-1,kind:f.kind,number:Number(f.number)},action,behavior:f.behavior,min:Number(f.min),max:Number(f.max),invert:!!f.invert,step:Number(f.step)};
}
export function profileEdit(profile,kind,index,value){
  const p=structuredClone(profile);
  if(kind==='add')p.mappings.push(value);
  else if(kind==='replace'){if(!p.mappings[index])throw Error('Select a mapping first');p.mappings[index]=value}
  else if(kind==='remove'){if(!p.mappings[index])throw Error('Select a mapping first');p.mappings.splice(index,1)}
  return p;
}
const $=id=>document.getElementById(id);
const option=(value,text=value)=>{const e=document.createElement('option');e.value=value;e.textContent=text;return e};
export class MidiPanel {
  constructor(transport,error){
    this.t=transport;this.error=e=>{error(e);$('settings-error').textContent=String(e)};this.profile=null;this.revision=-1;this.fetching=false;this.selected=null;this.portSignature='';
    for(const a of contract.midi_actions)$('map-action').append(option(actionName(a)));
    $('map-action').onchange=()=>{const continuous=contract.continuous_actions.some(a=>actionName(a)===$('map-action').value);$('map-behavior').value=continuous?'Absolute':'Button';this.showFields()};
    $('map-target').append(option(JSON.stringify('Selected'),'Selected part'));
    for(let slot=0;slot<8;slot++)$('map-target').append(option(JSON.stringify({FocusedDeck:{slot}}),`Focused deck ${slot+1}`));
    for(let deck=0;deck<2;deck++)for(let slot=0;slot<8;slot++)$('map-target').append(option(JSON.stringify({DeckSlot:{deck,slot}}),`${deck?'B':'A'}${slot+1}`));
    const bind=(id,fn)=>$(id).onclick=()=>Promise.resolve().then(fn).catch(this.error);
    bind('midi-connect',()=>this.t.call({kind:'midi_connect',input:$('midi-input').value||null,output:$('midi-output').value||null}));
    bind('midi-disconnect',()=>this.t.call({kind:'midi_connect',input:null,output:null}));
    bind('midi-rescan',()=>this.t.call({kind:'midi_rescan'}));
    bind('profile-select',async()=>{await this.t.call({kind:'midi_select_profile',name:$('profiles').value});await this.refreshProfile();this.message('Profile loaded. Connect / retry applies its device choices.')});
    bind('profile-save',()=>this.t.call({kind:'midi_save'}).then(()=>this.message('Profile saved')));
    bind('profile-save-as',async()=>{await this.t.call({kind:'midi_save_as',name:$('profile-name').value});await this.refreshNames();this.message('Profile saved; selected for next restart')});
    bind('profile-load',()=>this.t.call({kind:'midi_load'}).then(()=>this.refreshProfile()));
    bind('profile-factory',()=>this.t.call({kind:'midi_factory'}).then(()=>this.refreshProfile()));
    bind('map-add',()=>this.edit('add'));
    bind('map-replace',()=>this.edit('replace'));
    bind('map-learn',()=>this.learn(null));bind('map-learn-replace',()=>{if(this.selected===null)throw Error('Select a mapping first');return this.learn(this.selected)});
    bind('map-cancel-learn',()=>this.t.call({kind:'midi_cancel_learn'}));
    bind('led-add',()=>{this.leds.push({output:{channel:0,kind:'Note',number:0},deck:0,slot:0});this.renderLeds()});
    bind('led-apply',async()=>{const p=structuredClone(this.profile);p.leds=structuredClone(this.leds);for(const [key,e] of Object.entries(this.values))p.values[key]=Number(e.value);await this.replace(p)});
    this.values={};
    for(const key of ['stopped','queued','playing','stopping','replacing']){const label=document.createElement('label');label.textContent=key;const input=document.createElement('input');input.type='number';input.min=0;input.max=127;label.append(input);$('led-values').append(label);this.values[key]=input}
    this.showFields();
  }
  message(text){$('settings-error').textContent=text}
  async open(){await this.refreshNames();await this.refreshProfile()}
  async refreshNames(){const names=await this.t.call({kind:'midi_profiles'});const current=this.state?.profile_name??'default';if(!names.includes(current))names.push(current);$('profiles').replaceChildren(...names.map(n=>option(n)));$('profiles').value=current}
  async refreshProfile(){
    if(this.fetching)return;this.fetching=true;const revision=this.state?.midi_revision??0;
    try{this.profile=await this.t.call({kind:'midi_profile'});this.revision=revision;this.selected=null;this.renderMappings();this.leds=structuredClone(this.profile.leds);this.renderLeds();for(const[k,e]of Object.entries(this.values))e.value=this.profile.values[k];this.ports(true)}finally{this.fetching=false}
  }
  update(s){
    this.state=s;const m=s.midi??{};
    $('midi-status').textContent=`Input: ${m.input??'disconnected'} | LED output: ${m.output??'disconnected'} | ${m.message??''} | ${s.midi_notice??''}`;
    $('led-status').textContent=`${m.output?'LED output connected':'No LED output connected'} · sent ${m.feedback_sent??0} messages · input drops ${s.midi_dropped??0}${m.feedback_error?' · OUTPUT ERROR: '+m.feedback_error:''}`;
    $('profile-status').textContent=`${s.profile_name??'default'} · ${s.midi_dirty?'Unsaved changes':'Saved / loaded'}${s.midi_learning?' · LEARNING':''} · ${s.profile_path??''}`;
    this.ports();
    if($('settings').open&&this.revision!==s.midi_revision&&!this.fetching)this.refreshProfile().catch(this.error);
  }
  ports(force=false){
    const m=this.state?.midi??{},signature=JSON.stringify([m.inputs,m.outputs]);if(!force&&signature===this.portSignature)return;this.portSignature=signature;
    for(const[id,names,wanted]of [['midi-input',m.inputs,this.profile?.input],['midi-output',m.outputs,this.profile?.output]]){
      const old=force?wanted:$(id).value;const ports=[...(names??[])];if(old&&!ports.includes(old))ports.push(old);
      $(id).replaceChildren(option('','Disconnected'),...ports.map(n=>option(n)));$(id).value=old??'';
    }
  }
  form(){const f={};for(const key of ['channel','kind','number','action','target','deck','slot','mode','grid','behavior','min','max','step'])f[key]=$('map-'+key).value;f.invert=$('map-invert').checked;return mappingFromForm(f)}
  showFields(){
    const name=$('map-action').value;
    for(const [key,show]of Object.entries({target:['Launch','Stop','Cancel','Retrigger','Toggle','Gain','Cutoff','Context'].includes(name),deck:['SelectDeck','DeckGain','Cue','SelectPart'].includes(name),slot:['SelectSlot','SelectPart'].includes(name),mode:['Mode','Momentary'].includes(name),grid:name==='Quantize'}))$('map-'+key).hidden=!show;
  }
  async replace(profile){await this.t.call({kind:'midi_replace',profile,expected_revision:this.revision});await this.refreshProfile()}
  async edit(kind){if(!this.profile)throw Error('Load a profile first');return this.replace(profileEdit(this.profile,kind,this.selected,this.form()))}
  async learn(replace){await this.t.call({kind:'midi_learn',mapping:this.form(),replace});this.message('Learning. Move the control; input conflicts are checked by the shared mapper.')}
  loadForm(index){
    this.selected=index;const m=this.profile.mappings[index],name=actionName(m.action),payload=typeof m.action==='string'?null:m.action[name];
    for(const[k,v]of Object.entries({channel:m.input.channel+1,kind:m.input.kind,number:m.input.number,action:name,behavior:m.behavior,min:m.min,max:m.max,step:m.step}))$('map-'+k).value=v;
    $('map-invert').checked=m.invert;
    if(['Launch','Stop','Cancel','Retrigger','Toggle','Gain','Cutoff','Context'].includes(name))$('map-target').value=JSON.stringify(payload);
    if(['SelectDeck','DeckGain','Cue'].includes(name))$('map-deck').value=payload;
    if(name==='SelectSlot')$('map-slot').value=payload+1;
    if(name==='SelectPart'){$('map-deck').value=payload.deck;$('map-slot').value=payload.slot+1}
    if(['Mode','Momentary'].includes(name))$('map-mode').value=payload;
    if(name==='Quantize')$('map-grid').value=payload;
    this.showFields();
    this.renderMappings();
  }
  renderMappings(){
    $('mapping-list').replaceChildren();this.profile.mappings.forEach((m,i)=>{
      const row=document.createElement('div');row.className='mapping-row';const choose=document.createElement('button');choose.textContent=this.selected===i?'Selected':'Edit';choose.onclick=()=>this.loadForm(i);
      const text=document.createElement('span');text.textContent=`${i+1}: Ch${m.input.channel+1} ${m.input.kind} ${m.input.number} → ${JSON.stringify(m.action)} · ${m.behavior}`;
      const remove=document.createElement('button');remove.textContent='Remove';remove.onclick=()=>this.replace(profileEdit(this.profile,'remove',i)).catch(this.error);row.append(choose,text,remove);$('mapping-list').append(row);
    });
  }
  renderLeds(){
    $('led-list').replaceChildren();this.leds.forEach((led,i)=>{
      const row=document.createElement('div');row.className='led-row';
      for(const[key,label,min,max,get,set]of [
        ['channel','Ch',1,16,()=>led.output.channel+1,v=>led.output.channel=v-1],['number','No',0,127,()=>led.output.number,v=>led.output.number=v],
        ['deck','Deck 0=A',0,1,()=>led.deck,v=>led.deck=v],['slot','Slot',1,8,()=>led.slot+1,v=>led.slot=v-1]]){
        const l=document.createElement('label');l.textContent=label;const input=document.createElement('input');input.type='number';input.min=min;input.max=max;input.value=get();input.oninput=()=>set(Number(input.value));l.append(input);row.append(l);
      }
      const kind=document.createElement('select');kind.setAttribute('aria-label','LED message kind');kind.append(option('Note'),option('CC'));kind.value=led.output.kind;kind.onchange=()=>led.output.kind=kind.value;
      const remove=document.createElement('button');remove.textContent='Remove LED';remove.onclick=()=>{this.leds.splice(i,1);this.renderLeds()};row.append(kind,remove);$('led-list').append(row);
    });
  }
}
