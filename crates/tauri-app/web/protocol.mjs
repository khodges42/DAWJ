// SPDX-License-Identifier: GPL-3.0-or-later
// Wire types and version authority: dawj_core::ipc. Fixtures are checked by Rust and Node.
import {contract} from './contract.generated.mjs';
export const VERSION=contract.version;
const commands=new Set(contract.commands.map(c=>c.kind));
const actions=new Set(contract.actions.map(a=>a.kind));
let nextId=0;
/** @typedef {{kind:'launch'|'stop_part',deck:number,slot:number}|{kind:'crossfader',value:number}} PerformanceAction */
/** @typedef {{version:number,id:number,command:object}} Request */
/** @returns {Request} */
export function makeRequest(action,source='Gui') {
  if(!commands.has(action.kind)&&!actions.has(action.kind))throw Error('Unknown performance command');
  const command=commands.has(action.kind)?action:{kind:'perform',action,source};
  return {version:VERSION,id:nextId=(nextId+1)>>>0,command};
}
export function readReply(reply,expectedId) {
  if(reply.version!==VERSION)throw Error('Unsupported frontend protocol reply');
  if(reply.id!==expectedId)throw Error('Mismatched frontend reply ID');
  if(reply.result?.Err)throw Error(`${reply.result.Err.code}: ${reply.result.Err.message}`);
  const response=reply.result?.Ok;
  if(!response||!['snapshot','devices','midi_profile','midi_profiles','catalog'].includes(response.kind))throw Error('Invalid frontend reply');
  return response.data;
}
export async function invokeRequest(invoke,action,source='Gui') {
  const request=makeRequest(action,source);
  return readReply(await invoke('request',{request}),request.id);
}
