#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import sys
import json
import os
import time

GPIO_PINS = {
    'left1': 32,
    'left2': 33,
    'right1': 35,
    'right2': 36
}

# 动作映射 (left1, left2, right1, right2)
# 注意：left 和 right 已互换
ACTIONS = {
    'forward':  (0, 1, 1, 0),
    'backward': (1, 0, 0, 1),
    'left':     (0, 1, 0, 1),   # 原 right 的值
    'right':    (1, 0, 1, 0),   # 原 left 的值
    'stop':     (0, 0, 0, 0)
}

# 移动持续时间（秒），可根据需要调整
MOVE_DURATION = 0.8

def ensure_gpio_exported(pin):
    gpio_path = f"/sys/class/gpio/gpio{pin}"
    if not os.path.exists(gpio_path):
        with open("/sys/class/gpio/export", "w") as f:
            f.write(str(pin))
    with open(f"{gpio_path}/direction", "w") as f:
        f.write("out")

def set_gpio(pin, value):
    ensure_gpio_exported(pin)
    with open(f"/sys/class/gpio/gpio{pin}/value", "w") as f:
        f.write(str(value))

def apply_action(action_name):
    """根据动作名称设置GPIO"""
    values = ACTIONS[action_name]
    pins = [GPIO_PINS['left1'], GPIO_PINS['left2'], GPIO_PINS['right1'], GPIO_PINS['right2']]
    for pin, val in zip(pins, values):
        set_gpio(pin, val)

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

    direction = params.get("direction")
    if not direction:
        print("错误：缺少 direction 参数")
        sys.exit(1)

    if direction not in ACTIONS:
        print(f"错误：无效方向 '{direction}'，可选：{', '.join(ACTIONS.keys())}")
        sys.exit(1)

    try:
        if direction == 'stop':
            # 直接停止
            apply_action('stop')
            print("Robot stopped")
        else:
            # 移动一小段后自动停止
            apply_action(direction)      # 设置移动方向
            time.sleep(MOVE_DURATION)    # 保持移动
            apply_action('stop')          # 停止
            print(f"Robot moved {direction} for a short while and stopped")
    except Exception as e:
        # 发生异常时确保停止
        apply_action('stop')
        print(f"设置GPIO失败：{e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
