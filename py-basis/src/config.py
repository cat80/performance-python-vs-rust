"""
Configuration management for the Binance basis monitor.
"""

import os
from dataclasses import dataclass
from typing import List
from dotenv import load_dotenv

load_dotenv()


@dataclass
class Config:
    """Application configuration."""

    # Target symbols (comma-separated)
    symbols: List[str]

    # Time window interval in seconds
    window_interval: int

    # EMA window length for statistics
    ema_window: int

    # WebSocket configuration
    ws_reconnect_interval: int
    ws_timeout: int
    ws_max_retries: int

    # Performance monitoring
    ui_refresh_interval: int
    log_output_interval: int
    metrics_output_interval: int

    # Queue configuration
    queue_max_size: int
    queue_warning_threshold: int

    # Logging configuration
    log_level: str
    log_file: str
    log_max_size: str
    log_backup_count: int

    # Performance test mode
    performance_test_mode: bool
    test_symbol_count: int
    test_duration_hours: int

    @classmethod
    def load(cls) -> "Config":
        """Load configuration from environment variables."""

        # Parse symbols
        symbols_str = os.getenv("SYMBOLS", "SOLUSDT,BTCUSDT")
        symbols = [s.strip().lower() for s in symbols_str.split(",")]

        # Apply test mode if enabled
        if os.getenv("PERFORMANCE_TEST_MODE", "false").lower() == "true":
            test_count = int(os.getenv("TEST_SYMBOL_COUNT", "50"))
            # Generate test symbols if needed
            if len(symbols) < test_count:
                base_symbols = symbols.copy()
                symbols = base_symbols * (test_count // len(base_symbols) + 1)
                symbols = symbols[:test_count]

        return cls(
            symbols=symbols,
            window_interval=int(os.getenv("WINDOW_INTERVAL", "60")),
            ema_window=int(os.getenv("EMA_WINDOW", "30")),
            ws_reconnect_interval=int(os.getenv("WS_RECONNECT_INTERVAL", "5")),
            ws_timeout=int(os.getenv("WS_TIMEOUT", "30")),
            ws_max_retries=int(os.getenv("WS_MAX_RETRIES", "10")),
            ui_refresh_interval=int(os.getenv("UI_REFRESH_INTERVAL", "2")),
            log_output_interval=int(os.getenv("LOG_OUTPUT_INTERVAL", "10")),
            metrics_output_interval=int(os.getenv("METRICS_OUTPUT_INTERVAL", "60")),
            queue_max_size=int(os.getenv("QUEUE_MAX_SIZE", "10000")),
            queue_warning_threshold=int(os.getenv("QUEUE_WARNING_THRESHOLD", "8000")),
            log_level=os.getenv("LOG_LEVEL", "INFO"),
            log_file=os.getenv("LOG_FILE", "logs/py-basis.log"),
            log_max_size=os.getenv("LOG_MAX_SIZE", "100MB"),
            log_backup_count=int(os.getenv("LOG_BACKUP_COUNT", "10")),
            performance_test_mode=os.getenv("PERFORMANCE_TEST_MODE", "false").lower() == "true",
            test_symbol_count=int(os.getenv("TEST_SYMBOL_COUNT", "50")),
            test_duration_hours=int(os.getenv("TEST_DURATION_HOURS", "24")),
        )

    MAX_STREAMS_PER_CONNECTION = 120

    def get_spot_ws_url(self, symbol: str) -> str:
        """Get WebSocket URL for spot market (single symbol, legacy)."""
        return f"wss://stream.binance.com:9443/ws/{symbol}@bookTicker"

    def get_futures_ws_url(self, symbol: str) -> str:
        """Get WebSocket URL for futures market (single symbol, legacy)."""
        return f"wss://fstream.binance.com/ws/{symbol}@bookTicker"

    def get_spot_combined_ws_urls(self) -> List[str]:
        """Get combined stream URLs for spot (max 120 streams per connection)."""
        urls = []
        for i in range(0, len(self.symbols), self.MAX_STREAMS_PER_CONNECTION):
            chunk = self.symbols[i : i + self.MAX_STREAMS_PER_CONNECTION]
            streams = "/".join(f"{s}@bookTicker" for s in chunk)
            urls.append(f"wss://stream.binance.com:9443/stream?streams={streams}")
        return urls

    def get_futures_combined_ws_urls(self) -> List[str]:
        """Get combined stream URLs for futures (max 120 streams per connection)."""
        urls = []
        for i in range(0, len(self.symbols), self.MAX_STREAMS_PER_CONNECTION):
            chunk = self.symbols[i : i + self.MAX_STREAMS_PER_CONNECTION]
            streams = "/".join(f"{s}@bookTicker" for s in chunk)
            urls.append(f"wss://fstream.binance.com/stream?streams={streams}")
        return urls