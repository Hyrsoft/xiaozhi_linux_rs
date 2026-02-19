## aarch64-unknown-linux-gnu 交叉编译说明

### 工具链路径
需根据具体的交叉编译工具链路径和 sysroot 路径，修改 build.sh 脚本中的如下部分：
```bash
export TOOLCHAIN_PATH="/path/to/your/toolchain"
SYSROOT="/path/to/your/sysroot"
```

### 配置示例
以下是基于 RK3576 (Dshanpi-A1) 的配置示例：
```bash
# 工具链路径
TOOLCHAIN_PATH="/home/hao/projects/rk3506_build_env/100ask-rk3576_SDK/prebuilts/gcc/linux-x86/aarch64/gcc-arm-10.3-2021.07-x86_64-aarch64-none-linux-gnu"

# sysroot 路径
SYSROOT="/home/hao/projects/rk3506_build_env/100ask-rk3576_SDK/buildroot/output/rockchip_rk3576/host/aarch64-buildroot-linux-gnu/sysroot"

# 交叉编译工具
CROSS_GCC="$TOOLCHAIN_PATH/bin/aarch64-none-linux-gnu-gcc"
CROSS_CXX="$TOOLCHAIN_PATH/bin/aarch64-none-linux-gnu-g++"
```