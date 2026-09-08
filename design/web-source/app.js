"use strict";
/*
 * TUNDRA 07 — personal hardware-only wallet UI prototype.
 * Public descriptors can be imported and checked. Native QR camera access is
 * permission-gated. Bitcoin balances, signing, USB, and payments remain simulated.
 * A wallet owns its public policy and optional local payment draft. There is
 * deliberately no signer directory, participant model, or request inbox.
 */
// Wallet glyph adapted from Bitcoin Icons (Bitcoin Design Community, MIT). See THIRD-PARTY-NOTICES.md.
const ICONS = {
 sliders:'<path d="M4 7h5m4 0h7M4 17h9m4 0h3"/><circle cx="11" cy="7" r="2"/><circle cx="15" cy="17" r="2"/>',
 search:'<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',
 tag:'<path d="M3 3h8l10 10-8 8L3 11Z"/><circle cx="7.5" cy="7.5" r="1"/>',
 coins:'<ellipse cx="10" cy="6" rx="7" ry="3"/><path d="M3 6v5c0 1.7 3.1 3 7 3s7-1.3 7-3V6M3 11v5c0 1.7 3.1 3 7 3s7-1.3 7-3v-5M20 8v11c0 1.7-3.1 3-7 3"/>',
 unlock:'<rect x="5" y="10" width="14" height="11" rx="3"/><path d="M8 10V7a4 4 0 0 1 7-2M12 14v3"/>',
 upload:'<path d="M12 16V3m-5 5 5-5 5 5M4 16v5h16v-5"/>',
 spark:'<path d="m12 3 2.6 6.4L21 12l-6.4 2.6L12 21l-2.6-6.4L3 12l6.4-2.6Z"/>',
 sun:'<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42m0-14.14-1.42 1.42M6.35 17.65l-1.42 1.42"/>',
 moon:'<path d="M20.4 13.1A8.5 8.5 0 0 1 10.9 3.6 8.5 8.5 0 1 0 20.4 13.1Z"/>',
 tundra:'<path d="M3 12h4l4-5 4 5h6M5 17h14M8 21h8"/>',
 wallets:'<g stroke-width="1.5"><path d="M15 17.5h3.005a1.5 1.5 0 001.5-1.5V8a1.5 1.5 0 00-1.5-1.5H15A1.5 1.5 0 0116.5 8v8a1.5 1.5 0 01-1.5 1.5z"/><rect width="12" height="11" x="4.5" y="6.5" rx="1.5"/><circle cx="8.75" cy="11.75" r="1.25"/></g>',
 single:'<rect x="6" y="3" width="12" height="18" rx="3"/><path d="M9 7h6M10 16h4"/>',
 multi:'<rect x="3" y="3" width="7" height="7" rx="2"/><rect x="14" y="3" width="7" height="7" rx="2"/><rect x="8.5" y="14" width="7" height="7" rx="2"/><path d="m7 10 3 4m7-4-3 4"/>',
 send:'<path d="M6 18 18 6M7 6h11v11"/>', receive:'<path d="m18 6-12 12M6 7v11h11"/>',
 check:'<path d="m5 12 4 4L19 6"/>', chevron:'<path d="m9 5 7 7-7 7"/>', back:'<path d="m14 5-7 7 7 7M7 12h14"/>',
 plus:'<path d="M12 5v14M5 12h14"/>', close:'<path d="M6 6l12 12M6 18 18 6"/>',
 shield:'<path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6l8-3Z"/><path d="m8 11 3 3 5-5"/>',
 eye:'<path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/>',
 eyeoff:'<path d="m3 3 18 18M10 5a12 12 0 0 1 12 7s-2 4-6 6M7 6a17 17 0 0 0-5 6s3 7 10 7c1 0 2 0 3-.5M10 10a3 3 0 0 0 4 4"/>',
 lock:'<rect x="5" y="10" width="14" height="11" rx="3"/><path d="M8 10V7a4 4 0 0 1 8 0v3M12 14v3"/>',
 clock:'<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
 requests:'<path d="M5 7h14m-4-4 4 4-4 4M19 17H5m4-4-4 4 4 4"/>',
 settings:'<path d="m10 3-1 3-3 1-3 1v3l2 2-1 3 2 2 3-1 2 2h3l1-3 3-1 3-1v-3l-2-2 1-3-2-2-3 1-2-2Z"/><circle cx="12" cy="12" r="3"/>',
 qr:'<rect x="3" y="3" width="6" height="6" rx="1"/><rect x="15" y="3" width="6" height="6" rx="1"/><rect x="3" y="15" width="6" height="6" rx="1"/><path d="M15 15h3v3h3v3h-6v-3M12 3v3M3 12h6m3 0h3v3m6-3v3M12 18v3"/>',
 usb:'<path d="M12 20V4m-3 3 3-3 3 3M6 10v3l6 3m6-7v3l-6 3"/><circle cx="6" cy="8" r="2"/><rect x="16" y="5" width="4" height="4" rx=".5"/><circle cx="12" cy="20" r="1"/>',
 copy:'<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M16 8V3H3v13h5"/>',
 download:'<path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5"/>',
 info:'<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7h.01"/>',
 more:'<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>',
 scan:'<path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5M7 12h10"/>',
 fingerprint:'<path d="M5 10a7 7 0 0 1 14 0v3m-17 0v-3a10 10 0 0 1 20 0M8 19c1-2 1-5 1-9a3 3 0 0 1 6 0v4c0 4-1 6-2 8M5 14c0 3-1 5-1 5m8-9v4c0 4-1 7-2 8m8-5-1 4"/>',
 node:'<circle cx="5" cy="6" r="3"/><circle cx="19" cy="6" r="3"/><circle cx="12" cy="19" r="3"/><path d="M8 6h8M6.5 8.5l4 8m7-8-4 8"/>',
 refresh:'<path d="M20 7v5h-5M4 17v-5h5M6 6a8 8 0 0 1 14 6M4 12a8 8 0 0 0 14 6"/>',
 alert:'<path d="m12 3 10 18H2L12 3Z"/><path d="M12 9v5m0 3h.01"/>',
 arrow:'<path d="M4 12h16m-6-6 6 6-6 6"/>',
 coin:'<circle cx="12" cy="12" r="9"/><path d="M9 6v12m5-12v2m0 8v2M9 8h5a2 2 0 0 1 0 4H9m0 0h6a2 2 0 0 1 0 4H9"/>',
 trash:'<path d="M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7"/>',
};
const icon=(name,size=20,cls='')=>`<svg class="${cls}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name]||ICONS.info}</svg>`;
const escapeHtml=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const e=escapeHtml;
const STORE_KEY = 'tundra-coins-demo-v1';
const clone = value => JSON.parse(JSON.stringify(value));
const demoKeys = n => Array.from({length:n}, (_,i) => ({ref:`demo:public-key-${i+1}`, fingerprint:['24E6A103','790BD482','C1863F2A'][i]}));
const INITIAL = {
  version:3, activeWallet:'savings', unit:'BTC', hidden:false, nextId:3,
  wallets:[
    {id:'savings', name:'Savings', required:2, keys:demoKeys(3), balance:80000000, receiveIndex:1, addressVerified:false, draft:null,
     history:[
       {id:'h1', direction:'in', amount:10000000, label:'Received', subtitle:'Confirmed', date:'Sep 3', to:'demo:deposit-01'},
       {id:'h2', direction:'in', amount:65000000, label:'Received', subtitle:'Confirmed', date:'Aug 28', to:'demo:deposit-02'},
       {id:'h3', direction:'out', amount:2500000, label:'Sent', subtitle:'Confirmed', date:'Aug 22', to:'demo:recipient-03'}]},
    {id:'everyday', name:'Everyday', required:1, keys:[{ref:'demo:public-key-4',fingerprint:'84D723B1'}], balance:4265000, receiveIndex:1, addressVerified:false, draft:null,
     history:[
       {id:'h4', direction:'in', amount:1250000, label:'Received', subtitle:'Confirmed', date:'Sep 5', to:'demo:deposit-04'},
       {id:'h5', direction:'out', amount:200000, label:'Sent', subtitle:'Confirmed', date:'Sep 1', to:'demo:recipient-05'}]}
  ]
};
INITIAL.wallets.forEach(w=>TundraCoins.seed(w));
function validStore(value) {
  return value?.version===3 && ['BTC','sats'].includes(value.unit) && Number.isSafeInteger(value.nextId) &&
    Array.isArray(value.wallets) && value.wallets.length>0 && value.wallets.every(w =>
      typeof w.id==='string' && typeof w.name==='string' && w.name.length<=40 &&
      [1,2].includes(w.required) && Array.isArray(w.keys) && w.keys.length===(w.required===2?3:1) &&
      w.keys.every(k => /^demo:public-key-\d+$/.test(k.ref) && /^[A-F\d]{8}$/.test(k.fingerprint)) &&
      new Set(w.keys.map(k=>k.ref)).size===w.keys.length && Number.isSafeInteger(w.balance) && w.balance>=0 &&
      Array.isArray(w.coins) && w.coins.every(TundraCoins.validCoin) && new Set(w.coins.map(c=>c.id)).size===w.coins.length && new Set(w.coins.map(TundraCoins.outpoint)).size===w.coins.length && TundraCoins.total(TundraCoins.unspent(w))===w.balance &&
      Array.isArray(w.history) && w.history.every(h=>typeof h.id==='string'&&TundraCoins.labelOK(h.label||'')) && (w.draft===null || (typeof w.draft==='object' && Array.isArray(w.draft.signedKeys) &&
      Number.isSafeInteger(w.draft.amount) && Number.isSafeInteger(w.draft.fee) && typeof w.draft.to==='string' && typeof w.draft.binding==='string' && TundraCoins.validateDraft(w,w.draft)==='')));
}
let persistence = true;
function loadStore() {
  try {
    const current=localStorage.getItem(STORE_KEY);
    const raw=current===null?localStorage.getItem('relay-coins-demo-v1'):current;
    const v=JSON.parse(raw||'null');
    if(validStore(v)) {
      if(current===null) { try{localStorage.setItem(STORE_KEY,JSON.stringify(v));}catch(_){persistence=false;} }
      return v;
    }
  }
  catch (_) { persistence=false; }
  return clone(INITIAL);
}
let store=loadStore();
if(!store.wallets.some(w=>w.id===store.activeWallet)) store.activeWallet=store.wallets[0].id;
function saveStore(){
  store.wallets.forEach(w=>{if(!w.isImported)TundraCoins.seed(w);});
  // Public descriptors reveal financial information: never persist real imports
  // in this prototype's unencrypted demo storage.
  const safe={...store,wallets:store.wallets.filter(w=>!w.isImported)};
  if(!safe.wallets.some(w=>w.id===safe.activeWallet))safe.activeWallet=safe.wallets[0]?.id;
  try{localStorage.setItem(STORE_KEY,JSON.stringify(safe));}catch(_){persistence=false;}
}
const wallet = () => store.wallets.find(w=>w.id===store.activeWallet);
const policy = w => w.required===2?'2-of-3 multisig':'Single signature';
const btc = (sats, digits=8) => {
  const fraction=String(Math.abs(sats)%100000000).padStart(8,'0');
  return `${sats<0?'-':''}${Math.floor(Math.abs(sats)/100000000)}.${fraction.slice(0,digits)}`;
};
const compactBTC = sats => btc(sats).replace(/0+$/,'').replace(/\.$/,'');
const num = n => n.toLocaleString('en-US');
const displayAmount = (sats, privacy=false) => privacy&&store.hidden?'••••••':store.unit==='sats'?num(sats):btc(sats).replace(/0{1,2}$/,'');
const units = () => store.unit==='sats'?'sats':'BTC';
const amountWithUnit = sats => `${store.unit==='sats'?num(sats):btc(sats)} ${units()}`;
const receiveAddress = w => `demo:${w.id}/receive/${w.receiveIndex}`;
function parseSats(value) {
  if(typeof value!=='string' || !/^\d{1,8}(?:\.\d{1,8})?$/.test(value.trim())) return null;
  const [whole,frac='']=value.trim().split('.');
  const n=Number(whole)*100000000+Number(frac.padEnd(8,'0'));
  return Number.isSafeInteger(n)&&n<=2100000000000000?n:null;
}
function validRecipient(value) { return /^demo:[a-zA-Z0-9][a-zA-Z0-9_/:.-]{2,96}$/.test(value); }
const feeFor=(w,rate)=>rate*(w.required===2?310:140);
const signatureCount = (w,d) => new Set((d?.signedKeys||[]).filter(ref=>w.keys.some(k=>k.ref===ref))).size;
const ready = (w,d=w.draft) => !!d && signatureCount(w,d)>=w.required;
let route='home', sheet=null, form=null, setup=null, transport='qr', phase='show', error='', toastTimer=null;
let lastPayment=null, focusBeforeSheet=null;
let pendingDescriptor=null, appendDescriptor=false, importGeneration=0;
const descriptorCamera=new TundraDescriptorCamera();
const app=document.getElementById('app');
const button=(text, action, kind='primary', extra='')=>`<button class="btn ${kind}" data-action="${e(action)}" ${extra}>${text}</button>`;
const notice=(text,warning=false)=>`<div class="notice ${warning?'warning':''}">${icon(warning?'info':'shield',16)}<div>${text}</div></div>`;
const errorBox=(inSheet=false)=>(error&&(!sheet||inSheet))?`<div class="error" role="alert">${e(error)}</div>`:'';
const tabs = (action, selected=transport) => `<div class="tabs" role="group" aria-label="Hardware connection"><button data-action="${action}:qr" class="${selected==='qr'?'active':''}" aria-pressed="${selected==='qr'}">${icon('qr',16)} QR code</button><button data-action="${action}:usb" class="${selected==='usb'?'active':''}" aria-pressed="${selected==='usb'}">${icon('usb',16)} USB</button></div>`;
const qr=(kind='sign')=>`<div class="qr-wrap"><img src="${QR[kind]||QR.sign}" alt="Demo QR code. Contains non-Bitcoin text only."><p>DEMO ONLY · NOT A BITCOIN PAYLOAD</p></div>`;
const hardware=(approved=false)=>`<div class="hardware-visual" aria-hidden="true"><div class="display">${icon(approved?'check':'lock',21)}<span>${approved?'REVIEW':'YOUR HARDWARE'}</span></div><div class="device-button"></div></div>`;
const progress=(n,total,label)=>`<div class="progress-label"><span>${e(label)}</span><div class="progress-dots" aria-label="${n} of ${total}">${Array.from({length:total},(_,i)=>`<span class="progress-dot ${i<n?'done':''}">${i<n?icon('check',12):i+1}</span>`).join('')}</div></div>`;
function toast(message, isError=false) {
  document.querySelector('.toast')?.remove(); clearTimeout(toastTimer);
  const el=document.createElement('div'); el.className=`toast ${isError?'error-toast':''}`; el.role='status'; el.textContent=message; app.append(el);
  toastTimer=setTimeout(()=>el.remove(),3600);
}
const walletPanelScroll=new Map();
function go(next,{resetScroll=false}={}) {
  const switchingTab=!resetScroll&&['home','coins'].includes(route)&&['home','coins'].includes(next);
  const tabHadFocus=switchingTab&&document.activeElement?.getAttribute('role')==='tab';
  if(switchingTab)walletPanelScroll.set(`${wallet().id}:${route}`,document.querySelector('.screen')?.scrollTop||0);
  descriptorCamera.stop();importGeneration++;route=next; sheet=null; error=''; render();
  if(switchingTab){
    document.querySelector('.screen').scrollTop=walletPanelScroll.get(`${wallet().id}:${next}`)||0;
    if(tabHadFocus)document.querySelector('.wallet-sections [aria-selected="true"]')?.focus({preventScroll:true});
  }
}
function openSheet(value) { descriptorCamera.stop();importGeneration++;focusBeforeSheet=document.activeElement?.getAttribute('data-action'); sheet=value; error=''; render(true); }
function closeSheet() { descriptorCamera.stop();importGeneration++;const target=focusBeforeSheet; sheet=null; error=''; render(true); [...document.querySelectorAll('[data-action]')].find(el=>el.dataset.action===target)?.focus({preventScroll:true}); }
function selectWallet(id) {
  if(!store.wallets.some(w=>w.id===id)) return;
  store.activeWallet=id; saveStore(); form=null; clearCoinUI(); go('home',{resetScroll:true});
}
function startSend() { newForm(); }

