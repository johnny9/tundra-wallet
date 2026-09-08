"use strict";
/* UI extension for labeled coins. All balances/outpoints/transactions are demo data. */
let coinSelecting=false;
let coinSelection=[], coinFilter='all', coinSearch='', coinSort='newest';
let coinPicker=null, labelImportReview=null;
const coinById=(id,w=wallet())=>(w.coins||[]).find(c=>c.id===id);
const coinMoney=n=>store.hidden?'••••••':`${store.unit==='sats'?num(n):compactBTC(n)} ${units()}`;
const coinStatus=(w,c)=>c.spent?'Spent':TundraCoins.reserved(w,c)?'In draft':c.frozen?'Frozen':!c.confirmed?'Pending':'Available';
const shortOutpoint=c=>`${c.txid.slice(0,8)}…${c.txid.slice(-4)}:${c.vout}`;
function clearCoinUI(){coinSelecting=false;coinSelection=[];coinFilter='all';coinSearch='';coinSort='newest';coinPicker=null;labelImportReview=null;}
/* One fixed wallet overview is reused by Activity, Coins, and the input picker.
 * The balance is always the whole wallet; available and selected amounts are separate.
 */
function walletSections(selected='activity'){
  return `<div class="wallet-sections" role="tablist" aria-label="Wallet view"><button id="wallet-tab-activity" role="tab" aria-controls="screen" tabindex="${selected==='activity'?0:-1}" aria-selected="${selected==='activity'}" data-action="home">Activity</button><button id="wallet-tab-coins" role="tab" aria-controls="screen" tabindex="${selected==='coins'?0:-1}" aria-selected="${selected==='coins'}" data-action="coins">Coins <span>${wallet().isImported?'—':TundraCoins.unspent(wallet()).length}</span></button></div>`;
}
function walletIdentity(selectable=true){
  const w=wallet(),tag=selectable?'button':'div';
  return `<${tag} class="wallet-picker ${selectable?'wallet-switch':'wallet-context'}" ${selectable?`data-action="wallet-switcher" aria-label="Switch wallet, ${e(w.name)}"`: 'aria-label="Payment wallet"'}><span class="wallet-mark tundra-wallet-mark" aria-label="Tundra">${icon('tundra',25)}</span><span class="column"><span class="name">${e(w.name)}${selectable?icon('chevron',14):''}</span><span class="policy-label">${policy(w)}</span></span></${tag}>`;
}

function walletBalance(){
  const w=wallet(),imported=w.isImported,available=imported?[]:TundraCoins.available(w);
  const value=imported?'—':store.hidden?'••••••':store.unit==='sats'?num(w.balance):compactBTC(w.balance);
  const long=value.length>14?' balance-long':value.length>11?' balance-medium':'';
  const availableValue=store.hidden?'••••••':store.unit==='sats'?num(TundraCoins.total(available)):compactBTC(TundraCoins.total(available));
  return `<section class="balance-section compact-balance" aria-label="Wallet balance"><div class="balance-caption"><span>Total balance</span>${imported?'<span class="connection-badge">Not connected</span>':`<button class="icon-btn" data-action="hide" aria-label="${store.hidden?'Show':'Hide'} balances">${icon(store.hidden?'eyeoff':'eye',18)}</button>`}</div><div class="big-balance ${store.unit==='sats'?'sats':''}${long}"><span data-balance-amount>${value}</span><button class="unit-button" data-action="unit" aria-label="Change balance unit">${units()}</button></div>${imported?'<p class="balance-helper">No synced balance in this prototype.</p>':`<button class="balance-availability coin-availability" data-action="balance-details" aria-label="View available, frozen, pending, and reserved balance"><span>${availableValue} ${units()} available</span>${icon('chevron',12)}</button>`}</section>`;
}

function walletOverview(){
  if(!['home','coins','coinPicker'].includes(route))return '';
  const picking=route==='coinPicker';
  return `<div class="wallet-overview ${picking?'picker-overview':''}" role="region" aria-label="Wallet overview" ${sheet?'inert':''}>${picking?`<div class="picker-wallet-name">${e(wallet().name)} <span>· ${policy(wallet())}</span></div>`:''}${walletBalance()}${picking?'':walletSections(route==='coins'?'coins':'activity')}</div>`;
}

function visibleCoins(){
  const w=wallet(),query=coinSearch.trim().toLowerCase();
  const list=TundraCoins.unspent(w).filter(c=>{
    const filter=coinFilter==='all'||coinFilter==='available'&&TundraCoins.usable(w,c)||coinFilter==='frozen'&&c.frozen||coinFilter==='unlabeled'&&!c.label.trim();
    return filter&&(!query||[c.label,c.address,TundraCoins.outpoint(c)].some(v=>v.toLowerCase().includes(query)));
  });
  return list.sort((a,b)=>coinSort==='smallest'?a.sats-b.sats||a.id.localeCompare(b.id):coinSort==='largest'?b.sats-a.sats||a.id.localeCompare(b.id):b.time-a.time||a.id.localeCompare(b.id));
}
function selectedCoins(ids=coinSelection){return ids.map(id=>coinById(id)).filter(Boolean);}
function coinToolbar(){
  const active=coinFilter!=='all'||coinSort!=='newest';
  return `<div class="coin-tools-row"><label class="coin-search">${icon('search',18)}<input type="search" data-coin-search aria-label="Search coins by label, address, or transaction" placeholder="Search coins" value="${e(coinSearch)}" autocomplete="off"></label><button class="filter-button ${active?'has-filter':''}" data-action="coin-filters" aria-label="Filter and sort coins${active?', active filter':''}">${icon('sliders',19)}${active?'<span class="filter-dot" aria-hidden="true"></span>':''}</button></div>${active?`<div class="active-filter"><button data-action="coin-reset-filters" aria-label="Clear filter and sort">${e(({all:'All coins',available:'Available',frozen:'Frozen',unlabeled:'Unlabeled'})[coinFilter])}${coinSort!=='newest'?' · '+(coinSort==='smallest'?'Smallest first':'Largest first'):''} ${icon('close',12)}</button></div>`:''}`;
}
function coinListHeading(picking=false){
  const selecting=picking||coinSelecting;
  return `<div class="coin-list-heading"><span id="coin-result-count">${visibleCoins().length} coins</span><span class="coin-list-options">${selecting?`<button class="text-button" data-action="coin-select-visible">Select visible</button>`:''}${!picking?`<button class="text-button" data-action="${selecting?'coin-end-select':'coin-start-select'}">${selecting?'Cancel':'Select'}</button>`:''}</span></div>`;
}

