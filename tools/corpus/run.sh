#!/bin/sh
# Runs check_corpus.py over tests/corpus/ (fetched with tools/fetch-corpus.sh)
# with each project's include paths and defines, under the default config
# and the extra configs given as arguments (each a quoted list of --set).
#
# Usage: tools/corpus/run.sh [reasm] ["--set a=b --set c=d" ...]
set -u
cd "$(dirname "$0")/../.."
reasm=${1:-target/release/reasm}
[ $# -gt 0 ] && shift
here=$PWD/tools/corpus
status=0

project() {
    name=$1
    shift
    for sets in "" "$@"; do
        echo "== $name ${sets:-(defaults)}"
        # shellcheck disable=SC2086
        python3 -I tools/corpus/check_corpus.py "$reasm" "tests/corpus/$name" \
            --formats elf64 $common $sets || status=1
    done
}

common="--nasm=-Icommon/x86/ --nasm=-DARCH_X86_64=1 --nasm=-DBIT_DEPTH=8 \
 --nasm=-DHIGH_BIT_DEPTH=0 --nasm=-DSTACK_ALIGNMENT=64 --nasm=-DPREFIX \
 --set nasm.include_paths=[\"common/x86\"]"
project x264 "$@"
common="--nasm=-I$here/dav1d/ --nasm=-Isrc/ --set nasm.include_paths=[\"src\"]"
project dav1d "$@"
common="--nasm=-I$here/ffmpeg/ --nasm=-I./ --nasm=-Pconfig.asm"
project ffmpeg "$@"
exit $status
