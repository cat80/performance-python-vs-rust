"""
Dashboard UI for the basis monitor.
"""

import time
from datetime import datetime
from typing import Dict, Any, List
from rich.console import Console
from rich.layout import Layout
from rich.panel import Panel
from rich.table import Table
from rich.live import Live
from rich.text import Text
from rich.columns import Columns
from rich.progress import Progress, SpinnerColumn, TextColumn
from loguru import logger

from ..config import Config
from ..queue.manager import QueueManager


class Dashboard:
    """Dashboard UI for displaying system status and metrics."""

    def __init__(self, config: Config, queue_manager: QueueManager):
        """
        Initialize dashboard.

        Args:
            config: Application configuration
            queue_manager: Queue manager for statistics
        """
        self.config = config
        self.queue_manager = queue_manager
        self.console = Console()
        self.layout = Layout()

        # Performance data storage
        self.performance_history: Dict[str, List[float]] = {
            "receive_rate": [],
            "process_rate": [],
            "latency_p99": [],
        }

        # Configure layout
        self._setup_layout()

        # Start time
        self.start_time = time.time()

    def _setup_layout(self):
        """Setup the dashboard layout."""
        # Create main layout
        self.layout.split(
            Layout(name="header", size=3),
            Layout(name="main", ratio=1),
            Layout(name="footer", size=3),
        )

        # Split main area
        self.layout["main"].split_row(
            Layout(name="left", ratio=2),
            Layout(name="right", ratio=1),
        )

        # Split left area
        self.layout["left"].split(
            Layout(name="metrics", size=10),
            Layout(name="symbols", ratio=1),
        )

        # Split right area
        self.layout["right"].split(
            Layout(name="performance", ratio=1),
            Layout(name="alerts", size=6),
        )

    def _create_header(self) -> Panel:
        """Create header panel."""
        current_time = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        run_time = self._format_duration(time.time() - self.start_time)

        header_table = Table(show_header=False, box=None)
        header_table.add_column("left", justify="left")
        header_table.add_column("center", justify="center")
        header_table.add_column("right", justify="right")

        header_table.add_row(
            f"🚀 [bold cyan]Binance Basis Monitor[/bold cyan]",
            f"[bold]Python Implementation[/bold]",
            f"🕐 {current_time} | ⏱️ {run_time}"
        )

        return Panel(header_table, title="System Status", border_style="cyan")

    def _create_metrics_panel(self, stats: Dict[str, Any]) -> Panel:
        """Create metrics panel."""
        # Create metrics table
        metrics_table = Table(show_header=True, box=None)
        metrics_table.add_column("Metric", justify="left", style="cyan")
        metrics_table.add_column("Value", justify="right", style="green")
        metrics_table.add_column("Status", justify="center")

        # Queue metrics
        queue_usage = stats["queue_size"] / stats["max_size"] if stats["max_size"] > 0 else 0
        queue_status = "✅" if queue_usage < 0.7 else "⚠️" if queue_usage < 0.9 else "❌"

        metrics_table.add_row(
            "Queue Size",
            f"{stats['queue_size']:,}/{stats['max_size']:,}",
            queue_status
        )

        # Rate metrics
        receive_rate = stats["receive_rate"]
        process_rate = stats["process_rate"]
        rate_ratio = process_rate / receive_rate if receive_rate > 0 else 1
        rate_status = "✅" if rate_ratio > 0.95 else "⚠️" if rate_ratio > 0.8 else "❌"

        metrics_table.add_row(
            "Receive Rate",
            f"{receive_rate:,.0f} msg/s",
            "📥"
        )
        metrics_table.add_row(
            "Process Rate",
            f"{process_rate:,.0f} msg/s",
            rate_status
        )

        # Backlog (received - processed; negative = bug in receive counting)
        backlog = stats["backlog"]
        backlog_status = "✅" if 0 <= backlog < 100 else "⚠️" if backlog < 1000 else "❌"
        if backlog < 0:
            backlog_status = "⚠️"  # Negative = receive_rate not tracked correctly

        metrics_table.add_row(
            "Backlog",
            f"{backlog:,}",
            backlog_status
        )

        # Latency (cap display for erroneous values)
        latency_p99 = min(stats["latency_p99"] * 1000, 999999.9)  # Cap at 999999.9 ms
        latency_status = "✅" if latency_p99 < 100 else "⚠️" if latency_p99 < 500 else "❌"

        metrics_table.add_row(
            "P99 Latency (Queue)",
            f"{latency_p99:.1f} ms",
            latency_status
        )

        # E2E Latency (Binance E → processed)
        e2e_p99 = min(stats.get("latency_e2e_p99", 0) * 1000, 999999.9)
        e2e_status = "✅" if e2e_p99 < 100 else "⚠️" if e2e_p99 < 500 else "❌"

        metrics_table.add_row(
            "P99 E2E Latency",
            f"{e2e_p99:.1f} ms",
            e2e_status
        )

        return Panel(metrics_table, title="Performance Metrics", border_style="blue")

    def _create_symbols_panel(self, stats: Dict[str, Any]) -> Panel:
        """Create symbols panel."""
        symbol_stats = stats.get("symbol_stats", {})

        if not symbol_stats:
            return Panel("[italic]No symbol data yet[/italic]", title="Symbols", border_style="green")

        # Create table
        symbols_table = Table(show_header=True, box=None)
        symbols_table.add_column("Symbol", justify="left", style="yellow")
        symbols_table.add_column("Received", justify="right")
        symbols_table.add_column("Processed", justify="right")
        symbols_table.add_column("Rate", justify="right")

        # Add rows for top N symbols by activity
        sorted_symbols = sorted(
            symbol_stats.items(),
            key=lambda x: x[1]["received"],
            reverse=True
        )[:10]  # Show top 10

        for symbol, data in sorted_symbols:
            received = data.get("received", 0)
            processed = data.get("processed", 0)
            rate = processed / received if received > 0 else 0

            symbols_table.add_row(
                symbol.upper(),
                f"{received:,}",
                f"{processed:,}",
                f"{rate:.1%}"
            )

        return Panel(symbols_table, title=f"Symbols ({len(symbol_stats)} total)", border_style="green")

    def _create_performance_panel(self, stats: Dict[str, Any]) -> Panel:
        """Create performance chart panel."""
        # Simple text-based performance indicators
        text = Text()

        # CPU and memory info (simulated for now)
        text.append("📊 [bold]Performance Trends[/bold]\n\n", style="magenta")

        # Receive rate trend
        receive_rate = stats["receive_rate"]
        self.performance_history["receive_rate"].append(receive_rate)
        if len(self.performance_history["receive_rate"]) > 20:
            self.performance_history["receive_rate"].pop(0)

        text.append(f"📥 Receive Rate: [bold]{receive_rate:,.0f}[/bold] msg/s\n")

        # Process rate trend
        process_rate = stats["process_rate"]
        self.performance_history["process_rate"].append(process_rate)

        text.append(f"⚙️ Process Rate: [bold]{process_rate:,.0f}[/bold] msg/s\n")

        # Latency trend
        latency_p99 = stats["latency_p99"] * 1000
        self.performance_history["latency_p99"].append(latency_p99)

        text.append(f"⏱️ P99 Latency: [bold]{latency_p99:.1f}[/bold] ms\n\n")

        # Simple trend indicators
        if len(self.performance_history["receive_rate"]) > 5:
            recent_avg = sum(self.performance_history["receive_rate"][-5:]) / 5
            if receive_rate > recent_avg * 1.1:
                text.append("📈 Rate: [green]Increasing[/green]\n")
            elif receive_rate < recent_avg * 0.9:
                text.append("📉 Rate: [red]Decreasing[/red]\n")
            else:
                text.append("📊 Rate: [yellow]Stable[/yellow]\n")

        return Panel(text, title="Performance Trends", border_style="magenta")

    def _create_alerts_panel(self, stats: Dict[str, Any]) -> Panel:
        """Create alerts panel."""
        alerts = []

        # Check for alerts
        queue_usage = stats["queue_size"] / stats["max_size"] if stats["max_size"] > 0 else 0
        if queue_usage > 0.9:
            alerts.append(("❌", "Queue critical", f"{queue_usage:.0%} full"))
        elif queue_usage > 0.7:
            alerts.append(("⚠️", "Queue high", f"{queue_usage:.0%} full"))

        receive_rate = stats["receive_rate"]
        process_rate = stats["process_rate"]
        if receive_rate > 0:
            processing_ratio = process_rate / receive_rate
            if processing_ratio < 0.8:
                alerts.append(("⚠️", "Processing lag", f"{processing_ratio:.0%}"))
            elif processing_ratio < 0.95:
                alerts.append(("ℹ️", "Processing slow", f"{processing_ratio:.0%}"))

        latency_p99 = stats["latency_p99"] * 1000
        if latency_p99 > 500:
            alerts.append(("❌", "High latency", f"{latency_p99:.0f}ms"))
        elif latency_p99 > 100:
            alerts.append(("⚠️", "Elevated latency", f"{latency_p99:.0f}ms"))

        if stats["dropped_count"] > 0:
            alerts.append(("⚠️", "Messages dropped", f"{stats['dropped_count']:,}"))

        if not alerts:
            alerts.append(("✅", "All systems normal", ""))

        # Create alerts display
        alerts_text = Text()
        for icon, title, value in alerts:
            if value:
                alerts_text.append(f"{icon} {title}: {value}\n")
            else:
                alerts_text.append(f"{icon} {title}\n")

        return Panel(alerts_text, title="Alerts", border_style="red" if "❌" in str(alerts_text) else "yellow")

    def _create_footer(self) -> Panel:
        """Create footer panel."""
        symbols_count = len(self.config.symbols)
        test_mode = "🔄 TEST MODE" if self.config.performance_test_mode else ""

        footer_text = Text()
        footer_text.append(f"Monitoring {symbols_count} symbols", style="dim")
        if test_mode:
            footer_text.append(f" | {test_mode}", style="bold red")
        footer_text.append(f" | Window: {self.config.window_interval}s", style="dim")
        footer_text.append(f" | EMA: {self.config.ema_window}", style="dim")

        return Panel(footer_text, border_style="dim")

    def _format_duration(self, seconds: float) -> str:
        """Format duration in seconds to HH:MM:SS."""
        hours = int(seconds // 3600)
        minutes = int((seconds % 3600) // 60)
        seconds = int(seconds % 60)
        return f"{hours:02d}:{minutes:02d}:{seconds:02d}"

    async def update(self):
        """Update the dashboard with current statistics."""
        try:
            # Get current statistics
            stats_task = self.queue_manager.get_stats()
            stats = await stats_task

            # Update all panels
            self.layout["header"].update(self._create_header())
            self.layout["metrics"].update(self._create_metrics_panel(stats))
            self.layout["symbols"].update(self._create_symbols_panel(stats))
            self.layout["performance"].update(self._create_performance_panel(stats))
            self.layout["alerts"].update(self._create_alerts_panel(stats))
            self.layout["footer"].update(self._create_footer())

        except Exception as e:
            logger.error(f"Error updating dashboard: {e}")
            # Fallback to error display
            error_panel = Panel(f"[red]Error: {e}[/red]", title="Dashboard Error")
            self.layout["header"].update(error_panel)

    async def run(self):
        """Run the dashboard refresh loop."""
        try:
            with Live(self.layout, console=self.console, screen=True, refresh_per_second=4) as live:
                while True:
                    await self.update()
                    live.update(self.layout)
                    await asyncio.sleep(self.config.ui_refresh_interval)
        except KeyboardInterrupt:
            logger.info("Dashboard stopped by user")
        except Exception as e:
            logger.error(f"Dashboard error: {e}")


# Import asyncio here to avoid circular import
import asyncio