function coinRows(picking=false){
  const w=wallet(),list=visibleCoins(),ids=picking?(coinPicker?.ids||[]):coinSelection,selecting=picking||coinSelecting;
  if(!list.length)return `<div class="coin-empty">${icon('search',26)}<h2>No matching coins</h2><p>Try another label or clear the filters.</p>${button('Clear filters','coin-clear-filter','ghost compact')}</div>`;
  return `<div class="coin-list ${selecting?'selecting':''}">${list.map(c=>{
    const selected=ids.includes(c.id),status=coinStatus(w,c),disabled=!TundraCoins.usable(w,c);
    const action=selecting&&!disabled?'coin-toggle:':'coin-detail:';
    const role=selecting&&!disabled?`role="checkbox" aria-checked="${selected}"`:'';
    const hint=selecting?(disabled?`${status}. View details`:'Select'):'Edit label and view';
    return `<div class="coin-row ${selected?'is-selected':''} ${disabled?'is-unavailable':''}" data-coin-id="${e(c.id)}"><button class="coin-detail-button" ${role} data-action="${action}${e(c.id)}" aria-label="${hint} ${e(c.label||'unlabeled coin')}, ${e(shortOutpoint(c))}">${selecting?`<span class="coin-select-mark ${disabled?'unavailable':''}" aria-hidden="true">${selected?icon('check',14):disabled?icon(c.frozen?'lock':'clock',13):''}</span>`:''}<span class="coin-row-content"><span class="coin-row-top"><span class="coin-label ${!c.label?'no-label':''}">${e(c.label||'Add a label')}</span><span class="coin-value">${store.hidden?'••••••':store.unit==='sats'?num(c.sats):compactBTC(c.sats)}<span>${units()}</span></span></span><span class="coin-row-bottom"><span>${e(c.date)}${c.isChange?' · Change':''}</span>${status!=='Available'?`<span class="coin-status ${status.toLowerCase().replace(' ','-')}">${status==='Frozen'?icon('lock',11):''}${status}</span>`:''}</span></span></button></div>`;
  }).join('')}</div>`;
}

function coinsScreen(){
  const w=wallet();
  if(w.isImported)return `<div class="coin-empty">${icon('node',32)}<h1>Not connected</h1><p>No coins have been synced. This prototype has no Bitcoin backend.</p><p>Your imported public configuration is available in wallet settings.</p></div>`;
  return `${w.draft?`<button class="coin-draft-notice" data-action="resume">${icon('clock',15)} ${w.draft.inputs?.length||0} coins reserved in your draft ${icon('chevron',14)}</button>`:''}${coinToolbar()}${coinListHeading()}<div id="coin-list-region">${coinRows()}</div>${!coinSelecting?`<div class="coin-note-row"><p class="coin-local-note">Tap a coin to label it. Select to spend.</p><button class="icon-btn" data-action="coin-help" aria-label="About coins and privacy">${icon('info',18)}</button></div>`:''}${errorBox()}`;
}

function coinPickerScreen(){
  if(!coinPicker||!form)return '';
  const amount=parseSats(form.amount);
  return `<div class="coin-picker-target"><span>${form.mode==='consolidate'?'Consolidating':form.max?'Send maximum':'Payment'}</span><strong>${form.max||form.mode==='consolidate'?'Selected amount minus fee':amount?coinMoney(amount):'Amount not set'}</strong></div><div class="picker-selection-mode"><span>Only selected coins will be used.</span>${form.mode==='consolidate'?'':`<button class="text-button" data-action="coins-auto">Automatic</button>`}</div>${coinToolbar()}${coinListHeading(true)}<div id="coin-list-region">${coinRows(true)}</div>`;
}

function coinFooter(){
  const picking=route==='coinPicker',ids=picking?(coinPicker?.ids||[]):coinSelection,coins=selectedCoins(ids),n=coins.length;
  if(!picking&&!coinSelecting&&!n)return null;
  const disabled=!n||coins.some(c=>!TundraCoins.usable(wallet(),c));
  let feedback='';
  if(picking&&n){
    const p=formPlan(coinPicker.ids),hasAmount=!!parseSats(form.amount)||form.max||form.mode==='consolidate';
    feedback=p.ok?`<p class="picker-feedback">${icon('check',13)} ${form.mode==='consolidate'?'Ready to consolidate':form.max?'Ready to send maximum':'Covers payment and fee'}</p>`:hasAmount?`<p class="picker-feedback needs-more" role="status">${e(p.error)}</p>`:'';
  }
  return `<footer class="bottom-action coin-selection-footer" ${sheet?'inert':''}><div class="selection-summary" role="status" aria-live="polite"><div><strong>${n?n+' selected':'Select coins'}</strong><span>${n?coinMoney(TundraCoins.total(coins)):'Tap the coins you want to use'}</span></div>${n?`<button class="text-button" data-action="coin-clear-selection">Clear</button>`:''}</div>${feedback}${picking?button(`Use ${n||''} ${n===1?'coin':'coins'}`,'coins-use','primary full',disabled?'disabled':''):`<div class="selection-actions">${button('Send selected','coins-send','primary',disabled?'disabled':'')}<button class="btn secondary more-selected" data-action="coin-selection-actions" ${disabled?'disabled':''} aria-label="More selected coin actions">${icon('more',19)} More</button></div>`}</footer>`;
}

