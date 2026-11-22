#!/usr/bin/env bash
set -euo pipefail
mkdir -p evidence/runtime-io
python3 - <<'PY'
from pathlib import Path
import hashlib,json,os,subprocess
root=Path('evidence/runtime-io');exe=Path('target/release/sysy_compiler').resolve();lib=Path(os.environ.get('CDE_LIBRARY_PATH','/opt/lib'))
prototypes='int getint(void);int getch(void);void putint(int);void putch(int);void starttime(void);void stoptime(void);\n'
cases=[
 ('int main(){int a=getint(),b=getint();putint(a*17+b);putch(32);putint(a-b);putch(10);return 0;}',b'-12345 456\n',b'-209409 -12801\n'),
 ('int main(){int c=getch();while(c!=10&&c!=-1){putch(c);c=getch();}putch(10);return 0;}',b'SysY-2026\n',b'SysY-2026\n'),
 ('int side(int x){putint(x);putch(32);return x;}int main(){int z=0,o=1;z&&side(1);o||side(2);o&&side(3);z||side(4);putch(10);return 0;}',b'',b'3 4 \n'),
 ('void emit(int n){if(n){putint(n);putch(32);emit(n-1);}}int main(){emit(4);putch(10);return 0;}',b'',b'4 3 2 1 \n'),
 ('int main(){starttime();putint(-2147483647-1);putch(32);putint(2147483647);putch(10);stoptime();return 0;}',b'',b'-2147483648 2147483647\n'),
 ('int g;void read(){g=getint();}int twice(int x){return x*2;}int main(){read();putint(twice(g));putch(10);return 0;}',b'1234567\n',b'2469134\n'),
]
records=[]
for index,(source,inputs,expected) in enumerate(cases):
 folder=root/str(index);folder.mkdir(exist_ok=True);(folder/'input.c').write_text(source);(folder/'stdin.txt').write_bytes(inputs);(folder/'expected.txt').write_bytes(expected)
 (folder/'reference.c').write_text(prototypes+source)
 subprocess.run(['clang',str(folder/'reference.c'),'-L'+str(lib/'native'),'-lsysy','-o',str(folder/'reference')],check=True)
 def run(name,command):
  result=subprocess.run(command,input=inputs,capture_output=True,timeout=10);assert result.returncode==0 and result.stdout==expected,(index,name,result.returncode,result.stdout,expected,result.stderr)
  (folder/(name+'-stdout.txt')).write_bytes(result.stdout);(folder/(name+'-stderr.txt')).write_bytes(result.stderr)
  return {'returncode':result.returncode,'stdout_hex':result.stdout.hex(),'matches_expected_bytes':True}
 reference=run('reference',[str((folder/'reference').resolve())])
 subprocess.run([str(exe),'-koopa',str(folder/'input.c'),'-o',str(folder/'main.koopa')],check=True)
 with (folder/'main.ll').open('w') as output:subprocess.run(['koopac',str(folder/'main.koopa')],stdout=output,check=True)
 subprocess.run(['llc','--filetype=obj',str(folder/'main.ll'),'-o',str(folder/'native.o')],check=True)
 subprocess.run(['clang',str(folder/'native.o'),'-L'+str(lib/'native'),'-lsysy','-o',str(folder/'native')],check=True)
 native=run('native',[str((folder/'native').resolve())])
 subprocess.run([str(exe),'-riscv',str(folder/'input.c'),'-o',str(folder/'main.s')],check=True)
 subprocess.run(['clang','-target','riscv32-unknown-linux-elf','-march=rv32im','-mabi=ilp32','-c',str(folder/'main.s'),'-o',str(folder/'riscv.o')],check=True)
 subprocess.run(['ld.lld',str(folder/'riscv.o'),'-L'+str(lib/'riscv32'),'-lsysy','-o',str(folder/'riscv')],check=True)
 rv32=run('rv32',['qemu-riscv32-static',str(folder/'riscv')])
 records.append({'source':source,'stdin_hex':inputs.hex(),'expected_stdout_hex':expected.hex(),'reference':reference,'koopa_llvm':native,'rv32':rv32})
(root/'checks.json').write_text(json.dumps(records,indent=2)+'\n')
(root/'runtime-libraries.json').write_text(json.dumps({str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in [lib/'native/libsysy.a',lib/'riscv32/libsysy.a']},indent=2)+'\n')
print('PASS 6 real SysY runtime I/O cases: C reference, Koopa/LLVM and RV32 byte-exact stdout; actual library hashes recorded')
PY
