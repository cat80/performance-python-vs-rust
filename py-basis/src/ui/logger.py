"""
Logging configuration for the basis monitor.
"""

import sys
import os
from pathlib import Path
from loguru import logger
from rich.console import Console
from rich.text import Text


class RichInterceptHandler:
    """Intercept Loguru logs and send them to Rich console."""

    def __init__(self, console: Console):
        self.console = console

    def write(self, message):
        """Write message to Rich console."""
        # Remove loguru's formatting and create Rich Text
        text = Text.from_ansi(message.rstrip())
        self.console.print(text)

    def flush(self):
        """Flush is a no-op for Rich console."""
        pass


def setup_logger(config):
    """
    Setup Loguru logger with configuration.

    Args:
        config: Application configuration

    Returns:
        Configured logger
    """
    # Remove default logger
    logger.remove()

    # Create log directory if it doesn't exist
    log_file = config.log_file
    log_dir = os.path.dirname(log_file)
    if log_dir:
        Path(log_dir).mkdir(parents=True, exist_ok=True)

    # File logging configuration
    logger.add(
        log_file,
        rotation=config.log_max_size,
        retention=f"{config.log_backup_count} days",
        level=config.log_level.upper(),
        format="{time:YYYY-MM-DD HH:mm:ss.SSS} | {level: <8} | {name}:{function}:{line} - {message}",
        enqueue=True,  # Async logging
        backtrace=True,
        diagnose=True,
    )

    # Console logging configuration (with Rich interception)
    # We'll intercept stdout to prevent TUI flickering
    # In practice, we might want to redirect logs to a separate console or file
    # For now, we'll keep minimal console output

    # Only log WARNING and above to console to avoid TUI interference
    logger.add(
        sys.stderr,
        level="WARNING",
        format="[dim]{time:HH:mm:ss}[/dim] | {level: <8} | {message}",
        colorize=True,
    )

    # Custom logging format for basis data
    def basis_log_formatter(record):
        """Custom formatter for basis data logs."""
        if "basis_data" in record["extra"]:
            data = record["extra"]["basis_data"]
            return (
                f"{record['time'].strftime('%H:%M:%S')} | "
                f"{data['symbol']} | "
                f"现货: {data['spot_price']:.4f}, "
                f"合约: {data['futures_price']:.4f} | "
                f"基差: {data['basis']:.4%} | "
                f"MA: {data.get('ma_basis', 0):.4%} | "
                f"EMA: {data.get('ema_basis', 0):.4%} | "
                f"Z-Score: {data.get('z_score', 0):.2f}"
            )
        return None

    # Add basis data logger (separate file for clean basis data)
    basis_log_file = log_file.replace(".log", "_basis.log")
    logger.add(
        basis_log_file,
        rotation=config.log_max_size,
        retention=f"{config.log_backup_count} days",
        level="INFO",
        format=basis_log_formatter,
        filter=lambda record: "basis_data" in record["extra"],
    )

    return logger


def log_basis_data(
    symbol: str,
    spot_price: float,
    futures_price: float,
    basis: float,
    ma_basis: float = None,
    ema_basis: float = None,
    z_score: float = None,
):
    """
    Log basis data in a structured format.

    Args:
        symbol: Trading symbol
        spot_price: Spot mid price
        futures_price: Futures mid price
        basis: Basis value
        ma_basis: Moving average of basis (optional)
        ema_basis: Exponential moving average of basis (optional)
        z_score: Z-Score of basis (optional)
    """
    basis_data = {
        "symbol": symbol.upper(),
        "spot_price": spot_price,
        "futures_price": futures_price,
        "basis": basis,
    }

    if ma_basis is not None:
        basis_data["ma_basis"] = ma_basis
    if ema_basis is not None:
        basis_data["ema_basis"] = ema_basis
    if z_score is not None:
        basis_data["z_score"] = z_score

    # Log with custom extra data
    logger.bind(basis_data=basis_data).info("Basis data")