function formPlan(ids=undefined){
  if(!form)return {ok:false,error:'Start a payment first.',inputs:[],inputTotal:0};
  return TundraCoins.plan(wallet(),{selected:ids===undefined?form.selected:ids,amount:parseSats(form.amount)||0,rate:form.rate,max:form.max,mode:form.mode||'send'});
}
function newForm(ids=null,mode='send'){
  const w=wallet();
  if(w.isImported){toast('Imported configuration only. No synced coins in this prototype.');return;}
  if(w.draft){transport='qr';phase='show';go(ready(w)?'ready':'sign');toast('Finish or discard this draft before selecting other coins.');return;}
  form={to:'',amount:'',rate:mode==='consolidate'?2:4,selected:ids?ids.slice():null,mode,max:mode==='consolidate',label:'',privacyAck:false};
  if(mode==='consolidate'){
    const coins=selectedCoins(ids||[]),labels=[...new Set(coins.map(c=>c.label).filter(Boolean))];
    form.label=labels.length===1?labels[0]:`${w.name} · Consolidated`;
    form.to=`demo:${w.id}/change/${w.changeIndex||1}`;
  }
  coinPicker=null;go(mode==='consolidate'?'consolidate':'send');
}
function currentInputSummary(){
  const selected=form.selected!==null,n=selected?form.selected.length:0;
  return `<button class="coin-input-control" data-action="choose-coins"><span class="grow"><strong>Coins</strong></span><span class="control-value">${selected?`${n} selected`:'Automatic'}</span>${icon('chevron',15)}</button>`;
}

function currentFeeButton(){
  const p=formPlan();
  return `<button class="fee-button" data-action="fees"><span class="grow label">Network fee<small>Demo estimate</small></span><span class="value" id="fee-preview">${p.ok?num(p.fee)+' sats':'—'}<small>${form.rate} sat/vB</small></span>${icon('chevron',15)}</button>`;
}

