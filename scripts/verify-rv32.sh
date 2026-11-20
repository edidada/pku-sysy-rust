#!/usr/bin/env bash
set -euo pipefail
mkdir -p evidence/rv32
python3 - <<'PY'
from pathlib import Path
import subprocess,json
root=Path('evidence/rv32');exe=Path('target/release/sysy_compiler').resolve()
values=sorted(set([0,1,42,127,128,255,256,257,2047,2048,4095,4096,65535,65536,0x7fffffff]+[(i*2654435761)&0x7fffffff for i in range(64)]))
checks=[]
for index,value in enumerate(values):
 for base,literal in [('decimal',str(value)),('octal',f'0{value:o}'),('hex',f'0x{value:x}')]:
  source=root/'input.c';source.write_text(f'/* real RV32 execution */ int main(){{return {literal};}}')
  asm=root/'main.s';subprocess.run([str(exe),'-riscv',str(source),'-o',str(asm)],check=True)
  harness=root/'start.s';harness.write_text(f'.text\n.globl _start\n_start:\n call main\n li t0, {value}\n bne a0, t0, fail\n li a0, 0\n j exit\nfail:\n li a0, 1\nexit:\n li a7, 93\n ecall\n')
  subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(asm),'-o',str(root/'main.o')],check=True)
  subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(harness),'-o',str(root/'start.o')],check=True)
  subprocess.run(['riscv64-linux-gnu-ld','-m','elf32lriscv','-static','-e','_start',str(root/'start.o'),str(root/'main.o'),'-o',str(root/'program')],check=True)
  subprocess.run(['qemu-riscv32',str(root/'program')],check=True,timeout=10)
  checks.append({'value':value,'base':base,'full_a0_match':True})
(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
print('PASS',len(checks),'RV32 runtime checks of complete 32-bit a0, three integer bases')
PY
