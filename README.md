# Binance Basis Monitor - Python vs Rust Performance Comparison

[中文](#中文) | [English](#english)

## 中文

### 项目概述

本项目旨在通过监听币安(Binance)现货(Spot)与U本位合约(USDT-M Futures)的bookTicker实时数据流，计算目标代币的实时基差(Basis)及其统计学指标(MA, EMA, Z-Score)。核心目标是评估并量化在相同业务逻辑下，Python和Rust在处理高并发WebSocket消息时的性能差异。

### 项目特性

1. **实时数据采集**
   - 连接币安现货和合约WebSocket数据流
   - 支持多币对并行订阅（可配置）
   - 高性能JSON解析

2. **实时计算**
   - 实时基差计算：Basis = (Future_MidPrice - Spot_MidPrice) / Spot_MidPrice
   - 时间窗口聚合（Time Bucketing）
   - 统计指标计算（MA, EMA, Z-Score）

3. **监控展示**
   - 实时系统状态监控面板（TPS、队列积压等）
   - TUI（Text User Interface）界面
   - 详细的性能日志

4. **性能对比**
   - 相同业务逻辑下Python与Rust实现对比
   - 性能数据采集与展示

### 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                     币安实时基差监控系统                     │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    │
│  │  网络接入层  │    │  核心计算层  │    │   UI展示层   │    │
│  │ (Producer)  │───▶│ (Consumer)  │───▶│ (Observer)  │    │
│  └─────────────┘    └─────────────┘    └─────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### 技术栈

#### Python实现 (py-basis)
- **异步框架**: asyncio
- **WebSocket**: websockets
- **JSON解析**: orjson（C扩展，性能极佳）
- **计算框架**: polars
- **UI框架**: rich + loguru
- **包管理**: uv

#### Rust实现 (rust-basis)
- **异步框架**: tokio
- **WebSocket**: tokio-tungstenite
- **JSON解析**: serde_json（开启SIMD特性）
- **计算框架**: polars（Rust原生）
- **UI框架**: ratatui + tracing
- **包管理**: cargo

### 快速开始

#### 环境要求

- **Python版本**: >= 3.10（推荐3.12+）
- **Rust版本**: >= 1.70（推荐最新稳定版）
- **操作系统**: Linux/macOS/Windows (Windows推荐使用WSL2)
- **网络**: 可访问币安WebSocket API

#### 安装工具

**Python环境工具**：
```bash
# 安装uv（快速Python包管理工具）
curl -LsSf https://astral.sh/uv/install.sh | sh
```

**Rust环境工具**：
```bash
# 安装Rust（如果未安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### 配置说明

项目使用`.env`文件进行配置，主要配置项包括：

```env
# 目标币对（逗号分隔）
SYMBOLS=SOLUSDT,BTCUSDT,ETHUSDT,BNBUSDT

# 数据桶间隔（秒）
WINDOW_INTERVAL=60

# 统计窗口长度
EMA_WINDOW=30

# WebSocket配置
WS_RECONNECT_INTERVAL=5
WS_TIMEOUT=30

# 性能监控
UI_REFRESH_INTERVAL=2
LOG_OUTPUT_INTERVAL=10

# 队列配置
QUEUE_MAX_SIZE=10000
```

完整配置示例见`.env.example`文件。

#### 启动Python项目

```bash
# 方法1：使用脚本
./scripts/start-python.sh

# 方法2：手动启动
cd py-basis
uv venv
source .venv/bin/activate  # Linux/macOS
# 或 .venv\Scripts\activate  # Windows
uv pip install -e .
python main.py

# 使用CLI参数（可选）
python main.py --symbols SOLUSDT,BTCUSDT --window-interval 60 --test-mode
```

#### 启动Rust项目

```bash
# 方法1：使用脚本
./scripts/start-rust.sh

# 方法2：手动启动
cd rust-basis
cargo build --release
./target/release/rust-basis

# 使用环境变量配置
export SYMBOLS="SOLUSDT,BTCUSDT"
export WINDOW_INTERVAL=60
cargo run --release
```

#### 性能测试

```bash
# 运行性能对比测试
./scripts/run-performance-test.sh
```

### 性能评估指标

| 维度 | 具体指标 | 评估方法 |
|------|----------|----------|
| **吞吐量** | 接收速率 (msg/s) | 原子计数器统计 |
| | 处理速率 (msg/s) | 原子计数器统计 |
| | 队列积压量 | 队列长度监控 |
| **延迟** | 端到端延迟 | 时间戳差值计算 |
| | 延迟分布 | P50/P90/P99/P999 |
| | 延迟毛刺 | 标准差计算 |
| **资源占用** | CPU使用率 | 系统监控 |
| | 内存占用 (RSS) | 进程内存监控 |
| | 线程数 | 进程线程监控 |
| **稳定性** | 长时间运行 | 24小时连续测试 |
| | 网络恢复 | 模拟断连测试 |

### 项目结构

```
perform-python-vs-rust/
├── py-basis/                 # Python实现
│   ├── src/                 # 源代码
│   │   ├── config.py        # 配置管理
│   │   ├── websocket/       # WebSocket客户端
│   │   ├── calculator/      # 基差计算
│   │   ├── queue/           # 队列管理
│   │   └── ui/              # 用户界面
│   ├── main.py              # 主入口
│   └── pyproject.toml       # 项目配置
├── rust-basis/              # Rust实现
│   ├── src/                 # 源代码
│   │   ├── config/          # 配置管理
│   │   ├── websocket/       # WebSocket客户端
│   │   ├── calculator/      # 基差计算
│   │   ├── queue/           # 队列管理
│   │   ├── ui/              # 用户界面
│   │   └── metrics/         # 性能指标
│   ├── Cargo.toml           # 项目配置
│   └── src/main.rs          # 主入口
├── scripts/                 # 启动脚本
├── .env                     # 配置文件
├── .env.example             # 配置示例
├── pd-v0.1.md               # 原始需求文档
├── pd-v0.2.md               # 完整需求文档
└── README.md                # 本文档
```

### 使用说明

#### 界面操作

1. **启动应用**：按照上述步骤启动Python或Rust版本
2. **监控面板**：应用启动后会显示实时监控面板
3. **键盘操作**：
   - `q` 或 `Esc`：退出应用
   - 其他：实时刷新显示

#### 日志查看

```bash
# Python项目日志
tail -f logs/py-basis.log

# Rust项目日志
tail -f logs/rust-basis.log

# Basis数据日志（格式化的基差数据）
tail -f logs/py-basis_basis.log  # Python
tail -f logs/rust-basis_basis.log # Rust
```

#### 性能监控

```bash
# 使用htop查看资源占用
htop -p $(pgrep -f "python main.py")
htop -p $(pgrep -f "rust-basis")

# 查看网络连接
ss -tunap | grep -E "(python|rust-basis)"
```

### 性能优化建议

#### Python项目优化
1. **JSON解析**：使用`orjson`替代标准`json`模块
2. **Polars使用**：避免每条消息都创建新的DataFrame/Series
3. **异步队列**：使用`asyncio.Queue`实现无锁通信
4. **批量处理**：批量处理消息，减少上下文切换

#### Rust项目优化
1. **JSON解析**：`serde_json`开启SIMD特性
2. **通道选择**：根据场景选择`crossbeam-channel`或`tokio::sync::mpsc`
3. **内存管理**：使用原生Vec存储窗口数据，仅在需要时转换为Polars对象
4. **零拷贝**：尽可能使用引用避免数据复制

### 故障排除

#### 常见问题

1. **WebSocket连接失败**
   - 检查网络连接
   - 验证币安API可访问性
   - 检查防火墙设置

2. **内存使用过高**
   - 检查队列积压情况
   - 调整`QUEUE_MAX_SIZE`配置
   - 监控日志输出频率

3. **处理延迟增加**
   - 检查CPU使用率
   - 减少监控的币对数量
   - 调整窗口计算频率

4. **UI界面闪烁**
   - 确保日志输出不干扰TUI
   - 调整`UI_REFRESH_INTERVAL`
   - 使用单独的日志文件

#### 调试模式

```bash
# Python项目调试
LOG_LEVEL=DEBUG python main.py

# Rust项目调试
RUST_LOG=debug cargo run --release
```

### 开发指南

#### 添加新的指标计算
1. 在`calculator/indicators.py`（Python）或`calculator/indicators.rs`（Rust）中添加新指标
2. 更新UI显示逻辑
3. 更新日志输出格式

#### 扩展支持的交易所
1. 实现新的WebSocket客户端
2. 更新配置管理
3. 适配数据格式

#### 性能测试扩展
1. 修改测试配置（增加币对数量）
2. 延长测试时间
3. 添加更多性能指标

### 许可证

本项目采用MIT许可证。详见LICENSE文件。

### 贡献指南

1. Fork本仓库
2. 创建特性分支
3. 提交更改
4. 推送到分支
5. 创建Pull Request

### 联系方式

如有问题或建议，请提交Issue或通过Pull Request贡献代码。

---

## English

### Project Overview

This project aims to monitor Binance spot and USDT-M futures bookTicker real-time data streams, calculate real-time basis for target tokens, and compute statistical indicators (MA, EMA, Z-Score). The core objective is to evaluate and quantify the performance differences between Python and Rust when processing high-concurrency WebSocket messages with identical business logic.

### Features

1. **Real-time Data Collection**
   - Connect to Binance spot and futures WebSocket data streams
   - Support multiple symbol parallel subscriptions (configurable)
   - High-performance JSON parsing

2. **Real-time Calculation**
   - Real-time basis calculation: Basis = (Future_MidPrice - Spot_MidPrice) / Spot_MidPrice
   - Time window aggregation (Time Bucketing)
   - Statistical indicator calculation (MA, EMA, Z-Score)

3. **Monitoring & Display**
   - Real-time system status monitoring panel (TPS, queue backlog, etc.)
   - TUI (Text User Interface)
   - Detailed performance logging

4. **Performance Comparison**
   - Python vs Rust implementation with identical business logic
   - Performance data collection and display

### Quick Start

#### Prerequisites

- **Python version**: >= 3.10 (recommended 3.12+)
- **Rust version**: >= 1.70 (recommended latest stable)
- **Operating System**: Linux/macOS/Windows (WSL2 recommended for Windows)
- **Network**: Access to Binance WebSocket API

#### Installation

**Python Environment Tools**:
```bash
# Install uv (fast Python package manager)
curl -LsSf https://astral.sh/uv/install.sh | sh
```

**Rust Environment Tools**:
```bash
# Install Rust (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Configuration

The project uses `.env` file for configuration. Key configuration items:

```env
# Target symbols (comma-separated)
SYMBOLS=SOLUSDT,BTCUSDT,ETHUSDT,BNBUSDT

# Time window interval in seconds
WINDOW_INTERVAL=60

# EMA window length
EMA_WINDOW=30

# WebSocket configuration
WS_RECONNECT_INTERVAL=5
WS_TIMEOUT=30

# Performance monitoring
UI_REFRESH_INTERVAL=2
LOG_OUTPUT_INTERVAL=10

# Queue configuration
QUEUE_MAX_SIZE=10000
```

See `.env.example` for complete configuration example.

#### Start Python Project

```bash
# Method 1: Using script
./scripts/start-python.sh

# Method 2: Manual start
cd py-basis
uv venv
source .venv/bin/activate  # Linux/macOS
# or .venv\Scripts\activate  # Windows
uv pip install -e .
python main.py

# With CLI options (optional)
python main.py --symbols SOLUSDT,BTCUSDT --window-interval 60 --test-mode
```

#### Start Rust Project

```bash
# Method 1: Using script
./scripts/start-rust.sh

# Method 2: Manual start
cd rust-basis
cargo build --release
./target/release/rust-basis

# With environment variables
export SYMBOLS="SOLUSDT,BTCUSDT"
export WINDOW_INTERVAL=60
cargo run --release
```

#### Performance Test

```bash
# Run performance comparison test
./scripts/run-performance-test.sh
```

### Performance Evaluation Metrics

| Dimension | Specific Metrics | Evaluation Method |
|-----------|-----------------|-------------------|
| **Throughput** | Receive rate (msg/s) | Atomic counter statistics |
| | Process rate (msg/s) | Atomic counter statistics |
| | Queue backlog | Queue length monitoring |
| **Latency** | End-to-end latency | Timestamp difference calculation |
| | Latency distribution | P50/P90/P99/P999 |
| | Latency jitter | Standard deviation calculation |
| **Resource Usage** | CPU usage | System monitoring |
| | Memory usage (RSS) | Process memory monitoring |
| | Thread count | Process thread monitoring |
| **Stability** | Long-term running | 24-hour continuous test |
| | Network recovery | Simulated disconnection test |

### Project Status

✅ **Python Implementation**: Complete with full functionality
✅ **Rust Implementation**: Complete with full functionality
✅ **Performance Testing Framework**: Ready for comparison
✅ **Documentation**: Comprehensive documentation provided

### Next Steps

1. **Performance Benchmarking**: Run extended tests to collect performance data
2. **Optimization**: Fine-tune both implementations based on benchmark results
3. **Feature Extensions**: Add more indicators and monitoring capabilities
4. **Production Deployment**: Prepare for production use with enhanced reliability features

### License

This project is licensed under the MIT License. See LICENSE file for details.