function coinSendScreen(){
  const w=wallet(),p=formPlan();
  if(form.max&&p.ok)form.amount=btc(p.amount);
  const available=form.selected!==null?TundraCoins.total(selectedCoins(form.selected)):TundraCoins.total(TundraCoins.available(w));
  return `<p class="send-wallet-context">From <strong>${e(w.name)}</strong></p><label class="form-field"><span class="form-label">Recipient</span><span class="field-wrap"><input data-field="to" aria-label="Recipient" placeholder="Scan or paste an address" autocomplete="off" spellcheck="false" value="${e(form.to)}" maxlength="102"><button class="icon-btn" data-action="scan-recipient" aria-label="Scan recipient QR">${icon('scan',21)}</button></span></label><div class="field-footer"><span class="field-note">Demo addresses only.</span><button class="text-button" data-action="example">Use example</button></div><label class="form-field"><span class="form-label">Amount</span><span class="field-wrap amount"><input data-field="amount" aria-label="Amount in BTC" inputmode="decimal" autocomplete="off" placeholder="0.00" value="${e(form.amount)}"><span class="unit">BTC</span></span></label><div class="field-footer"><span class="field-note">${coinMoney(available)} ${form.selected!==null?'selected':'available'}</span><button class="text-button" data-action="max">${form.selected!==null?'Send selected max':'Send max'}</button></div><label class="form-field label-field"><span class="form-label">Payment label <span class="quiet">· optional</span></span><span class="field-wrap">${icon('tag',17)}<input data-field="label" aria-label="Payment label" maxlength="255" placeholder="What is this payment for?" value="${e(form.label)}" autocomplete="off"></span></label><div class="send-options">${currentInputSummary()}${currentFeeButton()}</div>${errorBox()}`;
}
function consolidationScreen(){
  const w=wallet(),p=formPlan();
  return `<div class="page-intro"><h1>Fewer coins.<br>Same wallet.</h1><p class="subhead">Combine selected UTXOs into one new coin in ${e(w.name)}.</p></div><div class="consolidation-flow"><div><span class="stack-symbol">${icon('coins',30)}</span><strong>${form.selected?.length||0} coins</strong><small>Selected inputs</small></div>${icon('arrow',23)}<div><span class="stack-symbol result">${icon('coin',30)}</span><strong>1 coin</strong><small>Your own wallet</small></div></div>${currentInputSummary()}<label class="form-field"><span class="form-label">Label the new coin</span><span class="field-wrap">${icon('tag',17)}<input data-field="label" aria-label="Consolidated coin label" maxlength="255" placeholder="e.g. Mining savings" value="${e(form.label)}" autocomplete="off"></span></label>${currentFeeButton()}${p.ok?`<div class="consolidation-amount"><span>New coin after fees</span><strong>${compactBTC(p.amount)} <small>BTC</small></strong><p>Only the ${num(p.fee)}-sat fee leaves your wallet.</p></div>`:errorBox()||`<div class="error" role="alert">${e(p.error)}</div>`}<div class="privacy-callout"><span>${icon('info',19)}</span><p>Combining inputs links them publicly. Keep coins separate when their sources should not be connected.</p></div><p class="field-note mt16">Next: review the inputs, your own destination, and the fee before hardware signing.</p>${p.ok?errorBox():''}`;
}
function inputBreakdown(p){
  return `<details class="input-breakdown"><summary><span>${icon('coins',18)} ${p.inputs.length} ${p.inputs.length===1?'input':'inputs'} selected</span><span>${p.selectionMode?p.selectionMode==='manual'?'Manual':'Automatic':form?.selected!==null?'Manual':'Automatic'} ${icon('chevron',14)}</span></summary><div class="input-rows">${p.inputs.map(c=>`<div><span><strong>${e(c.label||'Unlabeled coin')}</strong><small>${e(shortOutpoint(c))}</small></span><b>${compactBTC(c.sats)} BTC</b></div>`).join('')}</div></details>`;
}
function coinPlanSummary(p,w,details={}){
  const consolidating=p.mode==='consolidate',to=details.to||form?.to||'',label=details.label??form?.label??'';
  return `<div class="summary-panel coin-review-panel"><div class="review-label">${consolidating?'New coin in':'From'} ${e(w.name)}</div><div class="review-amount">${compactBTC(p.amount)} <span>BTC</span></div>${label?`<div class="review-tag">${icon('tag',14)}<span>${e(label)}</span></div>`:''}<div class="destination"><span class="review-label">${consolidating?'Fresh address in this same wallet':'Recipient'}</span><p class="address">${e(to)}</p></div><div class="summary-row"><span class="label">${p.inputs.length} input${p.inputs.length===1?'':'s'} total</span><span class="value">${btc(p.inputTotal)} BTC</span></div><div class="summary-row"><span class="label">Network fee</span><span class="value">${num(p.fee)} sats<br><small class="quiet">${p.rate} sat/vB · ~${p.vbytes} vB</small></span></div>${!consolidating?`<div class="summary-row"><span class="label">Change to this wallet</span><span class="value">${p.change?btc(p.change)+' BTC':'None'}</span></div>`:''}<div class="summary-row total"><span class="label">${consolidating?'Wallet cost':'Leaves your wallet'}</span><span class="value">${consolidating?num(p.fee)+' sats':btc(p.amount+p.fee)+' BTC'}</span></div></div>${inputBreakdown(p)}`;
}
function needsAcknowledgment(p){return p.mode==='consolidate'||p.mixed||p.remainder>0;}
function coinReviewScreen(){
  const w=wallet(),p=formPlan();
  if(!p.ok)return `<div class="error">${e(p.error)}</div>`;
  return `<div class="page-intro"><h1>${p.mode==='consolidate'?'Review consolidation.':'Check your payment.'}</h1><p class="subhead">${p.mode==='consolidate'?`${p.inputs.length} coins → 1 coin. Your keys stay on hardware.`:'Your selected inputs, amount, and change.'}</p></div>${coinPlanSummary(p,w)}${p.change?`<p class="field-note mt16">Change keeps the source coin labels in its history and receives “Change · ${e(form.label||'Payment')}” as its label.</p>`:''}${p.mixed?`<div class="privacy-callout">${icon('info',19)}<p>These coins have different or missing labels. Spending them together can link their sources. Labels are reminders, not a privacy guarantee.</p></div>`:p.mode==='consolidate'?`<div class="privacy-callout">${icon('info',19)}<p>All selected coins become linked in the same transaction, even when their labels match.</p></div>`:''}${p.remainder?`<div class="privacy-callout">${icon('info',19)}<p>${num(p.remainder)} sats of remainder would be too small for this demo’s change policy, so it is included in the ${num(p.fee)}-sat fee. Effective rate: ${p.effectiveRate.toFixed(2)} sat/vB.</p></div>`:''}${needsAcknowledgment(p)?`<label class="privacy-check"><input type="checkbox" data-privacy-ack ${form.privacyAck?'checked':''}><span>${p.mode==='consolidate'||p.mixed?'I understand that these coins will be linked.':'I accept the small remainder being added to the fee.'}${p.remainder&&(p.mode==='consolidate'||p.mixed)?' I also accept the displayed remainder fee.':''}</span></label>`:''}<div class="mt24">${notice(p.mode==='consolidate'?'Verify this is your wallet’s address, plus the amount and fee, on your hardware.':'Verify the full recipient, amount, change, and fee on your hardware.')}</div>${errorBox()}`;
}
function validateCoinForm(requireAcknowledgment=false){
  const w=wallet();if(!form)return 'Start a payment first.';
  if(form.mode==='consolidate')form.to=`demo:${w.id}/change/${w.changeIndex||1}`;
  form.to=form.to.trim();
  if(!validRecipient(form.to))return 'Use a demo: recipient. Real Bitcoin addresses are not accepted in this prototype.';
  if(!TundraCoins.labelOK(form.label))return 'Use a single-line label of at most 255 characters.';
  if(form.mode==='consolidate'&&!form.label.trim())return 'Give the consolidated coin a label.';
  const p=formPlan();if(!p.ok)return p.error;
  if(form.max)form.amount=btc(p.amount);
  if(requireAcknowledgment&&needsAcknowledgment(p)&&!form.privacyAck)return 'Confirm the privacy and fee notice before signing.';
  return '';
}
function coinStartSigning(){
  const w=wallet();
  error=validateCoinForm(true);if(error){render(true);document.querySelector('.error')?.scrollIntoView({block:'nearest'});return;}
  if(w.draft){error='Finish or discard the existing payment first.';render();return;}
  const p=formPlan(),id=`demo-payment-${store.nextId++}`,index=w.changeIndex||1;
  const d={...clone(p),id,to:form.to,label:form.label.trim(),changeAddress:p.change?`demo:${w.id}/change/${index}`:'',signedKeys:[],binding:'',selectionMode:form.selected===null?'automatic':'manual'};
  d.binding=TundraCoins.binding(w,d);
  if(p.change||p.mode==='consolidate')w.changeIndex=index+1;
  w.draft=d;coinSelection=[];transport='qr';phase='show';saveStore();go('sign');
}
function coinReadyScreen(){
  const w=wallet(),d=w.draft;
  return `<div class="success-mark">${icon('check',31)}</div><div class="page-intro"><h1>${d.mode==='consolidate'?'Ready to consolidate.':'Ready to send.'}</h1><p class="subhead">${w.required===2?'Both signatures are in.':'Your hardware has signed.'}<br>Nothing is broadcast until you confirm.</p></div>${coinPlanSummary(d,w,d)}<p class="small muted mt24">${w.required} of ${w.required} hardware signatures · demo.</p>${errorBox()}`;
}
function coinSuccessScreen(){
  const d=lastPayment;
  return `<div class="success-mark">${icon('check',31)}</div><div class="page-intro"><h1>${d.mode==='consolidate'?'Coins consolidated.':'Payment sent.'}</h1><p class="subhead">${d.mode==='consolidate'?`${d.inputs.length} inputs → 1 new coin. It is now pending.`:'Selected coins were spent. Any change is now pending.'}</p></div>${coinPlanSummary(d,{name:d.walletName},d)}<div class="mt24">${notice('Simulated broadcast only. No bitcoin was sent.',true)}</div>`;
}
function coinBroadcast(){
  const w=wallet(),d=w.draft;
  if(!ready(w,d)){error='Add the required hardware signatures first.';render();return false;}
  const invalid=TundraCoins.validateDraft(w,d);if(invalid){error=invalid;render();return false;}
  const txid=TundraCoins.demoTxid(100000+store.nextId++),sources=d.inputs.map(c=>({outpoint:TundraCoins.outpoint(c),label:c.label}));
  for(const input of d.inputs){const coin=coinById(input.id);coin.spent=true;coin.spentBy=txid;}
  const output=(sats,address,label,vout,isChange)=>({id:`c-${store.nextId++}`,txid,vout,sats,label,address,date:'Just now',time:Date.now(),confirmed:false,frozen:false,spent:false,isChange,sources});
  if(d.mode==='consolidate')w.coins.push(output(d.amount,d.to,d.label,0,false));
  else if(d.change)w.coins.push(output(d.change,d.changeAddress,`Change · ${d.label||'Payment'}`.slice(0,255),1,true));
  w.balance=TundraCoins.total(TundraCoins.unspent(w));
  w.history.unshift({id:d.id,txid,direction:d.mode==='consolidate'?'self':'out',amount:d.mode==='consolidate'?d.fee:d.amount,consolidatedAmount:d.mode==='consolidate'?d.amount:undefined,fee:d.fee,label:d.label||(d.mode==='consolidate'?'Consolidation':'Sent'),subtitle:'In mempool',date:'Just now',to:d.to,inputs:clone(d.inputs),change:d.change});
  lastPayment={...clone(d),walletName:w.name,txid};w.draft=null;coinSelection=[];coinSelecting=false;saveStore();go('success');return true;
}
function coinDetailBody(c){
  const w=wallet(),reserved=TundraCoins.reserved(w,c),status=coinStatus(w,c);
  return `<div class="coin-detail-amount">${coinMoney(c.sats)}<span class="coin-status ${status.toLowerCase().replace(' ','-')}">${e(c.date)} · ${e(status)}</span></div><label class="form-field"><span class="form-label">Label</span><span class="field-wrap">${icon('tag',17)}<input data-coin-label aria-label="Coin label" value="${e(sheet.value)}" maxlength="255" autocomplete="off" placeholder="Where did this coin come from?"></span></label><p class="field-note">Private to your wallet. Not sent with the payment.</p>${labelSuggestions()}${errorBox(true)}${button('Save label','coin-save-label','primary full')}<details class="coin-technical"><summary>Transaction details ${icon('chevron',15)}</summary><div class="coin-detail-data"><div><span>Outpoint · demo identifier</span><button class="icon-btn" data-action="coin-copy:${e(c.id)}" aria-label="Copy coin outpoint">${icon('copy',17)}</button></div><code>${e(TundraCoins.outpoint(c))}</code><div><span>Address · demo only</span></div><code>${e(c.address)}</code></div>${c.sources?.length?`<details class="input-breakdown"><summary>Original input labels</summary>${c.sources.map(s=>`<div class="provenance-row"><strong>${e(s.label||'Unlabeled')}</strong><code>${e(s.outpoint)}</code></div>`).join('')}</details>`:''}</details>${!c.spent?button(`${icon(c.frozen?'unlock':'lock',17)} ${c.frozen?'Unfreeze coin':'Freeze coin'}`,'coin-freeze:'+c.id,'ghost full',reserved?'disabled':''):''}<p class="field-note">${reserved?'Reserved by your draft. Discard the draft to release it.':c.frozen?'Unfreeze to make this coin available for spending.':'Freeze to exclude it from spending in this wallet. This does not lock bitcoin on-chain.'}</p>`;
}

