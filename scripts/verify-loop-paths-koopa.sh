#!/usr/bin/env bash
set -euo pipefail
ulimit -c 0
python3 - <<'PY'
from pathlib import Path
import json,subprocess
root=Path('evidence/loop-paths');cases=json.loads((root/'checks.json').read_text());records=[]
def build(folder,ir,harness):
 folder.mkdir(exist_ok=True);(folder/'program.koopa').write_text(ir.replace('fun @main(): i32','fun @compiled_main(): i32',1));(folder/'harness.c').write_text(harness)
 with (folder/'program.ll').open('w') as out:subprocess.run(['koopac',str(folder/'program.koopa')],stdout=out,check=True)
 subprocess.run(['llc','--filetype=obj',str(folder/'program.ll'),'-o',str(folder/'program.o')],check=True)
 subprocess.run(['clang',str(folder/'harness.c'),str(folder/'program.o'),'-o',str(folder/'program')],check=True)
 return str((folder/'program').resolve())
for index,case in enumerate(cases):
 folder=root/('native-'+str(index));harness='#include <stdio.h>\nstatic int count;void rhs_tick(void){++count;}extern int compiled_main(void);int main(void){int v=compiled_main();printf("%d %d\\n",v,count);return 0;}\n'
 exe=build(folder,case['koopa'],harness);value,count=map(int,subprocess.check_output([exe],text=True,timeout=10).split());assert (value,count)==(case['value'],0)
 marker='%'+case['rhs_label']+':';assert case['koopa'].count(marker)==1
 changed='decl @rhs_tick()\n'+case['koopa'].replace(marker,marker+'\n  call @rhs_tick()',1)
 exe=build(folder,changed,harness);value,count=map(int,subprocess.check_output([exe],text=True,timeout=10).split());assert (value,count)==(case['value'],case['expected_rhs_count'])
 records.append({'value':value,'observed_rhs_count':count,'expected_rhs_count':case['expected_rhs_count']})
folder=root/'liveness';harness='extern int compiled_main(void);int main(void){return compiled_main()!=3;}\n'
exe=build(folder/'native-original',(folder/'original.koopa').read_text(),harness);subprocess.run([exe],check=True,timeout=10)
exe=build(folder/'native-wrong',(folder/'wrong.koopa').read_text(),harness)
try:subprocess.run([exe],check=True,timeout=1)
except subprocess.TimeoutExpired:detected=True
else:raise AssertionError('wrong Koopa loop edge was not detected as a timeout')
(root/'native-checks.json').write_text(json.dumps(records,indent=2)+'\n');(folder/'native-check.json').write_text(json.dumps({'baseline_i32':3,'timeout_seconds':1,'timeout_detected':detected})+'\n')
print('PASS 6 official Koopa/LLVM per-iteration RHS counters and terminating/timeout liveness controls')
PY
