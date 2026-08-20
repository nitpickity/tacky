#!/usr/bin/env bash
# Benchmark against the C++ protobuf runtime, setting it up if needed.
#
#   scripts/bench_cpp.sh                                  # all encode groups
#   scripts/bench_cpp.sh --bench comparison -- '^encode_realistic'
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

# protoc on PATH is for prost-build, which runs regardless of this feature; the C++ codegen
# uses $PREFIX/bin/protoc. pkg-config is needed even with a prefix.
missing=()
for tool in cmake git pkg-config protoc; do
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
        echo "  brew install cmake ninja pkg-config protobuf" >&2
    elif command -v dnf >/dev/null; then
        echo "  sudo dnf install -y cmake ninja-build pkgconf-pkg-config protobuf-compiler git gcc-c++" >&2
    elif command -v apt-get >/dev/null; then
        echo "  sudo apt-get install -y cmake ninja-build pkg-config protobuf-compiler git g++" >&2
    elif command -v pacman >/dev/null; then
        echo "  sudo pacman -S --needed cmake ninja pkgconf protobuf git gcc" >&2
    elif command -v zypper >/dev/null; then
        echo "  sudo zypper install -y cmake ninja pkg-config protobuf-devel git gcc-c++" >&2
    else
        echo "  need: cmake, ninja, pkg-config, protoc, git, and a C++17 compiler" >&2
    fi
    exit 1
fi

# protobuf's install bakes its absolute prefix into ~190 files, so the tree cannot be moved
# — renaming the checkout leaves a prefix that hands the compiler stale include paths and
# fails deep inside cc-rs. Detect that and rebuild instead.
stale=""
pc=$PREFIX/lib/pkgconfig/protobuf.pc
[ -f "$pc" ] && ! grep -qxF "prefix=$PREFIX" "$pc" && stale=1

if [ -n "${TACKY_CPP_FORCE:-}" ] || [ -n "$stale" ] \
    || [ ! -f "$PREFIX/lib/libprotobuf.a" ] || [ ! -x "$PREFIX/bin/protoc" ]; then
    [ -n "$stale" ] && echo "=== prefix was built for another path; rebuilding" && rm -rf "$ROOT/protobuf/build-static" "$PREFIX"
    echo "=== building protobuf $VERSION as static libs into $PREFIX"
    echo "    (one time, several minutes)"
    TACKY_CPP_ROOT="$ROOT" scripts/build_cpp_static.sh "$VERSION"
else
    echo "=== reusing static protobuf at $PREFIX ($("$PREFIX/bin/protoc" --version))"
fi

# Encode only by default: the decode groups have no C++ arms.
if [ $# -eq 0 ]; then
    set -- -- '^encode_'
fi

echo "=== cargo bench -p testing --features cpp $*"
echo
TACKY_PROTOBUF_PREFIX="$PREFIX" cargo bench -p testing --features cpp "$@"

cat <<'EOF'

Compare against cpp-noutf8 for proto3, cpp for proto2. Not cpp-cached: it reuses an
already-populated message, which a real producer cannot.
EOF
