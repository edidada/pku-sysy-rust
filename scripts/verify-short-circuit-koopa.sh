#!/usr/bin/env bash
set -euo pipefail
# Expected trap/abort probes must not create core dumps.
ulimit -c 0
python3 - <<'PY'
from pathlib import Path
import json,subprocess
root=Path('evidence/short-circuit');cases=json.loads((root/'probes.json').read_text());results=[]
for index,case in enumerate(cases):
 folder=root/('native-'+str(index));folder.mkdir(exist_ok=True)
 harness='#include <stdint.h>\nextern int32_t compiled_main(void);\nint main(void){return compiled_main() != '+str(case['expected_i32'])+';}\n'
 (folder/'harness.c').write_text(harness)
 def execute(ir):
  ir=ir.replace('fun @main(): i32','fun @compiled_main(): i32',1)
  (folder/'program.koopa').write_text(ir)
  with (folder/'program.ll').open('w') as output:subprocess.run(['koopac',str(folder/'program.koopa')],stdout=output,check=True)
  subprocess.run(['llc','--filetype=obj',str(folder/'program.ll'),'-o',str(folder/'program.o')],check=True)
  subprocess.run(['clang',str(folder/'harness.c'),str(folder/'program.o'),'-o',str(folder/'program')],check=True)
  return subprocess.run([str((folder/'program').resolve())],capture_output=True,text=True,timeout=10)
 assert execute(case['koopa']).returncode==0
 marker='%'+case['rhs_label']+':'
 assert case['koopa'].count(marker)==1
 changed='decl @abort()\n'+case['koopa'].replace(marker,marker+'\n  call @abort()',1)
 result=execute(changed)
 if case['rhs_executed']:assert result.returncode in [-6,134],(case,result)
 else:assert result.returncode==0,(case,result)
 results.append({'case':case['case'],'rhs_label':case['rhs_label'],'rhs_executed':case['rhs_executed'],'native_status':result.returncode})
(root/'native-probes.json').write_text(json.dumps(results,indent=2)+'\n')
assert len(results)==18 and sum(x['rhs_executed'] for x in results)==10
print('PASS 18 official Koopa/LLVM RHS abort probes: 10 executed, 8 skipped')
PY
