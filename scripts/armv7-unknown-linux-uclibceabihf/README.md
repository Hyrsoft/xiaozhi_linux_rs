## armv7-unknown-linux-uclibceabihf 交叉编译说明

### 工具链路径
需根据具体的交叉编译工具链路径和 sysroot 路径，修改 build.sh 脚本中的如下部分：
```bash
export TOOLCHAIN_PATH="/path/to/your/toolchain"
SYSROOT="/path/to/your/sysroot"
```

### 配置示例
以下是基于 RV1106 (Luckfox Pico) 的配置示例：
```bash
# 工具链路径
TOOLCHAIN_PATH="/home/hao/projects/luckfox-pico/tools/linux/toolchain/arm-rockchip830-linux-uclibcgnueabihf"

# sysroot 路径（来自 buildroot 输出）
SYSROOT="/home/hao/projects/luckfox-pico/sysdrv/source/buildroot/buildroot-2023.02.6/output/host/arm-buildroot-linux-uclibcgnueabihf/sysroot"

# 交叉编译工具
CROSS_GCC="$TOOLCHAIN_PATH/bin/arm-rockchip830-linux-uclibcgnueabihf-gcc"
CROSS_CXX="$TOOLCHAIN_PATH/bin/arm-rockchip830-linux-uclibcgnueabihf-g++"
AR="$TOOLCHAIN_PATH/bin/arm-rockchip830-linux-uclibcgnueabihf-ar"
```

### uClibc 特别说明

经实际编译测试，RV1106 的 buildroot sdk，提供的 uClibc std 相对 rust 工具链提供的版本，缺少了`getauxval`函数的实现，因此在链接阶段会找不到函数定义。解决方法是手动实现一个空的`getauxval`函数，编译为静态库，在最后链接进可执行文件。