"""Build public-only, unfunded test fixtures, never a real wallet backup."""
from pathlib import Path
import hashlib,json
root=Path(__file__).resolve().parent
alpha='123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
chars="0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH`#\"\\ "
cs='qpzry9x8gf2tvdw0s3jn54khce6mua7l'
generator=[0xf5dee51989,0xa9fdca3312,0x1bab10e32d,0x3706b1677a,0x644d626ffd]
def checksum(s):
    values=[];groups=[]
    for c in s:
        v=chars.index(c);values.append(v&31);groups.append(v>>5)
        if len(groups)==3:values.append(groups[0]*9+groups[1]*3+groups[2]);groups=[]
    if len(groups)==1:values.append(groups[0])
    elif len(groups)==2:values.append(groups[0]*3+groups[1])
    chk=1
    for v in values+[0]*8:
        top=chk>>35;chk=((chk&0x7ffffffff)<<5)^v
        for i in range(5):
            if (top>>i)&1:chk^=generator[i]
    chk^=1
    return s+'#'+''.join(cs[(chk>>(5*(7-i)))&31] for i in range(8))
def b58(payload):
    data=payload+hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    n=int.from_bytes(data,'big');out=''
    while n:n,i=divmod(n,58);out=alpha[i]+out
    return '1'*(len(data)-len(data.lstrip(b'\0')))+out
points=[
'0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798',
'02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5',
'02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9']
def pub(i,depth,child,network='test'):
    payload=bytes.fromhex('043587cf' if network=='test' else '0488b21e')+bytes([depth])+bytes.fromhex('11223344')+child.to_bytes(4,'big')+hashlib.sha256(f'Tundra PUBLIC TEST fixture {i}'.encode()).digest()+bytes.fromhex(points[i])
    return b58(payload)
single=checksum('wpkh([a1b2c3d4/84h/1h/0h]'+pub(0,3,0x80000000)+'/<0;1>/*)')
keys=[f'[{fp}/48h/1h/0h/2h]{pub(i,4,0x80000002)}/<0;1>/*' for i,fp in enumerate(['a1b2c3d4','1122aabb','9988ccdd'])]
multi=checksum('wsh(sortedmulti(2,'+','.join(keys)+'))')
main=checksum('wpkh([a1b2c3d4/84h/0h/0h]'+pub(0,3,0x80000000,'main')+'/<0;1>/*)')
receive=checksum(multi.split('#')[0].replace('/<0;1>/*','/0/*'))
change=checksum(multi.split('#')[0].replace('/<0;1>/*','/1/*'))
examples={'single':{'name':'Everyday (test)','network':'signet','descriptor':single},'multi':{'name':'Savings (test)','network':'signet','descriptor':multi}}
(root/'examples.js').write_text('"use strict";\n// Public-only test fixtures. Never fund these descriptors.\nconst DESCRIPTOR_EXAMPLES = '+json.dumps(examples,indent=2)+';\n')
for name,raw in [('single-sig',single),('two-of-three',multi),('receive-only',receive),('change-only',change),('single-sig-mainnet-public-test',main)]:
    (root/'examples'/f'{name}.txt').write_text(raw+'\n')
for name,data in [('single-sig',examples['single']),('two-of-three',examples['multi']),('core-style-pair',{'name':'Savings (test)','network':'signet','descriptors':[{'desc':receive,'internal':False},{'desc':change,'internal':True}]})]:
    (root/'examples'/f'{name}.json').write_text(json.dumps(data,indent=2)+'\n')
# QR images are exact encodings of these public test descriptors, not mock art.
import qrcode
for name,raw in [('single-sig',single),('two-of-three',multi)]:
    q=qrcode.QRCode(error_correction=qrcode.constants.ERROR_CORRECT_M,box_size=7,border=4)
    q.add_data(raw);q.make(fit=True);q.make_image().save(root/'examples'/f'{name}-qr.png')
(root/'examples'/'README.md').write_text('# Public test fixtures only\n\nDo not send funds to addresses derived from these descriptors. They are fabricated public test data, not recoverable wallet backups. Test-network and mainnet-encoded examples are supplied for parser tests. The QR PNGs contain the exact descriptor text in the matching TXT files.\n')
print('Created public descriptor fixtures.')
