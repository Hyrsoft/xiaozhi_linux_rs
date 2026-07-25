#!/bin/bash
# =============================================================================
# ALSA 共享库交叉编译函数
#
# 用法:
#   source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/build_alsa.sh"
#   build_alsa_shared \
#       "$TARGET_DIR" \
#       "$BUILD_DIR" \
#       "$CROSS_PREFIX" \
#       "$ALSA_VERSION" \
#       "$NPROC"
#
# 参数:
#   $1 - 目标目录（alsa-shared 安装到此目录下）
#   $2 - 构建目录（源码解压到此目录下）
#   $3 - 交叉编译前缀（如 aarch64-linux-gnu）
#   $4 - alsa-lib 版本号（如 1.2.12）
#   $5 - 并行编译线程数
#
# 前置条件:
#   - CC, AR, RANLIB 等环境变量已设置
#   - download_helper.sh 已 source（需要 download_file 函数）
#
# 结果:
#   该函数执行后，会直接设置全局环境变量:
#   ALSA_SHARED_LIBDIR
#   ALSA_SHARED_PKGCONFIG
# =============================================================================

build_alsa_shared() {
    local target_dir="$1"
    local build_dir="$2"
    local cross_prefix="$3"
    local alsa_version="$4"
    local nproc="$5"

    local alsa_src_dir="$build_dir/alsa-lib-${alsa_version}"
    local alsa_install_dir="$target_dir/alsa-shared"

    echo ""
    echo "=== 编译 alsa-lib ${alsa_version} (共享库 .so，仅链接时使用) ==="

    if [ -f "$alsa_install_dir/usr/lib/libasound.so" ]; then
        echo "alsa-lib 共享库已存在，跳过编译。"
    else
        local alsa_tarball="alsa-lib-${alsa_version}.tar.bz2"
        local alsa_url="https://github.com/haoyn231/xiaozhi_linux_rs/releases/download/Source_Mirror/${alsa_tarball}"

        if [ ! -d "$alsa_src_dir" ]; then
            echo "下载 alsa-lib..."
            download_file "$alsa_url" "$build_dir/${alsa_tarball}"
            echo "解压 alsa-lib..."
            tar -xjf "$build_dir/${alsa_tarball}" -C "$build_dir"
            rm -f "$build_dir/${alsa_tarball}"
        fi

        local saved_dir="$(pwd)"
        cd "$alsa_src_dir"
        # 清理之前可能存在的编译产物
        make distclean 2>/dev/null || true
        echo "配置 alsa-lib (共享库模式)..."
        # LDFLAGS="-Wl,--as-needed": 仅为 libasound.so 实际使用符号的库添加 DT_NEEDED
        # 避免 libpthread.so.0 等作为间接依赖被加载，导致旧版 ld 的 --as-needed 冲突
        ./configure \
            --host="${cross_prefix}" \
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

        echo "编译 alsa-lib (使用 ${nproc} 线程)..."
        make -j"$nproc" --quiet
        mkdir -p "$alsa_install_dir"
        make DESTDIR="$alsa_install_dir" install --quiet

        # 修正 alsa.pc 中的 prefix 路径：/usr → 实际安装绝对路径
        # 否则 pkg-config 会返回 -L/usr/lib，指向宿主机的 x86_64 库
        sed -i "s|prefix=/usr|prefix=$alsa_install_dir/usr|" "$alsa_install_dir/usr/lib/pkgconfig/alsa.pc"

        echo "alsa-lib 共享库编译完成!"
        cd "$saved_dir"
    fi

    # 直接设置全局变量
    ALSA_SHARED_LIBDIR="$alsa_install_dir/usr/lib"
    ALSA_SHARED_PKGCONFIG="$alsa_install_dir/usr/lib/pkgconfig"
    echo "ALSA 共享库: $ALSA_SHARED_LIBDIR"
    ls -la "$ALSA_SHARED_LIBDIR"/libasound.so* 2>/dev/null || true
}
