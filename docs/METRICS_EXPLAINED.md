# Dashboard 指标说明

## Receive Rate（接收速率）

**含义**：每秒成功放入队列的消息数量（msg/s）。

**计算**：`received_count / run_time`

- `received_count`：WebSocket 客户端成功推入队列的消息总数
- 若显示为 0，通常是因为 WebSocket 直接写入队列，未通过 QueueManager 的 `put()` 更新计数

**修复**：WebSocket 客户端应使用 `queue_manager.put()` 而非直接 `queue.put()`，以便正确统计。

---

## Backlog（积压）

**含义**：队列中尚未被处理的消息数量。

**计算**：`received_count - processed_count`

- **正常**：≥ 0，表示还有多少消息在排队
- **异常（负数）**：说明 `processed_count` 大于 `received_count`，通常是 Receive Rate 统计错误（`received_count` 未正确增加）导致

---

## P99 Latency（P99 延迟）

**含义**：消息在队列中等待时间的第 99 百分位数（99% 的消息等待时间低于该值）。

**单位**：毫秒（ms）

**计算**：从消息入队（`queue_entry_timestamp`）到出队被处理的时间差，取所有样本的第 99 百分位。

**异常值**：若出现极大数值（如 1773328688790 ms），通常是因为：
- 消息入队时未设置 `queue_entry_timestamp`，计算时使用了默认值 0
- 延迟 = 当前时间 - 0 ≈ Unix 时间戳（秒），换算成 ms 后数值巨大

**修复**：确保入队时设置 `queue_entry_timestamp`（或 `queue_entry_millis`）。

---

## P99 E2E Latency（端到端 P99 延迟）

**含义**：从 Binance 事件时间（`E`）到本地处理完成的时间差，取第 99 百分位。

**单位**：毫秒（ms）

**计算**：`now_millis - Binance_E`，其中 `E` 为 bookTicker 消息中的事件时间字段（毫秒）。

**说明**：
- 需确保本地时钟与 Binance 同步（NTP），否则会出现虚假高延迟
- 包含网络传输、队列等待、业务处理全流程