function publicConfiguration(w) {
  if(w.isImported)return {format:'tundra-public-descriptor',version:1,name:w.name,network:w.descriptorData.network,descriptors:w.descriptorData.descriptors};
  return {format:'tundra-public-wallet-demo',version:2,notice:'DEMO ONLY. Not a real descriptor or recoverable Bitcoin wallet.',name:w.name,
    network:'demo',policy:{required:w.required,total:w.keys.length,publicKeys:clone(w.keys)}};
}
function download(name,value) {
  const blob=new Blob([JSON.stringify(value,null,2)],{type:'application/json'}), url=URL.createObjectURL(blob);
  const a=document.createElement('a');a.href=url;a.download=name;document.body.appendChild(a);a.click();a.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);
}
function themeToggle() {
  const dark=TundraTheme.get()==='dark';
  return `<button class="icon-btn theme-toggle" data-theme-toggle aria-label="Switch to ${dark?'light':'dark'} theme" title="Switch to ${dark?'light':'dark'} theme">${icon(dark?'sun':'moon',20)}</button>`;
}
function appearanceOptions() {
  return `<div class="appearance-options" role="group" aria-label="Appearance"><button data-theme-choice="dark" aria-pressed="${TundraTheme.get()==='dark'}">${icon('moon',18)} Dark <span class="appearance-default">Default</span></button><button data-theme-choice="light" aria-pressed="${TundraTheme.get()==='light'}">${icon('sun',18)} Light</button></div>`;
}
function syncThemeUI() {
  const dark=TundraTheme.get()==='dark';
  document.querySelectorAll('[data-theme-toggle]').forEach(el=>{ el.innerHTML=icon(dark?'sun':'moon',20);el.setAttribute('aria-label',`Switch to ${dark?'light':'dark'} theme`);el.title=`Switch to ${dark?'light':'dark'} theme`; });
  document.querySelectorAll('[data-theme-choice]').forEach(el=>el.setAttribute('aria-pressed',String(el.dataset.themeChoice===TundraTheme.get())));
}
window.addEventListener('tundra:themechange',syncThemeUI);
document.addEventListener('click',event=>{
  const control=event.target.closest('[data-theme-toggle],[data-theme-choice]');
  if(!control || control.closest('[inert]')) return;
  event.preventDefault();
  if(control.hasAttribute('data-theme-toggle')) TundraTheme.toggle(); else TundraTheme.set(control.dataset.themeChoice);
});
function chrome() {
  if(['home','coins'].includes(route)) return `<header class="chrome wallet-chrome" ${sheet?'inert':''}>${walletIdentity(true)}<span class="demo-tag">Demo</span><button class="icon-btn" data-action="settings" aria-label="Settings">${icon('settings',19)}</button></header>`;
  const titles={coins:'Coins',coinPicker:'Choose coins',consolidate:'Consolidate',send:'Send bitcoin',review:'Review',sign:'Sign payment',ready:'Send bitcoin',success:'Payment',receive:'Receive bitcoin',settings:'Settings',details:'Wallet details',setup:'Add a wallet',setupKeys:'Wallet setup',setupPolicy:'Wallet setup',setupBackup:'Wallet backup',descriptorScan:'Scan descriptor',descriptorReview:'Import wallet'};
  return `<header class="chrome" ${sheet?'inert':''}><button class="icon-btn" data-action="back" aria-label="${['sign','ready'].includes(route)?'Leave payment':'Go back'}">${icon(['sign','ready'].includes(route)?'close':'back',19)}</button><span class="page-title">${titles[route]||'Tundra'}</span><span class="demo-tag">Demo</span></header>`;
}

