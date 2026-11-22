#!/usr/bin/env bash
set -euo pipefail
mkdir -p evidence/loops
python3 - <<'PY'
from pathlib import Path
import json,random
bodies=[
 'int i=0,pow=1;while(i<7){pow=pow*2;i=i+1;}return pow;',
 'int i=0,r=9;while(i<0){r=1;}return r;',
 'int i=0;while(i<1)i=i+1;return i;',
 'int i=0,r=0;while(i<10){r=r+i;i=i+1;}return r;',
 'int i=0,r=0;while(1){i=i+1;if(i==4)break;r=r+i;}return r;',
 'int i=0,r=0;while(i<8){i=i+1;if(i%2)continue;r=r+i;}return r;',
 'int i=0,r=0;while(i<3){int j=0;i=i+1;while(j<4){j=j+1;if(j==2)break;r=r+i+j;}}return r;',
 'int i=0,r=0;while(i<3){int j=0;i=i+1;while(j<4){j=j+1;if(j%2)continue;r=r+i+j;}}return r;',
 'int i=0,r=0;while(i<4){i=i+1;while(1){r=r+1;break;}if(i%2)continue;r=r+10;}return r;',
 'int i=0,r=0;while(i<4){int x=0;i=i+1;x=x+1;r=r+x;}return r;',
 'int i=0,r=0;while(i<4){const int x=3;i=i+1;r=r+x;}return r;',
 'int i=0,r=0;while(i<3){i=i+1;{int i=7;r=r+i;}r=r+i;}return r;',
 'int i=0,r=0;while(i<8){r=r*2+((i%2)||0);i=i+1;}return r;',
 'int i=0,r=0;while(i<8){r=r*2+((i%2)&&7);i=i+1;}return r;',
 'int i=0,r=0;while(i<5&&i<7){r=r+i;i=i+1;}return r;',
 'int i=0,r=0;while(i<3||i<5){r=r+i;i=i+1;}return r;',
 'int i=0;while(i<3){i=i+1;if(i==2)return 42;}return i;',
 'while(1){{return 7;}}return 9;',
 'int i=3;while(i){i=i-1;}return i;',
 'int i=-3,r=0;while(i){r=r+1;i=i+1;}return r;',
 'int i=0;while(0){continue;break;}return i;',
 'int i=0;while(i<2){i=i+1;continue;i=99;}return i;',
 'int i=0;while(1){i=7;break;i=99;}return i;',
 'int i=0,r=0;while(i<3){i=i+1;{int x=2;while(x){x=x-1;if(x)continue;r=r+i;}}}return r;'
]
rng=random.Random(20261122)
for n in range(160):
 outer=rng.randrange(1,8);inner=rng.randrange(1,8);skip=rng.randrange(2,5);stop=rng.randrange(2,8)
 body=f'int i=0,r={rng.randrange(-20,21)};while(i<{outer}){{int j=0;i=i+1;'
 body+=f'if(i%{skip}==0)continue;while(j<{inner}){{j=j+1;if(j=={stop})break;'
 body+='if(j%2==0)continue;{int x=i+j;r=(r*3+x)%10007;}'
 body+='}r=(r+i)%10007;}return r+i;';bodies.append(body)
large=''.join('r=(r+1)%997;' for _ in range(2000))
bodies.append('int i=0,r=0;while(i<3){i=i+1;'+large+'}return r;')
bodies.append('int i=0,r=0;while(i<3){i=i+1;if(i<3)continue;'+large+'break;}return r;')
assert len(bodies)==186
invalid=[
 'break;return 0;','continue;return 0;','return 1;break;','return 1;continue;',
 'if(0)break;return 0;','if(0)continue;return 0;',
 'while(0){}break;return 0;','while(0){}continue;return 0;',
 'while(missing)break;return 0;','while(1)int x=2;return 0;',
 'while(1)const int x=2;return 0;','while()return 1;return 0;',
 'while(1){break 2;}return 0;','while(1){continue 2;}return 0;',
 'while(0){int x=1;}return x;','while(0){const int x=1;x=2;}return 0;',
 'while(1){break;missing=2;}return 0;','while(1){continue;missing=2;}return 0;',
 'while(0){int x=1;int x=2;}return 0;',
 'int x=0;while(x||missing){break;}return x;'
]
assert len(invalid)==20
root=Path('evidence/loops');(root/'bodies.json').write_text(json.dumps(bodies,indent=2)+'\n');(root/'invalid-bodies.json').write_text(json.dumps(invalid,indent=2)+'\n')
PY
SYSY_CASE_FILE=evidence/loops/bodies.json SYSY_CASE_EXPECTED=186 SYSY_CASE_ROOT=evidence/loops SYSY_INVALID_FILE=evidence/loops/invalid-bodies.json bash scripts/verify-declarations.sh
