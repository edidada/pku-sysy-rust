#!/usr/bin/env bash
set -euo pipefail
mkdir -p evidence/scopes
python3 - <<'PY'
from pathlib import Path
import random,json
bodies=[
 'int a=1,b=2;{int a=2;b=b+a;}return b;',
 'int x=1;{int x=5;x=x+1;}return x;',
 'int x=1;{x=5;}return x;',
 'const int x=3;int r=0;{int x=7;x=x+1;r=x;}return x+r;',
 'int x=3,r=0;{const int x=7;r=x;}x=x+1;return x+r;',
 'int x=1,r=0;{int x=2;{int x=3;r=r+x;}r=r+x;}return r+x;',
 'int r=0;{int x=1;r=r+x;}{int x=2;r=r+x;}return r;',
 'int x=1,r=0;{int prior=x;int x=prior+2;r=x;}return r*10+x;',
 'const int x=3;int r=0;{const int prior=x;const int x=prior+2;r=x;}return r+x;',
 'int x=1;;{}{{}}x;1+2;!x;(x+1);return x;',
 'int a=1,b=2;a==b;a!=b;a<b;a+b;return a+b;',
 'int x=3;{return x;x=9;}x=7;return x;',
 'int x=1;{{{return 42;}}}return x;',
 'int main=1;{int main=2;main=main+1;}return main;',
 'int x=1,r=0;{int x=2,y=x+1;r=y;}return x+r;',
 'const int x=3;int r=0;{const int y=x*7;r=y;}return r;',
 'int x=4,r=0;{int y=x;{const int x=7;r=x+y;}}return r+x;',
 'int x=1;{x=x+1;{int x=8;x=x+1;}x=x+1;}return x;',
 'int r=0;{int x=2;{const int x=5;r=x;}x=x+1;r=r+x;}return r;',
 'int x=5;{;{;}x*3;}return x;'
]
rng=random.Random(20261121)
def block(depth):
 c=rng.randrange(1,20);constant=rng.choice([True,False]);shadow_y=rng.choice([True,False])
 text='{const int c='+str(c)+';'+('const int ' if constant else 'int ')+f'x=c+{rng.randrange(1,20)};'
 if shadow_y:text+=f'int y={rng.randrange(1,20)};'
 text+='y=(y*3+x)%997;result=(result*3+x-y)%10007;x+y;;{}'
 if not constant:text+='x=x+2;'
 if depth:text+=block(depth-1)
 text+='result=(result+x*7+y)%10007;}'
 return text
for n in range(160):
 body=f'int x={rng.randrange(1,20)},y=2,result=0;'
 body+=block(rng.randrange(1,5))+block(rng.randrange(0,3))
 body+='result=(result+x+y)%10007;return result+y-x;';bodies.append(body)
bodies.append('int result=0;'+''.join('{int v='+str(i)+';result=result+v;}' for i in range(800))+'return result;')
text='int x=7,result=0;'
for i in range(128):text+='{int x='+str(i+1)+';result=(result+x)%10007;'
for i in range(128):text+='result=(result+x)%10007;}'
text+='return result+x;';bodies.append(text)
assert len(bodies)==182
invalid=[
 '{int x=1;int x=2;}return 0;',
 '{int x=1;}return x;',
 '{const int x=1;}return x;',
 'int x=1;{const int x=2;x=3;}return x;',
 'const int x=1;{int x=2;x=3;}x=4;return x;',
 'const int x=1;{int x=2;{const int y=x;}}return 0;',
 'const int x=1;{const int x=x+1;}return x;',
 '{int x=1;}{x=2;}return 0;',
 '{const int x=1;const int x=2;}return 0;',
 'return 1;{int x=2;}x=3;',
 'int x=1;{return x;}return y;',
 'return;',
 'int x=1;(x)=2;return x;',
 'int x=1;{int y=(x=2);}return x;',
 '{const int y=z;}int z=1;return z;',
 'int x=1;{int x=2,x=3;}return x;',
 '{int x=1;return x;',
 'int x=1;{x+;}return x;',
 'int x=1;{const int x=2;{x=3;}}return x;',
 '{int x=1;}const int y=x;return y;'
]
assert len(invalid)==20
root=Path('evidence/scopes');(root/'bodies.json').write_text(json.dumps(bodies,indent=2)+'\n');(root/'invalid-bodies.json').write_text(json.dumps(invalid,indent=2)+'\n')
PY
SYSY_CASE_FILE=evidence/scopes/bodies.json SYSY_CASE_EXPECTED=182 SYSY_CASE_ROOT=evidence/scopes SYSY_INVALID_FILE=evidence/scopes/invalid-bodies.json bash scripts/verify-declarations.sh
