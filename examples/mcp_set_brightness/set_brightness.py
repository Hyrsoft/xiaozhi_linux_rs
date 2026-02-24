#!/usr/bin/env python3
import sys
import json

def main():
    # 从标准输入读取来自 Rust 网关的 JSON 参数
    try:
        input_data = sys.stdin.read()
        if not input_data:
            print("错误：未收到任何输入数据")
            sys.exit(1)
            
        args = json.loads(input_data)
        
        # 获取亮度参数
        brightness = args.get("brightness")
        
        if brightness is None:
            print("错误：缺少必要的 'brightness' 参数")
            sys.exit(1)

        # 逻辑校验：范围 0-100
        try:
            val = int(brightness)
            if val < 0 or val > 100:
                print(f"执行失败：亮度值 {val} 超出范围 (0-100)")
                sys.exit(1)
                
            # --- 实际硬件控制代码（可选） ---
            # 假设硬件路径如下，取消注释可实际控制：
            # with open("/sys/class/backlight/backlight/brightness", "w") as f:
            #     # 某些板子可能需要将 0-100 映射到 0-255
            #     actual_val = int(val * 2.55)
            #     f.write(str(actual_val))
            # ------------------------------

            print(f"成功：已将设备屏幕亮度设置为 {val}%。")
            
        except ValueError:
            print(f"错误：亮度参数 '{brightness}' 不是有效的数字")
            sys.exit(1)

    except Exception as e:
        print(f"系统错误：{str(e)}")
        sys.exit(1)

if __name__ == "__main__":
    main()