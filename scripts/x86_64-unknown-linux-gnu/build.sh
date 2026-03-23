#!/bin/bash
set -e

# 加载共用下载函数（支持重试 + wget/curl 自动切换）
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../download_helper.sh"
# 加载 ALSA 共享库编译函数
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../build_alsa.sh"

# =============================================================================
# x86_64-unknown-linux-gnu 混合链接编译脚本
#
# 本脚本在 x86_64 宿主机上进行本地编译（无需交叉编译工具链），实现：
#   1. 从源码编译 alsa-lib 为共享库（.so），仅用于链接时符号解析
#      运行时使用宿主机系统的 libasound.so.2
#   2. opus、speexdsp 由 build.rs 自动从源码编译为静态库（.a）
#
# 链接策略：
#   - 动态链接 libc (GLIBC) + libasound.so
#   - 静态链接 opus + speexdsp（由 build.rs 自动处理）
#
# 目标平台：x86_64 Linux（Ubuntu/Debian 等 GLIBC 系统）
#
# 无需手动安装任何额外工具链，适用于本地开发和 GitHub Actions CI。
# =============================================================================

# 获取脚本所在目录的绝对路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 跳转到项目根目录（../../）
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")/../"
cd "$PROJECT_ROOT"
PROJECT_ROOT="$(pwd)"

echo "============================================="
echo "  混合链接编译 - x86_64-unknown-linux-gnu"
echo "============================================="
echo "Project root: $PROJECT_ROOT"

# =============================================================================
# 1. 基础配置
# =============================================================================

TARGET="x86_64-unknown-linux-gnu"

# 所有第三方内容统一放在 third_party/<target> 下，避免多目标冲突
THIRD_PARTY="$PROJECT_ROOT/third_party"
TARGET_DIR="$THIRD_PARTY/$TARGET"
mkdir -p "$TARGET_DIR"

# x86_64 本地编译，使用系统 gcc 工具链（无需下载交叉编译工具链）
CROSS_PREFIX="x86_64-linux-gnu"

# 源码下载与构建目录
BUILD_DIR="$TARGET_DIR/build"

# C 依赖库版本
ALSA_VERSION="1.2.12"

# 并行编译线程数
NPROC=$(nproc 2>/dev/null || echo 4)

# =============================================================================
# 2. 从源码编译 alsa-lib 为共享库（.so）
#    运行时使用宿主机系统的 libasound.so.2
# =============================================================================

mkdir -p "$BUILD_DIR"

# 设置编译环境变量（使用系统 gcc）
export CC="gcc"
export CXX="g++"
export AR="ar"
export RANLIB="ranlib"
export STRIP="strip"
export CFLAGS="-fPIC"
export CXXFLAGS="-fPIC"

# 编译 alsa-lib（共享库，仅用于链接时符号解析）
build_alsa_shared "$TARGET_DIR" "$BUILD_DIR" "$CROSS_PREFIX" "$ALSA_VERSION" "$NPROC"


# =============================================================================
# 3. 设置 Rust 编译环境
# =============================================================================

echo ""
echo "=== Step 3: 设置 Rust 编译环境 ==="

# 混合链接：保持 libc/libdl 动态链接
# -L 指向编译好的 ALSA 共享库目录
# --no-as-needed：确保 -lpthread -ldl -lm 不会被 Rust 注入的 --as-needed 丢弃
export RUSTFLAGS="-C link-arg=-L$ALSA_SHARED_LIBDIR -C link-arg=-Wl,--no-as-needed -C link-arg=-ldl -C link-arg=-lpthread -C link-arg=-lm"

# 告诉 audiopus_sys 使用静态链接 opus
export LIBOPUS_STATIC=1

# ALSA 动态链接：
#   - 不设置 ALSA_STATIC，让 alsa-sys 动态链接 libasound.so
#   - alsa.pc 已通过 sed 修正为实际安装路径，pkg-config 返回正确的 -L 路径
#   - 运行时由宿主机的系统 libasound.so.2 提供

# pkg-config 仅指向编译好的 alsa 共享库目录
# 确保 opus/speexdsp 不会被系统动态库解析，而是由 build.rs 从源码编译为静态库
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR="$ALSA_SHARED_PKGCONFIG"
export PKG_CONFIG_SYSROOT_DIR=""

# 通知 build.rs 对本地 x86_64 目标也使用静态链接路径（而非系统动态库）
export XIAOZHI_FORCE_STATIC_LIBS=1

echo "RUSTFLAGS:    $RUSTFLAGS"

# =============================================================================
# 4. 编译 Rust 项目
# =============================================================================

echo ""
echo "=== Step 4: 编译 Rust 项目 ==="
echo "Building in: $PROJECT_ROOT"
echo "Target: $TARGET (native)"

# 避免干扰 build.rs 的宿主端编译，取消通用 CC 变量
unset CC CXX AR RANLIB STRIP CFLAGS CXXFLAGS

# 创建 dummy libopus.a 桩文件，防止 audiopus_sys 尝试通过 cmake 自行编译 opus
# （audiopus_sys 发现 OPUS_LIB_DIR 中有 libopus.a 时会跳过 cmake 构建；
#   实际 opus 已由 build.rs 从源码编译为静态库并通过 cargo:rustc-link-lib 链接）
DUMMY_OPUS_DIR="$PROJECT_ROOT/target/dummy_opus"
mkdir -p "$DUMMY_OPUS_DIR/lib"
echo "void opus_dummy() {}" > "$DUMMY_OPUS_DIR/dummy.c"
gcc -c "$DUMMY_OPUS_DIR/dummy.c" -o "$DUMMY_OPUS_DIR/dummy.o"
ar rcs "$DUMMY_OPUS_DIR/lib/libopus.a" "$DUMMY_OPUS_DIR/dummy.o"
export OPUS_LIB_DIR="$DUMMY_OPUS_DIR"

cargo build --release

echo ""
echo "============================================="
echo "  编译完成!"
echo "============================================="

OUTPUT_BIN="$PROJECT_ROOT/target/release/xiaozhi_linux_rs"
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
