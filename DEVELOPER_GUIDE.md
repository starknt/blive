# BLive 开发者指南

欢迎开发者！本指南将帮助您了解 BLive 的架构、开发环境和贡献流程。

## 📖 目录

- [项目架构](#项目架构)
- [开发环境](#开发环境)
- [代码结构](#代码结构)
- [核心模块](#核心模块)
- [开发规范](#开发规范)
- [测试指南](#测试指南)
- [贡献流程](#贡献流程)

## 🏗️ 项目架构

### 整体架构

BLive 采用模块化设计，主要分为以下几个层次：

```
┌─────────────────────────────────────┐
│              UI 层                  │
│  (GPUI + gpui-component)           │
├─────────────────────────────────────┤
│             业务逻辑层               │
│  (应用状态管理 + 组件逻辑)           │
├─────────────────────────────────────┤
│             核心功能层               │
│  (下载器 + HTTP客户端 + 设置管理)     │
├─────────────────────────────────────┤
│             基础设施层               │
│  (日志 + 错误处理 + 工具函数)        │
└─────────────────────────────────────┘
```

### 技术栈

- **GUI 框架**: GPUI + gpui-component
- **HTTP 客户端**: reqwest
- **序列化**: serde + serde_json
- **异步运行时**: tokio
- **视频处理**: ffmpeg-sidecar
- **日志系统**: tracing
- **错误处理**: anyhow + thiserror

## 🛠️ 开发环境

### 系统要求

- **Rust**: 1.70+ (推荐最新稳定版)
- **操作系统**: Windows 10+, macOS 10.15+, Linux
- **内存**: 至少 8GB RAM (开发时)
- **存储**: 至少 20GB 可用磁盘空间

### 环境设置

1. **安装 Rust**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **克隆项目**
   ```bash
   git clone https://github.com/starknt/blive.git
   cd blive
   ```

3. **安装依赖**
   ```bash
   cargo build
   ```

4. **运行项目**
   ```bash
   cargo run --release
   ```

### 开发工具

- **代码格式化**: `cargo fmt`
- **代码检查**: `cargo clippy`
- **运行测试**: `cargo test`
- **生成文档**: `cargo doc`

## 📁 代码结构

### 目录结构

```
src/
├── main.rs              # 程序入口
├── lib.rs               # 库入口
├── app.rs               # 应用主界面
├── state.rs             # 应用状态管理
├── settings.rs          # 设置管理
├── themes.rs            # 主题管理
├── logger.rs            # 日志系统
├── title_bar.rs         # 标题栏
├── error.rs             # 错误处理
├── assets.rs            # 资源管理
├── core/                # 核心功能
│   ├── http_client.rs   # HTTP 客户端
│   ├── downloader.rs    # 下载器核心
│   ├── http_client/     # HTTP 客户端实现
│   └── downloader/      # 下载器实现
│       ├── http_stream.rs
│       └── http_hls.rs
└── components/          # UI 组件
    ├── mod.rs
    ├── room_card.rs
    ├── room_input.rs
    ├── settings_modal.rs
    └── app_settings.rs
```

### 模块依赖关系

```
main.rs
├── lib.rs
    ├── app.rs
    │   ├── state.rs
    │   ├── settings.rs
    │   ├── themes.rs
    │   └── components/
    ├── core/
    │   ├── http_client.rs
    │   └── downloader.rs
    └── logger.rs
```

## 🔧 核心模块

### 1. 应用状态管理 (state.rs)

负责全局状态管理，包括：
- 应用设置
- 房间列表
- HTTP 客户端
- 主题管理

```rust
pub struct AppState {
    pub settings: GlobalSettings,
    pub room_entities: Vec<RoomEntity>,
    pub client: HttpClient,
}
```

### 2. 设置管理 (settings.rs)

管理应用配置，包括：
- 录制质量设置
- 录制格式设置
- 录制编码设置
- 文件路径设置

```rust
pub struct GlobalSettings {
    pub strategy: Strategy,
    pub quality: Quality,
    pub format: VideoContainer,
    pub codec: StreamCodec,
    pub record_dir: String,
    pub rooms: Vec<RoomSettings>,
}
```

### 3. 下载器核心 (core/downloader.rs)

核心录制功能，包括：
- 流媒体下载
- 文件管理
- 错误处理
- 重连机制

```rust
pub struct BLiveDownloader {
    context: DownloaderContext,
    downloader: Option<DownloaderType>,
    max_reconnect_attempts: u32,
    reconnect_delay: Duration,
    is_auto_reconnect: bool,
}
```

### 4. HTTP 客户端 (core/http_client.rs)

处理与 Bilibili API 的通信：
- 房间信息获取
- 流媒体地址获取
- 用户信息获取

```rust
pub struct HttpClient {
    client: reqwest::Client,
}
```

### 5. UI 组件 (components/)

基于 GPUI 的用户界面组件：
- 房间卡片组件
- 设置模态框
- 房间输入组件
- 应用设置组件

## 📝 开发规范

### 代码风格

1. **命名规范**
   - 函数和变量使用 snake_case
   - 类型和常量使用 SCREAMING_SNAKE_CASE
   - 模块使用 snake_case

2. **注释规范**
   - 公共 API 必须有文档注释
   - 复杂逻辑需要行内注释
   - 使用中文注释

3. **错误处理**
   - 使用 `anyhow::Result` 进行错误传播
   - 定义具体的错误类型
   - 提供有意义的错误信息

### 代码示例

```rust
/// 下载器上下文
#[derive(Clone)]
pub struct DownloaderContext {
    pub entity: WeakEntity<RoomCard>,
    pub client: HttpClient,
    pub room_id: u64,
    pub quality: Quality,
    pub format: VideoContainer,
    pub codec: StreamCodec,
    stats: Arc<Mutex<DownloadStats>>,
    is_running: Arc<atomic::AtomicBool>,
    event_queue: Arc<Mutex<VecDeque<DownloadEvent>>>,
}

impl DownloaderContext {
    /// 创建新的下载器上下文
    pub fn new(
        entity: WeakEntity<RoomCard>,
        client: HttpClient,
        room_id: u64,
        quality: Quality,
        format: VideoContainer,
        codec: StreamCodec,
    ) -> Self {
        Self {
            entity,
            client,
            room_id,
            quality,
            format,
            codec,
            stats: Arc::new(Mutex::new(DownloadStats::default())),
            is_running: Arc::new(atomic::AtomicBool::new(false)),
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}
```

## 🧪 测试指南

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downloader_context_creation() {
        // 测试代码
    }
}
```

### 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    use crate::BLiveDownloader;

    #[tokio::test]
    async fn test_download_flow() {
        // 集成测试代码
    }
}
```

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_name

# 运行集成测试
cargo test --test integration_test
```

## 🤝 贡献流程

### 1. 准备工作

1. Fork 项目到您的 GitHub 账户
2. 克隆您的 Fork 到本地
3. 创建功能分支

```bash
git clone https://github.com/your-username/blive.git
cd blive
git checkout -b feature/your-feature
```

### 2. 开发流程

1. **编写代码**
   - 遵循代码规范
   - 添加必要的测试
   - 更新相关文档

2. **代码检查**
   ```bash
   cargo fmt
   cargo clippy
   cargo test
   ```

3. **提交代码**
   ```bash
   git add .
   git commit -m "feat: 添加新功能"
   git push origin feature/your-feature
   ```

### 3. 提交 Pull Request

1. 在 GitHub 上创建 Pull Request
2. 填写详细的描述
3. 等待代码审查
4. 根据反馈进行修改

### 4. 代码审查

- 确保代码符合项目规范
- 添加必要的测试
- 更新相关文档
- 处理审查意见

## 🔍 调试指南

### 日志系统

项目使用 `tracing` 进行日志记录：

```rust
use tracing::{info, warn, error, debug};

// 记录不同级别的日志
info!("录制开始: 房间 {}", room_id);
warn!("网络连接不稳定");
error!("录制失败: {}", error);
debug!("调试信息");
```

### 错误处理

使用 `anyhow` 进行错误处理：

```rust
use anyhow::{Context, Result};

pub async fn download_stream(&self) -> Result<()> {
    let response = self.client
        .get(&url)
        .await
        .context("网络请求失败")?;

    Ok(response)
}
```

### 性能分析

使用 `cargo` 内置的性能分析工具：

```bash
# 编译时优化
cargo build --release

# 运行性能分析
cargo bench
```

## 📚 学习资源

### Rust 相关

- [Rust 官方文档](https://doc.rust-lang.org/)
- [Rust 异步编程](https://rust-lang.github.io/async-book/)
- [Rust 错误处理](https://doc.rust-lang.org/book/ch09-00-error-handling.html)

### GPUI 相关

- [GPUI 文档](https://github.com/zed-industries/zed)
- [gpui-component 文档](https://github.com/longbridge/gpui-component)

### 项目相关

- [项目 Wiki](https://github.com/starknt/blive/wiki)
- [Issues](https://github.com/starknt/blive/issues)
- [Discussions](https://github.com/starknt/blive/discussions)

## 🆘 获取帮助

如果您在开发过程中遇到问题：

1. 查看项目文档
2. 搜索现有 Issues
3. 在 Discussions 中提问
4. 联系项目维护者

---

**注意**: 请确保您的贡献符合项目的代码规范和开发流程。
