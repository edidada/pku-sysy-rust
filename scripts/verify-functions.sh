#!/usr/bin/env bash
set -euo pipefail
ulimit -c 0
mkdir -p evidence/functions
python3 - <<'PY'
from pathlib import Path
import json,random,re,subprocess,os
root=Path(os.environ.get('SYSY_FULL_CASE_ROOT','evidence/functions'));root.mkdir(parents=True,exist_ok=True);array_mode=os.environ.get('SYSY_FULL_CASE_KIND')=='arrays';exe=Path('target/release/sysy_compiler').resolve()
if os.environ.get('SYSY_FULL_CASE_FILE'):
 cases=json.loads(Path(os.environ['SYSY_FULL_CASE_FILE']).read_text())
else:
 cases=[
  'int f(int x){return x+1;}int main(){return f(4)+f(6);}',
  'int f(int a,int b,int c,int d,int e,int f,int g,int h,int i,int j){return a+2*b+3*c+4*d+5*e+6*f+7*g+8*h+9*i+10*j;}int main(){return f(1,2,3,4,5,6,7,8,9,10);}',
  'int fact(int n){if(n<2)return 1;return n*fact(n-1);}int main(){return fact(7);}',
  'int fib(int n){if(n<2)return n;return fib(n-1)+fib(n-2);}int main(){return fib(12);}',
  'int g;void bump(int n){g=g+n;return;}int main(){bump(4);bump(7);return g;}',
  'const int a=3,b=a*7;int g=b,z;int f(int x){g=g+x;return g;}int main(){int q=f(4);q=q+f(5);return q+z;}',
  'int g=5;int f(int g){g=g+3;return g;}int main(){int a=f(7);return a+g;}',
  'int f(int x){x=x+100;return x;}int main(){int x=3;int y=f(x);return x+y;}',
  'int f(int x){return x*3;}int g(int x){return f(x)+f(x+1);}int main(){return f(2)+g(f(3))+f(g(4));}',
  'int hits;int tick(){hits=hits+1;return 1;}int main(){int z=0,o=1;z&&tick();o||tick();o&&tick();z||tick();return hits;}',
  'int f(int n){int r=0;while(n){n=n-1;if(n%2)continue;r=r+n;}return r;}int main(){return f(10);}',
  'void noop(){}int main(){noop();return 123456789;}',
  'int v0(int arg0){int entry=arg0+2;return entry;}int main(){return v0(8);}',
  'int __sysy_global_0(){return 4;}int g=7;int main(){return g+__sysy_global_0();}',
  'int g=1;void f(){if(g)return;g=4;}int main(){f();return g;}',
  'int f(int x){if(x)return x;else return 9;}int main(){return f(0)+f(8);}',
  'int depth;int main(){depth=depth+1;if(depth<5){int saved=main();return saved+depth;}return depth;}',
 ]
 rng=random.Random(20261123)
 for n in range(80):
  count=rng.choice([0,1,8,9,10,16,40]);params=','.join('int p'+str(i) for i in range(count));weights=[rng.randrange(1,7) for _ in range(count)]
  expr='+'.join(f'p{i}*{weights[i]}' for i in range(count)) or '17'
  args=[rng.randrange(-50,51) for _ in range(count)];call=','.join('bump('+str(x)+')' for x in args)
  cases.append('int state=0;int bump(int x){state=state+1;return x;}int f('+params+'){int r='+expr+';return r;}int main(){int saved=state+100;int v=f('+call+');return saved+v+state;}')
 large=','.join(f'v{i}={i}' for i in range(800))
 cases.append('int f(int x){int '+large+';return x+v0+v511+v799;}int main(){return f(7)+f(9);}')
 params=','.join(f'int p{i}' for i in range(160));args=','.join(str(i) for i in range(160))
 cases.append('int wide('+params+'){return p0+p7+p8+p100+p159;}int main(){return wide('+args+');}')