function homeScreen() {
  const w=wallet();
  if(w.isImported)return importedHomeScreen(w);
  return `<div class="actions">${button(`${icon('receive',17)} Receive`,'receive','secondary')}${button(`${icon('send',17)} Send`,'send')}</div>${w.draft?`<button class="draft-card" data-action="resume"><span class="accent">${icon(ready(w)?'check':'clock',21)}</span><span class="grow"><span class="label">Finish your payment</span><span class="sub" style="display:block">${store.hidden?'••••••':compactBTC(w.draft.amount)+' BTC'} · ${ready(w)?'Ready to send':signatureCount(w,w.draft)+' of '+w.required+' signed'}</span></span>${icon('chevron',16)}</button>`:''}<section class="activity-list" aria-label="Recent activity">${w.history.length?w.history.map(h=>`<button class="activity-row" data-action="history:${e(h.id)}"><span class="activity-icon ${h.direction}">${icon(h.direction==='in'?'receive':h.direction==='self'?'coins':'send',16)}</span><span class="grow"><span class="label">${e(h.label||'Add a label')}</span><span class="subtitle" style="display:block">${e(h.date)} · ${h.subtitle==='In mempool'?'Pending':e(h.subtitle)}</span></span><span class="activity-amount ${h.direction==='in'?'accent':''}">${store.hidden?'••••••':(h.direction==='in'?'+':'−')+(store.unit==='sats'?num(h.amount):compactBTC(h.amount))}<span class="unit">${units()}</span></span></button>`).join(''):`<div class="empty">No transactions yet.<br>Receive bitcoin to get started.</div>`}</section>`;
}

function sendScreen() { return coinSendScreen(); }

function paymentSummary(amount,to,fee,walletName) {
  return `<div class="summary-panel"><div class="review-label">From ${e(walletName)}</div><div class="review-amount">${compactBTC(amount)} <span>BTC</span></div><div class="destination"><span class="review-label">To</span><p class="address">${e(to)}</p></div><div class="summary-row"><span class="label">Network fee</span><span class="value">${num(fee)} sats</span></div><div class="summary-row total"><span class="label">Total</span><span class="value">${btc(amount+fee)} BTC</span></div></div>`;
}
function reviewScreen() { return coinReviewScreen(); }

function signScreen() {
  const w=wallet(), d=w.draft, n=signatureCount(w,d);
  const heading=w.required===1?'Sign with your hardware.':n===0?'Use your first key.':'Now use a second key.';
  const hint=w.required===1?'Your private key stays off this phone.':n===0?'Any two of your three keys can sign.':'Use a different key. The order doesn’t matter.';
  const caption=phase==='scan'?'Show the signed QR code to your phone.':'Scan this code with your hardware wallet.';
  return `${progress(n,w.required,`${n} of ${w.required} signed`)}<div class="sign-intro"><h1>${heading}</h1><p>${hint}</p></div>
    <p class="field-note mt16">${d.mode==='consolidate'?'Consolidation':'Payment'} · ${d.inputs.length} chosen inputs${d.label?' · '+e(d.label):''}</p><div class="payment-mini"><span class="amount">${compactBTC(d.amount)} <span>BTC</span></span><span class="to">${e(d.to)}</span></div>
    ${tabs('transport')}${transport==='qr'?(phase==='scan'?`<div class="scan-window" role="img" aria-label="Simulated camera viewfinder">${icon('qr',85)}<span class="scan-tag">DEMO · CAMERA IS NOT ACTIVE</span></div>`:qr('sign')):hardware(phase==='approve')}
    <p class="${transport==='qr'?'qr-caption':'hardware-caption'}">${transport==='qr'?caption:phase==='approve'?'Check this payment on the device, then approve it there.':'Connect and unlock your hardware wallet.'}</p>${errorBox()}
    <details class="demo-controls"><summary>Demo controls</summary><div>${w.keys.map((k,i)=>`<button data-action="test-key:${i}">Use key ${i+1}</button>`).join('')}<button data-action="test-wrong">Wrong wallet</button><button data-action="test-changed">Changed payment</button></div></details>`;
}
function readyScreen() { return coinReadyScreen(); }

function successScreen() { return coinSuccessScreen(); }

