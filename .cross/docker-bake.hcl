variable "REGISTRY" {
  default = "ghcr.io/haoyn231/xiaozhi-linux-rs-cross"
}

group "default" {
  targets = ["x86_64-gnu", "aarch64-gnu", "armv7-gnu", "armv7-uclibc"]
}

target "common" {
  context    = "."
  dockerfile = ".cross/Dockerfile"
  platforms  = ["linux/amd64"]
}

target "x86_64-gnu" {
  inherits = ["common"]
  tags     = ["${REGISTRY}:x86_64-gnu-sdk-v1"]
  args = {
    TARGET       = "x86_64-unknown-linux-gnu"
    CROSS_PREFIX = "x86_64-linux-gnu"
  }
}

target "aarch64-gnu" {
  inherits = ["common"]
  tags     = ["${REGISTRY}:aarch64-gnu-sdk-v1"]
  args = {
    TARGET              = "aarch64-unknown-linux-gnu"
    CROSS_PREFIX        = "aarch64-linux-gnu"
    TOOLCHAIN_ASSET     = "gcc-arm-8.3-2019.02-x86_64-aarch64-linux-gnu.tar.xz"
    TOOLCHAIN_SHA256    = "07d8ae8fa505ccadfc8cbac8b77d6f47740ecec2079267ad1d0a20bdc5342fde"
    TOOLCHAIN_DIR       = "gcc-arm-8.3-2019.02-x86_64-aarch64-linux-gnu"
  }
}

target "armv7-gnu" {
  inherits = ["common"]
  tags     = ["${REGISTRY}:armv7-gnu-sdk-v1"]
  args = {
    TARGET              = "armv7-unknown-linux-gnueabihf"
    CROSS_PREFIX        = "arm-linux-gnueabihf"
    TOOLCHAIN_ASSET     = "gcc-arm-8.3-2019.02-x86_64-arm-linux-gnueabihf.tar.xz"
    TOOLCHAIN_SHA256    = "59c094b8d8398c8ac6dece337fcab6f2f8f209b5e5d9d76d75d9087add01942d"
    TOOLCHAIN_DIR       = "gcc-arm-8.3-2019.02-x86_64-arm-linux-gnueabihf"
  }
}

target "armv7-uclibc" {
  inherits = ["common"]
  tags     = ["${REGISTRY}:armv7-uclibc-sdk-v1"]
  args = {
    TARGET              = "armv7-unknown-linux-uclibceabihf"
    CROSS_PREFIX        = "arm-rockchip830-linux-uclibcgnueabihf"
    TOOLCHAIN_ASSET     = "arm-rockchip830-linux-uclibcgnueabihf.tar.xz"
    TOOLCHAIN_SHA256    = "1482cc1b34e0b74362a89c734f78e3daa889dee8964d8153c239c335a1644ef2"
    TOOLCHAIN_DIR       = "arm-rockchip830-linux-uclibcgnueabihf"
  }
}

