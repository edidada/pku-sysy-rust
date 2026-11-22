#!/usr/bin/env bash
set -euo pipefail
# Expected trap/abort probes must not create core dumps.
ulimit -c 0
mkdir -p evidence/short-circuit
python3 - <<'PY'
from pathlib import Path
import subprocess,re,json
root=Path('evidence/short-circuit');exe=Path('target/release/sysy_compiler').resolve()
cases=[]
for op in ['&&','||']:
 for a in [0,2,-3]:
  cases.append({'source':f'int main(){{int a={a},b=7;return a{op}(b+3);}}','expected':int(bool(a) and True) if op=='&&' else 1,'rhs_executed':[bool(a) if op=='&&' else not bool(a)]})
for a,b in [(0,0),(1,0),(1,2)]:
 cases.append({'source':f'int main(){{int a={a},b={b},c=7;return a&&(b||c);}}','expected':int(bool(a)),'rhs_executed':[bool(a),bool(a) and not bool(b)]})
 cases.append({'source':f'int main(){{int a={a},b={b},c=7;return (a&&b)||c;}}','expected':1,'rhs_executed':[bool(a),not(bool(a) and bool(b))]})
assert len(cases)==12
probes=[]
for index,case in enumerate(cases):
 folder=root/str(index);folder.mkdir(exist_ok=True)
 (folder/'input.c').write_text(case['source'])
 for mode,name in [('-riscv','original.s'),('-koopa','original.koopa')]:
  subprocess.run([str(exe),"--object",mode,str(folder/'input.c'),'-o',str(folder/name)],check=True)
 asm=(folder/'original.s').read_text();ir=(folder/'original.koopa').read_text()
 labels=re.findall(r'^%(sc_rhs_\d+):',ir,re.M)
 assert len(labels)==len(case['rhs_executed'])
 harness=f'.text\n.globl _start\n_start:\n call main\n li t0,{case["expected"]}\n bne a0,t0,fail\n li a0,0\n j exit\nfail:\n li a0,1\nexit:\n li a7,93\n ecall\n'
 (folder/'start.s').write_text(harness)
 subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(folder/'start.s'),'-o',str(folder/'start.o')],check=True)
 def execute(text):
  (folder/'program.s').write_text(text)
  subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(folder/'program.s'),'-o',str(folder/'program.o')],check=True)
  subprocess.run(['riscv64-linux-gnu-ld','-m','elf32lriscv','-static','-e','_start',str(folder/'start.o'),str(folder/'program.o'),'-o',str(folder/'program')],check=True)
  return subprocess.run(['qemu-riscv32',str(folder/'program')],capture_output=True,text=True,timeout=10)
 assert execute(asm).returncode==0,case
 for label,executed in zip(labels,case['rhs_executed']):
  marker='.L0_'+label+':'
  assert asm.count(marker)==1
  changed=asm.replace(marker,marker+'\n  ebreak',1)
  result=execute(changed)
  if executed:
   assert result.returncode in [-5,133],(case,label,result)
  else:assert result.returncode==0,(case,label,result)
  probes.append({'case':index,'source':case['source'],'expected_i32':case['expected'],'koopa':ir,'rhs_label':label,'rhs_executed':executed,'rv32_status':result.returncode,'rv32_stderr':result.stderr})
(root/'probes.json').write_text(json.dumps(probes,indent=2)+'\n')
assert len(probes)==18 and sum(x['rhs_executed'] for x in probes)==10
print('PASS 12 RV32 baselines and 18 RHS trap probes: 10 executed, 8 skipped')
PY
