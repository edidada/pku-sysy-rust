#!/usr/bin/env bash
set -euo pipefail
mkdir -p evidence/declarations
python3 - <<'PY'
from pathlib import Path
import subprocess,json,random,re,os
root=Path(os.environ.get('SYSY_CASE_ROOT','evidence/declarations'));root.mkdir(parents=True,exist_ok=True);exe=Path('target/release/sysy_compiler').resolve()
if os.environ.get('SYSY_CASE_FILE'):
 bodies=json.loads(Path(os.environ['SYSY_CASE_FILE']).read_text())
 assert len(bodies)==int(os.environ['SYSY_CASE_EXPECTED'])
 fold_begin=len(bodies)
else:
 bodies=[
  'const int x=233*4;int y=10;y=y+x/2;return y;',
  'const int a=2,b=a*3,c=b+1;return c;',
  'int a=1,b=a+1,c=b*3;return c;',
  'int a;a=7;return a;',
  'int a=5;a=a+2;return a;',
  'const int a=-7/3,b=-7%3;return a*10+b;',
  'const int main=5;return main;',
  'int main=7;main=main*3;return main;',
  'int a=1;return a;a=999;return a;',
  'const int a=2147483647,b=-2147483647-1;return a>b;',
  'int a=-2147483647-1,b=2147483647;return a<b;',
  'const int a=2&&4,b=2||4;int x=a+b;return x;',
  'int x=1,y=2;x=y;y=x+1;return x*10+y;',
  'const int a=01,b=0x10,c=010;int x=a+b+c;return x;',
  'int a=7,b=3;return -a/b*10+(-a%b);',
  'int x=5;const int y=9;int z=x+y;return z;',
  'int a=1,b=2,c=3;a=a+b+c;b=a-b;c=b-c;return a*100+b*10+c;',
  'const int a=!(2-2),b=!!-3;return a+b;',
  'int x=2,y=4;return (x&&y)*100+(x||y);',
  'int a;const int b=5;a=b;return a;'
 ]
 rng=random.Random(20261121)
 for n in range(160):
  a=rng.randrange(1,24);x=rng.randrange(-50,51)
  body=f'const int a={a},b=a*3-2;int x={x},y=x+a,z;z=b;'
  for i in range(12):
   target=rng.choice(['x','y','z']);lhs=rng.choice(['x','y','z','a','b'])
   op=rng.choice(['+','-','*','/','%','<','>','<=','>=','==','!=','&&','||'])
   rhs=str(rng.randrange(1,24)) if op in ['/', '%'] else rng.choice(['x','y','z','a','b'])
   body+=f'{target}=(({lhs}{op}{rhs})%997);'
  body+='return x-y+z;';bodies.append(body)
 # 800 separate local cells plus later loads exercise both local and temporary stack offsets.
 bodies.append('int '+','.join(f'v{i}={i+1}' for i in range(800))+';return v0+v511+v799;')
 assert len(bodies)==181
 fold_begin=len(bodies)
 expressions=json.loads(Path('evidence/expressions/checks.json').read_text())
 assert len(expressions)==185
 bodies.extend('const int folded='+case['expression']+';return folded;' for case in expressions)
 assert len(bodies)==366
