# 🤝 贡献指南

感谢您对 Trae账号管理 项目的关注！我们欢迎并感谢所有形式的贡献。

## 📋 目录

- [行为准则](#行为准则)
- [如何贡献](#如何贡献)
- [开发环境搭建](#开发环境搭建)
- [提交规范](#提交规范)
- [代码风格](#代码风格)
- [Pull Request 流程](#pull-request-流程)
- [问题报告](#问题报告)

## 行为准则

本项目采用 [Contributor Covenant](https://www.contributor-covenant.org/) 行为准则。参与本项目即表示您同意遵守此准则。

## 如何贡献

### 报告 Bug

如果您发现了 Bug，请通过 Issues 报告，并包含以下信息：

- 问题的清晰描述
- 复现步骤
- 期望行为与实际行为
- 截图（如适用）
- 环境信息（操作系统、应用版本等）
- 相关日志或错误信息

### 建议新功能

我们欢迎新功能建议！请通过 Issues 提交，并描述：

- 功能的用途和价值
- 建议的实现方案
- 可能的替代方案

### 提交代码

1. Fork 本仓库
2. 创建您的功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交您的更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启一个 Pull Request

## 开发环境搭建

### 前置要求

- [Node.js](https://nodejs.org/) (推荐 v18+)
- [Rust](https://www.rust-lang.org/tools/install)
- [Git](https://git-scm.com/)

### 安装步骤

```bash
# 克隆仓库
git clone <your-repo-url>
cd Trae账号管理

# 安装前端依赖
npm install

# 安装 Tauri CLI（如未安装）
npm install -g @tauri-apps/cli

# 运行开发服务器
npm run tauri dev
```

### 构建生产版本

```bash
# 构建前端
npm run build

# 构建桌面应用
npm run tauri build
```

## 提交规范

我们使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>(<scope>): <subject>

<body>

<footer>
```

### 类型 (Type)

- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式（不影响代码运行的变动）
- `refactor`: 重构（既不是新增功能，也不是修改 Bug）
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建过程或辅助工具的变动

### 示例

```
feat(account): 添加账号批量导入功能

- 支持从 JSON 文件导入账号
- 添加导入结果统计
- 优化导入错误提示

Closes #123
```

## 代码风格

### TypeScript / React

- 使用 2 空格缩进
- 使用双引号
- 使用分号
- 组件和页面文件使用 PascalCase (如 `AccountCard.tsx`)
- 工具函数使用 camelCase

### Rust

- 遵循标准 `rustfmt` 布局
- 使用 4 空格缩进
- 运行 `cargo fmt` 格式化代码
- 运行 `cargo clippy` 检查代码

### 代码检查

提交前请确保：

```bash
# 前端代码检查
npm run lint

# Rust 代码检查
cd src-tauri
cargo check
cargo clippy
```

## Pull Request 流程

1. **更新文档**: 如果您的更改影响了功能，请更新相关文档
2. **添加测试**: 如适用，请添加测试用例
3. **确保构建通过**: 运行所有构建命令确保没有错误
4. **填写 PR 描述**: 清晰描述您的更改内容和原因
5. **关联 Issue**: 如有相关 Issue，请在 PR 描述中引用

### PR 审查清单

- [ ] 代码符合项目风格指南
- [ ] 所有测试通过
- [ ] 文档已更新
- [ ] 提交信息符合规范
- [ ] 没有引入不必要的依赖

## 问题报告

### 安全漏洞

如果您发现了安全漏洞，请不要在公共 Issue 中披露。请发送邮件至项目维护者（通过 GitHub 个人资料获取联系方式）。

### 一般问题

对于一般问题，请：

1. 先搜索现有 Issues，避免重复
2. 使用清晰的标题
3. 提供足够的信息以便复现
4. 保持礼貌和耐心

## 🙏 感谢

再次感谢您对 Trae账号管理 的贡献！您的参与让这个项目变得更好。

---

如有任何问题，欢迎通过 Issues 与我们联系。
