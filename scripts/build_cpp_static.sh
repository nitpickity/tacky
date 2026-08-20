#!/usr/bin/env bash
# Builds the C++ protobuf runtime from source as static libs, so the `cpp` bench arms
# aren't handicapped by a shared library. Installs to $TACKY_CPP_ROOT/prefix, by default
# third_party/protobuf-cpp/prefix in the repo, which testing/build.rs finds on its own.
# Gitignored, and ~215 MB with the source tree.
#
# Prefer scripts/bench_cpp.sh, which preflights tools and calls this only when cold.
set -euo pipefail

VERSION=${1:-v28.1}          # match whatever protoc generated the .pb.cc files
# Script-relative rather than via git, so this still works from an unpacked tarball.
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ROOT=${TACKY_CPP_ROOT:-$REPO/third_party/protobuf-cpp}
PREFIX=$ROOT/prefix
SRC=$ROOT/protobuf

mkdir -p "$ROOT"
if [ ! -d "$SRC" ]; then
    git clone --depth 1 --branch "$VERSION" --recurse-submodules --shallow-submodules \
        https://github.com/protocolbuffers/protobuf.git "$SRC"
fi

# On Apple, abseil emits both x86_64 and arm64 randen flags and relies on the
# compiler to discard the irrelevant ones. Current clang rejects `-msse4.1`
# outright even behind `-Xarch_x86_64`, so restrict the list to the host arch.
ARCH=$(uname -m)
perl -0pi -e "s/foreach\(_arch IN ITEMS \"x86_64\" \"arm64\"\)/foreach(_arch IN ITEMS \"$ARCH\")/" \
    "$SRC/third_party/abseil-cpp/absl/copts/AbseilConfigureCopts.cmake"

# CMake's Ninja generator looks for `ninja` by name; some distros only ship `ninja-build`.
NINJA_ARG=()
if ! command -v ninja >/dev/null && command -v ninja-build >/dev/null; then
    NINJA_ARG=(-DCMAKE_MAKE_PROGRAM="$(command -v ninja-build)")
fi

# On Linux this is just an "unused variable" warning at the end of configure.
OSX_ARG=()
[ "$(uname -s)" = Darwin ] && OSX_ARG=(-DCMAKE_OSX_ARCHITECTURES="$ARCH")

# The `[@]+` guard is for bash 3.2, still the /bin/bash on macOS, where expanding an empty
# array under `set -u` is an error rather than nothing.
cmake -G Ninja -B "$SRC/build-static" -S "$SRC" \
    ${NINJA_ARG[@]+"${NINJA_ARG[@]}"} ${OSX_ARG[@]+"${OSX_ARG[@]}"} \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DBUILD_SHARED_LIBS=OFF \
    -Dprotobuf_BUILD_TESTS=OFF \
    -Dprotobuf_BUILD_LIBUPB=OFF \
    -Dprotobuf_ABSL_PROVIDER=module \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
    -DCMAKE_CXX_STANDARD=17

cmake --build "$SRC/build-static" --target install -j

echo
echo "Installed to $PREFIX"
echo "  static libs: $(ls "$PREFIX"/lib/*.a | wc -l | tr -d ' ')"
echo "  protoc:      $("$PREFIX/bin/protoc" --version)"
echo
echo "Re-run the benches with:"
echo "  TACKY_PROTOBUF_PREFIX=$PREFIX cargo bench -p testing --features cpp --bench comparison -- encode"
