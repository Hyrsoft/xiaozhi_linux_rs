#!/bin/bash
set -e

# 获取脚本所在目录的绝对路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 跳转到项目根目录（../../）
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")/../"
cd "$PROJECT_ROOT"
PROJECT_ROOT="$(pwd)"  # 获取项目根的绝对路径

# 1. 基础路径配置
export TOOLCHAIN_PATH="/home/hao/projects/luckfox-pico/tools/linux/toolchain/arm-rockchip830-linux-uclibcgnueabihf"
# .pc 文件在 buildroot 输出的 host sysroot 中，而不是 tools 工具链中
SYSROOT="/home/hao/projects/luckfox-pico/sysdrv/source/buildroot/buildroot-2023.02.6/output/host/arm-buildroot-linux-uclibcgnueabihf/sysroot"
TARGET=armv7-unknown-linux-uclibceabihf

CROSS_GCC="$TOOLCHAIN_PATH/bin/arm-rockchip830-linux-uclibcgnueabihf-gcc"
AR="$TOOLCHAIN_PATH/bin/arm-rockchip830-linux-uclibcgnueabihf-ar"

# 静态库生成在项目根目录下
STUB_DIR="$PROJECT_ROOT/uclibc_stub"

echo "=== Step 1: build auxval stub ==="

mkdir -p "$STUB_DIR"

# 编译 stub（使用脚本目录下的源文件）
$CROSS_GCC -c "$SCRIPT_DIR/auxval_stub.c" -o "$STUB_DIR/auxval_stub.o"

# 生成静态库
$AR rcs "$STUB_DIR/libauxval_stub.a" "$STUB_DIR/auxval_stub.o"

echo "Stub library built at $STUB_DIR/libauxval_stub.a"

echo "=== Step 2: setup environment ==="

export CC_armv7_unknown_linux_uclibceabihf="$CROSS_GCC"
export CXX_armv7_unknown_linux_uclibceabihf="$TOOLCHAIN_PATH/bin/arm-rockchip830-linux-uclibcgnueabihf-g++"

export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_PATH=""
export PKG_CONFIG_LIBDIR="$SYSROOT/usr/lib/pkgconfig:$SYSROOT/usr/share/pkgconfig"
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"

echo "=== Step 3: build Rust project ==="

echo "Building in: $PROJECT_ROOT"

# 通过环境变量传递 linker
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_UCLIBCEABIHF_LINKER="$CROSS_GCC"

cargo +nightly build \
    -Z build-std=std,panic_abort \
    --target $TARGET \
    --release
