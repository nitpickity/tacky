#!/usr/bin/env bash
# Builds the official C++ protobuf runtime from source as static libraries, so
# the `cpp` arms of `testing/benches/comparison.rs` aren't handicapped by a
# Homebrew dylib (DYLD stubs on every cross-library call, no inlining across the
# library boundary).
#
# Then run the benches against it:
#   TACKY_PROTOBUF_PREFIX=~/.cache/tacky-cpp/prefix \
#     cargo bench -p testing --features cpp --bench comparison -- encode
#
# Needs: cmake, ninja, git (`brew install cmake ninja`).
set -euo pipefail

VERSION=${1:-v28.1}          # match whatever protoc generated the .pb.cc files
ROOT=${TACKY_CPP_ROOT:-$HOME/.cache/tacky-cpp}
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

cmake -G Ninja -B "$SRC/build-static" -S "$SRC" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DCMAKE_OSX_ARCHITECTURES="$ARCH" \
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
