#!/bin/bash
set -e

# 加载共用下载函数（支持重试 + wget/curl 自动切换）
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../download_helper.sh"

# =============================================================================
# armv7-unknown-linux-gnueabihf 混合链接编译脚本
#
# 本脚本会自动完成以下步骤：
#   1. 下载 GNU 交叉编译工具链（如已存在则跳过）
#   2. 下载并交叉编译 opus、speexdsp 为静态库（.a）
#   3. ALSA 动态链接系统的 libasound.so，Opus/SpeexDSP 静态链接
#
# ALSA 动态链接策略优势：
#   - 动态链接 libc + libasound，避免 GLIBC ABI 不匹配导致的 segfault
#   - 支持 dlopen 加载板子上的 ALSA 插件（如 PulseAudio）
#   - "default" 音频设备名可正常工作
#   - Opus/SpeexDSP 静态打入，部署时无需额外拷贝 .so 文件
#
# 前置要求（CI 中自动安装）：
#   sudo dpkg --add-architecture armhf
#   sudo apt-get install libasound2-dev:armhf
#
# 无需手动安装任何工具链，适用于本地开发和 GitHub Actions CI。
# =============================================================================

# 获取脚本所在目录的绝对路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 跳转到项目根目录（../../）
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")/../"
cd "$PROJECT_ROOT"
PROJECT_ROOT="$(pwd)"

echo "============================================="
echo "  混合链接编译 - armv7-unknown-linux-gnueabihf"
echo "============================================="
echo "Project root: $PROJECT_ROOT"

# =============================================================================
# 1. 基础配置
# =============================================================================

TARGET="armv7-unknown-linux-gnueabihf"
CROSS_PREFIX="arm-linux-gnueabihf"

# 所有第三方内容统一放在 third_party/<target> 下，避免多目标冲突
THIRD_PARTY="$PROJECT_ROOT/third_party"
TARGET_DIR="$THIRD_PARTY/$TARGET"
mkdir -p "$TARGET_DIR"

# --- 1A. 下载 GNU 交叉编译工具链 ---
TOOLCHAIN_NAME="gcc-arm-8.3-2019.02-x86_64-arm-linux-gnueabihf"
TOOLCHAIN_URL="https://github.com/Hyrsoft/xiaozhi_linux_rs/releases/download/Source_Mirror/${TOOLCHAIN_NAME}.tar.xz"

TOOLCHAIN_DIR=$(download_and_setup_toolchain \
    "$TARGET_DIR" \
    "$TOOLCHAIN_NAME" \
    "$CROSS_PREFIX" \
    "$TOOLCHAIN_URL")

# 设置交叉编译工具路径
CROSS_GCC="$TOOLCHAIN_DIR/bin/${CROSS_PREFIX}-gcc"
CROSS_CXX="$TOOLCHAIN_DIR/bin/${CROSS_PREFIX}-g++"
CROSS_AR="$TOOLCHAIN_DIR/bin/${CROSS_PREFIX}-ar"
CROSS_RANLIB="$TOOLCHAIN_DIR/bin/${CROSS_PREFIX}-ranlib"
CROSS_STRIP="$TOOLCHAIN_DIR/bin/${CROSS_PREFIX}-strip"

echo "CC: $CROSS_GCC"
echo "GCC version: $($CROSS_GCC --version | head -1)"

# GCC 工具链自带 GLIBC sysroot（位于 <toolchain>/arm-linux-gnueabihf/libc/），
# 包含完整的 libpthread, libdl, libm, libc 等，无需额外下载。
# GCC 会自动使用此内置 sysroot，不需要显式指定 --sysroot。

# 静态库输出目录
STATIC_SYSROOT="$TARGET_DIR/sysroot"
STATIC_LIBDIR="$STATIC_SYSROOT/usr/lib"
STATIC_INCDIR="$STATIC_SYSROOT/usr/include"

# 源码下载与构建目录
BUILD_DIR="$TARGET_DIR/build"

# C 依赖库版本
ALSA_VERSION="1.2.12"
OPUS_VERSION="1.5.2"
SPEEXDSP_VERSION="1.2.1"

# 并行编译线程数
NPROC=$(nproc 2>/dev/null || echo 4)

