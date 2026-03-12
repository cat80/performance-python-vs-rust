币安实时基差监控与性能对比项目 (Python vs Rust)
一、 项目背景与目标
本项目旨在通过监听币安 (Binance) 现货 (Spot) 与 U本位合约 (USDT-M Futures) 的 bookTicker 实时数据流，计算目标代币的实时基差 (Basis) 及其统计学指标 (MA, EMA, Z-Score)。
核心目标：评估并量化在相同业务逻辑下，Python 和 Rust 在处理高并发 WebSocket 消息时的性能差异，包括吞吐量 (Msg/s)、延迟、CPU/内存占用及 UI 渲染对主程序的阻塞影响。

二、 系统架构与数据流设计 (解耦与非阻塞)
为了保证网络层不被计算和 UI 渲染阻塞，两个项目都必须严格采用“生产者-消费者”模型。

网络接入层 (Producer)：

独立异步任务连接 Spot 和 Futures 的 WebSocket。

仅负责接收消息、反序列化 (JSON -> Object)，并打上时间戳。

将解析后的数据推入无锁队列 / 异步 Channel。

计数器：使用原子操作 (Atomic) 记录 received_msgs。

核心计算层 (Consumer / Worker)：

从队列中消费数据，撮合现货与合约的最新价格（基于 Symbol）。

计算实时基差，并维护以 WINDOW_INTERVAL (分钟) 为单位的 K 线/时间桶 (Time Bucket) 数据。

调用 Polars 执行窗口指标 (MA, EMA, Z-Score) 计算。

计数器：记录 processed_msgs。

UI 与日志层 (Observer)：

UI 刷新任务：每 2 秒读取一次原子计数器，计算并渲染顶部的 Msg/s 面板。

日志输出任务：每 10 秒读取当前最新状态，打印滚动日志。

三、 核心业务逻辑与指标定义
数据源 (Binance WebSocket)：

现货: wss://stream.binance.com:9443/ws/<symbol>@bookTicker

合约: wss://fstream.binance.com/ws/<symbol>@bookTicker

注：采用中间价 MidPrice = (bestBidPrice + bestAskPrice) / 2 作为计算基准。

基差计算公式：

Basis = (Future_MidPrice - Spot_MidPrice) / Spot_MidPrice

时间窗口聚合 (Time Bucketing)：

每 WINDOW_INTERVAL（如 60 秒）作为一个样本点。

当前分钟内的基差可以使用加权平均或简单的快照收集，满一分钟后将该分钟的 avg_basis 存入历史序列。

统计指标计算 (使用 Polars)：

基于最近 EMA_WINDOW（如 30）个样本点的数据：

MA (Price & Basis)：简单移动平均。

EMA (Basis)：指数移动平均。

Z-Score：(当前实时基差 - MA(Basis_Window)) / StdDev(Basis_Window)。

四、 界面与日志交互规范
终端采用 TUI (Text User Interface) 混合模式：

顶部固定面板 (每 2 秒刷新)：

Plaintext
===================================================================
🚀 [系统状态] 运行时间: 00:15:30 | 监控币对: SOLUSDT
📥 接收速率: 1250 msg/s  (Spot: 600 | Futures: 650)
⚙️ 处理速率: 1245 msg/s  | 积压量(Queue): 5
===================================================================
底部滚动日志 (每 10 秒输出一次)：

格式：[时间] [级别] [Symbol] 现货价: xx, 合约价: xx | 实时基差: xx% | MA_Price: xx | MA_Basis: xx% | EMA_Basis: xx% | Z-Score: xx

五、 技术栈与实现规范
1. 共享配置 (.env)
Ini, TOML
# 目标币对，逗号分隔，需转小写拼接ws
SYMBOLS=SOLUSDT,BTCUSDT
# K线/数据桶间隔（秒）
WINDOW_INTERVAL=60
# 窗口序列长度（用于MA/EMA/Z-score计算）
EMA_WINDOW=30
2. Python 项目 (py-basis)
包管理：uv (创建隔离的 venv，极速管理依赖)

网络与异步：asyncio + websockets (负责 WS 长连接)

JSON解析：orjson (由于是 C 扩展，反序列化极快，必须用它替代标准 json)

数据结构：asyncio.Queue 处理消息传递。

计算框架：polars (将 Python 的列表数据转为 pl.Series 计算 rolling EMA 和 std)。

UI 与日志：

rich.layout.Layout + rich.live.Live 构建顶部固定面板。

loguru 处理滚动输出，拦截 rich 的标准输出以防 TUI 闪烁。

3. Rust 项目 (rust-basis)
包管理：cargo

网络与异步：tokio (使用多线程 runtime)

WebSocket：tokio-tungstenite (更原生适合纯 ws 监听) 或开启了 ws feature 的 reqwest。

JSON解析：serde + serde_json (开启 simd feature 性能更好)。

数据结构：tokio::sync::mpsc 或 crossbeam-channel 处理跨任务消息。原子计数使用 std::sync::atomic::AtomicUsize。

计算框架：polars (Rust 原生 API，使用 Series 算子)。

UI 与日志：

crossterm + ratatui 构建 TUI 面板。

tracing + tracing-subscriber 处理日志。注意： 必须使用类似 tui-logger 的库，或者将日志输出重定向到文件，避免直接向 stdout 打印冲刷掉 ratatui 的界面。

六、 性能对比评估维度 (Benchmark)
在系统稳定运行 1 小时后，对比以下指标：

最大吞吐量抗压：尝试同时订阅 50-100 个 Symbols 的数据，观察 处理速率 是否能跟上 接收速率（Queue 积压情况）。

资源开销：

内存常驻 (RSS)：Python vs Rust 的内存占用对比。

CPU 占用率：多核情况下的调度效率。

延迟毛刺 (Latency Jitter)：计算从解析出 json 的 timestamp 到完成 Polars 计算输出结果的时间差，观察 99% 分位 (P99) 延迟。

给你的开发建议：
Polars 性能陷阱：在 Rust 和 Python 中，对于每毫秒到达的高频 tick 数据，不要每收到一条就创建一个新的 Polars DataFrame/Series，开销极大。正确做法是用原生的 List/Vec 存储这个 WINDOW_INTERVAL 的数据，只在达到 60 秒或每隔 10 秒需要打印结果时，才转换成 Polars 对象进行窗口指标计算。

终端刷新率：Python 的 rich 更新过快会占用大量 CPU，确保 UI 的刷新由独立的 Timer（比如 asyncio.sleep(2)）控制，严格遵守 2 秒一刷的规定。