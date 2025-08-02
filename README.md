# Bilibili 直播录制器 (BLive)

一个基于 Rust 和 GPUI 开发的跨平台 Bilibili 直播录制工具，提供现代化的图形用户界面和高效的录制功能。

## 📚 文档

- **[用户指南](USER_GUIDE.md)** - 详细的使用说明和功能介绍
- **[开发者指南](DEVELOPER_GUIDE.md)** - 开发环境设置和贡献指南
- **[贡献指南](CONTRIBUTING.md)** - 详细的贡献流程和规范
- **[TODO](TODO.md)** - 项目开发计划和功能状态

## 🚀 功能特性

- **现代化界面**: 基于 GPUI 框架构建的流畅、美观的用户界面
- **跨平台支持**: 支持 Windows、macOS 和 Linux 系统
- **实时录制**: 支持 Bilibili 直播间的实时录制功能
- **多房间管理**: 可以同时管理多个直播间的录制任务
- **智能文件管理**: 自动处理文件冲突，支持分P录制
- **多种流协议**: 支持 HTTP Stream 和 HTTP HLS 两种直播流协议
- **高质量录制**: 支持多种画质选择（杜比、4K、原画、蓝光、超清、高清、流畅）
- **多种格式支持**: 支持 FLV、FMP4、TS 等多种视频容器格式
- **编码选择**: 支持 AVC 和 HEVC 两种视频编码
- **自动重连**: 网络异常时自动重连，确保录制连续性
- **主题切换**: 支持明暗主题切换，提供更好的视觉体验
- **设置管理**: 灵活的录制设置和房间配置管理
- **日志系统**: 完整的日志记录和错误追踪

## 📋 系统要求

- **操作系统**: Windows 10+, macOS 10.15+, 或 Linux (Ubuntu 18.04+)
- **内存**: 至少 4GB RAM
- **存储**: 至少 1GB 可用磁盘空间
- **网络**: 稳定的互联网连接
- **FFmpeg**: 程序会自动下载和配置 FFmpeg

## 🛠️ 安装说明

### 从源码编译

1. **安装 Rust 工具链**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **克隆项目**
   ```bash
   git clone https://github.com/starknt/blive.git
   cd blive
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
- **gpui-component**: GPUI 组件库
- **reqwest**: HTTP 客户端
- **serde**: 序列化/反序列化
- **tokio**: 异步运行时
- **chrono**: 时间处理
- **ffmpeg-sidecar**: FFmpeg 集成
- **tracing**: 日志系统

## 🎯 使用方法

### 基本操作

1. **启动程序**: 运行编译后的可执行文件
2. **输入房间号**: 在输入框中输入 Bilibili 直播间房间号
3. **开始录制**: 点击录制按钮开始录制直播
4. **管理录制**: 在房间卡片中查看录制状态和控制录制

### 高级功能

- **设置配置**: 通过设置面板调整录制参数
  - 录制策略：优化CPU占用、优化硬盘占用、配置优先
  - 录制质量：杜比、4K、原画、蓝光、超清、高清、流畅
  - 录制格式：FLV、FMP4、TS
  - 录制编码：AVC、HEVC
  - 录制目录：自定义录制文件保存路径
- **主题切换**: 在界面中切换明暗主题
- **多房间管理**: 同时添加多个直播间进行录制
- **文件管理**: 自动处理文件冲突，支持分P录制

## 🏗️ 项目结构

```
blive/
├── src/
│   ├── main.rs              # 程序入口
│   ├── lib.rs               # 主应用逻辑
│   ├── app.rs               # 应用主界面
│   ├── state.rs             # 应用状态管理
│   ├── settings.rs          # 设置管理
│   ├── themes.rs            # 主题管理
│   ├── logger.rs            # 日志系统
│   ├── title_bar.rs         # 标题栏
│   ├── core/                # 核心功能模块
│   │   ├── http_client.rs   # HTTP 客户端
│   │   ├── downloader.rs    # 下载器核心逻辑
│   │   ├── http_client/     # HTTP 客户端实现
│   │   └── downloader/      # 下载器实现
│   │       ├── http_stream.rs # HTTP Stream 下载器
│   │       └── http_hls.rs    # HTTP HLS 下载器
│   ├── components/          # UI 组件
│   │   ├── mod.rs           # 组件模块
│   │   ├── room_card.rs     # 房间卡片组件
│   │   ├── room_input.rs    # 房间输入组件
│   │   ├── settings_modal.rs # 设置模态框
│   │   └── app_settings.rs  # 应用设置组件
│   └── error.rs             # 错误处理
├── assets/                  # 静态资源
├── resources/               # 资源文件
├── themes/                  # 主题文件
├── script/                  # 构建脚本
└── Cargo.toml              # 项目配置
```

## 🔧 开发指南

如果您想参与项目开发，请查看详细的 [贡献指南](CONTRIBUTING.md)。

### 快速开始

1. **克隆项目**
   ```bash
   git clone https://github.com/starknt/blive.git
   cd blive
   ```

2. **安装依赖**
   ```bash
   cargo build
   ```

3. **运行项目**
   ```bash
   cargo run --release
   ```

### 开发工具

- **代码格式化**: `cargo fmt`
- **代码检查**: `cargo clippy`
- **运行测试**: `cargo nextest run --all-features`

更多详细的开发指南、代码规范、测试指南和贡献流程，请参考 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 📝 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🤝 贡献指南

我们欢迎所有形式的贡献！请查看详细的 [贡献指南](CONTRIBUTING.md) 了解如何参与项目开发。

### 贡献方式

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 打开 Pull Request

详细的开发环境设置、代码规范、测试指南和贡献流程，请参考 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 🐛 问题反馈

如果您遇到任何问题或有改进建议，请通过以下方式联系我们：

- 在 GitHub 上创建 [Issue](https://github.com/starknt/blive/issues)
- 发送邮件至项目维护者

## 📄 更新日志

### v0.1.0 (当前版本)
- 初始版本发布
- 基础录制功能
- 现代化 GUI 界面
- 跨平台支持
- 支持 HTTP Stream 和 HTTP HLS 协议
- 多种画质和格式支持
- 自动重连功能
- 智能文件管理
- 完整的设置系统

## 🙏 致谢

感谢以下开源项目：

- [GPUI](https://github.com/zed-industries/zed) - 现代化的 GUI 框架
- [gpui-component](https://github.com/longbridge/gpui-component) - GPUI 组件库
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP 客户端
- [serde](https://github.com/serde-rs/serde) - 序列化框架
- [ffmpeg-sidecar](https://github.com/nathanbabcock/ffmpeg-sidecar) - FFmpeg 集成

## 🌟 项目统计

[![Star History Chart](https://api.star-history.com/svg?repos=starknt/blive&type=Date)](https://star-history.com/#starknt/blive&Date)

---

**注意**: 本项目仅供学习和个人使用，请遵守 Bilibili 的服务条款和相关法律法规。
