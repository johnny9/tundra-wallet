"use strict";
/* Pure, integer-satoshi demo coin control. NOT a Bitcoin transaction engine.
 * No chain lookup, address derivation, PSBT creation, signing or broadcasting.
 * Size estimates: 68.25 vB P2WPKH input, 105 vB 2-of-3 P2WSH input;
 * 31-byte demo recipient output, 31/43-byte change. Conservative signature sizes.
 * MIN_OUTPUT is this prototype's floor, not a Bitcoin consensus/dust threshold.
 */
(function(root){
  const MAX_MONEY=2100000000000000, MIN_OUTPUT=1000, LABEL_LIMIT=255, MAX_FILE=1048576;
  const copy=v=>JSON.parse(JSON.stringify(v));
  const own=(obj,key)=>Object.prototype.hasOwnProperty.call(obj,key);
  const sats=n=>Number.isSafeInteger(n)&&n>=0&&n<=MAX_MONEY;
  const labelOK=s=>typeof s==='string'&&s.length<=LABEL_LIMIT&&!/[\r\n\x00-\x08\x0b\x0c\x0e-\x1f]/.test(s);
  const outpoint=c=>`${c.txid}:${c.vout}`;
  const unspent=w=>(w.coins||[]).filter(c=>!c.spent);
  const total=coins=>coins.reduce((n,c)=>n+c.sats,0);
  const reserved=(w,c)=>!!w.draft?.inputs?.some(i=>i.id===c.id);
  const usable=(w,c,ignoreDraft=false)=>!!c&&!w.isImported&&!c.spent&&c.confirmed&&!c.frozen&&(ignoreDraft||!reserved(w,c));
  const available=w=>unspent(w).filter(c=>usable(w,c));
  const sameLabels=coins=>new Set(coins.map(c=>c.label.trim()||`unlabeled:${c.id}`)).size;
  const mixed=coins=>coins.length>1&&(coins.some(c=>!c.label.trim())||sameLabels(coins)>1);
  function validCoin(c){
    return !!c&&/^[a-zA-Z0-9_-]+$/.test(c.id)&&/^[a-f0-9]{64}$/.test(c.txid)&&
      Number.isSafeInteger(c.vout)&&c.vout>=0&&c.vout<=4294967295&&sats(c.sats)&&c.sats>0&&
      labelOK(c.label)&&typeof c.address==='string'&&c.address.startsWith('demo:')&&c.address.length<=150&&
      ['spent','frozen','confirmed'].every(k=>typeof c[k]==='boolean')&&Number.isSafeInteger(c.time);
  }
  function vbytes(w,count,change=false,consolidate=false){
    const ownOutput=w.required===2?43:31;
    return Math.ceil(10.5+(w.required===2?105:68.25)*count+(consolidate?ownOutput:31)+(change?ownOutput:0));
  }
  const validRate=rate=>Number.isInteger(rate)&&rate>=1&&rate<=1000;
  function estimate(w,coins,{amount=0,rate=4,max=false,mode='send'}={}){
    const inputTotal=total(coins), consolidate=mode==='consolidate';
    const fail=(error,extra={})=>({ok:false,error,inputs:coins.map(copy),inputTotal,...extra});
    if(!validRate(rate))return fail('Enter a whole-number fee rate from 1 to 1,000 sat/vB.');
    if(!coins.length)return fail('Choose at least one available coin.');
    if(coins.length>100)return fail('This demo supports up to 100 inputs per transaction.');
    if(!sats(inputTotal))return fail('The selected amount is outside the supported range.');
    if(consolidate&&coins.length<2)return fail('Choose at least two coins to consolidate.');
    const singleSize=vbytes(w,coins.length,false,consolidate), minimumFee=singleSize*rate;
    let fee=minimumFee,change=0,remainder=0,size=singleSize;
    if(max||consolidate) amount=inputTotal-fee;
    else {
      if(!sats(amount)||amount<MIN_OUTPUT)return fail('Enter at least 1,000 sats (0.00001000 BTC) for this demo.');
      const changeSize=vbytes(w,coins.length,true,false),changeFee=changeSize*rate;
      if(inputTotal-amount-changeFee>=MIN_OUTPUT){size=changeSize;fee=changeFee;change=inputTotal-amount-fee;}
      else if(inputTotal-amount>=minimumFee){fee=inputTotal-amount;remainder=fee-minimumFee;}
      else return fail('The chosen coins do not cover the amount and fee. Choose more coins or lower the amount.',{shortfall:amount+minimumFee-inputTotal,fee:minimumFee});
    }
    if(!sats(amount)||amount<MIN_OUTPUT)return fail('After fees, the output is below this demo’s 1,000-sat minimum.',{fee,shortfall:MIN_OUTPUT-amount});
    return {ok:true,inputs:coins.map(copy),inputTotal,amount,fee,change,remainder,vbytes:size,rate,mode,
      mixed:mixed(coins),outputCount:change?2:1,effectiveRate:fee/size};
  }
  function plan(w,options={}){
    const {selected=null,max=false,mode='send'}=options;
    if(w.isImported)return {ok:false,error:'Connect a Bitcoin backend before selecting real coins.',inputs:[],inputTotal:0};
    if(selected!==null){
      if(!Array.isArray(selected)||new Set(selected).size!==selected.length)return {ok:false,error:'Coin selection contains duplicates.',inputs:[],inputTotal:0};
      const coins=selected.map(id=>(w.coins||[]).find(c=>c.id===id));
      if(coins.some(c=>!usable(w,c)))return {ok:false,error:'A selected coin is frozen, pending, spent, or reserved by a draft. Review the selection.',inputs:coins.filter(Boolean),inputTotal:total(coins.filter(Boolean))};
      return estimate(w,coins,options);
    }
    const coins=available(w).slice().sort((a,b)=>a.sats-b.sats||a.id.localeCompare(b.id));
    if(max||mode==='consolidate')return estimate(w,coins,options);
    if(!coins.length)return estimate(w,coins,options);
    // A small, deterministic demo policy: first prefer a single sufficient coin;
    // then same-label groups, then mixed inputs. Not a production BnB selector.
    for(const c of coins){const p=estimate(w,[c],options);if(p.ok)return p;}
    const groups=new Map();
    for(const c of coins){const key=c.label.trim()||`unlabeled:${c.id}`;if(!groups.has(key))groups.set(key,[]);groups.get(key).push(c);}
    const candidates=[];
    for(const group of [...groups.values(),coins]){
      const chosen=[];
      for(const c of group.slice().sort((a,b)=>b.sats-a.sats||a.id.localeCompare(b.id))){chosen.push(c);const p=estimate(w,chosen,options);if(p.ok){candidates.push(p);break;}}
    }
    candidates.sort((a,b)=>Number(a.mixed)-Number(b.mixed)||a.inputs.length-b.inputs.length||a.inputTotal-b.inputTotal);
    return candidates[0]||estimate(w,coins,options);
  }
  // Synthetic 64-hex fixture identifiers, not transaction hashes.
  const demoTxid=n=>((n*2654435761)>>>0).toString(16).padStart(8,'0').repeat(8);
  function fixture(id,n,amount,label,day,extra={}){
    return {id,txid:demoTxid(n),vout:0,sats:amount,label,date:`Sep ${day}`,time:Date.UTC(2026,8,day),
      address:`demo:${id.startsWith('s')?'savings':'everyday'}/receive/${n}`,confirmed:true,frozen:false,spent:false,isChange:false,...extra};
  }
  function seed(w){
    if(Array.isArray(w.coins))return w;
    if(w.id==='savings')w.coins=[
      fixture('s1',1,40000000,'Long-term savings',1),
      fixture('s2',2,25000000,'Exchange withdrawal',2),
      fixture('s3',3,10000000,'Exchange withdrawal',3),
      fixture('s4',4,2500000,'Mining payouts',5),
      fixture('s5',5,1500000,'Mining payouts',4),
      fixture('s6',6,950000,'Travel reserve',2,{frozen:true}),
      fixture('s7',7,45000,'',6,{isChange:true}),
      fixture('s8',8,5000,'Awaiting confirmation',7,{confirmed:false})
    ];
    else if(w.id==='everyday')w.coins=[
      fixture('e1',101,2500000,'Monthly spending',3),fixture('e2',102,1250000,'Reimbursement',5),
      fixture('e3',103,400000,'Change · Groceries',6,{isChange:true}),fixture('e4',104,110000,'',4),
      fixture('e5',105,5000,'Do not spend',2,{frozen:true})
    ];
    else w.coins=[];
    w.changeIndex=1;w.addressLabels={};w.balance=total(unspent(w));
    w.history=w.coins.slice().sort((a,b)=>b.time-a.time).map(c=>({id:`h-${c.id}`,txid:c.txid,direction:'in',amount:c.sats,label:c.label||'Received',subtitle:c.confirmed?'Confirmed':'In mempool',date:c.date,to:c.address}));
    return w;
  }
  function binding(w,d){
    return JSON.stringify([d.id,w.id,d.mode,d.inputs.map(c=>[c.id,c.txid,c.vout,c.sats]),d.to,d.amount,d.fee,d.change,d.changeAddress,d.rate]);
  }
  function validateDraft(w,d){
    if(!d||!Array.isArray(d.inputs)||!d.inputs.length||!Array.isArray(d.signedKeys)||!['send','consolidate'].includes(d.mode))return 'The payment draft is invalid.';
    if(new Set(d.inputs.map(c=>c.id)).size!==d.inputs.length)return 'The draft contains duplicate inputs.';
    for(const input of d.inputs){
      const c=(w.coins||[]).find(c=>c.id===input.id);
      if(!usable(w,c,true)||c.txid!==input.txid||c.vout!==input.vout||c.sats!==input.sats)return 'A chosen coin is no longer available. Discard this draft and select coins again.';
    }
    const p=estimate(w,d.inputs,{amount:d.amount,rate:d.rate,max:d.mode==='consolidate',mode:d.mode});
    if(!p.ok||p.fee!==d.fee||p.change!==d.change||p.amount!==d.amount||p.inputTotal!==d.amount+d.fee+d.change)return 'The input, output, or fee amounts changed. Start a new payment.';
    if(!/^demo:[a-zA-Z0-9][a-zA-Z0-9_/:.-]{2,96}$/.test(d.to))return 'The demo destination is invalid.';
    if(d.mode==='consolidate'&&(!d.to.startsWith(`demo:${w.id}/change/`)||d.change!==0))return 'Consolidation must return to the same wallet.';
    if(d.change&&!d.changeAddress.startsWith(`demo:${w.id}/change/`))return 'The change destination is invalid.';
    if(binding(w,d)!==d.binding)return 'The payment changed after signing began. Start a new payment.';
    return '';
  }
  function labelExport(w){
    const records=(w.coins||[]).map(c=>({type:'output',ref:outpoint(c),label:c.label,spendable:!c.frozen}));
    for(const h of w.history||[])if(/^[a-f0-9]{64}$/.test(h.txid||''))records.push({type:'tx',ref:h.txid,label:h.label||''});
    return records.map(r=>JSON.stringify(r)).join('\n')+(records.length?'\n':'');
  }
  function labelImport(text,w){
    if(typeof text!=='string'||new TextEncoder().encode(text).length>MAX_FILE)throw new Error('Choose a UTF-8 label file no larger than 1 MB.');
    const lines=text.replace(/^\uFEFF/,'').split(/\r?\n/);if(lines.length>5000)throw new Error('Choose a file with at most 5,000 records.');
    const pending=new Map();let skipped=0,duplicates=0;
    for(let i=0;i<lines.length;i++){
      if(!lines[i].trim())continue;
      let r;try{r=JSON.parse(lines[i]);}catch(_){throw new Error(`Line ${i+1} is not valid JSON. No labels were changed.`);}
      if(!r||Array.isArray(r)||typeof r.type!=='string'||typeof r.ref!=='string')throw new Error(`Line ${i+1} needs a type and ref. No labels were changed.`);
      if(!['output','tx'].includes(r.type)){skipped++;continue;}
      const output=r.type==='output';
      if(output?!/^[a-fA-F0-9]{64}:(0|[1-9][0-9]{0,9})$/.test(r.ref)||Number(r.ref.split(':')[1])>4294967295:!/^[a-fA-F0-9]{64}$/.test(r.ref))throw new Error(`Line ${i+1} has an invalid ${r.type} reference.`);
      if(own(r,'label')&&!labelOK(r.label))throw new Error(`Line ${i+1} needs a single-line label of at most ${LABEL_LIMIT} characters.`);
      if(own(r,'spendable')&&(!output||typeof r.spendable!=='boolean'))throw new Error(`Line ${i+1}: spendable must be a boolean on an output record.`);
      const ref=r.ref.toLowerCase(),target=output?(w.coins||[]).find(c=>outpoint(c)===ref):(w.history||[]).find(h=>h.txid===ref);
      if(!target){skipped++;continue;}
      const key=`${r.type}:${ref}`;
      if(pending.has(key))duplicates++;
      const previous=pending.get(key)||{type:r.type,ref,id:target.id,before:target.label||'',beforeFrozen:output?target.frozen:null,label:target.label||'',frozen:output?target.frozen:null};
      if(own(r,'label'))previous.label=r.label;
      if(own(r,'spendable'))previous.frozen=!r.spendable;
      pending.set(key,previous);
    }
    const changes=[...pending.values()].filter(c=>c.label!==c.before||c.frozen!==c.beforeFrozen);
    return {walletId:w.id,changes,matched:pending.size,skipped,duplicates};
  }
  root.TundraCoins=Object.freeze({MAX_MONEY,MIN_OUTPUT,LABEL_LIMIT,MAX_FILE,sats,labelOK,outpoint,unspent,total,reserved,usable,available,mixed,validCoin,validRate,vbytes,estimate,plan,demoTxid,seed,binding,validateDraft,labelExport,labelImport});
  if(typeof module==='object'&&module.exports)module.exports=root.TundraCoins;
})(typeof window==='object'?window:globalThis);