function receiveScreen() {
  const w=wallet();
  return `<div class="page-intro"><h1>Receive bitcoin.</h1><p class="subhead">Into ${e(w.name)}</p></div>${qr('receive')}<div class="receive-address">${e(receiveAddress(w))}</div><div class="receive-status ${w.addressVerified?'verified':''}">${icon(w.addressVerified?'shield':'info',14)} ${w.addressVerified?'Compared on hardware · demo':'Not yet checked on hardware'}</div>
    <div class="mt24">${notice('Compare the full address on your hardware before sharing it.')}</div><p class="field-note center mt16">This is a demo address. Do not send bitcoin.</p>`;
}
function settingsScreen() {
  const w=wallet();
  const row=(ic,title,description,action,value='')=>`<button class="settings-row" data-action="${action}">${icon(ic,19)}<span class="grow"><span class="name">${title}</span><span class="description" style="display:block">${description}</span></span>${value?`<span class="value">${value}</span>`:icon('chevron',14)}</button>`;
  return `<div class="tundra-signature">${icon('tundra',32)}<div><strong>Tundra</strong><p>Your bitcoin. Your hardware.</p></div></div><div class="settings-heading">${e(w.name)}</div>${row('wallets','Wallet details','Name and public wallet configuration','details')}${row('download','Export wallet backup',w.isImported?'Public descriptor · no private keys':'Public configuration only · demo file','export')}${!w.isImported?row('tag','Labels & coin metadata','Import / export labels and frozen status','labels'):''}<section class="appearance-section"><div class="appearance-label">Appearance</div>${appearanceOptions()}<p class="appearance-hint">Dark by default. Your choice is remembered on this browser.</p></section><div class="settings-heading">Display</div>${row('coin','Bitcoin unit','Tap to switch','unit',units())}${row('eyeoff','Hide balances','On the wallet and activity screens','hide',store.hidden?'On':'Off')}<div class="settings-heading">Prototype</div>${row('node','Connection','Offline demo · no network access','about','Demo')}${!w.isImported?row('check','Confirm pending coins','Advance simulated confirmations','confirm-pending'):''}${row('refresh','Reset demo','Restore the example wallets','reset')}
    <div class="mt32">${notice('No private keys. No recovery words. Every payment is signed on hardware.')}</div><p class="design-credit">Adapted from <a href="https://www.bitcoinuikit.com/" target="_blank" rel="noreferrer noopener">Bitcoin UI Kit</a> by the Bitcoin Design Community. CC BY 4.0. Wallet icon: Bitcoin Icons, MIT.</p>${!persistence?`<p class="field-note mt16">Browser storage is unavailable. Demo changes may not survive closing this page.</p>`:''}`;
}
function detailsScreen() {
  const w=wallet();
  if(w.isImported)return importedDetailsScreen(w);
  return `<div class="page-intro"><h1>${e(w.name)}</h1><p class="subhead">${policy(w)}</p></div><div class="panel"><div class="summary-row"><span class="label">To spend</span><span class="value">${w.required===2?'Any 2 of 3 keys':'1 hardware key'}</span></div><div class="summary-row"><span class="label">Private keys on phone</span><span class="value">None</span></div><div class="summary-row"><span class="label">Network</span><span class="value">Demo only</span></div><details class="key-details"><summary>Public wallet data</summary><pre class="code-block">${e(JSON.stringify(publicConfiguration(w).policy,null,2))}</pre><p class="field-note">These are synthetic key references, not real extended public keys.</p></details></div><div class="mt24">${button('Rename wallet','rename','secondary full')}${button(`${icon('download',17)} Export backup`,'export','ghost full','style="margin-top:12px"')}</div><div class="mt24">${notice('The app keeps public wallet information, not device profiles or private signing keys.')}</div>`;
}
function setupScreen() {
  return `<div class="import-hero"><span class="import-emblem">${icon('wallets',29)}</span><p class="eyebrow">PUBLIC INFORMATION. YOUR HARDWARE.</p><h1>Bring your wallet.</h1><p class="subhead">Scan or import its public descriptor.<br>Tundra detects how it signs.</p></div>
    <button class="choice import-choice" data-action="scan-descriptor"><span class="wallet-mark">${icon('scan',25)}</span><span class="grow"><span class="title">Scan descriptor QR</span><span class="description" style="display:block">Use the camera on your phone.</span></span>${icon('chevron',15,'chevron')}</button>
    <button class="choice import-choice" data-action="pick-descriptor"><span class="wallet-mark">${icon('download',23)}</span><span class="grow"><span class="title">Import descriptor file</span><span class="description" style="display:block">Choose a public .txt or .json export.</span></span>${icon('chevron',15,'chevron')}</button>
    <div class="center mt24"><button class="text-button" data-action="paste-descriptor">Paste descriptor instead</button></div>
    <div class="import-supported"><span>${icon('single',15)} Single-sig</span><span>${icon('multi',15)} 2-of-3 multisig</span></div>
    <p class="field-note center">No private keys. No signer profiles.</p>${errorBox()}
    <details class="demo-controls"><summary>Try a public test descriptor</summary><div><button data-action="descriptor-example:single">Single-sig example</button><button data-action="descriptor-example:multi">2-of-3 example</button></div></details>`;
}
function setupKeysScreen() {
  const total=setup.required===2?3:1;
  return `<div class="page-intro"><h1>${setup.required===2?'Add your three keys.':'Connect your hardware.'}</h1><p class="subhead">Read public information from your hardware.<br>No private key is imported.</p></div>
    <label class="form-field"><span class="form-label">Wallet name</span><span class="field-wrap"><input data-setup-name aria-label="Wallet name" maxlength="32" value="${e(setup.name)}" autocomplete="off"></span></label>
    <div class="mt24">${Array.from({length:total},(_,i)=>{const k=setup.keys[i];return `<div class="key-row"><span class="key-num ${k?'added':''}">${k?icon('check',14):i+1}</span><span class="grow"><span class="label">Public key ${i+1}</span>${k?`<span class="sub" style="display:block">${k.fingerprint}</span>`:''}</span><span class="status">${k?'Added':'Not added'}</span></div>`;}).join('')}</div>
    <div class="mt24">${notice('These keys belong to this wallet. There is no separate hardware directory to manage.')}</div>${errorBox()}`;
}
function setupPolicyScreen() {
  const total=setup.keys.length, n=setup.confirmed.length;
  return `${progress(n,total,`${n} of ${total} confirmed`)}<div class="sign-intro"><h1>Confirm on your hardware.</h1><p>Check the same 2-of-3 wallet on all three devices. This is a one-time setup step.</p></div>${tabs('setup-transport',setup.transport)}${setup.transport==='qr'?qr('policy'):hardware(true)}<p class="${setup.transport==='qr'?'qr-caption':'hardware-caption'}">${n<total?`Confirm the wallet policy on the hardware for key ${n+1}.`:'All three keys have confirmed this wallet.'}</p><div class="mt24">${notice('Check the full public-key set and 2-of-3 rule on trusted hardware, not just the wallet name.')}</div>${errorBox()}`;
}
function setupBackupScreen() {
  return `<div class="success-mark">${icon('download',30)}</div><div class="page-intro"><h1>Keep a wallet backup.</h1><p class="subhead">The wallet configuration belongs with your backups, not just on this phone.</p></div><div class="panel"><div class="row">${icon(setup.required===2?'multi':'single',25)}<div class="grow"><h2>${e(setup.name)}</h2><p class="small muted mt8">${setup.required===2?'2-of-3 multisig':'Single signature'}</p></div></div><p class="small muted mt24">Contains public wallet information.<br>Contains no recovery words or private keys.</p></div><div class="mt24">${notice('Prototype export only. This file is not a usable Bitcoin wallet backup.',true)}</div>${setup.backupSaved?`<p class="small accent mt24">${icon('check',15)} Demo configuration exported.</p>`:''}`;
}
function content() {
  return ({home:homeScreen,coins:coinsScreen,coinPicker:coinPickerScreen,consolidate:consolidationScreen,send:sendScreen,review:reviewScreen,sign:signScreen,ready:readyScreen,success:successScreen,receive:receiveScreen,settings:settingsScreen,details:detailsScreen,setup:setupScreen,setupKeys:setupKeysScreen,setupPolicy:setupPolicyScreen,setupBackup:setupBackupScreen,descriptorScan:descriptorScanScreen,descriptorReview:descriptorReviewScreen}[route]||homeScreen)();
}
function footer() {
  if(['coins','coinPicker'].includes(route)){const custom=coinFooter();if(custom)return custom;}
  let label='',action='',note='';
  if(route==='send'){label='Review payment';action='review';}
  if(route==='consolidate'){label='Review consolidation';action='review';}
  if(route==='review'){label=`${form?.mode==='consolidate'?'Sign consolidation':'Sign payment'} ${icon('arrow',17)}`;action='start-sign';note=wallet().required===2?'Use any two of your three hardware keys.':'Use your hardware key.';}
  if(route==='sign'){
    if(transport==='qr'){label=phase==='scan'?'Simulate signed QR scan':`${icon('scan',17)} Scan signed QR`;action=phase==='scan'?'simulate-sign':'scan-response';}
    else {label=phase==='approve'?'Simulate hardware approval':'Simulate USB connection';action=phase==='approve'?'simulate-sign':'usb-connect';}
    note='Demo only · no real hardware connection';
  }
  if(route==='ready'){label=`${icon(wallet().draft?.mode==='consolidate'?'coins':'send',17)} ${wallet().draft?.mode==='consolidate'?'Consolidate coins':'Send bitcoin'}`;action='broadcast';note='Simulated broadcast. No real funds.';}
  if(route==='success'){label='View coins';action='coins';}
  if(route==='receive'){
    label=wallet().addressVerified?`${icon('copy',17)} Copy demo address`:'Check on hardware';action=wallet().addressVerified?'copy-address':'verify-address';
  }
  if(route==='setupKeys'){
    const full=setup.keys.length===(setup.required===2?3:1);
    label=full?'Continue':`${icon('plus',17)} Read public key ${setup.keys.length+1}`;action=full?'setup-keys-done':'read-key';
  }
  if(route==='setupPolicy') {const full=setup.confirmed.length===setup.keys.length;label=full?'Continue':'Simulate hardware confirmation';action=full?'setup-policy-done':'confirm-policy';note='Public wallet policy only. No signing secrets.';}
  if(route==='setupBackup') {label=setup.backupSaved?'Open wallet':`${icon('download',17)} Save demo wallet backup`;action=setup.backupSaved?'finish-setup':'setup-export';}
  if(route==='descriptorScan'){label=`${icon('download',17)} Import file instead`;action='pick-descriptor';note='Plain-text QR only. No animated UR or BBQr support.';}
  if(route==='descriptorReview'){
    return `<footer class="bottom-action" ${sheet?'inert':''}>${button('Add wallet','confirm-descriptor','primary full',pendingDescriptor?.complete?'':'disabled')}<p>Prototype import · no live wallet connection</p></footer>`;
  }
  if(!label) return `<footer class="privacy-footer" ${sheet?'inert':''}>${icon('tundra',15)} <strong>Tundra</strong><span aria-hidden="true">·</span> Your bitcoin. Your hardware.</footer>`;
  return `<footer class="bottom-action" ${sheet?'inert':''}>${button(label,action,'primary full')}${note?`<p>${note}</p>`:''}</footer>`;
}
function sheetContent() {
  const w=wallet();
  let title='',body='';
  const custom=coinSheet();
  if(custom){title=custom.title;body=custom.body;}
  else switch(sheet.kind) {
    case 'wallets':
      title='Your wallets';body=store.wallets.map(v=>`<button class="choice ${v.id===w.id?'selected':''}" data-action="select-wallet:${e(v.id)}" aria-pressed="${v.id===w.id}"><span class="wallet-mark">${icon(v.required===2?'multi':'single',19)}</span><span class="grow"><span class="title">${e(v.name)}</span><span class="description" style="display:block">${policy(v)}</span></span><span class="wallet-choice-amount">${v.isImported?'—':displayAmount(v.balance,true)}<span>${v.isImported?'NOT CONNECTED':units()}</span></span></button>`).join('')+button(`${icon('plus',17)} Add a wallet`,'setup','ghost full');break;
    case 'fees':
      title='Network fee';body=`<p class="sheet-sub">Choose a demo fee rate. These are not live estimates.</p>${[[2,'Low'],[4,'Standard'],[8,'High']].map(([rate,name])=>`<button class="choice ${form.rate===rate?'selected':''}" data-action="fee:${rate}"><span class="grow"><span class="title">${name}</span><span class="description" style="display:block">${rate} sat/vB · ${num(feeFor(w,rate))} sats</span></span>${form.rate===rate?icon('check',19):''}</button>`).join('')}`;break;
    case 'recipient':
      title='Scan an address';body=`<p class="sheet-sub">Point your phone at the recipient’s QR code.</p><div class="scan-window">${icon('qr',80)}<span class="scan-tag">DEMO · NO CAMERA ACCESS</span></div>${button('Use demo recipient','recipient-scanned','primary full')}<p class="field-note center">Simulation only. Real addresses are not accepted.</p>`;break;
    case 'leave':
      title='Keep this payment?';body=`<p class="sheet-sub">Keep a local draft to finish later, or discard it. Nothing has been sent.</p>${button('Keep draft','keep-draft','primary full')}${button('Discard payment','discard-draft','ghost full')}`;break;
    case 'verify':
      title='Check your address';body=`<p class="sheet-sub">Compare the full address with the one shown by your hardware.</p>${tabs('sheet-transport',sheet.transport)}${sheet.transport==='qr'?qr('receive'):hardware(true)}<div class="receive-address">${e(receiveAddress(w))}</div>${button('Simulate matching address','confirm-address','primary full')}<p class="field-note center">One trusted hardware comparison · demo only.</p>`;break;
    case 'rename':
      title='Rename wallet';body=`<label class="form-field"><span class="form-label">Wallet name</span><span class="field-wrap"><input data-rename aria-label="New wallet name" maxlength="32" value="${e(sheet.value)}" autocomplete="off"></span></label>${errorBox(true)}${button('Save name','save-name','primary full')}`;break;
    case 'readKey':
      title=`Read public key ${setup.keys.length+1}`;body=`<p class="sheet-sub">Export the public account information from your hardware. Never enter recovery words.</p>${tabs('sheet-transport',sheet.transport)}${sheet.transport==='qr'?`<div class="scan-window">${icon('qr',80)}<span class="scan-tag">DEMO · NO CAMERA ACCESS</span></div>`:hardware(false)}${errorBox(true)}${button('Simulate public-key import','simulate-key','primary full')}${setup.keys.length?`<details class="demo-controls"><summary>Demo controls</summary><div><button data-action="duplicate-key">Try the same key again</button></div></details>`:''}<p class="field-note center">Only public information is added to this wallet.</p>`;break;
    case 'importWallet':
      title='Import your wallet';body=`<p class="sheet-sub">Read its public descriptor, not individual signer profiles.</p>${button(`${icon('scan',17)} Scan descriptor QR`,'scan-descriptor','primary full')}${button(`${icon('download',17)} Import descriptor file`,'pick-descriptor','secondary full')}`;break;
    case 'pasteDescriptor':
      title=appendDescriptor?'Add missing descriptor':'Paste a descriptor';body=`<p class="sheet-sub">Public descriptors only, including their #checksum. Never paste recovery words or private keys.</p><label class="form-field"><span class="form-label">Descriptor or descriptor JSON</span><textarea id="descriptor-text" aria-label="Public descriptor" spellcheck="false" autocomplete="off" autocapitalize="off" maxlength="65536" placeholder="wpkh([fingerprint/path]xpub…/&#60;0;1&#62;/*)#checksum"></textarea></label>${errorBox(true)}${button('Read descriptor','read-descriptor-text','primary full')}<p class="field-note mt16">Use test data. Imports remain in this tab only.</p>`;break;
    case 'reset':
      title='Reset the demo?';body=`<p class="sheet-sub">This clears imported descriptors from this tab, removes local demo changes, and restores the example wallets.</p>${button('Reset demo','reset-confirm','danger full')}${button('Keep my demo','close','ghost full')}`;break;
    case 'about':
      title='About Tundra';body=`<p class="sheet-sub">One owner. Single-sig or 2-of-3. QR or USB signing. No device address book, participant roles, or approval inbox.</p><div class="panel mt24"><h2>Working in this prototype</h2><p class="small muted mt16">Labeled UTXOs, automatic / manual coin selection, consolidation, freeze controls, output / transaction label import and export, descriptor imports, camera QR in compatible browsers, and simulated hardware signing.</p></div><div class="mt24">${notice('Not implemented: address derivation, blockchain sync, PSBTs, transaction-signature verification, real USB signing, animated QR formats, encryption, or app lock. Imported descriptors cannot receive or spend funds here.',true)}</div><p class="field-note mt16">Synthetic wallet data and the labels you enter are saved in unencrypted local storage. Use test labels. Imported public descriptors stay in memory and disappear on reload. Nothing is uploaded. Camera QR decoding requires browser support.</p>`;break;
    case 'history': {
      const h=w.history.find(h=>h.id===sheet.id);title=h.direction==='in'?'Received':h.direction==='self'?'Consolidated':'Sent';
      body=`<div class="review-amount">${store.hidden?'••••••':compactBTC(h.amount)} <span>BTC</span></div><div class="panel mt24"><div class="summary-row"><span class="label">Wallet</span><span class="value">${e(w.name)}</span></div><div class="summary-row"><span class="label">Date</span><span class="value">${e(h.date)}</span></div><div class="summary-row"><span class="label">Status</span><span class="value">${e(h.subtitle)} · demo</span></div>${h.fee?`<div class="summary-row"><span class="label">Fee</span><span class="value">${num(h.fee)} sats</span></div>`:''}</div>${button(`${icon('tag',17)} Edit label`,'tx-label:'+h.id,'secondary full')}<p class="small muted mt16">${e(h.label||'Unlabeled transaction')}</p>${h.inputs?.length?`<details class="input-breakdown"><summary>${h.inputs.length} original input labels</summary>${h.inputs.map(c=>`<div class="provenance-row"><strong>${e(c.label||'Unlabeled')}</strong><code>${e(TundraCoins.outpoint(c))}</code></div>`).join('')}</details>`:''}<p class="field-note mt16">This is a simulated history entry, not an on-chain transaction.</p>`;break;
    }
  }
  return `<div class="sheet-backdrop" data-backdrop></div><section class="sheet" role="dialog" aria-modal="true" aria-labelledby="sheet-title" tabindex="-1"><div class="sheet-header"><h2 id="sheet-title">${title}</h2><button class="icon-btn" data-action="close" aria-label="Close dialog">${icon('close',19)}</button></div>${body}</section>`;
}
function render(preserve=false) {
  if(descriptorCamera.stream)descriptorCamera.stop();
  const oldScroll=document.querySelector('.screen')?.scrollTop||0;
  const controlsOpen=document.querySelector('.demo-controls')?.open;
  const focusedAction=preserve&&!sheet?document.activeElement?.getAttribute('data-action'):null;
  if(['sign','ready'].includes(route)&&!wallet().draft) route='home';
  if(route==='ready'&&!ready(wallet())) route='sign';
  if(['send','review','consolidate','coinPicker'].includes(route)&&!form) route='home';
  if(route==='coinPicker'&&!coinPicker)route='send';
  if(route.startsWith('setup')&&route!=='setup'&&!setup) route='setup';
  if(route==='descriptorReview'&&!pendingDescriptor)route='setup';
  const walletPanel=['home','coins'].includes(route);
  app.innerHTML=chrome()+walletOverview()+`<div class="screen ${walletPanel?'wallet-panel '+route+'-panel':route==='coinPicker'?'coin-picker-panel':''}" id="screen" ${walletPanel?`role="tabpanel" aria-labelledby="wallet-tab-${route==='home'?'activity':'coins'}" tabindex="0"`:''} ${sheet?'inert':''}>${content()}</div>`+footer()+(sheet?sheetContent():'')+`<input type="file" id="labels-file" accept=".jsonl,.json,.txt,application/x-ndjson,text/plain" hidden><input type="file" id="descriptor-file" accept=".txt,.json,.desc,.descriptor,text/plain,application/json" hidden><input type="file" id="descriptor-image" accept="image/png,image/jpeg,image/webp" hidden>`;
  if(preserve) {document.querySelector('.screen').scrollTop=oldScroll;if(controlsOpen&&document.querySelector('.demo-controls'))document.querySelector('.demo-controls').open=true;}
  if(sheet) requestAnimationFrame(()=>document.querySelector('.sheet')?.focus({preventScroll:true}));
  else if(focusedAction){const target=[...app.querySelectorAll('[data-action]')].find(el=>el.getAttribute('data-action')===focusedAction);if(target&&!target.disabled)target.focus({preventScroll:true});}
}
function validateForm() { return validateCoinForm(false); }

