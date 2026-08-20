#!/usr/bin/env bash
# Benchmark against the C++ protobuf runtime, setting it up if needed.
#
#   scripts/bench_cpp.sh                                  # all encode groups
#   scripts/bench_cpp.sh --groups 'encode_fds_*'          # glob over group names
#   scripts/bench_cpp.sh --groups 'encode_fds_*,encode_otlp_*'     # several, comma separated
#   scripts/bench_cpp.sh --groups 'encode_fds_*' --groups 'enc*1'  # or a repeated flag
#   scripts/bench_cpp.sh --bench comparison -- '^encode_realistic'
#
# criterion's own filter is an unanchored regex, so a glob typed straight at it either
# misbehaves quietly (`encode_fds_*` means "fds then any number of underscores") or fails
# outright (`*otlp*` is not a valid regex). `--groups` takes a real glob and translates it,
# anchored at the start, with the arm suffix left free. Use it or a raw `--` filter, not
# both. Quote the glob, or the shell expands it against the cwd first.
#
# Always builds protobuf from source as static libs, because `cargo bench --features cpp`
# alone links whatever the system has, dynamically, and that understates the C++ arms.
# Fails early with an actionable message rather than dying inside build.rs, where a missing
# tool takes the tacky and prost arms down too.
#
# First run builds the runtime (minutes, once) into $TACKY_CPP_ROOT, default
# third_party/protobuf-cpp in the repo (gitignored, ~215 MB). TACKY_CPP_FORCE=1 rebuilds.
set -euo pipefail

VERSION=${PROTOBUF_VERSION:-v28.1}
# Script-relative rather than via git, so this still works from an unpacked tarball.
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ROOT=${TACKY_CPP_ROOT:-$REPO/third_party/protobuf-cpp}
PREFIX=$ROOT/prefix

cd "$REPO"

# No protoc here: the source build produces one, and build.rs prefers it over the system
# package, which on some distros is years old. pkg-config is needed even with a prefix.
missing=()
for tool in cmake git pkg-config; do
    command -v "$tool" >/dev/null || missing+=("$tool")
done
# Some distros ship the binary as `ninja-build` with no `ninja` alias.
command -v ninja >/dev/null || command -v ninja-build >/dev/null || missing+=(ninja)
command -v c++ >/dev/null || command -v g++ >/dev/null || command -v clang++ >/dev/null \
    || missing+=("a C++17 compiler")

if [ ${#missing[@]} -gt 0 ]; then
    echo "missing required tools: ${missing[*]}" >&2
    echo >&2
    if command -v brew >/dev/null; then
        echo "  brew install cmake ninja pkg-config" >&2
    elif command -v dnf >/dev/null; then
        echo "  sudo dnf install -y cmake ninja-build pkgconf-pkg-config git gcc-c++" >&2
    elif command -v apt-get >/dev/null; then
        echo "  sudo apt-get install -y cmake ninja-build pkg-config git g++" >&2
    elif command -v pacman >/dev/null; then
        echo "  sudo pacman -S --needed cmake ninja pkgconf git gcc" >&2
    elif command -v zypper >/dev/null; then
        echo "  sudo zypper install -y cmake ninja pkg-config git gcc-c++" >&2
    else
        echo "  need: cmake, ninja, pkg-config, git, and a C++17 compiler" >&2
    fi
    exit 1
fi

# protobuf's install bakes its absolute prefix into ~190 files, so the tree cannot be moved
# — renaming the checkout leaves a prefix that hands the compiler stale include paths and
# fails deep inside cc-rs. Detect that and rebuild instead.
# `lib` is what build_cpp_static.sh pins; `lib64` is GNUInstallDirs' default on Fedora and
# RHEL x86_64, so an older prefix may be laid out that way.
lib=$PREFIX/lib
[ -f "$PREFIX/lib64/libprotobuf.a" ] && lib=$PREFIX/lib64

stale=""
pc=$lib/pkgconfig/protobuf.pc
[ -f "$pc" ] && ! grep -qxF "prefix=$PREFIX" "$pc" && stale=1

if [ -n "${TACKY_CPP_FORCE:-}" ] || [ -n "$stale" ] \
    || [ ! -f "$lib/libprotobuf.a" ] || [ ! -x "$PREFIX/bin/protoc" ]; then
    [ -n "$stale" ] && echo "=== prefix was built for another path; rebuilding" && rm -rf "$ROOT/protobuf/build-static" "$PREFIX"
    echo "=== building protobuf $VERSION as static libs into $PREFIX"
    echo "    (one time, several minutes)"
    TACKY_CPP_ROOT="$ROOT" scripts/build_cpp_static.sh "$VERSION"
else
    echo "=== reusing static protobuf at $PREFIX ($("$PREFIX/bin/protoc" --version))"
fi

# Glob -> regex body, unanchored so several can be alternated. Character classes pass
# through; the rest of regex's metacharacters are escaped so a group name containing `.`
# cannot act as a wildcard. `,` is not a metacharacter, so it survives to become the `|`
# separator below.
glob_to_re() {
    local g=$1 out='' i c
    for ((i = 0; i < ${#g}; i++)); do
        c=${g:i:1}
        case $c in
            '*') out+='.*' ;;
            '?') out+='.' ;;
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '\\') out+="\\$c" ;;
            *) out+=$c ;;
        esac
    done
    printf '%s' "$out"
}

GROUP_GLOB="" # not GROUPS: bash owns that name (your group ids) and ignores assignment
args=()
while [ $# -gt 0 ]; do
    case $1 in
        --groups)
            if [ -z "$GROUP_GLOB" ]; then
                GROUP_GLOB=${2:?--groups needs a pattern}
            else
                GROUP_GLOB="$GROUP_GLOB,${2:?--groups needs a pattern}"
            fi
            shift 2
            ;;
        # Everything from `--` on belongs to criterion verbatim, including a literal
        # `--groups` if it ever grows one.
        --)
            args+=("$@")
            break
            ;;
        *)
            args+=("$1")
            shift
            ;;
    esac
done
set -- ${args[@]+"${args[@]}"}

if [ -n "$GROUP_GLOB" ]; then
    # The filter is criterion's first positional, so slot it in right after `--` when the
    # caller supplied one; that way --groups composes with --measurement-time and friends.
    body=$(glob_to_re "$GROUP_GLOB")
    re="^(${body//,/|})"
    out=()
    put=""
    for a in "$@"; do
        out+=("$a")
        if [ -z "$put" ] && [ "$a" = "--" ]; then
            out+=("$re")
            put=1
        fi
    done
    [ -z "$put" ] && out+=(-- "$re")
    set -- ${out[@]+"${out[@]}"}
elif [ $# -eq 0 ]; then
    # Encode only by default: the decode groups have no C++ arms.
    set -- -- '^encode_'
fi

# A filter matching nothing leaves criterion exiting 0 having measured nothing, which reads
# as a clean run. Build once via --list and count first; the real run then reuses the build.
n=$(TACKY_PROTOBUF_PREFIX="$PREFIX" cargo bench -p testing --features cpp "$@" --list 2>/dev/null |
    grep -c ': benchmark' || true)
if [ "$n" -eq 0 ]; then
    echo "filter matched no benchmarks: ${*}" >&2
    exit 1
fi

echo "=== cargo bench -p testing --features cpp $* ($n benchmarks)"
echo
TACKY_PROTOBUF_PREFIX="$PREFIX" cargo bench -p testing --features cpp "$@"

cat <<'EOF'

Compare against cpp-noutf8 for proto3, cpp for proto2. Not cpp-cached: it reuses an
already-populated message, which a real producer cannot.
EOF