reference='#include <stdio.h>\n'+''.join(f'int case_{i}(void){{{body}}}\n' for i,body in enumerate(bodies))
reference+='int main(void){'+''.join(f'printf("%d\n",case_{i}());' for i in range(len(bodies)))+'return 0;}'
# Escape the C printf newline rather than creating a physical line inside its literal.
reference=reference.replace('%d\n"','%d\\n"')
(root/'oracle.c').write_text(reference)
subprocess.run(['gcc','-std=c11','-O2','-fsanitize=undefined','-fno-sanitize-recover=all',str(root/'oracle.c'),'-o',str(root/'oracle')],check=True)
expected=list(map(int,subprocess.check_output([str(root/'oracle')],text=True).splitlines()))
assert len(expected)==len(bodies)
checks=[]
for index,(body,value) in enumerate(zip(bodies,expected)):
 function_name=f'answer_{index}' if index%7==0 else 'main'
 source='int '+function_name+'(){'+body+'}';(root/'input.c').write_text(source)
 subprocess.run([str(exe),"--object",'-riscv',str(root/'input.c'),'-o',str(root/'main.s')],check=True)
 subprocess.run([str(exe),"--object",'-koopa',str(root/'input.c'),'-o',str(root/'main.koopa')],check=True)
 harness='.data\nsaved_sp: .word 0\n.text\n.globl _start\n_start:\n la t0,saved_sp\n sw sp,0(t0)\n'
 harness+=''.join(f' li s{i},{305419896+i}\n' for i in range(12))
 harness+=f' call {function_name}\n li t0,{value}\n bne a0,t0,fail\n la t0,saved_sp\n lw t1,0(t0)\n bne sp,t1,fail\n'
 harness+=''.join(f' li t0,{305419896+i}\n bne s{i},t0,fail\n' for i in range(12))
 harness+=' li a0,0\n j exit\nfail:\n li a0,1\nexit:\n li a7,93\n ecall\n'
 (root/'start.s').write_text(harness)
 for name in ['start','main']:
  subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(root/(name+'.s')),'-o',str(root/(name+'.o'))],check=True)
 subprocess.run(['riscv64-linux-gnu-ld','-m','elf32lriscv','-static','-e','_start',str(root/'start.o'),str(root/'main.o'),'-o',str(root/'program')],check=True)
 subprocess.run(['qemu-riscv32',str(root/'program')],check=True,timeout=10)
 if index==0:
  wrong=harness.replace(f'li t0,{value}\n bne a0',f'li t0,{value+256}\n bne a0')
  assert wrong!=harness
  (root/'wrong-start.s').write_text(wrong)
  subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(root/'wrong-start.s'),'-o',str(root/'wrong-start.o')],check=True)
  subprocess.run(['riscv64-linux-gnu-ld','-m','elf32lriscv','-static','-e','_start',str(root/'wrong-start.o'),str(root/'main.o'),'-o',str(root/'wrong-program')],check=True)
  assert subprocess.run(['qemu-riscv32',str(root/'wrong-program')],timeout=10).returncode==1
 ir=(root/'main.koopa').read_text()
 if index>=fold_begin:
  assert not re.search(r'\b(?:alloc|load|store|add|sub|mul|div|mod|lt|gt|le|ge|eq|ne|and|or)\b',ir),ir
  assert re.search(r'\bret\s+'+re.escape(str(value))+r'\s*\n',ir),ir
 checks.append({'function_name':function_name,'constant_folded_to_literal':index>=fold_begin,'source':source,'gcc_i32':value,'full_a0_and_all_saved_match':True,'koopa':(root/'main.koopa').read_text()})
(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
(root/'elf-header.txt').write_text(subprocess.check_output(['riscv64-linux-gnu-readelf','-h',str(root/'program')],text=True))
invalid=['const int a;return 0;','int a=1;int a=2;return a;','const int a=1;a=2;return a;',
 'return x;int x=1;','x=2;return 0;','int a=1;const int b=a;return b;','const int a=b,b=1;return a;',
 'const int a=a;return a;','const int a=1/0;return a;','const int a=2147483647+1;return a;',
 'int a=1;return (a=2);','int int=1;return 0;','int a=1,;return a;']
if os.environ.get('SYSY_INVALID_FILE'):
 invalid=json.loads(Path(os.environ['SYSY_INVALID_FILE']).read_text())
for body in invalid:
 (root/'invalid.c').write_text('int main(){'+body+'}')
 for mode in ['-koopa','-riscv']:
  output=root/'invalid-output';output.write_text('preserved')
  result=subprocess.run([str(exe),"--object",mode,str(root/'invalid.c'),'-o',str(output)],capture_output=True,text=True)
  assert result.returncode and result.stderr and output.read_text()=='preserved',(body,mode)
(root/'invalid-checks.json').write_text(json.dumps({'cases':invalid,'modes':['-koopa','-riscv'],'diagnostics_and_output_preservation':True},indent=2)+'\n')
print('PASS',len(checks),'GCC/RV32 scalar program cases, full a0, sp and all 12 saved registers; same-low-byte negative rejected;',len(invalid)*2,'invalid diagnostics preserved outputs')
PY
