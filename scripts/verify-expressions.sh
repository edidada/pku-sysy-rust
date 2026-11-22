#!/usr/bin/env bash
set -euo pipefail
mkdir -p evidence/expressions
python3 - <<'PY'
from pathlib import Path
import subprocess,json,random
root=Path('evidence/expressions');exe=Path('target/release/sysy_compiler').resolve()
rng=random.Random(20261120)
cases=['1+2*-3','9-4-2','20/3/2','-7/3','-7%3','7%-3','-7%-3','2&&4','2||4','!(-3)','!!42','1<2<3','10<2<1','1==2<3','1||0&&0','1+2*3<=7==1','(1+2)*3','2147483647','-2147483647-1','(-2147483647-1)<2147483647','2147483647>=(-2147483647-1)','0x7fffffff%017','0x1234+0777-42','+(- -!6)', '+'.join(['1']*800)]
def literal():
 n=rng.randrange(101)
 return rng.choice([str(n),f'0{n:o}',f'0x{n:x}'])
def generate(depth):
 if depth==0:return literal()
 if rng.randrange(5)==0:return rng.choice(['+','-','!'])+'('+generate(depth-1)+')'
 op=rng.choice(['+','-','*','/','%','<','>','<=','>=','==','!=','&&','||'])
 lhs=generate(depth-1)
 rhs=str(rng.randrange(1,24)) if op in ['/','%'] else generate(depth-1)
 # Every generated subexpression is bounded by 996, hence even a product
 # is in signed range; no signed overflow/divide-zero oracle ambiguity.
 return f'((({lhs}){op}({rhs}))%997)'
cases += [generate(rng.randrange(1,5)) for _ in range(160)]
# GCC provides values independently of our lexer, parser and IR evaluator.
reference='#include <stdio.h>\nint main(void){\n'+''.join(f'printf("%d\\n",(int)({x}));\n' for x in cases)+'return 0;\n}'
(root/'oracle.c').write_text(reference)
subprocess.run(['gcc','-std=c11','-O2','-fsanitize=undefined','-fno-sanitize-recover=all',str(root/'oracle.c'),'-o',str(root/'oracle')],check=True)
values=[int(x)for x in subprocess.check_output([str(root/'oracle')],text=True).splitlines()]
assert len(values)==len(cases)
checks=[]
for index,(expression,value) in enumerate(zip(cases,values)):
 source=root/'input.c';source.write_text(f'int main(){{return {expression};}}')
 asm=root/'main.s';subprocess.run([str(exe),"--object",'-riscv',str(source),'-o',str(asm)],check=True)
 # Preserve sp and callee-saved sentinel across the generated leaf function.
 harness=root/'start.s';harness.write_text(f'.text\n.globl _start\n_start:\n mv s1, sp\n li s0, 305419896\n call main\n li t0, {value}\n bne a0, t0, fail\n bne sp, s1, fail\n li t0, 305419896\n bne s0, t0, fail\n li a0, 0\n j exit\nfail:\n li a0, 1\nexit:\n li a7, 93\n ecall\n')
 for name in ['main','start']:
  subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(root/(name+'.s')),'-o',str(root/(name+'.o'))],check=True)
 subprocess.run(['riscv64-linux-gnu-ld','-m','elf32lriscv','-static','-e','_start',str(root/'start.o'),str(root/'main.o'),'-o',str(root/'program')],check=True)
 subprocess.run(['qemu-riscv32',str(root/'program')],check=True,timeout=10)
 # Keep all inputs, expected signed values and generated IR for audit.
 ir=subprocess.check_output([str(exe),"--object",'-koopa',str(source),'-o',str(root/'last.koopa')])
 checks.append({'expression':expression,'gcc_i32':value,'full_a0_and_sp_match':True,'koopa':(root/'last.koopa').read_text()})
(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
print('PASS',len(checks),'GCC-oracle/RV32 expression cases, signed full a0, stack and callee-saved checks')
PY
