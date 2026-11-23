#!/usr/bin/env python3
"""Formats a tree of NASM sources with reasm and checks (TESTING.md §4):
  - the safety check never fails (exit 3),
  - formatting is idempotent,
  - every file that assembles gives identical bytes before and after
    formatting. The whole tree is formatted first, so included files are
    formatted too.

Usage:
  python3 -I tools/corpus/check_corpus.py REASM DIR [options]
Options:
  --set KEY=VALUE   passed to reasm (repeatable)
  --nasm ARG        passed to nasm, e.g. --nasm=-DARCH_X86_64=1 (repeatable);
                    relative -I paths are relative to DIR
  --formats LIST    nasm output formats to try, default elf64,bin
  -v                list every file that does not assemble

Each file is assembled from DIR's root with the same relative path in the
original and the formatted copy, so the object files are comparable.
Exits 1 on any unsafe, non-idempotent or differently assembling file.
"""
import pathlib, shutil, subprocess, sys, tempfile

def main():
    args = sys.argv[1:]
    reasm, root = pathlib.Path(args[0]).resolve(), pathlib.Path(args[1]).resolve()
    sets, nasm_args, formats, verbose = [], [], ['elf64', 'bin'], False
    i = 2
    while i < len(args):
        a = args[i]
        val = lambda: a.split('=', 1)[1] if '=' in a else args[i + 1]
        if a.startswith('--set'):
            sets += ['--set', val()]
        elif a.startswith('--nasm'):
            nasm_args.append(val())
        elif a.startswith('--formats'):
            formats = val().split(',')
        elif a == '-v':
            verbose = True
            i += 1
            continue
        else:
            sys.exit(f'unknown option {a}')
        i += 1 if '=' in a else 2

    orig = None  # set below; fmt runs from the original tree's root
    stats = dict(files=0, changed=0, unsafe=0, nonidem=0, asm_same=0, asm_diff=0, asm_skip=0)
    run = lambda cmd, inp=None, cwd=None: subprocess.run(
        cmd, input=inp, capture_output=True, cwd=cwd, timeout=120)
    # The real path, so reasm can read the file's includes (ADR-0022).
    # Run from the tree's root, like the project's build runs nasm.
    fmt = lambda path, data: run(
        [reasm, '--no-config', '-q', '--stdin-filepath', str(path), *sets], data,
        cwd=orig)

    with tempfile.TemporaryDirectory() as tmp:
        orig, new = pathlib.Path(tmp) / 'a', pathlib.Path(tmp) / 'b'
        ignore = shutil.ignore_patterns('.git')
        shutil.copytree(root, orig, ignore=ignore)
        shutil.copytree(root, new, ignore=ignore)
        files = sorted(p.relative_to(orig) for p in orig.rglob('*')
                       if p.suffix in ('.asm', '.inc', '.mac') and p.is_file())
        for rel in files:
            src = (orig / rel).read_bytes()
            try:
                src.decode()
            except UnicodeDecodeError:
                continue
            stats['files'] += 1
            r = fmt(orig / rel, src)
            if r.returncode == 3:
                stats['unsafe'] += 1
                print('UNSAFE', rel, r.stderr.decode()[:300])
                continue
            out = r.stdout
            if out != src:
                stats['changed'] += 1
            if fmt(orig / rel, out).stdout != out:
                stats['nonidem'] += 1
                print('NONIDEM', rel)
            (new / rel).write_bytes(out)

        for rel in files:
            if rel.suffix != '.asm':
                continue
            res = None
            for f in formats:
                cmd = ['nasm', '-f', f, *nasm_args, '-o', 'reasm-check.o', str(rel)]
                ra = run(cmd, cwd=orig)
                if ra.returncode != 0:
                    continue
                rb = run(cmd, cwd=new)
                res = rb.returncode == 0 and \
                    (orig / 'reasm-check.o').read_bytes() == (new / 'reasm-check.o').read_bytes()
                break
            if res is None:
                stats['asm_skip'] += 1
                if verbose:
                    print('SKIP', rel, ra.stderr.decode().splitlines()[:1])
            elif res:
                stats['asm_same'] += 1
            else:
                stats['asm_diff'] += 1
                print('ASMDIFF', rel)
    print(stats)
    sys.exit(1 if stats['unsafe'] or stats['nonidem'] or stats['asm_diff'] else 0)

main()
