from pathlib import Path
import subprocess
import tempfile
exe = Path('target/release/sysy_compiler').resolve()
with tempfile.TemporaryDirectory() as directory:
    source = Path(directory)/'input.c'
    output = Path(directory)/'output.koopa'
    source.write_text('int main(){return 0x2a;}')
    subprocess.run([exe,'-koopa',source,'-o',output],check=True)
    assert 'ret 42' in output.read_text()
    source.write_text('int main(){return 08;}')
    output.write_text('preserved')
    assert subprocess.run([exe,'-koopa',source,'-o',output]).returncode != 0
    assert output.read_text() == 'preserved'
    assert subprocess.run([exe]).returncode != 0
print('CLI input/output, diagnostics, and invalid-output preservation passed')
