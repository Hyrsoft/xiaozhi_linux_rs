#!/bin/bash
set -e

# 加载共用下载函数（支持重试 + wget/curl 自动切换 + 工具链下载）
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../download_helper.sh"

# =============================================================================
# armv7-unknown-linux-uclibceabihf 混合链接编译脚本
#
# 本脚本会自动完成以下步骤：
#   1. 下载 uClibc 交叉编译工具链（如已存在则跳过）
#   2. 下载并交叉编译 alsa-lib 为共享库（.so），仅用于链接时符号解析
#   3. Opus/SpeexDSP 由 build.rs 自动从源码编译为静态库（.a）
#   4. 使用 uClibc 工具链编译 Rust 项目
#
# 链接策略：
#   - 动态链接 libc (uClibc) + libasound.so
#   - 静态链接 opus + speexdsp（由 build.rs 自动处理）
#   - 需要 auxval_stub 提供 getauxval 空实现
#
# 目标设备：RV1106 (Luckfox Pico) 等使用 uClibc 的 ARM 设备
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
echo "  混合链接编译 - armv7-unknown-linux-uclibceabihf"
echo "============================================="
echo "Project root: $PROJECT_ROOT"

# =============================================================================
# 1. 基础配置
# =============================================================================

TARGET="armv7-unknown-linux-uclibceabihf"
CROSS_PREFIX="arm-rockchip830-linux-uclibcgnueabihf"

# 所有第三方内容统一放在 third_party/<target> 下，避免多目标冲突
THIRD_PARTY="$PROJECT_ROOT/third_party"
TARGET_DIR="$THIRD_PARTY/$TARGET"
mkdir -p "$TARGET_DIR"

# --- 1A. 下载 uClibc 交叉编译工具链 ---
TOOLCHAIN_NAME="${CROSS_PREFIX}"
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

# 源码下载与构建目录
BUILD_DIR="$TARGET_DIR/build"

# C 依赖库版本
ALSA_VERSION="1.2.12"

# 并行编译线程数
NPROC=$(nproc 2>/dev/null || echo 4)

# =============================================================================
# 2. 下载并编译 C 依赖库
#    - alsa-lib: 编译为共享库 (.so)，仅用于链接时符号解析
#      运行时使用目标设备上的系统 libasound.so.2
#    - opus, speexdsp: 由 build.rs 自动从源码编译为静态库（.a）
# =============================================================================

mkdir -p "$BUILD_DIR"

# 通用交叉编译环境变量
export CC="$CROSS_GCC"
export CXX="$CROSS_CXX"
export AR="$CROSS_AR"
export RANLIB="$CROSS_RANLIB"
export STRIP="$CROSS_STRIP"
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
    # 清理之前可能存在的编译产物
    make distclean 2>/dev/null || true
    echo "配置 alsa-lib (共享库模式)..."
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

# CC / CXX 环境变量（Cargo 使用下划线格式的目标三元组）
export CC_armv7_unknown_linux_uclibceabihf="$CROSS_GCC"
export CXX_armv7_unknown_linux_uclibceabihf="$CROSS_CXX"
export AR_armv7_unknown_linux_uclibceabihf="$CROSS_AR"

# Cargo linker
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_UCLIBCEABIHF_LINKER="$CROSS_GCC"

# 告诉 Rust cc crate 编译 C 源码时使用 -fPIC
export CFLAGS_armv7_unknown_linux_uclibceabihf="-fPIC"

# 混合链接：动态链接 uClibc + libasound，静态链接 opus + speexdsp
export RUSTFLAGS="-C link-arg=-L$ALSA_SHARED_LIBDIR -C link-arg=-Wl,--no-as-needed -C link-arg=-ldl -C link-arg=-lpthread -C link-arg=-lm"

# 告诉 audiopus_sys 使用静态链接 opus
export LIBOPUS_STATIC=1

# ALSA 动态链接：不设置 ALSA_STATIC

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

# 避免干扰 host build 脚本的编译，取消通用 CC 变量
unset CC CXX AR RANLIB STRIP CFLAGS CXXFLAGS

# 创建 dummy libopus.a 桩文件，防止 audiopus_sys 尝试自行编译 opus
DUMMY_OPUS_DIR="$PROJECT_ROOT/target/dummy_opus"
mkdir -p "$DUMMY_OPUS_DIR/lib"
echo "void opus_dummy() {}" > "$DUMMY_OPUS_DIR/dummy.c"
$CROSS_GCC -c "$DUMMY_OPUS_DIR/dummy.c" -o "$DUMMY_OPUS_DIR/dummy.o"
$CROSS_AR rcs "$DUMMY_OPUS_DIR/lib/libopus.a" "$DUMMY_OPUS_DIR/dummy.o"
export OPUS_LIB_DIR="$DUMMY_OPUS_DIR"

# uClibc 目标需要 nightly + build-std（从源码构建 std）
cargo +nightly build \
    -Z build-std=std,panic_abort \
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
    echo "  （应显示 'dynamically linked'，表示 uClibc 动态链接）"
    echo ""
    echo "  readelf -d $OUTPUT_BIN | grep NEEDED"
    echo "  （应包含 libasound.so.2、libc.so.0 等系统库）"
    echo "  （不应出现 libopus/libspeexdsp，它们已静态链接）"
else
    echo "警告: 未找到输出文件 $OUTPUT_BIN"
    echo "请检查编译日志。"
fi
