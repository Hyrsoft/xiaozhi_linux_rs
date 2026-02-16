#!/bin/bash
set -e

# 获取脚本所在目录的绝对路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 跳转到项目根目录（../../）
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")/../"
cd "$PROJECT_ROOT"
PROJECT_ROOT="$(pwd)"  # 获取项目根的绝对路径

echo "Project root: $PROJECT_ROOT"

# 1. 基础路径配置
export TOOLCHAIN_PATH="/home/hao/projects/rk3506_build_env/100ask-rk3576_SDK/prebuilts/gcc/linux-x86/aarch64/gcc-arm-10.3-2021.07-x86_64-aarch64-none-linux-gnu"
# .pc 文件在 buildroot 输出的 host sysroot 中
SYSROOT="/home/hao/projects/rk3506_build_env/100ask-rk3576_SDK/buildroot/output/rockchip_rk3576/host/aarch64-buildroot-linux-gnu/sysroot"
TARGET="aarch64-unknown-linux-gnu"

CROSS_GCC="$TOOLCHAIN_PATH/bin/aarch64-none-linux-gnu-gcc"
CROSS_CXX="$TOOLCHAIN_PATH/bin/aarch64-none-linux-gnu-g++"

echo "=== Step 1: setup environment ==="

export CC_aarch64_unknown_linux_gnu="$CROSS_GCC"
export CXX_aarch64_unknown_linux_gnu="$CROSS_CXX"

# 告诉链接器使用正确的 sysroot
export RUSTFLAGS="-C link-arg=--sysroot=$SYSROOT"

export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_PATH=""
export PKG_CONFIG_LIBDIR="$SYSROOT/usr/lib/pkgconfig:$SYSROOT/usr/share/pkgconfig"
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"

echo "Toolchain: $TOOLCHAIN_PATH"
echo "Sysroot: $SYSROOT"
echo "Target: $TARGET"

echo "=== Step 2: build Rust project ==="

echo "Building in: $PROJECT_ROOT"

# 通过环境变量传递 linker
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="$CROSS_GCC"

cargo build \
    --target $TARGET \
    --release
