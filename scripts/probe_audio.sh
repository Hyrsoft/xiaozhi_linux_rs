#!/bin/bash

echo "================================================="
echo "   Xiaozhi Linux rust - 音频硬件能力探测工具"
echo "================================================="

# 检查依赖
if ! command -v aplay &> /dev/null; then
    echo "❌ 未找到 aplay/arecord，请先安装 alsa-utils！"
    echo "💡 Debian/Ubuntu: sudo apt install alsa-utils"
    echo "💡 Arch Linux: sudo pacman -S alsa-utils"
    exit 1
fi

# 1. 探测播放设备
echo -e "\n🔍 [1] 正在扫描播放设备 (Playback Devices)..."
aplay -l | grep "^card"
PLAY_CARD=$(aplay -l | grep "^card" | head -n 1 | sed -E 's/card ([0-9]+):.*/\1/')
PLAY_DEV=$(aplay -l | grep "^card" | head -n 1 | sed -E 's/.*device ([0-9]+):.*/\1/')

# 2. 探测录音设备
echo -e "\n🎤 [2] 正在扫描录音设备 (Capture Devices)..."
arecord -l | grep "^card"
CAP_CARD=$(arecord -l | grep "^card" | head -n 1 | sed -E 's/card ([0-9]+):.*/\1/')
CAP_DEV=$(arecord -l | grep "^card" | head -n 1 | sed -E 's/.*device ([0-9]+):.*/\1/')

# 3. 测试硬件采样率支持 (使用 hw 直通层避免软件重采样干扰)
BEST_RATE=48000
if [ -n "$PLAY_CARD" ] && [ -n "$PLAY_DEV" ]; then
    HW_PLAY="hw:$PLAY_CARD,$PLAY_DEV"
    PLUG_PLAY="plughw:$PLAY_CARD,$PLAY_DEV"
    echo -e "\n⚙️  [3] 正在深度探测声卡 ($HW_PLAY) 硬件原生支持的采样率..."
    
    # 常见采样率梯度测试
    RATES=(48000 44100 24000 16000 8000)
    SUPPORTED_RATES=()
    
    for rate in "${RATES[@]}"; do
        # 注入 0.2 秒的纯净静音 PCM 数据直接轰炸硬件，测试是否报错
        if head -c $((rate * 2 * 2 / 5)) /dev/zero | aplay -D "$HW_PLAY" -t raw -f S16_LE -r "$rate" -c 2 -q 2>/dev/null; then
            echo "   ✅ 硬件原生支持: ${rate}Hz"
            SUPPORTED_RATES+=($rate)
        else
            echo "   ❌ 硬件拒绝直通: ${rate}Hz"
        fi
    done
    
    # 取最高支持的采样率作为最佳推荐
    if [ ${#SUPPORTED_RATES[@]} -gt 0 ]; then
        BEST_RATE=${SUPPORTED_RATES[0]}
        echo "   👉 探测到最佳匹配采样率: ${BEST_RATE}Hz"
    else
        echo "   ⚠️ 硬件层拒绝了所有标准测试，可能被 PulseAudio/PipeWire 独占，建议配置回退为 default"
        HW_PLAY="default"
        PLUG_PLAY="default"
    fi
else
    echo -e "\n❌ 未探测到有效的物理播放声卡！"
    PLUG_PLAY="default"
fi

# 4. 生成推荐配置
echo -e "\n================================================="
echo "📋 探测完毕！为你推荐的 xiaozhi_config.json 音频配置参数："
echo "================================================="
cat << EOF
{
  "capture_device": "default",
  "playback_device": "$PLUG_PLAY",
  "stream_format": "opus",
  "playback_sample_rate": $BEST_RATE,
  "playback_channels": 2,
  "playback_period_size": 1024
}
EOF
echo "================================================="