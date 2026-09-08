"use strict";
/*
 * Narrow, public-only descriptor importer for a UI proof of concept.
 * BIP 380 checksum algorithm, BIP 32 public-key serialization, BIP 389 branches.
 * Not a production wallet engine. No derivation, signing, or network access.
 */
const TundraDescriptor = (() => {
  const INPUT = "0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH`#\"\\ ";
  const CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';
  const GENERATOR = [0xf5dee51989n,0xa9fdca3312n,0x1bab10e32dn,0x3706b1677an,0x644d626ffdn];
  const BASE58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  const MAX_SIZE = 65536;
  const fail = message => { throw new Error(message); };
  function checksum(body) {
    let c=1n, groups=[];
    const step=v=>{const top=c>>35n;c=((c&0x7ffffffffn)<<5n)^BigInt(v);for(let i=0;i<5;i++)if((top>>BigInt(i))&1n)c^=GENERATOR[i];};
    for(const char of body){const v=INPUT.indexOf(char);if(v<0)fail('The descriptor contains an unsupported character.');step(v&31);groups.push(v>>5);if(groups.length===3){step(groups[0]*9+groups[1]*3+groups[2]);groups=[];}}
    if(groups.length===1)step(groups[0]);else if(groups.length===2)step(groups[0]*3+groups[1]);
    for(let i=0;i<8;i++)step(0);c^=1n;
    return Array.from({length:8},(_,i)=>CHARSET[Number((c>>BigInt(5*(7-i)))&31n)]).join('');
  }
  const checked=body=>body+'#'+checksum(body);
  const hex=bytes=>Array.from(bytes,b=>b.toString(16).padStart(2,'0')).join('');
  const u32=(bytes,offset)=>new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength).getUint32(offset,false);
  function base58decode(s){
    if(s.length<100||s.length>120||!/^[1-9A-HJ-NP-Za-km-z]+$/.test(s))fail('An extended public key has an invalid encoding.');
    let n=0n;for(const c of s)n=n*58n+BigInt(BASE58.indexOf(c));
    let h=n.toString(16);if(h.length%2)h='0'+h;
    const bytes=Array.from(h.match(/../g)||[],v=>parseInt(v,16));
    for(const c of s){if(c!=='1')break;bytes.unshift(0);}
    return Uint8Array.from(bytes);
  }
  async function sha256(bytes){
    if(!globalThis.crypto?.subtle)fail('Public-key checks need a secure browser context. Open this file in a browser, or serve the prototype over HTTPS or localhost.');
    return new Uint8Array(await crypto.subtle.digest('SHA-256',bytes));
  }
  function modpow(a,b,p){let r=1n;for(;b;b>>=1n,a=a*a%p)if(b&1n)r=r*a%p;return r;}
  function validPoint(bytes){
    if(bytes.length!==33||![2,3].includes(bytes[0]))return false;
    const p=0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2fn;
    const x=BigInt('0x'+hex(bytes.slice(1)));if(x>=p)return false;
    const y2=(x*x%p*x+7n)%p,y=modpow(y2,(p+1n)/4n,p);
    return y*y%p===y2;
  }
  async function publicKey(s){
    const b=base58decode(s);if(b.length!==82)fail('An extended public key has the wrong length.');
    const payload=b.slice(0,78),sum=(await sha256(await sha256(payload))).slice(0,4);
    if(!sum.every((v,i)=>v===b[i+78]))fail('An extended public key failed its Base58 checksum. Re-export the wallet.');
    const version=u32(b,0);
    if(![0x0488b21e,0x043587cf].includes(version))fail('Only standard xpub and tpub public keys are supported. Export a public descriptor, not a private key or vendor-specific key string.');
    if(!validPoint(b.slice(45,78)))fail('The descriptor contains an invalid public key.');
    const depth=b[4],child=u32(b,9);
    if(depth===0&&(child!==0||b.slice(5,9).some(Boolean)))fail('The extended public key has invalid root metadata.');
    return {network:version===0x0488b21e?'mainnet':'test-family',depth,child,material:hex(b.slice(13,78))};
  }
  function pathIndex(s,hardenedAllowed){
    if(!/^\d+(?:h|')?$/.test(s)||(!hardenedAllowed&&/[h']$/.test(s)))fail('The derivation path is unsupported. Use unhardened public receive/change branches.');
    const hardened=/[h']$/.test(s),n=Number(s.replace(/[h']$/,''));
    if(!Number.isSafeInteger(n)||n<0||n>=2147483648)fail('A derivation index is out of range.');
    return n+(hardened?2147483648:0);
  }
  async function keyExpression(text){
    const m=/^\[([0-9a-fA-F]{8})((?:\/\d+(?:h|')?)*)\]((?:xpub|tpub)[1-9A-HJ-NP-Za-km-z]+)((?:\/[^/]+)+)$/.exec(text);
    if(!m)fail('Each key needs its master fingerprint, complete origin path, standard xpub/tpub, and a ranged receive/change path.');
    const originSteps=m[2]?m[2].slice(1).split('/').map(s=>pathIndex(s,true)):[];
    const pub=await publicKey(m[3]);
    if(originSteps.length!==pub.depth||(pub.depth&&originSteps.at(-1)!==pub.child))fail('A key origin does not match the extended public key depth or child number. Export the full public wallet configuration from hardware.');
    const suffix=m[4].slice(1).split('/');
    if(suffix.pop()!=='*'||!suffix.length)fail('Use ranged public descriptors ending in /0/*, /1/*, or /<0;1>/*.');
    const branch=suffix.pop();if(!['0','1','<0;1>'].includes(branch))fail('This prototype supports receive /0/*, change /1/*, and combined /<0;1>/* branches.');
    const prefix=suffix.map(s=>pathIndex(s,false));
    return {fingerprint:m[1].toUpperCase(),origin:m[2],originSteps,xpub:m[3],suffix:m[4],branch,prefix,...pub};
  }
  async function parseDescriptor(value){
    if(typeof value!=='string'||value.length>MAX_SIZE)fail('Use a public descriptor smaller than 64 KB.');
    const text=value.trim();
    if(/(?:[xtyzuvYZUV]prv|\b(?:seed|mnemonic|private[_ -]?key|recovery\s+words)\b)/i.test(text))fail('Private material is not accepted. Export a public-only descriptor. Never enter recovery words or private keys.');
    if(/^(?:ur:|B\$)/i.test(text))fail('Animated UR and BBQr payloads are not supported in this prototype. Use a plain-text descriptor QR or export a descriptor file.');
    if(text.split('#').length!==2)fail('The descriptor needs its 8-character checksum (#xxxxxxxx). Re-export it with the checksum included.');
    const [body,sum]=text.split('#');
    if(!/^[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{8}$/.test(sum)||checksum(body)!==sum)fail('Descriptor checksum mismatch. Nothing was imported. Re-scan or re-export the descriptor.');
    let required,expressions;
    if(/^wpkh\(.+\)$/.test(body)){required=1;expressions=[body.slice(5,-1)];}
    else if(/^wsh\(sortedmulti\(2,.+\)\)$/.test(body)){required=2;expressions=body.slice(18,-2).split(',');if(expressions.length!==3)fail('A 2-of-3 descriptor must contain exactly three public keys.');}
    else fail('Supported policies are native SegWit single-sig wpkh(...) and 2-of-3 wsh(sortedmulti(2,...)). No policy was changed.');
    const keys=await Promise.all(expressions.map(keyExpression));
    if(new Set(keys.map(k=>k.material)).size!==keys.length)fail('The multisig descriptor repeats an extended public key. Use three distinct public keys.');
    if(new Set(keys.map(k=>k.network)).size!==1)fail('The descriptor mixes mainnet and test-network public keys.');
    if(new Set(keys.map(k=>k.branch)).size!==1)fail('All keys must use the same receive/change branch.');
    const identity=JSON.stringify([required,keys[0].network,keys.map(k=>[k.material,k.fingerprint,k.originSteps,k.prefix]).sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)))]);
    return {text,body,required,keys,network:keys[0].network,branch:keys[0].branch,identity};
  }
  function extract(raw){
    if(typeof raw!=='string'||new TextEncoder().encode(raw).length>MAX_SIZE)fail('Use a public descriptor text or JSON file smaller than 64 KB.');
    const text=raw.trim();if(!text)fail('The file or QR code is empty.');
    if(/(?:[xtyzuvYZUV]prv|"(?:seed|mnemonic|private[_ -]?key|xprv|seed_words)"\s*:)/i.test(text))fail('This export appears to contain private material. Nothing was imported. Use a public-only descriptor export.');
    if(/^(?:ur:|B\$)/i.test(text))fail('Animated UR and BBQr payloads are not supported in this prototype. Use a plain-text descriptor QR or descriptor file.');
    let value;
    if(text[0]==='{') {try{value=JSON.parse(text);}catch(_){fail('The descriptor JSON file could not be parsed.');}}
    else if(text[0]==='['&&text.includes('"')) {try{value=JSON.parse(text);}catch(_){fail('The descriptor JSON file could not be parsed.');}}
    else return {name:'',entries:text.split(/\r?\n/).map(v=>v.trim()).filter(Boolean).map(desc=>({desc})),network:null};
    if(['tundra-public-wallet-demo','relay-public-wallet-demo'].includes(value?.format))fail('This is an older simulated backup, not a Bitcoin descriptor. Use a public descriptor export.');
    const name=typeof value.name==='string'?value.name.slice(0,32):typeof value.wallet_name==='string'?value.wallet_name.slice(0,32):'';
    const entries=[];
    const add=(v,internal)=>{if(typeof v!=='string')fail('A descriptor field is not text.');entries.push({desc:v,internal});};
    if(Array.isArray(value)||Array.isArray(value.descriptors)){
      const items=Array.isArray(value)?value:value.descriptors;
      if(items.length>2)fail('Import one wallet policy at a time: one combined descriptor, or its receive/change pair.');
      for(const item of items){if(typeof item==='string')add(item);else if(item&&typeof item==='object'){if(item.internal!==undefined&&typeof item.internal!=='boolean')fail('The JSON internal flag must be true or false.');add(item.desc??item.descriptor,item.internal);}else fail('A descriptor record could not be read.');}
    }else{
      for(const field of ['descriptor','desc'])if(value[field]!==undefined)add(value[field]);
      for(const field of ['receive_descriptor','external_descriptor'])if(value[field]!==undefined)add(value[field],false);
      for(const field of ['change_descriptor','internal_descriptor'])if(value[field]!==undefined)add(value[field],true);
    }
    if(!entries.length)fail('No public descriptor was found. Choose plain descriptor text or supported descriptor JSON, not wallet.dat or an encrypted wallet backup.');
    return {name,entries,network:value.network??null};
  }
  async function parse(raw){
    const {name,entries,network}=extract(raw);
    if(entries.length<1||entries.length>2)fail('Import one combined descriptor or one receive/change pair.');
    const parsed=[];
    for(const item of entries){
      const p=await parseDescriptor(item.desc);
      if(item.internal!==undefined&&p.branch==='<0;1>')fail('A combined descriptor must not be marked as only receive or only change.');
      if(item.internal!==undefined&&item.internal!==(p.branch==='1'))fail('The descriptor branch conflicts with its receive/change label.');
      parsed.push(p);
    }
    if(parsed.some(p=>p.identity!==parsed[0].identity))fail('The receive and change descriptors are not for the same public wallet policy.');
    if(new Set(parsed.map(p=>p.branch)).size!==parsed.length)fail('The same receive/change branch was supplied more than once.');
    if(parsed.length===2&&parsed.some(p=>p.branch==='<0;1>'))fail('A combined descriptor already includes both branches. Do not add a second descriptor.');
    const first=parsed[0],dual=first.branch==='<0;1>';
    let receive=parsed.find(p=>p.branch==='0')?.text??null,change=parsed.find(p=>p.branch==='1')?.text??null;
    if(dual){receive=checked(first.body.replaceAll('/<0;1>/*','/0/*'));change=checked(first.body.replaceAll('/<0;1>/*','/1/*'));}
    if(network!==null){
      if(!['mainnet','bitcoin','testnet','testnet3','testnet4','signet','regtest','test-family'].includes(network))fail('The JSON network value is unsupported.');
      if((['mainnet','bitcoin'].includes(network))!==(first.network==='mainnet'))fail('The JSON network label conflicts with the extended public keys.');
    }
    return {name,required:first.required,network:network==='bitcoin'?'mainnet':network??first.network,
      keys:first.keys.map((k,i)=>({ref:'public:descriptor-key-'+(i+1),fingerprint:k.fingerprint,origin:k.origin,xpub:k.xpub})),
      descriptors:parsed.map(p=>p.text),receive,change,complete:!!receive&&!!change,identity:first.identity,combined:dual};
  }
  async function merge(previous,raw){
    const next=await parse(raw);
    if(previous.identity!==next.identity)fail('That descriptor belongs to a different wallet. The original import was left unchanged.');
    if(previous.network!==next.network&&previous.network!=='test-family'&&next.network!=='test-family')fail('The receive/change files have conflicting network labels.');
    if(previous.receive&&next.receive||previous.change&&next.change)fail('This branch is already included. Import the missing receive or change descriptor.');
    return parse(JSON.stringify({name:previous.name,network:previous.network==='test-family'?next.network:previous.network,descriptors:[...previous.descriptors,...next.descriptors]}));
  }
  return Object.freeze({parse,merge,checksum,checked,parseDescriptor,MAX_SIZE});
})();
if(typeof module!=='undefined')module.exports=TundraDescriptor;
