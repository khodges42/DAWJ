// SPDX-License-Identifier: GPL-3.0-or-later
// Wire types and version authority: dawj_core::ipc. Fixtures are checked by Rust and Node.
export const VERSION=1;
let nextId=0;
/** @typedef {{kind:'launch'|'stop_part',deck:number,slot:number}|{kind:'crossfader',value:number}} PerformanceAction */
/** @typedef {{version:number,id:number,command:object}} Request */
/** @returns {Request} */
export function makeRequest(action) {
  const command=['devices','start','stop','snapshot','release_modes','midi_profile','midi_replace','midi_connect','midi_save','midi_load','midi_rescan','midi_learn','midi_cancel_learn'].includes(action.kind)
    ? action : {kind:'perform',action,source:'Gui'};
  return {version:VERSION,id:nextId=(nextId+1)>>>0,command};
}
export function readReply(reply,expectedId) {
  if(reply.version!==VERSION)throw Error('Unsupported frontend protocol reply');
  if(reply.id!==expectedId)throw Error('Mismatched frontend reply ID');
  if(reply.result?.Err)throw Error(`${reply.result.Err.code}: ${reply.result.Err.message}`);
  const response=reply.result?.Ok;
  if(!response||!['snapshot','devices','midi_profile'].includes(response.kind))throw Error('Invalid frontend reply');
  return response.data;
}
export async function invokeRequest(invoke,action) {
  const request=makeRequest(action);
  return readReply(await invoke('request',{request}),request.id);
}
