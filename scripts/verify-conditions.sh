#!/usr/bin/env bash
set -euo pipefail
mkdir -p evidence/conditions
python3 - <<'PY'
from pathlib import Path
import random,json
bodies=[
 'int a=1;if(a==2||a==3){return 0;}else{return a+1;}',
 'int a=1,b=0,r=0;if(a)if(b)r=2;else r=3;return r;',
 'int a=0,b=0,r=0;if(a)if(b)r=2;else r=3;return r;',
 'int a=0,r=0;if(a){if(1)r=2;}else r=3;return r;',
 'int a=-3;if(a)return 7;return 9;',
 'int a=-2147483647-1;if(a)return 7;else return 8;',
 'int a=2147483647;if(a)return 7;else return 8;',
 'int a=0;if(a)return 1;else a=2;return a+3;',
 'int a=1;if(a)a=2;else return 7;return a+3;',
 'int a=0;if(a)return 1;else return 7;a=99;return a;',
 'int a=1,r=0;if(a){int a=3;r=a;}else{int a=5;r=a;}return r+a;',
 'int a=0,r=0;if(a){int a=3;r=a;}else{const int a=5;r=a;}return r+a;',
 'int a=1,r=0;if(a);else r=2;return r;',
 'int a=0,r=0;if(a){}else{}return r;',
 'int a=0;if(a)a=3;return a;',
 'int a=2,b=4;return a&&b;',
 'int a=2,b=4;return a||b;',
 'int a=0,b=2,c=4;return (a||b)&&c;',
 'int a=1,b=0,c=3;return a&&(b||c);',
 'const int a=0&&(1/0),b=1||(1/0);return a+b;',
 'int a=0;return a&&(1/0);',
 'int a=1;return a||(1/0);',
 'int a=2;if(a<0)return 1;else if(a==0)return 2;else if(a==1)return 3;else return 4;',
 'int a=1;{if(a){return 3;}else{return 4;}}return 5;'
]
rng=random.Random(20261122)
def stmt(depth):
 condition=rng.choice(['x<y','x==y','r%3','x-y','(x<y)||(r%3)','(x!=y)&&(r>=0)','!(r%2)'])
 def arm():
  text='{r=(r*3+x-y)%10007;'
  if rng.randrange(3)==0:text+='int x='+str(rng.randrange(-20,21))+';r=(r+x)%10007;'
  if depth:text+=stmt(depth-1)
  elif rng.randrange(4)==0:text+='return r+x-y;'
  text+='y=(y+x)%997;}'
  return text
 return 'if('+condition+')'+arm()+'else'+arm()
for n in range(160):
 body=f'int x={rng.randrange(-30,31)},y={rng.randrange(-30,31)},r=1;'
 body+=stmt(rng.randrange(0,4));body+='return r+x+y;';bodies.append(body)
large=''.join('r=(r+1)%997;' for _ in range(2000))
for selector in [0,1]:bodies.append(f'int flag={selector},r=0;if(flag){{{large}}}else{{r=7;}}return r;')
assert len(bodies)==186
invalid=[
 'if(missing)return 1;return 0;',
 'if(0)return missing;else return 2;',
 'if(1)return 1;else return missing;',
 'if(0){const int x=1;x=2;}return 0;',
 'int a=0;return a&&missing;',
 'int a=1;return a||missing;',
 'int a=1;const int b=1||a;return b;',
 'const int a=1||missing;return a;',
 'if(1)int x=2;return 0;',
 'if(0)const int x=2;return 0;',
 'else return 1;',
 'if(1)return 1;else return 2;else return 3;',
 'if(1{return 1;}return 0;',
 'if()return 1;return 0;',
 'if(1){int x=2;}return x;',
 'if(1)return 1;else return 2;missing=3;',
 'if(1)return;else return 0;',
 'const int a=1&&(1/0);return a;',
 'if(1){int x=1;int x=2;}return 0;',
 'if(0){int x=1;}{x=2;}return 0;'
]
assert len(invalid)==20
root=Path('evidence/conditions');(root/'bodies.json').write_text(json.dumps(bodies,indent=2)+'\n');(root/'invalid-bodies.json').write_text(json.dumps(invalid,indent=2)+'\n')
PY
SYSY_CASE_FILE=evidence/conditions/bodies.json SYSY_CASE_EXPECTED=186 SYSY_CASE_ROOT=evidence/conditions SYSY_INVALID_FILE=evidence/conditions/invalid-bodies.json bash scripts/verify-declarations.sh
