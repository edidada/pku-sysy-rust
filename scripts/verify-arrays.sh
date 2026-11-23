#!/usr/bin/env bash
set -euo pipefail
mkdir -p evidence/arrays
python3 - <<'PY'
from pathlib import Path
import random,json
cases=[
 'int sum(int a[],int n){int i=0,r=0;while(i<n){r=r+a[i];i=i+1;}return r;}int main(){int a[3]={2,3,5};return sum(a,3);}',
 'int f(int a,int b,int c,int d,int e,int f,int g,int h,int p[],int j){return p[1]+a+b+c+d+e+f+g+h+j;}int main(){int a[3]={11,22,33};return f(1,2,3,4,5,6,7,8,a,10);}',
 'int g[2][3]={{11,12,13},{21,22,23}};int main(){int i=1,j=1;return g[i][j];}',
 'int main(){int a[2][3]={{1},{2,3}};return a[0][1]+a[1][2];}',
 'const int a[3]={2,4,6};int main(){return a[1]+a[2];}',
 'int main(){const int n=3;const int a[n]={1,2};return a[0]*100+a[1]*10+a[2];}',
 'int main(){int a[3][2]={1,2,{3},5};return a[0][0]+a[0][1]*2+a[1][0]*3+a[1][1]*4+a[2][0]*5+a[2][1]*6;}',
 'int a[2][3][4]={1,2,3,4,{5},{6},{7,8}};int main(){int i=0,j=0,k=0,r=0;while(i<2){j=0;while(j<3){k=0;while(k<4){r=(r*3+a[i][j][k])%10007;k=k+1;}j=j+1;}i=i+1;}return r;}',
 'int main(){int x=7;int a[2][2]={{x},{x+1,x+2}};return a[0][0]+a[0][1]*2+a[1][0]*3+a[1][1]*4;}',
 'void set(int a[]){a[1]=a[0]+7;}int main(){int a[2][3]={{1,2},{3}};set(a[1]);return a[0][1]+a[1][1];}',
 'int sum(int a[],int n){if(n==0)return 0;return a[n-1]+sum(a,n-1);}int main(){int a[5]={2,3,5,7,11};return sum(a,5);}',
 'int row(int a[]){return a[0]+a[1]+a[2];}int sum(int a[][3]){return row(a[1])+a[0][0];}int main(){int a[2][3]={{1},{2,3,4}};return sum(a);}',
 'int f(int a[],int b[]){a[0]=9;return b[0];}int main(){int a[3]={1,2,3};return f(a,a);}',
 'const int n=3;int f(int a[][n]){return a[1][2];}int main(){int a[2][n]={{},{3,4,5}};return f(a);}',
 'int main(){int i=0,r=0;while(i<4){int a[8][9]={};a[0][0]=a[0][0]+1;r=r+a[0][0]+a[7][8];a[7][8]=99;i=i+1;}return r;}',
 'int v0(int a[]){int t0=a[0];return t0+a[1];}int main(){int entry[2]={7,9};return v0(entry);}',
]
rng=random.Random(20261123)
for n in range(80):
 rows=rng.randrange(1,6);columns=rng.randrange(1,6);length=rng.randrange(rows*columns+1)
 values=[rng.randrange(-50,51) for _ in range(length)];initializer='{'+','.join(map(str,values))+'}'
 fold=f'int fold(int a[][{columns}],int rows){{int i=0,j=0,r=0;while(i<rows){{j=0;while(j<{columns}){{r=(r*3+a[i][j])%10007;j=j+1;}}i=i+1;}}return r;}}'
 if n%2:source=f'int a[{rows}][{columns}]={initializer};'+fold+'int main(){return fold(a,'+str(rows)+');}'
 else:source=fold+f'int main(){{int a[{rows}][{columns}]={initializer};return fold(a,{rows});}}'
 cases.append(source)
cases.append('int main(){int a[2048]={};int i=0,r=0;while(i<2048){r=r+a[i];i=i+1;}i=0;while(i<2048){a[i]=i%31;r=r+a[i];i=i+1;}return r+a[2047];}')
cases.append('int a[4096];int main(){a[0]=7;a[2047]=11;a[4095]=13;return a[0]+a[99]+a[2047]+a[4095];}')
parameters=','.join('int p'+str(i)+('[]' if i in [8,39] else '') for i in range(40))
arguments=','.join('a' if i==8 else 'b' if i==39 else str(i+1) for i in range(40))
cases.append('int f('+parameters+'){return p8[2]+p39[1]+p0+p7+p9+p38;}int main(){int a[3]={1,2,3},b[2]={4,5};return f('+arguments+');}')
assert len(cases)==99
Path('evidence/arrays/sources.json').write_text(json.dumps(cases,indent=2)+'\n')
PY
SYSY_FULL_CASE_FILE=evidence/arrays/sources.json SYSY_FULL_CASE_COUNT=99 SYSY_FULL_CASE_ROOT=evidence/arrays SYSY_FULL_CASE_KIND=arrays bash scripts/verify-functions.sh