function coinSheet(){
  const w=wallet();let title='',body='';
  switch(sheet.kind){
    case 'balanceDetails':{
      title='Your balance';
      const groups={Available:[],Frozen:[],Pending:[],'Reserved in draft':[]};
      for(const c of TundraCoins.unspent(w))groups[TundraCoins.reserved(w,c)?'Reserved in draft':c.frozen?'Frozen':!c.confirmed?'Pending':'Available'].push(c);
      body=`<p class="sheet-sub">${e(w.name)} · ${coinMoney(w.balance)} total</p><div class="balance-breakdown">${Object.entries(groups).map(([label,list])=>`<div class="summary-row"><span class="label">${label}<small>${list.length} ${list.length===1?'coin':'coins'}</small></span><span class="value">${coinMoney(TundraCoins.total(list))}</span></div>`).join('')}</div><p class="field-note mt16">Only available coins can be spent. All balances are simulated.</p>`;break;
    }
    case 'coinFilters':{
      title='Filter & sort';
      const options=[['all','All coins'],['available','Available'],['frozen','Frozen'],['unlabeled','Unlabeled']];
      body=`<fieldset class="filter-fieldset"><legend>Show</legend>${options.map(([v,t])=>`<label class="filter-option"><span>${t}</span><input type="radio" name="coin-filter-draft" value="${v}" ${sheet.filter===v?'checked':''}></label>`).join('')}</fieldset><label class="filter-sort"><span>Sort by</span><select data-filter-sort aria-label="Sort coins"><option value="newest" ${sheet.sort==='newest'?'selected':''}>Newest first</option><option value="smallest" ${sheet.sort==='smallest'?'selected':''}>Smallest first</option><option value="largest" ${sheet.sort==='largest'?'selected':''}>Largest first</option></select></label>${button('Apply','coin-apply-filters','primary full')}`;break;
    }
    case 'coinActions':{
      title=`${coinSelection.length} coins selected`;
      const row=(ic,title,desc,action,disabled=false)=>`<button class="context-action" data-action="${action}" ${disabled?'disabled':''}>${icon(ic,21)}<span class="grow"><strong>${title}</strong><small>${desc}</small></span>${icon('chevron',15)}</button>`;
      body=`<p class="sheet-sub">${coinMoney(TundraCoins.total(selectedCoins()))} in ${e(w.name)}</p>${row('tag','Label coins','Apply one label to your selection','coin-bulk-label')}${row('coins','Consolidate','Combine into one coin in this wallet','coins-consolidate',coinSelection.length<2)}${row('lock','Freeze coins','Exclude these coins from spending','coin-freeze-selected')}`;break;
    }
    case 'coinDetail':{const c=coinById(sheet.id);if(!c)return null;title='Coin details';body=coinDetailBody(c);break;}
    case 'coinBulkLabel':title=`Label ${sheet.ids.length} coins`;body=`<p class="sheet-sub">Apply one label to your selection. Unselected coins keep their labels.</p><label class="form-field"><span class="form-label">Label</span><span class="field-wrap">${icon('tag',17)}<input data-coin-label aria-label="Label selected coins" maxlength="255" value="${e(sheet.value)}" placeholder="e.g. Mining payouts" autocomplete="off"></span></label>${labelSuggestions()}${errorBox(true)}${button('Apply label','coin-save-bulk','primary full')}<p class="field-note mt16">An empty label clears the selected coin labels. This changes reminders, not their transaction history.</p>`;break;
    case 'txLabel':title='Payment label';body=`<label class="form-field"><span class="form-label">Label</span><span class="field-wrap"><input data-coin-label aria-label="Transaction label" maxlength="255" value="${e(sheet.value)}" autocomplete="off"></span></label>${errorBox(true)}${button('Save label','coin-save-tx-label','primary full')}<p class="field-note mt16">Coin labels are independent and will not be overwritten.</p>`;break;
    case 'coinHelp':title='A coin is a UTXO.';body=`<p class="sheet-sub">Each received output is a separate piece of your balance. You can label it and decide exactly when to spend it.</p><div class="panel"><h2>Keep it simple</h2><p class="small muted mt16">Select one or more coins to send. Select two or more to consolidate into a new coin in the same wallet. Freeze coins you do not want to spend.</p></div><div class="privacy-callout">${icon('info',19)}<p>Spending coins together links them publicly. Matching labels do not guarantee the sources are safe to combine.</p></div><p class="field-note mt16">All coins here are synthetic. Pending outputs are excluded until confirmed. Fees use simplified size estimates, not a live mempool.</p>`;break;
    case 'labels':title='Your labels, portable.';body=`<p class="sheet-sub">Back up this wallet’s coin and transaction labels, including frozen status.</p>${button(`${icon('download',18)} Export labels`,'label-export','primary full')}${button(`${icon('upload',18)} Import labels`,'label-file','secondary full')}<div class="panel mt24"><strong>BIP 329 · JSON Lines</strong><p class="small muted mt16">This demo supports output and transaction records. Imports match existing references in this wallet only. Other record types and unknown references are skipped.</p></div><div class="mt24">${notice('Labels may reveal who paid you or why. Exports and browser storage are unencrypted. Use test labels; keep backups private.',true)}</div>${errorBox(true)}`;break;
    case 'labelReview':{
      const p=labelImportReview;if(!p)return null;
      title='Review label import';body=`<p class="sheet-sub">${p.changes.length} changes · ${p.matched} matched · ${p.skipped} skipped${p.duplicates?' · '+p.duplicates+' repeated records':''}. Nothing changes until you apply.</p><div class="label-import-list">${p.changes.map(c=>`<div class="label-change"><span>${c.type==='output'?'Coin':'Transaction'}</span><code>${e(c.ref)}</code><strong>${e(c.before||'Unlabeled')} ${icon('arrow',12)} ${e(c.label||'Unlabeled')}</strong>${c.beforeFrozen!==c.frozen?`<small class="accent">${c.frozen?'Will freeze this coin':'Will unfreeze this coin'}</small>`:''}</div>`).join('')||'<p class="small muted">No label or frozen-state changes for this wallet.</p>'}</div>${errorBox(true)}${button('Apply changes','label-apply','primary full',p.changes.length?'':'disabled')}${button('Cancel','close','ghost full')}<p class="field-note mt16">Missing label or spendable fields leave existing values unchanged. Unsupported records are not retained.</p>`;break;
    }
    case 'coinFees':{
      title='Network fee';body=`<p class="sheet-sub">Demo rates, not live estimates. Fees change with the selected inputs and outputs.</p>${[[1,'1'],[2,'2'],[4,'4'],[8,'8']].map(([rate,text])=>{const p=TundraCoins.plan(w,{selected:form.selected,amount:parseSats(form.amount)||1000,rate,max:form.max,mode:form.mode});return `<button class="choice ${form.rate===rate?'selected':''}" data-action="fee:${rate}"><span class="grow"><span class="title">${text} sat/vB</span><span class="description" style="display:block">${p.ok?num(p.fee)+' sats · ~'+p.vbytes+' vB':'Adjust the amount or selection'}</span></span>${form.rate===rate?icon('check',19):''}</button>`;}).join('')}<label class="form-field"><span class="form-label">Custom rate · sat/vB</span><span class="field-wrap"><input type="number" data-custom-fee aria-label="Custom fee rate" min="1" max="1000" step="1" inputmode="numeric" value="${form.rate}"></span></label>${errorBox(true)}${button('Use custom rate','coin-custom-fee','secondary full')}`;break;
    }
    default:return null;
  }
  return {title,body};
}
function labelSuggestions(){const values=[...new Set((wallet().coins||[]).map(c=>c.label).filter(Boolean))].slice(0,4);return `<div class="label-suggestions">${values.map((label,i)=>`<button data-action="label-suggestion:${i}">${e(label)}</button>`).join('')}</div>`;}
function setCoinLabel(ids,value){
  if(!TundraCoins.labelOK(value))return false;
  for(const id of ids){const c=coinById(id);if(c)c.label=value.trim();}
  saveStore();return true;
}
function openCoinPicker(){
  const p=formPlan();coinPicker={ids:(form.selected===null?p.inputs.map(c=>c.id):form.selected).slice(),returnRoute:route==='consolidate'?'consolidate':'send'};
  coinSearch='';coinFilter='all';coinSort='newest';go('coinPicker');
}
function newLabelImport(raw){
  try{labelImportReview=TundraCoins.labelImport(raw,wallet());openSheet({kind:'labelReview'});return {ok:true,...clone(labelImportReview)};}
  catch(err){error=err.message;if(!sheet)openSheet({kind:'labels'});error=err.message;render(true);return {ok:false,error:err.message};}
}
function applyLabelImport(){
  const p=labelImportReview,w=wallet();if(!p||w.id!==p.walletId)return false;
  // Validate the whole staged import before any writes. Never unfreeze a draft's inputs silently.
  for(const change of p.changes){
    const target=change.type==='output'?coinById(change.id):(w.history||[]).find(h=>h.id===change.id);
    if(!target||(target.label||'')!==change.before||change.type==='output'&&target.frozen!==change.beforeFrozen){error='A label changed since this preview. Re-import the file to review again.';render(true);return false;}
    if(change.type==='output'&&change.frozen!==change.beforeFrozen&&TundraCoins.reserved(w,target)){error='A coin in this import is reserved by your draft. Finish or discard that draft before changing its frozen state.';render(true);return false;}
  }
  for(const change of p.changes){const target=change.type==='output'?coinById(change.id):w.history.find(h=>h.id===change.id);target.label=change.label;if(change.type==='output')target.frozen=change.frozen;}
  coinSelection=coinSelection.filter(id=>TundraCoins.usable(w,coinById(id)));saveStore();const n=p.changes.length;labelImportReview=null;closeSheet();toast(`${n} label / frozen-state changes applied.`);return true;
}
function coinAction(cmd,arg){
  switch(cmd){
    case 'coins':go('coins');return true;
    case 'balance-details':openSheet({kind:'balanceDetails'});return true;
    case 'coin-filters':openSheet({kind:'coinFilters',filter:coinFilter,sort:coinSort});return true;
    case 'coin-apply-filters':coinFilter=sheet.filter;coinSort=sheet.sort;closeSheet();return true;
    case 'coin-reset-filters':coinFilter='all';coinSort='newest';render(true);return true;
    case 'coin-start-select':coinSelecting=true;render(true);return true;
    case 'coin-end-select':coinSelecting=false;coinSelection=[];render(true);return true;
    case 'coin-selection-actions':if(coinSelection.length)openSheet({kind:'coinActions'});return true;
    case 'coin-help':openSheet({kind:'coinHelp'});return true;
    case 'coin-filter':if(['all','available','frozen','unlabeled'].includes(arg))coinFilter=arg;render(true);return true;
    case 'coin-clear-filter':coinFilter='all';coinSearch='';coinSort='newest';render(true);return true;
    case 'coin-toggle':{
      const c=coinById(arg);if(!TundraCoins.usable(wallet(),c))return true;
      if(route!=='coinPicker')coinSelecting=true;
      const ids=route==='coinPicker'?coinPicker.ids:coinSelection,at=ids.indexOf(arg);if(at>=0)ids.splice(at,1);else ids.push(arg);render(true);return true;
    }
    case 'coin-clear-selection':if(route==='coinPicker')coinPicker.ids=[];else coinSelection=[];render(true);return true;
    case 'coin-select-visible':{if(route!=='coinPicker')coinSelecting=true;const ids=route==='coinPicker'?coinPicker.ids:coinSelection;for(const c of visibleCoins())if(TundraCoins.usable(wallet(),c)&&!ids.includes(c.id))ids.push(c.id);render(true);return true;}
    case 'coin-detail':{const c=coinById(arg);if(c)openSheet({kind:'coinDetail',id:arg,value:c.label});return true;}
    case 'coin-save-label':if(setCoinLabel([sheet.id],sheet.value)){closeSheet();toast('Coin label saved.');}else{error='Use a single-line label of at most 255 characters.';render(true);}return true;
    case 'coin-bulk-label':if(coinSelection.length)openSheet({kind:'coinBulkLabel',ids:coinSelection.slice(),value:''});return true;
    case 'label-suggestion':{const labels=[...new Set((wallet().coins||[]).map(c=>c.label).filter(Boolean))].slice(0,4);sheet.value=labels[Number(arg)]||'';render(true);return true;}
    case 'coin-save-bulk':if(setCoinLabel(sheet.ids,sheet.value)){closeSheet();toast('Selected coin labels saved.');}else{error='Use a single-line label of at most 255 characters.';render(true);}return true;
    case 'coin-freeze':{const c=coinById(arg);if(c&&!TundraCoins.reserved(wallet(),c)){if(sheet?.kind==='coinDetail'&&sheet.value!==c.label){error='Save your label before changing the frozen state.';render(true);return true;}c.frozen=!c.frozen;coinSelection=coinSelection.filter(id=>id!==arg);if(coinPicker)coinPicker.ids=coinPicker.ids.filter(id=>id!==arg);saveStore();closeSheet();toast(c.frozen?'Coin frozen. Excluded from spending.':'Coin unfrozen.');}return true;}
    case 'coin-freeze-selected':for(const c of selectedCoins())if(!TundraCoins.reserved(wallet(),c))c.frozen=true;coinSelection=[];coinSelecting=false;saveStore();closeSheet();toast('Selected coins frozen.');return true;
    case 'coins-send':if(coinSelection.length)newForm(coinSelection);return true;
    case 'coins-consolidate':if(coinSelection.length>=2)newForm(coinSelection,'consolidate');return true;
    case 'choose-coins':openCoinPicker();return true;
    case 'coins-auto':{if(form.mode==='consolidate')return true;const next=coinPicker?.returnRoute||'send';form.selected=null;form.privacyAck=false;coinPicker=null;if(form.mode==='consolidate'){form.selected=TundraCoins.available(wallet()).map(c=>c.id);}go(next);return true;}
    case 'coins-use':{if(!coinPicker?.ids.length)return true;form.selected=coinPicker.ids.slice();form.privacyAck=false;const next=coinPicker.returnRoute;coinPicker=null;go(next);return true;}
    case 'coin-custom-fee':{const rate=Number(document.querySelector('[data-custom-fee]')?.value);if(!TundraCoins.validRate(rate)){error='Enter a whole-number rate from 1 to 1,000 sat/vB.';render(true);}else{form.rate=rate;form.privacyAck=false;closeSheet();}return true;}
    case 'coin-copy':{const c=coinById(arg);if(c&&!navigator.clipboard){toast('Copy unavailable. Select the outpoint text to copy.',true);return true;}if(c)navigator.clipboard.writeText(TundraCoins.outpoint(c)).then(()=>toast('Demo outpoint copied.')).catch(()=>toast('Copy unavailable in this browser.',true));return true;}
    case 'labels':openSheet({kind:'labels'});return true;
    case 'label-file':document.getElementById('labels-file').click();return true;
    case 'label-export':{
      const text=TundraCoins.labelExport(wallet()),blob=new Blob([text],{type:'application/x-ndjson;charset=utf-8'}),url=URL.createObjectURL(blob),a=document.createElement('a');
      a.href=url;a.download=`Tundra-${wallet().id}-DEMO-labels.jsonl`;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);toast('Demo labels exported. Keep this unencrypted file private.');return true;
    }
    case 'label-apply':applyLabelImport();return true;
    case 'tx-label':{const h=wallet().history.find(h=>h.id===arg);if(h)openSheet({kind:'txLabel',id:arg,value:h.label||''});return true;}
    case 'coin-save-tx-label':{if(!TundraCoins.labelOK(sheet.value)){error='Use a single-line label of at most 255 characters.';render(true);return true;}const h=wallet().history.find(h=>h.id===sheet.id);if(h)h.label=sheet.value.trim();saveStore();closeSheet();toast('Payment label saved.');return true;}
    case 'confirm-pending':for(const c of TundraCoins.unspent(wallet()))c.confirmed=true;for(const h of wallet().history)h.subtitle='Confirmed';saveStore();render(true);toast('Pending coins confirmed in the demo only.');return true;
    default:return false;
  }
}
document.addEventListener('input',event=>{
  const el=event.target;
  if(el.hasAttribute('data-coin-label')&&sheet)sheet.value=el.value;
  if(el.hasAttribute('data-coin-search')){
    coinSearch=el.value;document.getElementById('coin-list-region').innerHTML=coinRows(route==='coinPicker');const n=document.getElementById('coin-result-count');if(n)n.textContent=`${visibleCoins().length} coins`;
  }
  if(el.dataset.field&&form){
    if(el.dataset.field==='amount'){form.max=false;form.privacyAck=false;}
    if(['amount','to'].includes(el.dataset.field))form.privacyAck=false;
    // Update the estimate without replacing the focused amount input.
    if(route==='send'){const p=formPlan(),n=document.getElementById('fee-preview');if(n)n.innerHTML=`${p.ok?num(p.fee)+' sats':'—'}<small>${form.rate} sat/vB</small>`;}
  }
});
document.addEventListener('change',async event=>{
  const el=event.target;
  if(el.name==='coin-filter-draft'&&sheet?.kind==='coinFilters')sheet.filter=el.value;
  if(el.hasAttribute('data-filter-sort')&&sheet?.kind==='coinFilters')sheet.sort=el.value;
  if(el.hasAttribute('data-coin-sort')){coinSort=el.value;render(true);}
  if(el.hasAttribute('data-privacy-ack')&&form){form.privacyAck=el.checked;error='';document.querySelector('.screen .error')?.remove();}
  if(el.id==='labels-file'){
    const file=el.files?.[0];el.value='';if(!file)return;const walletId=wallet().id;
    try{if(file.size>TundraCoins.MAX_FILE)throw new Error('Choose a label file smaller than 1 MB.');const raw=await file.text();if(wallet().id===walletId)newLabelImport(raw);}
    catch(err){error=err.message;render(true);}
  }
});
window.TundraCoinDemo=Object.freeze({getSelection:()=>coinSelection.slice(),getForm:()=>form?clone(form):null,getPicker:()=>coinPicker?clone(coinPicker):null,
  getPlan:()=>clone(formPlan()),openSend:(ids=null)=>newForm(ids),openConsolidation:ids=>newForm(ids,'consolidate'),
  setLabel:(ids,value)=>setCoinLabel(ids,value),exportLabels:()=>TundraCoins.labelExport(wallet()),importLabels:newLabelImport,applyLabels:applyLabelImport,
  getImport:()=>labelImportReview?clone(labelImportReview):null});
