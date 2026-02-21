#!/bin/bash
# =============================================================================
# 共用下载函数 —— 支持重试和 wget/curl 自动切换
#
# 用法：source scripts/download_helper.sh
#       download_file <URL> <输出文件路径>
# =============================================================================

download_file() {
    local url="$1"
    local output="$2"
    local max_retries=3
    local retry_delay=5

    echo "下载: $url"

    for i in $(seq 1 $max_retries); do
        # 优先使用 wget
        if command -v wget &>/dev/null; then
            if wget --timeout=60 --tries=1 -q --show-progress -O "$output" "$url" 2>/dev/null; then
                return 0
            fi
        fi

        # wget 不可用或失败时使用 curl
        if command -v curl &>/dev/null; then
            if curl -fSL --connect-timeout 60 --retry 0 -o "$output" "$url" 2>/dev/null; then
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
