#!/bin/sh
# Runs check_corpus.py over tests/corpus/ (fetched with tools/fetch-corpus.sh)
# with each project's include paths and defines, under the default config
# and the extra configs given as arguments (each a quoted list of --set).
#
# Usage: tools/corpus/run.sh [reasm] ["--set a=b --set c=d" ...]
# The two configs that turn on almost every option are in
# tools/corpus/configs/; pass them with "$(cat tools/corpus/configs/upper)".
set -u
cd "$(dirname "$0")/../.."
reasm=${1:-target/release/reasm}
[ $# -gt 0 ] && shift
here=$PWD/tools/corpus
status=0

# project NAME DIR [CONFIG...]; uses $common for nasm and reasm options.
project() {
    name=$1 dir=$2
    shift 2
    for sets in "" "$@"; do
        echo "== $name ${sets:-(defaults)}"
        # shellcheck disable=SC2086
        python3 -I tools/corpus/check_corpus.py "$reasm" "tests/corpus/$dir" \
            $common $sets || status=1
    done
}

common="--formats elf64 --nasm=-Icommon/x86/ --nasm=-DARCH_X86_64=1 --nasm=-DBIT_DEPTH=8 \
 --nasm=-DHIGH_BIT_DEPTH=0 --nasm=-DSTACK_ALIGNMENT=64 --nasm=-DPREFIX \
 --set nasm.include_paths=[\"common/x86\"]"
project x264 x264 "$@"
common="--formats elf64 --nasm=-I$here/dav1d/ --nasm=-Isrc/ --set nasm.include_paths=[\"src\"]"
project dav1d dav1d "$@"
common="--formats elf64 --nasm=-I$here/ffmpeg/ --nasm=-I./ --nasm=-Pconfig.asm"
project ffmpeg ffmpeg "$@"
# NASM's tests: many are flat binaries, so -f bin is tried after elf64.
common="--formats elf64,bin"
project nasm-test nasm/test "$@"
exit $status
