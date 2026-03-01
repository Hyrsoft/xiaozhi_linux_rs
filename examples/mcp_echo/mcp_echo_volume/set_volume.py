#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import sys
import json
import subprocess
import re

# 声卡配置（可根据实际情况修改）
CARD = "0"
CONTROL = "DAC LINEOUT"

def validate_volume(volume):
    if not isinstance(volume, int) or volume < 0 or volume > 100:
        raise ValueError(f"音量必须为0-100的整数，收到: {volume}")

def set_volume(volume):
    try:
        cmd = ["amixer", "-c", CARD, "sset", CONTROL, f"{volume}%"]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
        if result.returncode != 0:
            raise RuntimeError(f"amixer 执行失败: {result.stderr}")
        return f"音量已设置为 {volume}%"
    except subprocess.TimeoutExpired:
        raise RuntimeError("amixer 命令超时")
    except FileNotFoundError:
        raise RuntimeError("未找到 amixer 命令，请检查是否安装了 alsa-utils")

def main():
    try:
        data = sys.stdin.read()
        if not data:
            print("错误：未收到输入")
            sys.exit(1)
        params = json.loads(data)
    except Exception as e:
        print(f"解析JSON失败：{e}")
        sys.exit(1)

    volume = params.get("volume")
    if volume is None:
        print("错误：缺少 volume 参数")
        sys.exit(1)

    try:
        validate_volume(volume)
        message = set_volume(volume)
        print(message)
    except (ValueError, RuntimeError) as e:
        print(f"错误：{e}")
        sys.exit(1)

if __name__ == "__main__":
    main()