#!/bin/sh
# Downloads pinned revisions of NASM-heavy projects into tests/corpus/
# (git-ignored; TESTING.md §4). Only the x86 assembly directories are
# checked out (sparse, shallow), so this needs ~30 MB.
#
# Usage: tools/fetch-corpus.sh [dest]   (default: tests/corpus)
set -eu
dest=${1:-tests/corpus}
mkdir -p "$dest"

# name  url  revision  sparse paths...
fetch() {
    name=$1 url=$2 rev=$3
    shift 3
    dir="$dest/$name"
    if [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD)" = "$rev" ]; then
        echo "$name: up to date"
        return
    fi
    rm -rf "$dir"
    git init -q "$dir"
    git -C "$dir" remote add origin "$url"
    git -C "$dir" sparse-checkout set --no-cone "$@"
    git -C "$dir" fetch -q --depth 1 --filter=blob:none origin "$rev"
    git -C "$dir" checkout -q FETCH_HEAD
    echo "$name: $rev"
}

fetch x264 https://code.videolan.org/videolan/x264.git \
    0480cb05fa188d37ae87e8f4fd8f1aea3711f7ee '/common/x86/' '/tools/checkasm-a.asm'
fetch dav1d https://code.videolan.org/videolan/dav1d.git \
    0218c8dd014e7aafe0e96c62a8fcb32eb67b5ac7 '/src/x86/' '/src/ext/x86/' '/tests/checkasm/x86/'
fetch ffmpeg https://github.com/FFmpeg/FFmpeg.git \
    3a0867c2bfda4a4d4309ca1a8cbdc6175e67f587 '/libavcodec/x86/*.asm' '/libavutil/x86/*.asm' \
    '/libavfilter/x86/*.asm' '/libswscale/x86/*.asm' '/libswresample/x86/*.asm' \
    '/libavcodec/x86/*/*.asm'