def log_performance_metrics(
    receive_rate: float,
    process_rate: float,
    queue_backlog: int,
    latency_p99: float,
    symbol_count: int,
    latency_e2e_p99: float = 0,
):
    """
    Log performance metrics.

    Args:
        receive_rate: Message receive rate (msg/s)
        process_rate: Message process rate (msg/s)
        queue_backlog: Queue backlog count
        latency_p99: P99 queue latency in seconds
        symbol_count: Number of symbols being monitored
        latency_e2e_p99: P99 end-to-end latency in seconds (Binance E → processed)
    """
    logger.info(
        f"Performance | "
        f"Symbols: {symbol_count} | "
        f"Receive: {receive_rate:.0f} msg/s | "
        f"Process: {process_rate:.0f} msg/s | "
        f"Backlog: {queue_backlog} | "
        f"P99 Queue: {latency_p99*1000:.1f}ms | "
        f"P99 E2E: {latency_e2e_p99*1000:.1f}ms"
    )


def log_system_status(
    config,
    queue_manager,
    basis_calculator=None,
    indicator_calculator=None,
):
    """
    Log comprehensive system status.

    Args:
        config: Application configuration
        queue_manager: Queue manager instance
        basis_calculator: Basis calculator instance (optional)
        indicator_calculator: Indicator calculator instance (optional)
    """
    import asyncio
    import time

    async def _get_status():
        # Get queue stats
        stats_task = queue_manager.get_stats()
        stats = await stats_task

        # Build status message
        status_lines = [
            "=" * 60,
            "SYSTEM STATUS REPORT",
            "=" * 60,
            f"Time: {time.strftime('%Y-%m-%d %H:%M:%S')}",
            f"Run Time: {stats['run_time']:.0f}s",
            f"Symbols: {len(config.symbols)} ({'TEST MODE' if config.performance_test_mode else 'NORMAL'})",
            "",
            "[QUEUE STATS]",
            f"  Received: {stats['received_count']:,}",
            f"  Processed: {stats['processed_count']:,}",
            f"  Dropped: {stats['dropped_count']:,}",
            f"  Queue Size: {stats['queue_size']:,}/{stats['max_size']:,}",
            f"  Receive Rate: {stats['receive_rate']:.0f} msg/s",
            f"  Process Rate: {stats['process_rate']:.0f} msg/s",
            f"  Backlog: {stats['backlog']:,}",
            "",
            "[LATENCY]",
            f"  P50: {stats['latency_p50']*1000:.1f}ms",
            f"  P90: {stats['latency_p90']*1000:.1f}ms",
            f"  P99: {stats['latency_p99']*1000:.1f}ms",
            f"  P999: {stats['latency_p999']*1000:.1f}ms",
            "",
            "[E2E LATENCY] (Binance E → processed)",
            f"  P50: {stats.get('latency_e2e_p50', 0)*1000:.1f}ms",
            f"  P90: {stats.get('latency_e2e_p90', 0)*1000:.1f}ms",
            f"  P99: {stats.get('latency_e2e_p99', 0)*1000:.1f}ms",
            f"  P999: {stats.get('latency_e2e_p999', 0)*1000:.1f}ms",
        ]

        # Add basis calculator info if available
        if basis_calculator:
            status_lines.extend([
                "",
                "[BASIS CALCULATOR]",
                f"  Window Interval: {config.window_interval}s",
                f"  Symbols with data: {len(basis_calculator.basis_history)}",
            ])

        # Add indicator calculator info if available
        if indicator_calculator:
            status_lines.extend([
                "",
                "[INDICATOR CALCULATOR]",
                f"  Window Size: {config.ema_window}",
                f"  Symbols with indicators: {len(indicator_calculator.basis_history)}",
            ])

        status_lines.extend([
            "",
            "=" * 60,
        ])

        # Log each line
        for line in status_lines:
            logger.info(line)

    # Run async function
    asyncio.create_task(_get_status())