function startSigning() { return coinStartSigning(); }

function receiveSignature(response) {
  const w=wallet(),d=w.draft;
  if(!d) return {ok:false,error:'There is no payment to sign.'};
  const invalid=TundraCoins.validateDraft(w,d);if(invalid)return {ok:false,error:invalid};
  if(ready(w,d)) return {ok:false,error:'This payment already has enough signatures.'};
  if(response.binding!==d.binding) return {ok:false,error:'That response is for a different payment. Nothing was added.'};
  if(!w.keys.some(k=>k.ref===response.keyRef)) return {ok:false,error:'That key doesn’t belong to this wallet. Try one of this wallet’s hardware keys.'};
  if(d.signedKeys.includes(response.keyRef)) return {ok:false,error:'This key has already signed. Use a different one of your hardware keys.'};
  d.signedKeys.push(response.keyRef);saveStore();return {ok:true,count:signatureCount(w,d),ready:ready(w,d)};
}
function applySignature(index=null, mode='normal') {
  const w=wallet(),d=w.draft;if(!d)return;
  const key=index!==null?w.keys[index]:w.keys.find(k=>!d.signedKeys.includes(k.ref));
  if(!key&&mode==='normal')return;
  const result=receiveSignature({keyRef:mode==='wrong'?'demo:unrelated-key':key?.ref,binding:mode==='changed'?'different-payment':d.binding});
  if(!result.ok){error=result.error;render(true);document.querySelector('.error')?.scrollIntoView({block:'nearest'});return;}
  error='';phase='show';if(result.ready)go('ready');else {render();toast('First signature added. Now use another key.');}
}
function broadcast() { return coinBroadcast(); }

