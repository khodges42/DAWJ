// SPDX-License-Identifier: GPL-3.0-or-later
import {invokeRequest} from './protocol.mjs';
export class Transport {
  constructor(invoke,onSnapshot=()=>{},onError=()=>{}){
    this.invoke=invoke;this.show=onSnapshot;this.error=onError;this.serial=Promise.resolve();this.admitted=0;
    this.latest=new Map();this.sending=false;this.epochs={Gui:0,Keyboard:0,Midi:0};this.releasing=new Map();
  }
  async send(command,source='Gui'){
    const data=await invokeRequest(this.invoke,command,source);
    if(data&&typeof data.online==='boolean')this.show(data);
    return data;
  }
  call(command,source='Gui'){
    if(this.admitted>=24)return Promise.reject(Error('Frontend admission full; wait for output'));
    this.admitted++;const epoch=this.epochs[source];
    const run=()=>{
      if(command.kind==='hold_mode'&&command.pressed&&epoch!==this.epochs[source])return undefined;
      return this.send(command,source);
    };
    const p=this.serial.then(run);this.serial=p.catch(()=>{}).finally(()=>this.admitted--);return p;
  }
  coalesce(key,command){if(this.latest.size>=24&&!this.latest.has(key))return this.error(Error('Parameter capacity reached'));this.latest.set(key,command)}
  dropContinuous(){this.latest.clear()}
  async flush(){
    if(this.sending||!this.latest.size)return;
    const [key,command]=this.latest.entries().next().value;this.latest.delete(key);this.sending=true;
    try{await this.call(command)}catch(e){this.error(e)}finally{this.sending=false}
  }
  release(source){
    this.epochs[source]++;this.dropContinuous();
    // Reach Rust's out-of-band release latch even when the normal frontend queue is full.
    if(this.releasing.has(source))return this.releasing.get(source);
    const release=()=>this.send({kind:'release_modes',source}).catch(e=>this.error(e));
    void release();
    // A serialized barrier also covers presses already dispatched to the IPC runtime.
    const p=this.serial.then(release);this.serial=p.finally(()=>this.releasing.delete(source));this.releasing.set(source,p);return p;
  }
}