# =============================================================================
# 2. 下载并编译 C 依赖库
#    - alsa-lib: 编译为共享库 (.so)，仅用于链接时符号解析
#      运行时使用目标设备上的系统 libasound.so.2
#    - opus, speexdsp: 编译为静态库 (.a)，直接打入二进制
# =============================================================================

mkdir -p "$STATIC_SYSROOT" "$STATIC_LIBDIR" "$STATIC_INCDIR" "$BUILD_DIR"

# 通用交叉编译环境变量
export CC="$CROSS_GCC"
export CXX="$CROSS_CXX"
export AR="$CROSS_AR"
export RANLIB="$CROSS_RANLIB"
export STRIP="$CROSS_STRIP"
# GNU 目标编译为 PIE，静态库必须使用 -fPIC 才能链接进 PIE 二进制
export CFLAGS="-fPIC"
export CXXFLAGS="-fPIC"

# --- 2A. 编译 alsa-lib（共享库，仅用于链接时符号解析）---
echo ""
echo "=== Step 2A: 编译 alsa-lib ${ALSA_VERSION} (共享库 .so，仅链接时使用) ==="

ALSA_SRC_DIR="$BUILD_DIR/alsa-lib-${ALSA_VERSION}"
ALSA_INSTALL_DIR="$TARGET_DIR/alsa-shared"
if [ -f "$ALSA_INSTALL_DIR/usr/lib/libasound.so" ]; then
    echo "alsa-lib 共享库已存在，跳过编译。"
else
    ALSA_TARBALL="alsa-lib-${ALSA_VERSION}.tar.bz2"
    ALSA_URL="https://github.com/Hyrsoft/xiaozhi_linux_rs/releases/download/Source_Mirror/${ALSA_TARBALL}"

    if [ ! -d "$ALSA_SRC_DIR" ]; then
        echo "下载 alsa-lib..."
        download_file "$ALSA_URL" "$BUILD_DIR/${ALSA_TARBALL}"
        echo "解压 alsa-lib..."
        tar -xjf "$BUILD_DIR/${ALSA_TARBALL}" -C "$BUILD_DIR"
        rm -f "$BUILD_DIR/${ALSA_TARBALL}"
    fi

    cd "$ALSA_SRC_DIR"
    # 清理之前可能存在的静态编译产物
    make distclean 2>/dev/null || true
    echo "配置 alsa-lib (共享库模式)..."
    # LDFLAGS="-Wl,--as-needed": 仅为 libasound.so 实际使用符号的库添加 DT_NEEDED
    # 避免 libpthread.so.0 等作为间接依赖被加载，导致旧版 ld 的 --as-needed 冲突
    ./configure \
        --host="${CROSS_PREFIX}" \
        --enable-shared \
        --disable-static \
        --disable-python \
        --disable-alisp \
        --disable-old-symbols \
        --disable-topology \
        --with-configdir="/usr/share/alsa" \
        --with-plugindir="/usr/lib/alsa-lib" \
        --prefix="/usr" \
        LDFLAGS="-Wl,--as-needed" \
        --quiet

    echo "编译 alsa-lib (使用 ${NPROC} 线程)..."
    make -j"$NPROC" --quiet
    mkdir -p "$ALSA_INSTALL_DIR"
    make DESTDIR="$ALSA_INSTALL_DIR" install --quiet

    # 修正 alsa.pc 中的 prefix 路径：/usr → 实际安装绝对路径
    # 否则 pkg-config 会返回 -L/usr/lib，指向宿主机的 x86_64 库
    sed -i "s|prefix=/usr|prefix=$ALSA_INSTALL_DIR/usr|" "$ALSA_INSTALL_DIR/usr/lib/pkgconfig/alsa.pc"

echo "alsa-lib 共享库编译完成!"
fi

ALSA_SHARED_LIBDIR="$ALSA_INSTALL_DIR/usr/lib"
ALSA_SHARED_PKGCONFIG="$ALSA_INSTALL_DIR/usr/lib/pkgconfig"
echo "ALSA 共享库: $ALSA_SHARED_LIBDIR"
ls -la "$ALSA_SHARED_LIBDIR"/libasound.so* 2>/dev/null || true

cd "$PROJECT_ROOT"

# =============================================================================
# 3. 设置 Rust 交叉编译环境
# =============================================================================

echo ""
echo "=== Step 3: 设置 Rust 编译环境 ==="

# 安装 gnu target（如果尚未安装）
rustup target add "$TARGET" 2>/dev/null || true