function startSetup() {setup=null;pendingDescriptor=null;appendDescriptor=false;go('setup');}
function setSetupType(required) {
  setup={required,name:required===2?'Savings':'Everyday',keys:[],confirmed:[],transport:'qr',backupSaved:false};go('setupKeys');
}
function addSetupKey(key) {
  if(!setup||setup.keys.length>=(setup.required===2?3:1)) return {ok:false,error:'All public keys have been added.'};
  if(setup.keys.some(k=>k.ref===key.ref||k.fingerprint===key.fingerprint)) return {ok:false,error:'This key is already in this wallet. Use a different hardware key.'};
  setup.keys.push(clone(key));setup.confirmed=[];setup.backupSaved=false;return {ok:true};
}
function finishSetup() {
  if(!setup||!setup.backupSaved||setup.keys.length!==(setup.required===2?3:1)||(setup.required===2&&setup.confirmed.length!==3)) return false;
  const id=`wallet-${store.nextId++}`;
  store.wallets.push({id,name:setup.name.trim(),required:setup.required,keys:clone(setup.keys),balance:0,receiveIndex:0,addressVerified:false,draft:null,history:[]});
  store.activeWallet=id;saveStore();setup=null;go('home');toast('Demo wallet added.');return true;
}
function parseDemoConfiguration(raw) {
  if(typeof raw!=='string'||raw.length>32768) throw new Error('Use a small demo JSON backup exported by this prototype.');
  let v;try{v=JSON.parse(raw);}catch(_){throw new Error('This is not a valid demo JSON backup.');}
  if(!['tundra-public-wallet-demo','relay-public-wallet-demo'].includes(v?.format)||v.version!==2||v.network!=='demo'||typeof v.name!=='string'||!v.name.trim()||v.name.length>32)
    throw new Error('Only Tundra demo backups are accepted. Do not import real keys or recovery words.');
  const p=v.policy;
  if(!p||![1,2].includes(p.required)||p.total!==(p.required===2?3:1)||!Array.isArray(p.publicKeys)||p.publicKeys.length!==p.total||
     !p.publicKeys.every(k=>/^demo:public-key-\d+$/.test(k.ref)&&/^[A-F\d]{8}$/.test(k.fingerprint))||new Set(p.publicKeys.map(k=>k.ref)).size!==p.total)
    throw new Error('The demo public-key configuration is incomplete or contains duplicate keys.');
  return {required:p.required,name:v.name.trim(),keys:p.publicKeys.map(k=>({ref:k.ref,fingerprint:k.fingerprint})),confirmed:[],transport:'qr',backupSaved:false};
}
function importConfiguration(text) {
  try{setup=parseDemoConfiguration(text);sheet=null;go(setup.required===2?'setupPolicy':'setupBackup');return {ok:true};}
  catch(err){error=err.message;render(true);return {ok:false,error:err.message};}
}
function back() {
  if(sheet){closeSheet();return;}
  switch(route){
    case 'descriptorScan':go(pendingDescriptor?'descriptorReview':'setup');return;
    case 'descriptorReview':pendingDescriptor=null;appendDescriptor=false;go('setup');return;
    case 'sign':case 'ready':openSheet({kind:'leave'});return;
    case 'coinPicker':{const next=coinPicker?.returnRoute||'send';coinPicker=null;go(next);return;}
    case 'consolidate':go('coins');return;
    case 'review':go(form?.mode==='consolidate'?'consolidate':'send');return;
    case 'details':go('settings');return;
    case 'setupKeys':go('setup');return;
    case 'setupPolicy':go('setupKeys');return;
    case 'setupBackup':go(setup.required===2?'setupPolicy':'setupKeys');return;
    default:go('home');
  }
}
function act(action) {
  const [cmd,arg]=action.split(':');
  if(coinAction(cmd,arg))return;
  switch(cmd) {
    case 'home':go('home');break;
    case 'back':back();break;
    case 'close':closeSheet();break;
    case 'wallet-switcher':openSheet({kind:'wallets'});break;
    case 'select-wallet':selectWallet(arg);break;
    case 'settings':go('settings');break;
    case 'details':go('details');break;
    case 'about':openSheet({kind:'about'});break;
    case 'unit':store.unit=store.unit==='BTC'?'sats':'BTC';saveStore();render(true);break;
    case 'hide':store.hidden=!store.hidden;saveStore();render(true);break;
    case 'receive':if(wallet().isImported){toast('No receive addresses are generated for imported descriptors in this prototype.');break;}go('receive');break;
    case 'send':case 'resume':startSend();break;
    case 'example':form.to='demo:recipient-01';if(!form.amount)form.amount='0.005';error='';render(true);break;
    case 'scan-recipient':openSheet({kind:'recipient'});break;
    case 'recipient-scanned':form.to='demo:recipient-01';closeSheet();break;
    case 'max':form.max=true;form.privacyAck=false;error='';render(true);break;
    case 'fees':openSheet({kind:'coinFees'});break;
    case 'fee':if(TundraCoins.validRate(Number(arg)))form.rate=Number(arg);form.privacyAck=false;closeSheet();break;
    case 'review':error=validateForm();if(error){render(true);document.querySelector('.error')?.scrollIntoView({block:'nearest'});}else {form.privacyAck=false;go('review');}break;
    case 'start-sign':startSigning();break;
    case 'transport':transport=arg;phase='show';error='';render(true);break;
    case 'scan-response':phase='scan';error='';render(true);break;
    case 'usb-connect':phase='approve';error='';render(true);break;
    case 'simulate-sign':applySignature();break;
    case 'test-key':applySignature(Number(arg));break;
    case 'test-wrong':applySignature(null,'wrong');break;
    case 'test-changed':applySignature(null,'changed');break;
    case 'broadcast':broadcast();break;
    case 'keep-draft':go('home');break;
    case 'discard-draft':wallet().draft=null;saveStore();form=null;go('home');break;
    case 'verify-address':openSheet({kind:'verify',transport:'qr'});break;
    case 'sheet-transport':sheet.transport=arg;error='';render(true);break;
    case 'confirm-address':wallet().addressVerified=true;saveStore();closeSheet();toast('Address comparison simulated. This is not a payment address.');break;
    case 'copy-address':
      if(!wallet().addressVerified)return;
      (async()=>{try{await navigator.clipboard.writeText(receiveAddress(wallet()));toast('Demo address copied. Do not send bitcoin to it.');}
      catch(_){toast('Clipboard is unavailable. Select the demo address to copy it.');}})();break;
    case 'export':download(wallet().isImported?'Tundra-public-descriptor.json':`Tundra-DEMO-${wallet().id}-public-wallet.json`,publicConfiguration(wallet()));toast(wallet().isImported?'Public descriptor exported. Keep wallet backups private.':'Demo public configuration exported. Not a real wallet backup.');break;
    case 'rename':openSheet({kind:'rename',value:wallet().name});break;
    case 'save-name':if(!sheet.value.trim()){error='Give this wallet a name.';render(true);break;}wallet().name=sheet.value.trim().slice(0,32);saveStore();closeSheet();break;
    case 'history':openSheet({kind:'history',id:arg});break;
    case 'setup':startSetup();break;
    case 'scan-descriptor':appendDescriptor=false;go('descriptorScan');break;
    case 'scan-missing':appendDescriptor=true;go('descriptorScan');break;
    case 'start-descriptor-camera':startDescriptorCamera();break;
    case 'pick-descriptor':document.getElementById('descriptor-file')?.click();break;
    case 'pick-missing':appendDescriptor=true;document.getElementById('descriptor-file')?.click();break;
    case 'pick-descriptor-image':document.getElementById('descriptor-image')?.click();break;
    case 'paste-descriptor':appendDescriptor=false;openSheet({kind:'pasteDescriptor'});break;
    case 'paste-missing':appendDescriptor=true;openSheet({kind:'pasteDescriptor'});break;
    case 'read-descriptor-text':{const field=document.getElementById('descriptor-text'),value=field?.value||'';if(field)field.value='';readDescriptor(value);break;}
    case 'descriptor-example':appendDescriptor=false;readDescriptor(JSON.stringify(DESCRIPTOR_EXAMPLES[arg==='single'?'single':'multi']));break;
    case 'confirm-descriptor':confirmDescriptor();break;
    case 'setup-type':setSetupType(Number(arg));break;
    case 'read-key':openSheet({kind:'readKey',transport:'qr'});break;
    case 'simulate-key':{const key=demoKeys(3).find(k=>!setup.keys.some(v=>v.ref===k.ref));const result=addSetupKey(key);if(result.ok)closeSheet();else{error=result.error;render(true);}break;}
    case 'duplicate-key':{const result=addSetupKey(setup.keys[0]);error=result.error;render(true);break;}
    case 'setup-keys-done':
      if(!setup.name.trim()){error='Give your wallet a name.';render();return;}
      if(setup.keys.length!==(setup.required===2?3:1))return;
      setup.name=setup.name.trim();go(setup.required===2?'setupPolicy':'setupBackup');break;
    case 'setup-transport':setup.transport=arg;render(true);break;
    case 'confirm-policy':if(setup.confirmed.length<setup.keys.length)setup.confirmed.push(setup.keys[setup.confirmed.length].ref);render();break;
    case 'setup-policy-done':if(setup.confirmed.length===setup.keys.length)go('setupBackup');break;
    case 'setup-export':download('Tundra-DEMO-public-wallet.json',publicConfiguration(setup));setup.backupSaved=true;render();break;
    case 'finish-setup':finishSetup();break;
    case 'import-wallet':openSheet({kind:'importWallet'});break;
    case 'pick-file':document.getElementById('demo-file')?.click();break;
    case 'example-import':importConfiguration(JSON.stringify(publicConfiguration({...store.wallets[0],name:'Imported savings'})));break;
    case 'reset':openSheet({kind:'reset'});break;
    case 'reset-confirm':clearCoinUI();store=clone(INITIAL);form=null;setup=null;pendingDescriptor=null;appendDescriptor=false;saveStore();go('home');break;
  }
}
app.addEventListener('click',event=>{
  const b=event.target.closest('[data-action]');
  if(b&&!b.disabled){event.preventDefault();act(b.dataset.action);return;}
  if(event.target.matches('[data-backdrop]'))closeSheet();
});
app.addEventListener('input',event=>{
  const el=event.target;
  if(el.dataset.field&&form){form[el.dataset.field]=el.value;error='';}
  if(el.hasAttribute('data-setup-name')&&setup)setup.name=el.value;
  if(el.hasAttribute('data-rename')&&sheet)sheet.value=el.value;
  if(el.hasAttribute('data-descriptor-name')&&pendingDescriptor)pendingDescriptor.name=el.value;
});
app.addEventListener('change',async event=>{
  if(['descriptor-file','descriptor-image'].includes(event.target.id)){
    const file=event.target.files?.[0];event.target.value='';if(!file)return;
    const generation=importGeneration;
    try{
      if(event.target.id==='descriptor-image'){
        const raw=await descriptorCamera.readImage(file);if(generation!==importGeneration)return;await readDescriptor(raw);
      }else{
        if(file.size>TundraDescriptor.MAX_SIZE)throw new Error('Choose a public descriptor file smaller than 64 KB.');
        const raw=await file.text();if(generation!==importGeneration)return;await readDescriptor(raw);
      }
    }catch(err){if(generation===importGeneration){error=err.message;render(true);}}
    return;
  }
  if(event.target.id!=='demo-file')return;
  const file=event.target.files[0];if(!file)return;
  if(file.size>32768){error='Use a small demo backup. Real wallet files are not accepted.';render(true);return;}
  try {importConfiguration(await file.text());}catch(_){error='The file could not be read.';render(true);}
});
document.addEventListener('keydown',event=>{
  if(!sheet&&event.target.closest('.wallet-sections')&&['ArrowLeft','ArrowRight','Home','End'].includes(event.key)){
    event.preventDefault();
    const next=event.key==='Home'?'home':event.key==='End'?'coins':route==='home'?'coins':'home';
    go(next);document.querySelector('.wallet-sections [aria-selected="true"]')?.focus({preventScroll:true});return;
  }
  if(event.key==='Escape'&&sheet){closeSheet();return;}
  if(event.key==='Tab'&&sheet){
    const box=document.querySelector('.sheet');
    const targets=[...box.querySelectorAll('button:not(:disabled),input:not(:disabled):not([hidden]),textarea:not(:disabled),select:not(:disabled),summary,[tabindex="0"]')].filter(el=>el.getClientRects().length);
    const first=targets[0],last=targets[targets.length-1];
    if(event.shiftKey&&(document.activeElement===first||document.activeElement===box)){event.preventDefault();last?.focus();}
    else if(!event.shiftKey&&(document.activeElement===last||document.activeElement===box)){event.preventDefault();first?.focus();}
  }
});
document.querySelector('.wordmark')?.addEventListener('click',event=>{event.preventDefault();go('home');});
document.querySelectorAll('[data-tour]').forEach(b=>b.addEventListener('click',()=>{
  if(['savings','everyday'].includes(b.dataset.tour)){selectWallet(b.dataset.tour);return;}
  if(b.dataset.tour==='coins'){selectWallet('savings');go('coins');return;}
  if(b.dataset.tour==='send'){selectWallet('savings');startSend();return;}
  startSetup();
}));
function descriptorNetworkLabel(network){return ({mainnet:'Bitcoin mainnet','test-family':'Test-network keys (tpub)',testnet:'Testnet',testnet3:'Testnet3',testnet4:'Testnet4',signet:'Signet',regtest:'Regtest'})[network]||network;}
function importedHomeScreen(w){
  return `    <div class="actions">${button(`${icon('receive',17)} Receive`,'receive','secondary','disabled')}${button(`${icon('send',17)} Send`,'send','primary','disabled')}</div>
    <div class="descriptor-state"><span class="wallet-mark">${icon('check',20)}</span><span class="grow"><strong>Wallet configuration added</strong><span>${e(descriptorNetworkLabel(w.descriptorData.network))}<br>Receive + change descriptors included.</span></span></div>
    <p class="small muted mt16">This prototype is not connected to Bitcoin. It cannot check your balance, generate receive addresses, or send payments.</p>
    ${button('View public descriptor','details','ghost full','style="margin-top:18px"')}
    <div class="mt24">${notice('Imported data stays in this tab and is cleared on reload. Use test descriptors.',true)}</div>`;
}
function descriptorScanScreen(){
  return `<div class="page-intro"><h1>${appendDescriptor?'Scan the missing branch.':'Scan your descriptor.'}</h1><p class="subhead">Show the wallet’s public descriptor QR code.<br>Never scan a seed or a private key.</p></div>
    <div class="descriptor-camera" id="descriptor-camera-panel"><video id="descriptor-video" muted playsinline aria-label="Descriptor QR camera preview"></video><span class="camera-placeholder">${icon('scan',65)}</span><span class="camera-corner tl"></span><span class="camera-corner tr"></span><span class="camera-corner bl"></span><span class="camera-corner br"></span><span class="camera-label" id="camera-label">CAMERA OFF</span></div>
    <div class="mt24">${button(`${icon('scan',17)} Enable camera`,'start-descriptor-camera','primary full','id="camera-enable"')}</div>
    <p id="scan-status" class="field-note center mt16" role="status">Camera access requires a compatible browser and your permission. Images are not uploaded.</p>${errorBox()}
    <div class="center mt16"><button class="text-button" data-action="pick-descriptor-image">Choose a QR image</button></div>`;
}
function descriptorReviewScreen(){
  const d=pendingDescriptor,missing=d.receive?'change':'receive';
  return `<div class="import-verified">${icon('check',14)} CHECKSUM CHECKED</div><div class="page-intro"><h1>${d.complete?'Your wallet, found.':'One more descriptor.'}</h1><p class="subhead">${d.complete?'Confirm the details. That’s all you need.':`Add the matching ${missing} descriptor to complete this wallet.`}</p></div>
    <label class="form-field"><span class="form-label">Wallet name</span><span class="field-wrap"><input data-descriptor-name aria-label="Wallet name" maxlength="32" autocomplete="off" value="${e(d.name)}"></span></label>
    <div class="panel mt24"><div class="summary-row"><span class="label">Wallet type</span><span class="value">${policy(d)}</span></div><div class="summary-row"><span class="label">To spend</span><span class="value">${d.required===2?'Any 2 of your 3 keys':'Your hardware key'}</span></div><div class="summary-row"><span class="label">Network</span><span class="value">${e(descriptorNetworkLabel(d.network))}</span></div><div class="summary-row"><span class="label">Receive + change</span><span class="value ${d.complete?'accent':''}">${d.complete?'Both included':`${missing==='change'?'Change':'Receive'} missing`}</span></div></div>
    ${!d.complete?`<div class="mt16">${button(`${icon('scan',17)} Scan ${missing} QR`,'scan-missing','secondary full')}${button(`${icon('download',17)} Import ${missing} file`,'pick-missing','ghost full','style="margin-top:10px"')}</div>`:''}
    ${d.network==='test-family'?'<p class="field-note mt16">tpub keys do not distinguish testnet, signet, and regtest. A live wallet must select the exact network before connecting.</p>':''}
    <details class="key-details"><summary>View public descriptor${d.descriptors.length>1?'s':''}</summary>${d.descriptors.map(desc=>`<pre class="code-block">${e(desc)}</pre>`).join('')}<p class="field-note mt16">${d.keys.length} public ${d.keys.length===1?'key':'keys'}, with origins and derivation paths. No signer profiles. A checksum is not proof that this is your wallet; verify on trusted hardware before using a real wallet.</p></details>
    <p class="field-note mt24">Prototype only. Use test descriptors. Imports stay in memory for this tab and disappear on reload. No blockchain connection.</p>${errorBox()}`;
}
function importedDetailsScreen(w){
  const d=w.descriptorData;
  return `<div class="page-intro"><h1>${e(w.name)}</h1><p class="subhead">${policy(w)} · imported descriptor</p></div><div class="panel"><div class="summary-row"><span class="label">To spend</span><span class="value">${w.required===2?'Any 2 of 3 keys':'1 hardware key'}</span></div><div class="summary-row"><span class="label">Private keys on phone</span><span class="value">None imported</span></div><div class="summary-row"><span class="label">Network</span><span class="value">${e(descriptorNetworkLabel(d.network))}</span></div><div class="summary-row"><span class="label">Status</span><span class="value">Not connected</span></div><details class="key-details"><summary>Public wallet descriptors</summary>${d.descriptors.map(desc=>`<pre class="code-block">${e(desc)}</pre>`).join('')}</details></div><div class="mt24">${button('Rename wallet','rename','secondary full')}${button(`${icon('download',17)} Export public descriptor`,'export','ghost full','style="margin-top:12px"')}</div><div class="mt24">${notice('Public descriptors reveal wallet information. Keep exports private. This tab does not persist imported data.',true)}</div>`;
}
async function readDescriptor(raw){
  descriptorCamera.stop();const generation=++importGeneration;
  const previous=pendingDescriptor,append=appendDescriptor;
  try{
    const parsed=append&&previous?await TundraDescriptor.merge(previous,raw):await TundraDescriptor.parse(raw);
    if(generation!==importGeneration)return {ok:false,cancelled:true};
    parsed.name=parsed.name.trim()||(parsed.required===2?'Savings':'Everyday');pendingDescriptor=parsed;appendDescriptor=!parsed.complete;
    go('descriptorReview');return {ok:true};
  }catch(err){
    if(generation!==importGeneration)return {ok:false,cancelled:true};
    error=err.message;render(true);return {ok:false,error:err.message};
  }
}
function confirmDescriptor(){
  if(!pendingDescriptor?.complete)return false;
  const name=pendingDescriptor.name.trim();
  if(!name){error='Give your wallet a name.';render(true);return false;}
  const existing=store.wallets.find(w=>w.isImported&&w.descriptorData.identity===pendingDescriptor.identity);
  if(existing){pendingDescriptor=null;appendDescriptor=false;selectWallet(existing.id);toast('This wallet is already imported.');return true;}
  const d=clone(pendingDescriptor),id=`imported-${store.nextId++}`;
  store.wallets.push({id,name,required:d.required,keys:d.keys,isImported:true,descriptorData:d,balance:0,receiveIndex:0,addressVerified:false,draft:null,history:[]});
  store.activeWallet=id;pendingDescriptor=null;appendDescriptor=false;saveStore();go('home');toast('Public wallet imported. Not connected to Bitcoin.');return true;
}
function startDescriptorCamera(){
  const video=document.getElementById('descriptor-video');if(!video)return;
  const button=document.getElementById('camera-enable');if(button){button.disabled=true;button.textContent='Opening camera…';}
  descriptorCamera.start(video,readDescriptor,(message,isError,active)=>{
    const status=document.getElementById('scan-status');if(status){status.className=isError?'error':'field-note center mt16';status.textContent=message;}
    document.getElementById('descriptor-camera-panel')?.classList.toggle('active',active);
    const label=document.getElementById('camera-label');if(label)label.textContent=active?'CAMERA ACTIVE':'CAMERA OFF';
    const control=document.getElementById('camera-enable');if(control){control.disabled=active;control.textContent=active?'Scanning descriptor…':'Enable camera';}
  });
}
document.addEventListener('visibilitychange',()=>{if(document.hidden){descriptorCamera.stop();if(route==='descriptorScan'){error='Camera paused. Enable it again to scan.';render(true);}}});
window.addEventListener('pagehide',()=>descriptorCamera.stop());

// Test hooks: descriptor imports remain memory-only; signing hooks are simulations.
window.TundraDemo=Object.freeze({
  getState:()=>clone(store), getView:()=>({route,transport,phase,sheet:sheet?.kind,setup:clone(setup)}),
  reset:()=>{clearCoinUI();walletPanelScroll.clear();store=clone(INITIAL);setup=null;form=null;pendingDescriptor=null;appendDescriptor=false;saveStore();go('home',{resetScroll:true});},
  selectWallet, parseSats, parseDemoConfiguration, receiveSignature,
  getConfiguration:()=>publicConfiguration(wallet()),
  addSetupKey, finishSetup, broadcast, readDescriptor, getPendingDescriptor:()=>clone(pendingDescriptor),
  navigate:next=>{if(next==='send')startSend();else if(next==='setup')startSetup();else go(next);},
  getTheme:()=>TundraTheme.get(),
  refresh:()=>render()
});
render();
syncThemeUI();
