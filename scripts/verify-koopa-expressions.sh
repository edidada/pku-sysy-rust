#!/usr/bin/env bash
set -euo pipefail
python3 - <<'PY'
from pathlib import Path
import json, subprocess
root=Path('evidence/koopa-expressions');root.mkdir(parents=True,exist_ok=True)
cases=json.loads(Path('evidence/expressions/checks.json').read_text())
assert len(cases)==185, 'missing GCC/RV32 reference cases'
results=[]
for index,case in enumerate(cases):
    ir=case['koopa']
    assert ir.count('fun @main(): i32')==1
    # Rename only the exported symbol so an independent C harness can compare full i32.
    (root/'case.koopa').write_text(ir.replace('fun @main(): i32','fun @compiled_main(): i32',1))
    with (root/'case.ll').open('w') as out:
        subprocess.run(['koopac',str(root/'case.koopa')],stdout=out,check=True)
    subprocess.run(['llc','--filetype=obj',str(root/'case.ll'),'-o',str(root/'case.o')],check=True)
    expected=case['gcc_i32']
    def run(expected):
        harness='#include <stdint.h>\nextern int32_t compiled_main(void);\nint main(void){return compiled_main() != (int32_t)('+str(expected)+');}\n'
        (root/'harness.c').write_text(harness)
        subprocess.run(['clang',str(root/'harness.c'),str(root/'case.o'),'-o',str(root/'program')],check=True)
        return subprocess.run([str((root/'program').resolve())],timeout=10).returncode
    assert run(expected)==0,case
    if index==0:
        assert run(expected+256)==1,'full-width comparison failed to reject same-low-byte value'
    results.append({'expression':case['expression'],'gcc_i32':expected,'official_koopa_llvm_i32_match':True})
(root/'checks.json').write_text(json.dumps(results,indent=2)+'\n')
print('PASS',len(results),'official Koopa/LLVM native i32 expression cases; same-low-byte negative control rejected')
PY