# CC / CXX 环境变量（Cargo 使用下划线格式的目标三元组）
export CC_armv7_unknown_linux_gnueabihf="$CROSS_GCC"
export CXX_armv7_unknown_linux_gnueabihf="$CROSS_CXX"
export AR_armv7_unknown_linux_gnueabihf="$CROSS_AR"

# Cargo linker
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER="$CROSS_GCC"

# 告诉 Rust cc crate 编译 C 源码时使用 -fPIC（PIE 二进制必需）
export CFLAGS_armv7_unknown_linux_gnueabihf="-fPIC"

# 混合链接：不使用 +crt-static，保持 libc/libdl 动态链接
# GCC 自带 sysroot 提供 libpthread/libdl/libm/libc 等系统库，无需 --sysroot
# -L 指向 ALSA 共享库目录
# --no-as-needed：确保 -lpthread -ldl -lm 不会被 Rust 注入的 --as-needed 丢弃
export RUSTFLAGS="-C link-arg=-L$ALSA_SHARED_LIBDIR -C link-arg=-Wl,--no-as-needed -C link-arg=-ldl -C link-arg=-lpthread -C link-arg=-lm"

# 告诉 audiopus_sys 使用静态链接 opus
export LIBOPUS_STATIC=1

# ALSA 动态链接：
#   - 不设置 ALSA_STATIC，让 alsa-sys 动态链接 libasound.so
#   - alsa.pc 已通过 sed 修正为实际安装路径，pkg-config 返回正确的 -L 路径
#   - 运行时由目标设备的系统 libasound.so.2 提供

# pkg-config 配置
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR="$ALSA_SHARED_PKGCONFIG"
export PKG_CONFIG_SYSROOT_DIR=""

echo "CC:           $CROSS_GCC"
echo "RUSTFLAGS:    $RUSTFLAGS"

# =============================================================================
# 4. 编译 Rust 项目
# =============================================================================

echo ""
echo "=== Step 4: 编译 Rust 项目 ==="
echo "Building in: $PROJECT_ROOT"
echo "Target: $TARGET"

# 避免干扰 host build 脚本的编译，取消通用 CC 变量，依赖特化的 CC_armv7_unknown...
unset CC CXX AR RANLIB STRIP CFLAGS CXXFLAGS

# 绕过 audiopus_sys 的 cmake 构建（因为 cmake < 3.5 报错，且我们在 build.rs 中统一编译）
# 创建一个真的 libopus.a 桩文件，欺骗 audiopus_sys 让它不要自己编译
DUMMY_OPUS_DIR="$PROJECT_ROOT/target/dummy_opus"
mkdir -p "$DUMMY_OPUS_DIR/lib"
echo "void opus_dummy() {}" > "$DUMMY_OPUS_DIR/dummy.c"
$CROSS_GCC -c "$DUMMY_OPUS_DIR/dummy.c" -o "$DUMMY_OPUS_DIR/dummy.o"
$CROSS_AR rcs "$DUMMY_OPUS_DIR/lib/libopus.a" "$DUMMY_OPUS_DIR/dummy.o"
export OPUS_LIB_DIR="$DUMMY_OPUS_DIR"

cargo build -vv \
    --target "$TARGET" \
    --release

echo ""
echo "============================================="
echo "  编译完成!"
echo "============================================="

OUTPUT_BIN="$PROJECT_ROOT/target/$TARGET/release/xiaozhi_linux_rs"
if [ -f "$OUTPUT_BIN" ]; then
    echo "输出文件: $OUTPUT_BIN"
    echo "文件大小: $(du -h "$OUTPUT_BIN" | cut -f1)"
    echo ""
    echo "文件信息:"
    file "$OUTPUT_BIN"
    echo ""
    echo "提示: 可使用以下命令验证链接方式:"
    echo "  file $OUTPUT_BIN"
    echo "  （应显示 'dynamically linked'，表示 libc 动态链接）"
    echo ""
    echo "  readelf -d $OUTPUT_BIN | grep NEEDED"
    echo "  （应包含 libasound.so.2、libc.so.6 等系统库）"
    echo "  （不应出现 libopus/libspeexdsp，它们已静态链接）"
else
    echo "警告: 未找到输出文件 $OUTPUT_BIN"
    echo "请检查编译日志。"
fi