assert len(cases)==int(os.environ.get('SYSY_FULL_CASE_COUNT','99'))
checks=[];negatives=[]
for index,source in enumerate(cases):
 folder=root/str(index);folder.mkdir(exist_ok=True);(folder/'input.c').write_text(source)
 oracle='#include <stdio.h>\n#define main compiled_main\n'+source+'\n#undef main\nint main(void){printf("%d\\n",compiled_main());return 0;}\n'
 (folder/'oracle.c').write_text(oracle)
 subprocess.run(['gcc','-std=c11','-O2','-fsanitize=undefined','-fno-sanitize-recover=all',str(folder/'oracle.c'),'-o',str(folder/'oracle')],check=True)
 value=int(subprocess.check_output([str((folder/'oracle').resolve())],text=True,timeout=10))
 for mode,name in [('-riscv','main.s'),('-koopa','main.koopa')]:subprocess.run([str(exe),mode,str(folder/'input.c'),'-o',str(folder/name)],check=True)
 asm=(folder/'main.s').read_text()
 # Check actual stack alignment at each generated function entry, before its prologue.
 names=re.findall(r'^  \.globl (\w+)$',asm,re.M);assert names
 for name in names:
  marker=name+':\n';assert asm.count(marker)==1
  asm=asm.replace(marker,marker+'  andi t0, sp, 15\n  bnez t0, .Labi_fail\n',1)
 asm+='\n.Labi_fail:\n  li a0, 1\n  li a7, 93\n  ecall\n'
 if array_mode and index==3:
  # Poison the actual six-cell array before its initializer. A missing zero store must leave 99.
  offsets=re.findall(r'li t3, (\d+)\n  add t0, sp, t3',asm);assert len(set(offsets))==1
  offset=int(offsets[0]);poison='  li t4, 99\n  li t5, '+str(offset)+'\n  add t5, sp, t5\n'+''.join('  sw t4, '+str(i*4)+'(t5)\n' for i in range(6))
  assert asm.count('.L0_entry:\n')==1;asm=asm.replace('.L0_entry:\n','.L0_entry:\n'+poison,1)
 (folder/'checked.s').write_text(asm)
 harness='.option norelax\n.data\nsaved_sp:.word 0\n.text\n.globl _start\n_start:\n la t0,saved_sp\n sw sp,0(t0)\n'
 harness+=''.join(f' li s{i},{305419896+i}\n' for i in range(12))
 harness+=f' call main\n li t0,{value}\n bne a0,t0,fail\n la t0,saved_sp\n lw t1,0(t0)\n bne sp,t1,fail\n'
 harness+=''.join(f' li t0,{305419896+i}\n bne s{i},t0,fail\n' for i in range(12))
 harness+=' li a0,0\n j exit\nfail:\n li a0,1\nexit:\n li a7,93\n ecall\n';(folder/'start.s').write_text(harness)
 subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(folder/'start.s'),'-o',str(folder/'start.o')],check=True)
 def build_run(text,timeout=10):
  (folder/'probe.s').write_text(text)
  subprocess.run(['riscv64-linux-gnu-as','-march=rv32im','-mabi=ilp32',str(folder/'probe.s'),'-o',str(folder/'main.o')],check=True)
  subprocess.run(['riscv64-linux-gnu-ld','-m','elf32lriscv','-static','-e','_start',str(folder/'start.o'),str(folder/'main.o'),'-o',str(folder/'program')],check=True)
  return subprocess.run(['qemu-riscv32',str(folder/'program')],timeout=timeout).returncode
 assert build_run(asm)==0,(index,source,value)
 if index in [0,1]:
  if index==0:
   assert 'lw ra, 0(t3)' in asm;wrong=asm.replace('lw ra, 0(t3)','nop');kind='ra_restore_removed'
  else:
   marker='  li t3, 0\n  add t3, sp, t3\n  sw t0, 0(t3)'
   before,main=asm.rsplit('\nmain:\n',1);assert main.count(marker)==1,'outgoing argument marker must be unique in main'
   wrong=before+'\nmain:\n'+main.replace(marker,marker.replace('li t3, 0','li t3, 4'),1);kind='ninth_argument_overwrites_tenth'
  (folder/'wrong.s').write_text(wrong)
  try:status=build_run(wrong,1);assert status!=0,'corrupted ABI escaped checks';detected='nonzero';
  except subprocess.TimeoutExpired:status=None;detected='timeout'
  negatives.append({'kind':kind,'detected':detected,'returncode':status})
 if array_mode and index in [2,3]:
  if index==2:
   marker='  li t2, 12\n';assert asm.count(marker)==1
   wrong=asm.replace(marker,'  li t2, 4\n',1);kind='row_stride_12_changed_to_4'
  else:
   marker='  li t0, 0\n  sw t0, 0(t1)';assert asm.count(marker)==3
   wrong=asm.replace(marker,'  li t0, 0\n  nop',1);kind='implicit_zero_store_removed_after_poison'
  (folder/'wrong-array.s').write_text(wrong)
  try:status=build_run(wrong,1);assert status!=0,'corrupted array layout escaped checks';detected='nonzero'
  except subprocess.TimeoutExpired:status=None;detected='timeout'
  negatives.append({'kind':kind,'detected':detected,'returncode':status})
 checks.append({'source':source,'function_name':'main','gcc_i32':value,'koopa':(folder/'main.koopa').read_text(),'rv32_full_i32_match':True,'entry_stack_alignment_checked':True,'sp_and_12_saved_registers_checked':True})
