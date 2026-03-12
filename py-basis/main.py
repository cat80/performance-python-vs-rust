"""
Binance Basis Monitor - Python Implementation

Main entry point for the application.
"""

import asyncio
import signal
import sys
from typing import List
import click

from src.config import Config
from src.queue.manager import QueueManager
from src.websocket.handler import MessageHandler
from src.calculator.basis import BasisCalculator
from src.calculator.indicators import IndicatorCalculator
from src.ui.dashboard import Dashboard
from src.ui.logger import setup_logger, log_system_status, log_performance_metrics
from loguru import logger


class BasisMonitor:
    """Main application class for Binance Basis Monitor."""

    def __init__(self, config: Config):
        """
        Initialize the basis monitor.

        Args:
            config: Application configuration
        """
        self.config = config
        self.running = False

        # Setup logger
        self.logger = setup_logger(config)
        logger.info("=" * 60)
        logger.info("🚀 Starting Binance Basis Monitor - Python Implementation")
        logger.info("=" * 60)

        # Initialize components
        self.queue_manager = QueueManager(
            max_size=config.queue_max_size,
            warning_threshold=config.queue_warning_threshold,
        )

        self.message_handler = MessageHandler()
        self.basis_calculator = BasisCalculator(
            window_interval=config.window_interval
        )
        self.indicator_calculator = IndicatorCalculator(
            window_size=config.ema_window
        )

        self.dashboard = Dashboard(config, self.queue_manager)

        # WebSocket tasks
        self.ws_tasks: List[asyncio.Task] = []

        # Worker tasks
        self.worker_task: asyncio.Task = None
        self.ui_task: asyncio.Task = None
        self.logger_task: asyncio.Task = None

        # Performance monitoring
        self.last_log_time = 0

    async def start_websocket_clients(self):
        """Start combined WebSocket clients (max 120 symbols per connection)."""
        from src.websocket.combined_client import run_combined_client

        spot_urls = self.config.get_spot_combined_ws_urls()
        futures_urls = self.config.get_futures_combined_ws_urls()

        logger.info(
            f"Starting combined WebSocket: {len(spot_urls)} spot + {len(futures_urls)} futures "
            f"for {len(self.config.symbols)} symbols"
        )

        for url in spot_urls:
            task = asyncio.create_task(
                run_combined_client(url, "spot", self.queue_manager)
            )
            self.ws_tasks.append(task)

        for url in futures_urls:
            task = asyncio.create_task(
                run_combined_client(url, "futures", self.queue_manager)
            )
            self.ws_tasks.append(task)

        logger.info(f"Started {len(self.ws_tasks)} combined WebSocket connections")

    async def calculation_worker(self):
        """Worker task to process messages from queue."""
        logger.info("Starting calculation worker")

        while self.running:
            try:
                # Get message from queue
                data = await self.queue_manager.get()

                # Process message through handler
                result = self.message_handler.process_message(data)

                if result:
                    spot_data, futures_data = result
                    symbol = spot_data["symbol"]

                    # Calculate basis
                    basis_data = self.basis_calculator.calculate_basis(
                        spot_data, futures_data
                    )

                    if basis_data:
                        # Add to indicator calculator
                        self.indicator_calculator.add_data(
                            basis_data.symbol,
                            basis_data.basis,
                            basis_data.spot_price,
                        )

                        # Calculate indicators
                        indicators = self.indicator_calculator.calculate_indicators(
                            basis_data.symbol
                        )

                        # Log basis data (every 10 seconds per symbol)
                        current_time = asyncio.get_event_loop().time()
                        if current_time - self.last_log_time > 10:
                            from src.ui.logger import log_basis_data
                            log_basis_data(
                                symbol=basis_data.symbol,
                                spot_price=basis_data.spot_price,
                                futures_price=basis_data.futures_price,
                                basis=basis_data.basis,
                                ma_basis=indicators.ma_basis if indicators else None,
                                ema_basis=indicators.ema_basis if indicators else None,
                                z_score=indicators.z_score if indicators else None,
                            )
                            self.last_log_time = current_time

                # Mark task as done
                self.queue_manager.task_done()

                # Mark as processed for statistics (pass data for E2E latency from Binance E)
                symbol = data.get("symbol", "unknown")
                await self.queue_manager.mark_processed(symbol, data)

                # Cleanup stale prices periodically
                if self.queue_manager.processed_count % 1000 == 0:
                    self.message_handler.cleanup_stale_prices()

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in calculation worker: {e}")
                # Continue processing other messages

        logger.info("Calculation worker stopped")

    async def logger_worker(self):
        """Worker task for periodic logging."""
        logger.info("Starting logger worker")

        while self.running:
            try:
                await asyncio.sleep(self.config.log_output_interval)

                # Get current stats
                stats_task = self.queue_manager.get_stats()
                stats = await stats_task

                # Log performance metrics
                log_performance_metrics(
                    receive_rate=stats["receive_rate"],
                    process_rate=stats["process_rate"],
                    queue_backlog=stats["backlog"],
                    latency_p99=stats["latency_p99"],
                    latency_e2e_p99=stats.get("latency_e2e_p99", 0),
                    symbol_count=len(self.config.symbols),
                )

                # Log detailed system status every minute
                if stats["run_time"] % 60 < self.config.log_output_interval:
                    log_system_status(
                        config=self.config,
                        queue_manager=self.queue_manager,
                        basis_calculator=self.basis_calculator,
                        indicator_calculator=self.indicator_calculator,
                    )

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in logger worker: {e}")

        logger.info("Logger worker stopped")

    async def run(self):
        """Run the main application."""
        self.running = True

        # Setup signal handlers (add_signal_handler not supported on Windows)
        if sys.platform != "win32":
            loop = asyncio.get_event_loop()
            for sig in (signal.SIGINT, signal.SIGTERM):
                loop.add_signal_handler(sig, lambda: asyncio.create_task(self.stop()))

        try:
            # Start WebSocket clients
            await self.start_websocket_clients()

            # Start calculation worker
            self.worker_task = asyncio.create_task(self.calculation_worker())

            # Start logger worker
            self.logger_task = asyncio.create_task(self.logger_worker())

            # Start dashboard
            logger.info("Starting dashboard UI")
            await self.dashboard.run()

        except KeyboardInterrupt:
            logger.info("Application interrupted by user")
        except Exception as e:
            logger.error(f"Application error: {e}")
            raise
        finally:
            await self.stop()

    async def stop(self):
        """Stop the application."""
        if not self.running:
            return

        logger.info("Stopping application...")
        self.running = False

        # Stop WebSocket clients
        for task in self.ws_tasks:
            task.cancel()

        # Stop worker tasks
        if self.worker_task:
            self.worker_task.cancel()
        if self.logger_task:
            self.logger_task.cancel()

        # Wait for tasks to complete
        tasks = [t for t in self.ws_tasks if not t.done()]
        tasks.extend([t for t in [self.worker_task, self.logger_task] if t and not t.done()])

        if tasks:
            await asyncio.wait(tasks, timeout=5.0)

        logger.info("Application stopped")


@click.command()
@click.option("--symbols", help="Comma-separated list of symbols (overrides .env)")
@click.option("--window-interval", type=int, help="Time window interval in seconds (overrides .env)")
@click.option("--ema-window", type=int, help="EMA window size (overrides .env)")
@click.option("--test-mode", is_flag=True, help="Enable performance test mode")
def main(symbols, window_interval, ema_window, test_mode):
    """
    Binance Basis Monitor - Python Implementation.

    Monitor basis between Binance spot and futures markets in real-time.
    """
    # Load configuration
    config = Config.load()

    # Override with CLI options
    if symbols:
        config.symbols = [s.strip().lower() for s in symbols.split(",")]
    if window_interval:
        config.window_interval = window_interval
    if ema_window:
        config.ema_window = ema_window
    if test_mode:
        config.performance_test_mode = True

    # Create and run application
    app = BasisMonitor(config)

    try:
        asyncio.run(app.run())
    except KeyboardInterrupt:
        logger.info("Application terminated by user")
    except Exception as e:
        err_msg = str(e) or type(e).__name__
        logger.error(f"Fatal error: {err_msg}")
        sys.exit(1)


if __name__ == "__main__":
    main()
