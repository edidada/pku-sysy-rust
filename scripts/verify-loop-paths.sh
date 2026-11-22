#!/usr/bin/env bash
set -euo pipefail
ulimit -c 0
mkdir -p evidence/loop-paths
python3 - <<'PY'
from pathlib import Path
import json,re,subprocess,struct
root=Path('evidence/loop-paths');exe=Path('target/release/sysy_compiler').resolve()
cases=[
 ('int i=0,r=0;while(i<6){r=r+((i%2)&&7);i=i+1;}return r;',3,3),
 ('int i=0,r=0;while(i<6){r=r+((i%2)||0);i=i+1;}return r;',3,3),
 ('int i=0,r=0;while(i<5&&i<7){r=r+i;i=i+1;}return r;',10,5),
 ('int i=0,r=0;while(i<3||i<5){r=r+i;i=i+1;}return r;',10,3),
 ('int i=0,r=0;while(i<6){i=i+1;if(i%2)continue;r=r+(i>2&&7);}return r;',2,2),
 ('int i=0,r=0;while(i<8){if(i==3)break;r=r+((i%2)||0);i=i+1;}return r;',1,2)
]
records=[]
for index,(body,value,count) in enumerate(cases):
 folder=root/str(index);folder.mkdir(exist_ok=True);source='int main(){'+body+'}';(folder/'input.c').write_text(source)
 for mode,name in [('-riscv','original.s'),('-koopa','original.koopa')]:subprocess.run([str(exe),mode,str(folder/'input.c'),'-o',str(folder/name)],check=True)
 asm=(folder/'original.s').read_text();ir=(folder/'original.koopa').read_text();labels=re.findall(r'^%(sc_rhs_\d+):',ir,re.M);assert len(labels)==1
 label=labels[0];marker='.L0_'+label+':';assert asm.count(marker)==1
 hook='\n  la t4, rhs_count_probe\n  lw t5, 0(t4)\n  addi t5, t5, 1\n  sw t5, 0(t4)'
 changed=asm.replace(marker,marker+hook,1);(folder/'counter.s').write_text(changed)
 harness='.data\n.globl rhs_count_probe\nrhs_count_probe:.word 0\nprobe_output:.word 0,0\n.text\n.globl _start\n_start:\n call main\n la t0,probe_output\n sw a0,0(t0)\n la t2,rhs_count_probe\n lw t1,0(t2)\n sw t1,4(t0)\n li a0,1\n mv a1,t0\n li a2,8\n li a7,64\n ecall\n li a0,0\n li a7,93\n ecall\n'
 (folder/'start.s').write_text(harness)
 subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(folder/'start.s'),'-o',str(folder/'start.o')],check=True)
 def execute(name):
  subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(folder/(name+'.s')),'-o',str(folder/'main.o')],check=True)
  subprocess.run(['riscv64-linux-gnu-ld','-m','elf32lriscv','-static','-e','_start',str(folder/'start.o'),str(folder/'main.o'),'-o',str(folder/'program')],check=True)
  result=subprocess.run(['qemu-riscv32',str(folder/'program')],capture_output=True,check=True,timeout=10)
  assert len(result.stdout)==8;return struct.unpack('<ii',result.stdout)
 assert execute('original')==(value,0)
 observed=execute('counter');assert observed==(value,count)
 records.append({'source':source,'value':value,'expected_rhs_count':count,'rv32_observed_value':observed[0],'rv32_observed_rhs_count':observed[1],'rhs_label':label,'koopa':ir})
# Check the timeout detector using a wrong condition edge on an otherwise terminating loop.
folder=root/'liveness';folder.mkdir(exist_ok=True);(folder/'input.c').write_text('int main(){int i=0;while(i<3){i=i+1;}return i;}')
for mode,name in [('-riscv','original.s'),('-koopa','original.koopa')]:subprocess.run([str(exe),mode,str(folder/'input.c'),'-o',str(folder/name)],check=True)
asm=(folder/'original.s').read_text();ir=(folder/'original.koopa').read_text();test=re.findall(r'^%(while_cond_\d+):',ir,re.M)[0];body=re.findall(r'^%(while_body_\d+):',ir,re.M)[0]
needle='la t3, .L0_'+body;assert asm.count(needle)==1
(folder/'wrong.s').write_text(asm.replace(needle,'la t3, .L0_'+test,1))
(folder/'wrong.koopa').write_text(ir.replace('%'+body+'\n','%'+body+'\n'))
# Change only the first true edge, retaining its false exit and all other instructions.
pattern=r'(br [^,\n]+, %)'+re.escape(body)+r'(, %)'
wrong_ir,n=re.subn(pattern,lambda m:m.group(1)+test+m.group(2),ir,count=1);assert n==1
(folder/'wrong.koopa').write_text(wrong_ir)
harness='.text\n.globl _start\n_start:\n call main\n li t0,3\n bne a0,t0,fail\n li a0,0\n j exit\nfail:\n li a0,1\nexit:\n li a7,93\n ecall\n';(folder/'start.s').write_text(harness)
subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(folder/'start.s'),'-o',str(folder/'start.o')],check=True)
for name in ['original','wrong']:
 subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(folder/(name+'.s')),'-o',str(folder/(name+'.o'))],check=True)
 subprocess.run(['riscv64-linux-gnu-ld','-m','elf32lriscv','-static','-e','_start',str(folder/'start.o'),str(folder/(name+'.o')),'-o',str(folder/name)],check=True)
subprocess.run(['qemu-riscv32',str(folder/'original')],check=True,timeout=10)
try:subprocess.run(['qemu-riscv32',str(folder/'wrong')],timeout=1,check=True)
except subprocess.TimeoutExpired:detected=True
else:raise AssertionError('wrong loop edge was not detected as a timeout')
(root/'checks.json').write_text(json.dumps(records,indent=2)+'\n');(folder/'rv32-check.json').write_text(json.dumps({'baseline_i32':3,'wrong_true_edge_target':test,'timeout_seconds':1,'timeout_detected':detected})+'\n')
print('PASS 6 RV32 per-iteration RHS counters and terminating/timeout liveness controls')
PY