invalid=[
 'int f(){return 0;}', 'void main(){}', 'int main(int x){return x;}',
 'int main(){return 0;}int main(){return 1;}',
 'int main(){return missing();}', 'int main(){return f();}int f(){return 1;}',
 'int f(int x){return x;}int main(){return f();}',
 'int f(int x){return x;}int main(){return f(1,2);}',
 'void f(){}int main(){return f();}', 'void f(){return 1;}int main(){return 0;}',
 'int main(){return;}', 'int f(int x,int x){return x;}int main(){return 0;}',
 'int g;int g(){return 1;}int main(){return 0;}',
 'int getint(){return 1;}int main(){return 0;}',
 'int g=1;int h=g;int main(){return h;}', 'const int g=g;int main(){return g;}',
 'int f(){return 1;}int main(){int f=2;return f();}',
 'void f(){}int main(){return 1+f();}', 'int main(){return putint(1);}',
 'int main(){return 1;}int f(){return missing();}',
]
if array_mode:
 invalid.extend([
  'int main(){const int a[2]={};a[0]=1;return a[0];}',
  'const int a[2]={1,2};int main(){a[1]=3;return a[1];}',
  'int main(){int a[-1]={};return 0;}',
  'int main(){int n=3;int a[n]={};return 0;}',
  'int a[getint()];int main(){return 0;}',
  'int main(){int a[2]=3;return 0;}',
  'int main(){int a={};return 0;}',
  'int main(){int a[2]={1,2,3};return 0;}',
  'int main(){int a[2][2][2]={{},1,{}};return 0;}',
  'int main(){int a[2]={};a=3;return 0;}',
  'int main(){int a[2]={};return a;}',
  'int main(){int a[2]={};return getarray(a[0]);}',
  'int main(){int a[2][3]={};putarray(2,a);return 0;}',
  'int f(int a[][3]){return a[0][0];}int main(){int a[2][2]={};return f(a);}',
  'int f(int x){return x;}int main(){int a[2]={};return f(a);}',
  'int main(){int a[2]={};return a[0][1];}',
  'int main(){int a[2]={};return a[missing];}',
  'const int a[2]={1,2};const int b=a[0];int main(){return b;}',
  'int main(){int a[2]={};int a;return 0;}',
  'int f(int a[3]){return a[0];}int main(){return 0;}',
 ])
diagnostics=[]
for index,source in enumerate(invalid):
 (root/'invalid.c').write_text(source)
 for mode in ['-koopa','-riscv']:
  output=root/'preserved-output.txt';output.write_text('preserved')
  result=subprocess.run([str(exe),mode,str(root/'invalid.c'),'-o',str(output)],capture_output=True,text=True)
  assert result.returncode!=0 and result.stderr.strip() and output.read_text()=='preserved',(source,mode)
  diagnostics.append({'source':source,'mode':mode,'returncode':result.returncode,'diagnostic':result.stderr,'output_preserved':True})
(root/'diagnostics.json').write_text(json.dumps(diagnostics,indent=2)+'\n')
(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n');(root/'abi-negatives.json').write_text(json.dumps(negatives,indent=2)+'\n')
print('PASS',len(checks),'GCC/RV32 full-program cases, every entry alignment, sp/12 saved registers;',len(negatives),'controlled corruptions and',len(diagnostics),'invalid/output-preservation checks detected')
PY
