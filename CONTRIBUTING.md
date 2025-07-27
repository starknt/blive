# 贡献指南

感谢您对 Bilibili 直播录制器项目的关注！我们欢迎所有形式的贡献，包括但不限于：

- 🐛 报告 Bug
- 💡 提出新功能建议
- 📝 改进文档
- 🔧 提交代码修复
- 🎨 改进用户界面
- 🌍 翻译和本地化

## 📋 贡献前准备

### 开发环境设置

1. **安装 Rust 工具链**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **安装开发工具**
   ```bash
   cargo install cargo-watch    # 开发时自动重新编译
   cargo install cargo-nextest  # 运行测试
   cargo install cargo-fmt      # 代码格式化
   cargo install cargo-clippy   # 代码检查
   ```

3. **克隆项目**
   ```bash
   git clone https://github.com/starknt/bilibili-recoder.git
   cd bilibili-recoder
   ```

4. **验证环境**
   ```bash
   cargo build
   cargo test
   ```

## 🔄 贡献流程

### 1. Fork 项目

在 GitHub 上 Fork 本项目到您的账户。

### 2. 创建功能分支

```bash
git checkout -b feature/your-feature-name
# 或者
git checkout -b fix/your-bug-fix
```

**分支命名规范：**
- `feature/` - 新功能
- `fix/` - Bug 修复
- `docs/` - 文档更新
- `refactor/` - 代码重构
- `test/` - 测试相关
- `chore/` - 构建工具或辅助工具的变动

### 3. 开发您的功能

- 遵循项目的代码规范
- 编写清晰的提交信息
- 添加必要的测试
- 更新相关文档

### 4. 提交代码

```bash
git add .
git commit -m "feat: 添加新功能描述"
```

**提交信息规范：**
- `feat:` - 新功能
- `fix:` - Bug 修复
- `docs:` - 文档更新
- `style:` - 代码格式调整
- `refactor:` - 代码重构
- `test:` - 测试相关
- `chore:` - 构建过程或辅助工具的变动

### 5. 推送分支

```bash
git push origin feature/your-feature-name
```

### 6. 创建 Pull Request

在 GitHub 上创建 Pull Request，并填写详细的描述。

## 📝 代码规范

### Rust 代码规范

1. **格式化代码**
   ```bash
   cargo fmt
   ```

2. **代码检查**
   ```bash
   cargo clippy
   ```

3. **运行测试**
   ```bash
   cargo nextest run --all-features
   ```

### 代码风格要求

- 使用有意义的变量和函数名
- 添加适当的注释
- 遵循 Rust 官方编码规范
- 保持函数简洁，单一职责
- 使用 `Result` 和 `Option` 进行错误处理

### 文档规范

- 为公共 API 添加文档注释
- 更新 README.md 中的相关部分
- 添加必要的使用示例

## 🐛 报告 Bug

### Bug 报告模板

请在创建 Issue 时包含以下信息：

```markdown
**Bug 描述**
简要描述 Bug 的内容

**重现步骤**
1. 打开应用
2. 执行操作 A
3. 执行操作 B
4. 看到错误

**预期行为**
描述您期望看到的行为

**实际行为**
描述实际发生的行为

**环境信息**
- 操作系统：Windows 10 / macOS 12 / Ubuntu 20.04
- Rust 版本：1.70.0
- 应用版本：v0.1.0

**附加信息**
截图、日志文件或其他相关信息
```

## 💡 功能建议

### 功能建议模板

```markdown
**功能描述**
简要描述您希望添加的功能

**使用场景**
描述这个功能的使用场景和解决的问题

**实现建议**
如果有的话，提供实现建议

**优先级**
高/中/低
```

## 🧪 测试指南

### 运行测试

```bash
# 运行所有测试
cargo nextest run --all-features

# 运行特定测试
cargo nextest run test_name

# 运行测试并显示输出
cargo nextest run -- --nocapture
```

### 编写测试

- 为每个新功能编写测试
- 测试应该覆盖正常情况和边界情况
- 使用描述性的测试名称
- 确保测试是独立的和可重复的

## 📚 文档贡献

### 文档类型

- **API 文档**: 为公共函数和结构体添加文档注释
- **用户指南**: 更新 README.md 和用户文档
- **开发文档**: 更新开发指南和架构文档
- **示例代码**: 提供使用示例和代码片段

### 文档规范

- 使用清晰、简洁的语言
- 提供实际的代码示例
- 保持文档与代码同步
- 使用适当的 Markdown 格式

## 🔍 代码审查

### 审查要点

- 代码质量和可读性
- 功能实现的正确性
- 测试覆盖率
- 文档完整性
- 性能影响
- 安全性考虑

### 审查流程

1. 自动检查（CI/CD）
2. 维护者审查
3. 社区反馈
4. 合并到主分支

## 🎉 贡献者名单

感谢所有为项目做出贡献的开发者！

<!-- 这里会自动生成贡献者列表 -->

## 📞 联系我们

如果您在贡献过程中遇到任何问题，请通过以下方式联系我们：

- 在 GitHub 上创建 Issue
- 发送邮件至项目维护者
- 参与项目讨论

## 📄 许可证

通过贡献代码，您同意您的贡献将在 MIT 许可证下发布。

---

再次感谢您的贡献！您的参与让这个项目变得更好。🌟
