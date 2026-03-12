本项目需要对比评估python,rust 在网络监听听下的性能的差别对比和评估。
通过订阅币安spot/futrue的bookTicker实计算代币的基差(futrue-spot)/spot,以及每分钟为一个间隔样本点，保存序列基差，用于窗口计算avg,和ema，和z-score。每10秒，打印一个刚处理完的symbol的最新价格（现货，期货的），基差，以及窗口下的，价格ma,基差ma,基差ema,最新的zscore。在命令行窗口的最顶方，展示websockets的msg/s 每秒收到的消息量，以及处理的消息量(每2s刷新）。为了不阻塞websocket数据，消息收到后

python项目(py-basis)，使用uv 管理包。使用的asyncio,orjson序列化，websockets连接,polars做窗口计算，命令行工作rich，loguru日志。

rust (rust-basis)项目使用 cargo 管理包,tokio网民步处理，reqwest异步 连接，serde序列化，polars做窗口计算。ratatui+ crossterm,tracing日志

共用的配置在根目录的.env下，
SYMOBLS=SOLUSDT #监听的数据
WINDOW_INTERVAL-60 #窗口
EMA_WINDOW=30 # 窗口长度