# Bilibili 直播录制器 (LiveRecorder)

一个基于 Rust 和 GPUI 开发的跨平台 Bilibili 直播录制工具，提供现代化的图形用户界面和高效的录制功能。

## 🚀 功能特性

- **现代化界面**: 基于 GPUI 框架构建的流畅、美观的用户界面
- **跨平台支持**: 支持 Windows、macOS 和 Linux 系统
- **实时录制**: 支持 Bilibili 直播间的实时录制功能
- **多房间管理**: 可以同时管理多个直播间的录制任务
- **主题切换**: 支持明暗主题切换，提供更好的视觉体验
- **设置管理**: 灵活的录制设置和房间配置管理
- **高质量录制**: 支持多种画质选择，确保录制质量

## 📋 系统要求

- **操作系统**: Windows 10+, macOS 10.15+, 或 Linux (Ubuntu 18.04+)
- **内存**: 至少 4GB RAM
- **存储**: 至少 1GB 可用磁盘空间
- **网络**: 稳定的互联网连接

## 🛠️ 安装说明

### 从源码编译

1. **安装 Rust 工具链**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **克隆项目**
   ```bash
   git clone https://github.com/starknt/bilibili-recoder.git
   cd bilibili-recoder
   ```

3. **编译项目**
   ```bash
   cargo build --release
   ```

4. **运行程序**
   ```bash
   cargo run --release
   ```

### 依赖项

项目使用以下主要依赖：

- **GPUI**: 现代化的 GUI 框架
- **reqwest**: HTTP 客户端
- **serde**: 序列化/反序列化
- **tokio**: 异步运行时
- **chrono**: 时间处理

## 🎯 使用方法

### 基本操作

1. **启动程序**: 运行编译后的可执行文件
2. **输入房间号**: 在输入框中输入 Bilibili 直播间房间号
3. **开始录制**: 点击录制按钮开始录制直播
4. **管理录制**: 在房间卡片中查看录制状态和控制录制

### 高级功能

- **设置配置**: 通过设置面板调整录制参数
- **主题切换**: 在界面中切换明暗主题
- **多房间管理**: 同时添加多个直播间进行录制

## 🏗️ 项目结构

```
bilibili-recoder/
├── src/
│   ├── main.rs              # 程序入口
│   ├── lib.rs               # 主应用逻辑
│   ├── api/                 # API 接口模块
│   │   ├── mod.rs           # API 客户端
│   │   ├── room.rs          # 房间信息 API
│   │   ├── stream.rs        # 流媒体 API
│   │   └── user.rs          # 用户信息 API
│   ├── components/          # UI 组件
│   │   ├── mod.rs           # 组件模块
│   │   ├── room_card.rs     # 房间卡片组件
│   │   └── settings_modal.rs # 设置模态框
│   ├── settings.rs          # 设置管理
│   ├── state.rs             # 应用状态
│   ├── themes.rs            # 主题管理
│   └── title_bar.rs         # 标题栏
├── assets/                  # 静态资源
│   └── icons/               # 图标文件
├── resources/               # 资源文件
├── themes/                  # 主题文件
├── script/                  # 构建脚本
└── Cargo.toml              # 项目配置
```

## 🔧 开发指南

### 环境设置

1. **安装开发依赖**
   ```bash
   cargo install cargo-watch  # 用于开发时自动重新编译
   ```

2. **运行开发模式**
   ```bash
   cargo watch -x run
   ```

### 代码规范

- 遵循 Rust 官方代码规范
- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 进行代码检查

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_name
```

## 📝 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🤝 贡献指南

我们欢迎所有形式的贡献！请查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解如何参与项目开发。

### 贡献方式

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 打开 Pull Request

## 🐛 问题反馈

如果您遇到任何问题或有改进建议，请通过以下方式联系我们：

- 在 GitHub 上创建 [Issue](https://github.com/your-username/bilibili-recoder/issues)
- 发送邮件至项目维护者

## 📄 更新日志

### v0.1.0 (当前版本)
- 初始版本发布
- 基础录制功能
- 现代化 GUI 界面
- 跨平台支持

## 🙏 致谢

感谢以下开源项目：

- [GPUI](https://github.com/zed-industries/zed) - 现代化的 GUI 框架
- [gpui-component](https://github.com/longbridge/gpui-component) - GPUI 组件库
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP 客户端
- [serde](https://github.com/serde-rs/serde) - 序列化框架

---

**注意**: 本项目仅供学习和个人使用，请遵守 Bilibili 的服务条款和相关法律法规。
