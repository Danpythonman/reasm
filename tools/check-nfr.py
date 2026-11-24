#!/usr/bin/env python3
"""Checks the release binary against NFR-1 (size) and NFR-2 (speed).

Usage: python3 -I tools/check-nfr.py [REASM] [--budget-ms N] [--runs N]

NFR-1: the stripped binary is under 2 MiB.
NFR-2: formatting 100,000 lines takes under --budget-ms (default 100)
milliseconds, measured as the median of --runs (default 15) whole-process
runs. The input is the golden-test inputs that assemble (no `no-assemble`
file), repeated to 100,000 lines: typical hand-written code.
Exits 1 if either check fails.
"""
import pathlib, statistics, subprocess, sys, tempfile, time

ROOT = pathlib.Path(__file__).resolve().parent.parent
MAX_SIZE = 2 * 1024 * 1024
LINES = 100_000


def main():
    args = sys.argv[1:]
    reasm = ROOT / 'target' / 'release' / 'reasm'
    budget, runs = 100.0, 15
    i = 0
    while i < len(args):
        if args[i] == '--budget-ms':
            budget = float(args[i + 1]); i += 2
        elif args[i] == '--runs':
            runs = int(args[i + 1]); i += 2
        else:
            reasm = pathlib.Path(args[i]); i += 1
    if not reasm.exists() and reasm.with_suffix('.exe').exists():
        reasm = reasm.with_suffix('.exe')

    ok = True
    size = reasm.stat().st_size
    print(f'NFR-1 binary size: {size / 1024 / 1024:.2f} MiB (limit 2 MiB)')
    if size >= MAX_SIZE:
        print('  FAIL: binary too large'); ok = False

    cases = sorted(p for p in (ROOT / 'tests' / 'cases').iterdir()
                   if p.is_dir() and not (p / 'no-assemble').exists())
    chunk = ''.join((p / 'input.asm').read_text() for p in cases)
    lines = chunk.splitlines(keepends=True)
    text = ''.join(lines[i % len(lines)] for i in range(LINES)).encode()

    with tempfile.TemporaryDirectory() as tmp:
        cmd = [str(reasm), '--no-config', '-q', '--stdin-filepath',
               str(pathlib.Path(tmp) / 'bench.asm')]
        first = subprocess.run(cmd, input=text, capture_output=True)
        if first.returncode != 0:
            sys.exit(f'reasm failed on the benchmark input:\n{first.stderr.decode()}')
        times = []
        for _ in range(runs):
            t = time.perf_counter()
            subprocess.run(cmd, input=text, capture_output=True)
            times.append((time.perf_counter() - t) * 1000)
    med = statistics.median(times)
    print(f'NFR-2 {LINES} lines ({len(text) / 1e6:.1f} MB): median {med:.1f} ms, '
          f'min {min(times):.1f} ms over {runs} runs (budget {budget:g} ms)')
    if med >= budget:
        print('  FAIL: too slow'); ok = False
    sys.exit(0 if ok else 1)


main()
