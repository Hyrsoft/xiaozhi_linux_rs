# 贡献指南 / Contributing Guidelines

感谢你对 Xiaozhi Linux 项目的贡献！/ Thank you for your interest in contributing to Xiaozhi Linux!

为了保证项目的稳定性和代码质量，请在提交 PR 前阅读以下指南。
To ensure project stability and code quality, please read the following guidelines before submitting a PR.

## 分支管理 / Branch Management

- **`main` 分支**: 该分支仅包含经过验证的、相对稳定的发布代码。**请勿直接向 `main` 分支提交 PR。**
- **`dev` 分支**: 开发分支，所有的功能新增、bug 修复都应该基于 `dev` 分支进行，并将 PR 目标分支设置为 `dev`。

- **`main` branch**: Contains validated and stable release code. **Do not submit PRs directly to `main`.**
- **`dev` branch**: Development branch. All new features and bug fixes should be based on `dev`, and the target branch of your PR must be `dev`.

## 提交 PR 前的检查 / Pre-PR Checklist

1. **编译测试 (Compilation Test)**:
   - 确保你的代码能在本地 Linux 环境正常编译通过 (`cargo build`)。
   - 确保交叉编译通过。可使用 `scripts` 目录下的提供的脚本测试（如 `armv7-unknown-linux-uclibceabihf`）。

2. **功能测试 (Functional Test)**:
   - 请针对修改的功能进行实际测试（包括环境运行或实体开发板如 RV1106 上）。
   - 保证原有核心功能不会因为修改而导致崩溃或异常。

3. **代码清理与规范 (Code Style & Cleanup)**:
   - 尽量使用 `cargo fmt` 格式化代码，保持和现有代码一致的风格。
   - 消除明显的 `cargo clippy` 警告。
   - 确保不要提交临时的测试文件、多余的日志打印。

## 提交 Pull Request / Pull Request Process

1. Fork 本仓库。
2. 创建你自己的功能分支 (例如 `git checkout -b feature/AmazingFeature`)。
3. 提交你的修改 (例如 `git commit -m 'Add some AmazingFeature'`)。
4. 推送至你自己的仓库 (例如 `git push origin feature/AmazingFeature`)。
5. 向我们的 **`dev`** 分支提交 Pull Request，并在提交时按照模板填写检查单。

## 提交 Issue / Issues

如果你发现了 bug，或者有好的新功能建议，欢迎先提交 Issue 与我们讨论，这样能提高 PR 的通过效率。
If you find a bug or have a feature suggestion, please open an Issue to discuss it with us first.
