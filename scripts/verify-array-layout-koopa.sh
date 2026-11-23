#!/usr/bin/env bash
set -euo pipefail
python3 - <<'PY'
from pathlib import Path
import json,re,subprocess
root=Path('evidence/array-layout-native');root.mkdir(parents=True,exist_ok=True)
cases=json.loads(Path('evidence/arrays/checks.json').read_text());assert len(cases)==99
records=[]
def run(folder,ir,expected):
 folder.mkdir(exist_ok=True);(folder/'program.koopa').write_text(ir.replace('@main(', '@compiled_main('))
 with (folder/'program.ll').open('w') as out:subprocess.run(['koopac',str(folder/'program.koopa')],stdout=out,check=True)
 subprocess.run(['llc','--filetype=obj',str(folder/'program.ll'),'-o',str(folder/'program.o')],check=True)
 (folder/'harness.c').write_text('extern int compiled_main(void);int main(void){return compiled_main()!=('+str(expected)+');}\n')
 subprocess.run(['clang',str(folder/'harness.c'),str(folder/'program.o'),'-o',str(folder/'program')],check=True)
 return subprocess.run([str((folder/'program').resolve())],timeout=10).returncode
# A valid typed IR pointer calculation is redirected to row zero, preserving its column step.
case=cases[2];ir=case['koopa'];expected=case['gcc_i32'];assert expected==22
wrong,n=re.subn(r'(getelemptr @__sysy_global_\w+, )[^\n]+',lambda m:m.group(1)+'0',ir,count=1);assert n==1
assert run(root/'stride-original',ir,expected)==0
status=run(root/'stride-wrong',wrong,expected);assert status==1
records.append({'kind':'first_row_address_redirected_to_zero','baseline_i32':expected,'wrong_returncode':status,'detected':True})
# Poison the actual [0][1] cell before the initializer, then remove its first implicit-zero store.
case=cases[3];ir=case['koopa'];expected=case['gcc_i32'];assert expected==0
marker='  %v0 = alloc [[i32, 3], 2]\n';assert ir.count(marker)==1
poison='  %probe_row = getelemptr %v0, 0\n  %probe_cell = getelemptr %probe_row, 1\n  store 99, %probe_cell\n'
poisoned=ir.replace(marker,marker+poison,1);assert len(re.findall(r'^  store 0, %t\d+$',poisoned,re.M))==3
assert run(root/'zero-original',poisoned,expected)==0
wrong,n=re.subn(r'^  store 0, %t\d+\n','',poisoned,count=1,flags=re.M);assert n==1
status=run(root/'zero-wrong',wrong,expected);assert status==1
records.append({'kind':'implicit_zero_store_removed_after_poison','baseline_i32':expected,'poison_i32':99,'wrong_returncode':status,'detected':True})
(root/'checks.json').write_text(json.dumps(records,indent=2)+'\n')
print('PASS 2 official Koopa/LLVM array address and poisoned implicit-zero negative controls')
PY
