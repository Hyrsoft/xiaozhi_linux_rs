#!/bin/bash
# =============================================================================
# 共用下载函数 —— 支持重试和 wget/curl 自动切换
# =============================================================================

download_file() {
    local url="$1"
    local output="$2"
    local max_retries=3
    local retry_delay=5
    local ua="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36"

    echo "下载: $url"

    for i in $(seq 1 $max_retries); do
        if command -v wget &>/dev/null; then
            if wget --timeout=120 --tries=1 -U "$ua" -q --show-progress -O "$output" "$url"; then
                return 0
            fi
        fi

        if command -v curl &>/dev/null; then
            if curl -fSL --connect-timeout 120 -A "$ua" --retry 0 -o "$output" "$url"; then
                return 0
            fi
        fi

        if [ "$i" -lt "$max_retries" ]; then
            echo "下载失败 (尝试 $i/$max_retries)，${retry_delay}s 后重试..."
            rm -f "$output"
            sleep $retry_delay
            retry_delay=$((retry_delay * 2))
        fi
    done

    echo "错误：下载失败（已重试 $max_retries 次）: $url"
    rm -f "$output"
    return 1
}

# =============================================================================
# 工具链下载与解压函数
#
# 用法:
#   TOOLCHAIN_DIR=$(download_and_setup_toolchain \
#       "$TARGET_DIR" \
#       "toolchain-name" \
#       "cross-prefix" \
#       "https://example.com/toolchain.tar.xz")
#
# 参数:
#   $1 - 目标目录（工具链解压到此目录下）
#   $2 - 工具链目录名（解压后的顶层目录名）
#   $3 - 交叉编译前缀（用于检测 gcc 是否存在，如 aarch64-linux-gnu）
#   $4 - 工具链下载 URL
#
# 返回: 通过 stdout 输出工具链安装目录的绝对路径
# =============================================================================

download_and_setup_toolchain() {
    local target_dir="$1"
    local toolchain_name="$2"
    local cross_prefix="$3"
    local toolchain_url="$4"

    local toolchain_dir="$target_dir/$toolchain_name"
    local toolchain_tarball
    toolchain_tarball=$(basename "$toolchain_url")

    if [ -x "$toolchain_dir/bin/${cross_prefix}-gcc" ]; then
        echo "工具链已存在，跳过下载。" >&2
    else
        echo "=== 下载交叉编译工具链 ===" >&2
        download_file "$toolchain_url" "$target_dir/${toolchain_tarball}" >&2
        echo "解压工具链..." >&2
        case "$toolchain_tarball" in
            *.tar.xz)  tar -xJf "$target_dir/${toolchain_tarball}" -C "$target_dir" ;;
            *.tar.gz|*.tgz)  tar -xzf "$target_dir/${toolchain_tarball}" -C "$target_dir" ;;
            *.tar.bz2) tar -xjf "$target_dir/${toolchain_tarball}" -C "$target_dir" ;;
            *) echo "未知的压缩格式: $toolchain_tarball" >&2; return 1 ;;
        esac
        rm -f "$target_dir/${toolchain_tarball}"
        echo "工具链安装完成: $toolchain_dir" >&2
    fi

    echo "$toolchain_dir"
}
