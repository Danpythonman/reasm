#!/usr/bin/env python3
"""Formats every .asm file in a directory with reasm and checks (TESTING.md §4):
  - the safety check never fails (exit 3),
  - formatting is idempotent,
  - files that assemble (nasm -f elf64, else -f bin) give identical bytes
    before and after formatting.

Usage: python3 -I tools/corpus/check_corpus.py target/release/reasm <dir> [--set k=v ...]
Example corpus: the test/ directory of the NASM 2.16.01 source tarball.
"""
import pathlib, subprocess, sys, tempfile
reasm, testdir = sys.argv[1], pathlib.Path(sys.argv[2])
extra = sys.argv[3:]
stats = dict(files=0, changed=0, unsafe=0, nonidem=0, asm_same=0, asm_diff=0, asm_skip=0)
def run(args, inp=None, cwd=None):
    return subprocess.run(args, input=inp, capture_output=True, cwd=cwd, timeout=60)
for f in sorted(testdir.glob('*.asm')):
    src = f.read_bytes()
    try: src.decode()
    except UnicodeDecodeError: continue
    stats['files'] += 1
    r = run([reasm, '--no-config', '-q', '--stdin-filepath', f.name, *extra], src)
    if r.returncode == 3:
        stats['unsafe'] += 1; print('UNSAFE', f.name, r.stderr.decode()[:300]); continue
    out = r.stdout
    if out != src: stats['changed'] += 1
    r2 = run([reasm, '--no-config', '-q', '--stdin-filepath', f.name, *extra], out)
    if r2.stdout != out:
        stats['nonidem'] += 1; print('NONIDEM', f.name)
    with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
        pa, pb = pathlib.Path(a)/f.name, pathlib.Path(b)/f.name
        pa.write_bytes(src); pb.write_bytes(out)
        res = None
        for fmt in ('elf64', 'bin'):
            ra = run(['nasm', '-f', fmt, '-I', str(testdir)+'/', '-o', 'o', f.name], cwd=a)
            rb = run(['nasm', '-f', fmt, '-I', str(testdir)+'/', '-o', 'o', f.name], cwd=b)
            if ra.returncode == 0:
                same = rb.returncode == 0 and (pathlib.Path(a)/'o').read_bytes() == (pathlib.Path(b)/'o').read_bytes()
                res = same; break
        if res is None: stats['asm_skip'] += 1
        elif res: stats['asm_same'] += 1
        else: stats['asm_diff'] += 1; print('ASMDIFF', f.name)
print(stats)
sys.exit(1 if stats['unsafe'] or stats['nonidem'] or stats['asm_diff'] else 0)
