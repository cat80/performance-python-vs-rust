# 币安实时基差监控与性能对比项目 (Python vs Rust) v0.2

## 版本历史
| 版本 | 日期 | 作者 | 变更说明 |
|------|------|------|----------|
| v0.1 | 2026-03-12 | 原始作者 | 初始需求文档，包含基本架构和功能需求 |
| v0.2 | 2026-03-12 | Claude Code | 完整需求文档，增加详细设计、技术实现、测试策略、部署指南、时间预估等 |

## 目录
- [一、项目概述](#一-项目概述)
- [二、需求分析](#二-需求分析)
- [三、系统架构](#三-系统架构)
- [四、数据流设计](#四-数据流设计)
- [五、技术实现](#五-技术实现)
- [六、性能评估方案](#六-性能评估方案)
- [七、部署与运行](#七-部署与运行)
- [八、测试策略](#八-测试策略)
- [九、风险评估与缓解](#九-风险评估与缓解)
- [十、时间预估与里程碑](#十-时间预估与里程碑)
- [十一、附录](#十一-附录)

## 一、 项目概述

### 1.1 项目背景
随着加密货币市场的快速发展，量化交易对实时数据处理性能的要求日益提高。本项目旨在通过监听币安(Binance)现货(Spot)与U本位合约(USDT-M Futures)的bookTicker实时数据流，计算目标代币的实时基差(Basis)及其统计学指标(MA, EMA, Z-Score)，为量化策略提供实时数据支持。

### 1.2 核心目标
评估并量化在相同业务逻辑下，Python和Rust在处理高并发WebSocket消息时的性能差异，包括：
- 吞吐量 (Msg/s)：消息处理能力
- 延迟：从数据接收到处理完成的时间
- CPU/内存占用：资源消耗效率
- UI渲染对主程序的阻塞影响：界面刷新对核心业务的影响

### 1.3 项目价值
1. **技术选型参考**：为高频数据处理场景提供Python与Rust的性能对比数据
2. **架构实践**：实现解耦、非阻塞的生产者-消费者模型
3. **工程实践**：展示两种语言在相同业务需求下的不同实现方案

## 二、 需求分析

### 2.1 功能需求
1. **数据采集**：
   - 同时连接币安现货和合约WebSocket数据流
   - 支持多币对并行订阅（可配置）
   - 数据解析与时间戳打点

2. **实时计算**：
   - 实时计算现货与合约中间价(MidPrice)
   - 计算实时基差：Basis = (Future_MidPrice - Spot_MidPrice) / Spot_MidPrice
   - 时间窗口聚合（Time Bucketing）
   - 统计指标计算（MA, EMA, Z-Score）

3. **监控展示**：
   - 实时系统状态监控面板（TPS、队列积压等）
   - 滚动日志输出
   - 支持TUI（Text User Interface）界面

4. **性能对比**：
   - 相同业务逻辑下Python与Rust实现对比
   - 性能数据采集与展示

### 2.2 非功能需求
1. **性能要求**：
   - 单币对处理延迟P99 < 100ms
   - 支持50-100个币对同时订阅
   - 内存占用稳定，无内存泄漏

2. **可靠性要求**：
   - 7x24小时稳定运行
   - 网络断连自动重连
   - 异常情况优雅降级

3. **可维护性**：
   - 代码结构清晰，模块化设计
   - 配置化参数管理
   - 完整的日志记录

## 三、 系统架构

### 3.1 架构设计原则
1. **解耦设计**：网络层、计算层、展示层分离
2. **非阻塞**：各层之间使用异步队列通信
3. **可扩展**：支持水平扩展（多币对、多指标）

### 3.2 系统架构图
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

### 3.3 组件详细设计

#### 3.3.1 网络接入层 (Producer)
- **职责**：
  - 连接币安WebSocket API
  - 接收bookTicker消息
  - JSON反序列化
  - 添加接收时间戳
  - 推送至消息队列

- **数据源URL**：
  - 现货: `wss://stream.binance.com:9443/ws/<symbol>@bookTicker`
  - 合约: `wss://fstream.binance.com/ws/<symbol>@bookTicker`
  - 注：`<symbol>`需小写，如`solusdt`

- **技术实现**：
  - Python: asyncio + websockets + orjson
  - Rust: tokio + tokio-tungstenite + serde_json

#### 3.3.2 核心计算层 (Consumer/Worker)
- **职责**：
  - 从队列消费数据
  - 现货与合约价格撮合
  - 实时基差计算
  - 时间窗口聚合
  - 统计指标计算（Polars）

- **技术实现**：
  - Python: asyncio.Queue + polars
  - Rust: tokio::sync::mpsc + polars

#### 3.3.3 UI与日志层 (Observer)
- **职责**：
  - 系统状态监控面板
  - 实时数据展示
  - 日志记录与输出

- **技术实现**：
  - Python: rich + loguru
  - Rust: ratatui + tracing

## 四、 数据流设计

### 4.1 数据流图
```
币安WS ──▶ 网络层 ──▶ 消息队列 ──▶ 计算层 ──▶ 状态存储 ──▶ UI层
   │          │           │           │           │         │
   │          │           │           │           │         │
   │          │           │           │           │         │
   ▼          ▼           ▼           ▼           ▼         ▼
 接收计数   解析计数    队列长度    处理计数    指标数据   展示输出
```

### 4.2 消息格式
```json
{
  "symbol": "SOLUSDT",
  "best_bid_price": "123.45",
  "best_bid_qty": "12.3",
  "best_ask_price": "123.46",
  "best_ask_qty": "15.6",
  "timestamp": 1741766400000
}
```

### 4.3 计算流程
1. **价格计算**：
   - MidPrice = (best_bid_price + best_ask_price) / 2

2. **基差计算**：
   - Basis = (Future_MidPrice - Spot_MidPrice) / Spot_MidPrice

3. **时间聚合**：
   - 每WINDOW_INTERVAL秒聚合一次
   - 计算窗口内平均基差

4. **统计指标**：
   - MA（移动平均）
   - EMA（指数移动平均）
   - Z-Score = (当前基差 - MA) / StdDev

## 五、 技术实现

### 5.1 共享配置
配置文件：`.env`
```env
# 目标币对，逗号分隔，需转小写拼接ws
SYMBOLS=SOLUSDT,BTCUSDT,ETHUSDT

# K线/数据桶间隔（秒）
WINDOW_INTERVAL=60

# 窗口序列长度（用于MA/EMA/Z-score计算）
EMA_WINDOW=30

# WebSocket连接参数
WS_RECONNECT_INTERVAL=5
WS_TIMEOUT=30

# 性能监控参数
UI_REFRESH_INTERVAL=2
LOG_OUTPUT_INTERVAL=10

# 队列配置
QUEUE_MAX_SIZE=10000
```

### 5.2 Python实现 (py-basis)

#### 5.2.1 项目结构
```
py-basis/
├── pyproject.toml          # 项目配置
├── main.py                 # 主入口
├── src/
│   ├── __init__.py
│   ├── config.py           # 配置管理
│   ├── websocket/
│   │   ├── __init__.py
│   │   ├── client.py       # WebSocket客户端
│   │   └── handler.py      # 消息处理器
│   ├── calculator/
│   │   ├── __init__.py
│   │   ├── basis.py        # 基差计算
│   │   └── indicators.py   # 指标计算
│   ├── queue/
│   │   ├── __init__.py
│   │   └── manager.py      # 队列管理
│   └── ui/
│       ├── __init__.py
│       ├── dashboard.py    # 仪表板
│       └── logger.py       # 日志
└── tests/                  # 测试
```

#### 5.2.2 依赖配置
```toml
[project]
name = "py-basis"
version = "0.1.0"
description = "Binance Basis Monitor - Python Implementation"
requires-python = ">=3.10"

dependencies = [
    "websockets>=12.0",
    "orjson>=3.9.10",
    "polars>=0.20.0",
    "rich>=13.7.0",
    "loguru>=0.7.2",
    "python-dotenv>=1.0.0",
    "click>=8.1.0",  # CLI支持
]

[project.optional-dependencies]
dev = [
    "pytest>=7.4.0",
    "pytest-asyncio>=0.21.0",
    "black>=23.0.0",
    "mypy>=1.6.0",
]
```

#### 5.2.3 核心实现要点
1. **异步架构**：
   ```python
   async def main():
       # 1. 初始化配置
       config = load_config()

       # 2. 创建消息队列
       queue = asyncio.Queue(maxsize=config.queue_max_size)

       # 3. 启动WebSocket客户端
       ws_tasks = []
       for symbol in config.symbols:
           task = asyncio.create_task(
               websocket_client(symbol, queue)
           )
           ws_tasks.append(task)

       # 4. 启动计算worker
       worker_task = asyncio.create_task(
           calculation_worker(queue)
       )

       # 5. 启动UI刷新
       ui_task = asyncio.create_task(
           ui_dashboard()
       )

       # 6. 等待所有任务
       await asyncio.gather(*ws_tasks, worker_task, ui_task)
   ```

2. **性能优化**：
   - 使用`orjson`替代标准`json`模块（C扩展，性能极佳）
   - 批量处理消息，减少Polars对象创建
   - 使用`asyncio.Queue`实现无锁通信
   - **Polars性能陷阱避免**：对于高频tick数据，不要每条消息都创建新的DataFrame/Series。应使用原生List存储窗口数据，仅在需要计算时转换为Polars对象。

### 5.3 Rust实现 (rust-basis)

#### 5.3.1 项目结构
```
rust-basis/
├── Cargo.toml              # 项目配置
├── src/
│   ├── main.rs             # 主入口
│   ├── config/
│   │   ├── mod.rs
│   │   └── settings.rs     # 配置管理
│   ├── websocket/
│   │   ├── mod.rs
│   │   ├── client.rs       # WebSocket客户端
│   │   └── handler.rs      # 消息处理器
│   ├── calculator/
│   │   ├── mod.rs
│   │   ├── basis.rs        # 基差计算
│   │   └── indicators.rs   # 指标计算
│   ├── queue/
│   │   ├── mod.rs
│   │   └── manager.rs      # 队列管理
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── dashboard.rs    # 仪表板
│   │   └── logger.rs       # 日志
│   └── metrics/
│       ├── mod.rs
│       └── collector.rs    # 性能指标收集
└── tests/                  # 测试
```

#### 5.3.2 依赖配置
```toml
[package]
name = "rust-basis"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.0", features = ["full"] }
tokio-tungstenite = "0.20"
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0", features = ["simd"] }
polars = { version = "0.36.0", features = ["lazy", "temporal", "strings"] }
ratatui = "0.25"
tracing = "0.1"
tracing-subscriber = "0.3"
dotenvy = "0.15"
crossbeam-channel = "0.5"
atomic-counter = "1.0"

[dev-dependencies]
criterion = "0.5"
tokio-test = "0.4"
```

#### 5.3.3 核心实现要点
1. **异步架构**：
   ```rust
   #[tokio::main]
   async fn main() -> Result<()> {
       // 1. 初始化配置
       let config = Config::load()?;

       // 2. 创建消息通道
       let (tx, rx) = crossbeam_channel::unbounded();

       // 3. 启动WebSocket客户端
       let mut ws_handles = Vec::new();
       for symbol in &config.symbols {
           let tx_clone = tx.clone();
           let handle = tokio::spawn(async move {
               websocket_client(symbol, tx_clone).await
           });
           ws_handles.push(handle);
       }

       // 4. 启动计算worker
       let worker_handle = tokio::spawn(async move {
           calculation_worker(rx).await
       });

       // 5. 启动UI
       let ui_handle = tokio::spawn(async move {
           ui_dashboard().await
       });

       // 6. 等待所有任务
       for handle in ws_handles {
           handle.await??;
       }
       worker_handle.await??;
       ui_handle.await??;

       Ok(())
   }
   ```

2. **性能优化**：
   - 使用`crossbeam-channel`实现高效跨线程通信
   - `serde_json`开启SIMD特性加速JSON解析
   - 使用原子计数器记录性能指标
   - **Polars性能陷阱避免**：对于高频tick数据，不要每条消息都创建新的DataFrame/Series。应使用原生Vec存储窗口数据，仅在需要计算时转换为Polars对象。

## 六、 性能评估方案

### 6.1 评估指标与对比维度
| 维度 | 具体指标 | 评估方法 | 目标值 |
|------|----------|----------|--------|
| **吞吐量** | 接收速率 (msg/s) | 原子计数器统计 | Python vs Rust 对比 |
| | 处理速率 (msg/s) | 原子计数器统计 | 处理速率应接近接收速率 |
| | 队列积压量 | 队列长度监控 | 积压 < 1000（正常） |
| **延迟** | 端到端延迟 | 时间戳差值计算 | P99 < 100ms |
| | 延迟分布 | P50/P90/P99/P999 | 对比延迟曲线 |
| | 延迟毛刺 | 标准差计算 | 稳定性分析 |
| **资源占用** | CPU使用率 | 系统监控 | 多核利用率对比 |
| | 内存占用 (RSS) | 进程内存监控 | 常驻内存对比 |
| | 线程数 | 进程线程监控 | 并发能力评估 |
| **稳定性** | 长时间运行 | 24小时连续测试 | 无崩溃、无内存泄漏 |
| | 网络恢复 | 模拟断连测试 | 自动重连时间 < 10s |
| | 错误处理 | 异常注入测试 | 优雅降级能力 |

### 6.2 测试场景
1. **基准测试**：
   - 单币对，正常负载
   - 运行1小时，采集性能数据

2. **压力测试**：
   - 50-100个币对同时订阅
   - 运行2小时，观察系统表现

3. **稳定性测试**：
   - 7x24小时连续运行
   - 模拟网络异常（断连、重连）

### 6.3 数据采集
```python
# Python性能数据采集点
performance_metrics = {
    "timestamp": datetime.now(),
    "received_msgs": atomic_counter.get(),
    "processed_msgs": worker_counter.get(),
    "queue_backlog": queue.qsize(),
    "cpu_percent": psutil.cpu_percent(),
    "memory_mb": psutil.Process().memory_info().rss / 1024 / 1024,
    "latency_p99": latency_collector.get_p99(),
}
```

```rust
// Rust性能数据采集点
struct PerformanceMetrics {
    timestamp: DateTime<Utc>,
    received_msgs: usize,
    processed_msgs: usize,
    queue_backlog: usize,
    cpu_percent: f32,
    memory_mb: f32,
    latency_p99: Duration,
}
```

## 七、 部署与运行

### 7.1 环境要求与前置准备

#### 7.1.1 基础环境
- **Python版本**: >= 3.10（推荐3.12+）
- **Rust版本**: >= 1.70（推荐最新稳定版）
- **操作系统**: Linux/macOS/Windows (Windows推荐使用WSL2)
- **网络**: 可访问币安WebSocket API（需要稳定的网络连接）

#### 7.1.2 工具安装
**Python环境工具**：
```bash
# 安装uv（快速Python包管理工具）
# Linux/macOS
curl -LsSf https://astral.sh/uv/install.sh | sh

# Windows (PowerShell)
powershell -c "irm https://astral.sh/uv/install.ps1 | iex"
```

**Rust环境工具**：
```bash
# 安装Rust（如果未安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 验证安装
rustc --version
cargo --version
```

#### 7.1.3 项目准备
```bash
# 克隆项目（如果未克隆）
git clone <项目地址>
cd perform-python-vs-rust

# 检查配置文件
cp .env.example .env  # 如果存在示例配置文件
# 编辑.env文件配置相关参数
```

### 7.2 启动方式

#### Python项目启动
```bash
# 1. 进入项目目录
cd py-basis

# 2. 创建虚拟环境（使用uv）
uv venv
source .venv/bin/activate  # Linux/macOS
# 或 .venv\Scripts\activate  # Windows

# 3. 安装依赖
uv pip install -e .

# 4. 运行程序
python main.py

# 5. 使用CLI参数（可选）
python main.py --symbols SOLUSDT,BTCUSDT --window-interval 60
```

#### Rust项目启动
```bash
# 1. 进入项目目录
cd rust-basis

# 2. 编译（开发模式）
cargo build

# 3. 运行程序
cargo run

# 4. 发布模式运行（性能测试）
cargo build --release
./target/release/rust-basis

# 5. 使用环境变量配置
export SYMBOLS="SOLUSDT,BTCUSDT"
export WINDOW_INTERVAL=60
cargo run
```

### 7.3 监控与运维
1. **日志查看**：
   ```bash
   # Python项目日志
   tail -f logs/py-basis.log

   # Rust项目日志
   tail -f logs/rust-basis.log
   ```

2. **性能监控**：
   ```bash
   # 使用htop查看资源占用
   htop -p $(pgrep -f "python main.py")
   htop -p $(pgrep -f "rust-basis")
   ```

3. **进程管理**：
   ```bash
   # 启动脚本
   ./scripts/start.sh

   # 停止脚本
   ./scripts/stop.sh

   # 重启脚本
   ./scripts/restart.sh
   ```

## 八、 测试策略

### 8.1 单元测试
- WebSocket连接与断开
- 消息解析与验证
- 基差计算逻辑
- 指标计算正确性

### 8.2 集成测试
- 端到端数据流测试
- 多币对并发测试
- 队列满负荷测试

### 8.3 性能测试
- 基准性能测试
- 压力测试
- 长时间稳定性测试

### 8.4 测试工具
- Python: pytest + pytest-asyncio
- Rust: cargo test + criterion

## 九、 风险评估与缓解

### 9.1 技术风险
1. **网络不稳定**：
   - 风险：WebSocket连接断开导致数据丢失
   - 缓解：实现自动重连机制，缓冲队列

2. **性能瓶颈**：
   - 风险：高并发下处理延迟增加
   - 缓解：优化算法，增加worker数量

3. **内存泄漏**：
   - 风险：长时间运行内存持续增长
   - 缓解：定期内存检查，使用内存分析工具

### 9.2 项目风险
1. **进度风险**：
   - 风险：技术难点导致开发延期
   - 缓解：分阶段开发，先实现核心功能

2. **对比结果不显著**：
   - 风险：Python与Rust性能差异不大
   - 缓解：设计更复杂的计算场景，增加压力测试

## 十、 时间预估与里程碑

### 10.1 开发阶段（总时长：4-6周）

#### 第一阶段：基础框架搭建（1-2周）
- 项目初始化与配置管理
- WebSocket客户端实现
- 消息队列基础功能
- 基础计算模块

#### 第二阶段：核心功能实现（1-2周）
- 实时基差计算
- 时间窗口聚合
- 统计指标计算
- 基础UI展示

#### 第三阶段：性能优化与测试（1-2周）
- 性能优化调整
- 单元测试与集成测试
- 性能对比测试
- 文档完善

### 10.2 里程碑
- **M1**（第1周）：基础框架完成，单币对数据流通
- **M2**（第2周）：核心计算功能完成，基础UI展示
- **M3**（第3周）：多币对支持，性能优化
- **M4**（第4周）：完整测试，性能对比报告

### 10.3 交付物
1. **代码仓库**：
   - Python实现完整代码
   - Rust实现完整代码
   - 测试代码与性能测试脚本

2. **文档**：
   - 需求文档（本文档）
   - 技术设计文档
   - API接口文档
   - 部署运维文档
   - 性能对比报告

3. **数据**：
   - 性能测试原始数据
   - 性能对比图表
   - 测试日志

## 十一、 附录

### 11.1 参考资料
1. 币安WebSocket API文档：https://binance-docs.github.io/apidocs/spot/cn/#websocket
2. Python asyncio文档：https://docs.python.org/3/library/asyncio.html
3. Rust tokio文档：https://tokio.rs/
4. Polars文档：https://pola-rs.github.io/polars-book/

### 11.2 术语表
- **基差(Basis)**：期货价格与现货价格之差
- **中间价(MidPrice)**：(买一价 + 卖一价) / 2
- **时间窗口(Time Window)**：固定时间间隔的数据聚合
- **Z-Score**：数据点偏离均值的标准差倍数

### 11.3 配置示例
完整配置文件示例：
```env
# ======================
# 币安实时基差监控配置
# ======================

# 目标币对（逗号分隔）
SYMBOLS=SOLUSDT,BTCUSDT,ETHUSDT,BNBUSDT

# 数据桶间隔（秒）
WINDOW_INTERVAL=60

# 统计窗口长度
EMA_WINDOW=30

# WebSocket配置
WS_RECONNECT_INTERVAL=5
WS_TIMEOUT=30
WS_MAX_RETRIES=10

# 队列配置
QUEUE_MAX_SIZE=10000
QUEUE_WARNING_THRESHOLD=8000

# 性能监控
UI_REFRESH_INTERVAL=2
LOG_OUTPUT_INTERVAL=10
METRICS_OUTPUT_INTERVAL=60

# 日志级别
LOG_LEVEL=INFO
LOG_FILE=logs/basis_monitor.log
LOG_MAX_SIZE=100MB
LOG_BACKUP_COUNT=10

# 性能测试模式（true/false）
PERFORMANCE_TEST_MODE=false
TEST_SYMBOL_COUNT=50
TEST_DURATION_HOURS=24
```

---

**文档版本**: v0.2
**创建日期**: 2026-03-12
**最后更新**: 2026-03-12
**作者**: Claude Code
**状态**: 草案 (Draft)