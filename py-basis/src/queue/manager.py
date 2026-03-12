"""
Message queue management for producer-consumer pattern.
"""

import asyncio
import time
from typing import Dict, Any
from collections import defaultdict
from loguru import logger


class QueueManager:
    """Manager for message queues and performance metrics."""

    def __init__(self, max_size: int = 10000, warning_threshold: int = 8000):
        """
        Initialize queue manager.

        Args:
            max_size: Maximum queue size
            warning_threshold: Queue size threshold for warnings
        """
        self.queue = asyncio.Queue(maxsize=max_size)
        self.max_size = max_size
        self.warning_threshold = warning_threshold

        # Performance counters (atomic-like using asyncio locks)
        self.received_count = 0
        self.processed_count = 0
        self.dropped_count = 0
        self._lock = asyncio.Lock()

        # Latency tracking
        self.latencies = []
        self.max_latency_samples = 10000

        # End-to-end latency (Binance E → mark_processed)
        self.e2e_latencies = []
        self.max_e2e_samples = 10000

        # Per-symbol counters
        self.symbol_stats = defaultdict(lambda: {"received": 0, "processed": 0})

        # Start time for rate calculation
        self.start_time = time.time()

    async def put(self, data: Dict[str, Any]):
        """
        Put data into the queue.

        Args:
            data: Data to put in queue
        """
        try:
            # Add queue entry timestamp
            data["queue_entry_timestamp"] = time.time()

            # Try to put without blocking
            self.queue.put_nowait(data)

            # Update counters
            async with self._lock:
                self.received_count += 1
                symbol = data.get("symbol", "unknown")
                self.symbol_stats[symbol]["received"] += 1

            # Log warning if queue is getting full
            if self.queue.qsize() > self.warning_threshold:
                logger.warning(f"Queue approaching capacity: {self.queue.qsize()}/{self.max_size}")

        except asyncio.QueueFull:
            async with self._lock:
                self.dropped_count += 1
            logger.warning(f"Queue full, dropped message")

    async def get(self) -> Dict[str, Any]:
        """
        Get data from the queue.

        Returns:
            Data from queue
        """
        data = await self.queue.get()
        data["queue_exit_timestamp"] = time.time()

        # Calculate queue latency
        queue_latency = data["queue_exit_timestamp"] - data.get("queue_entry_timestamp", 0)
        data["queue_latency"] = queue_latency

        # Update latency tracking
        async with self._lock:
            self.latencies.append(queue_latency)
            if len(self.latencies) > self.max_latency_samples:
                self.latencies.pop(0)

        return data

    def task_done(self):
        """Mark task as done."""
        self.queue.task_done()

    async def mark_processed(self, symbol: str = "unknown", data: Dict[str, Any] = None):
        """Mark a message as processed. Record E2E latency (Binance E → processed)."""
        async with self._lock:
            self.processed_count += 1
            self.symbol_stats[symbol]["processed"] += 1

            # E2E latency: Binance E (event time ms) → now
            if data:
                binance_e = data.get("E")
                if binance_e is not None:
                    try:
                        e2e_ms = (time.time() * 1000) - float(binance_e)
                        if e2e_ms >= 0:
                            self.e2e_latencies.append(e2e_ms / 1000.0)
                            if len(self.e2e_latencies) > self.max_e2e_samples:
                                self.e2e_latencies.pop(0)
                    except (TypeError, ValueError):
                        pass

    def get_stats(self) -> Dict[str, Any]:
        """
        Get current queue statistics.

        Returns:
            Dictionary with queue statistics
        """
        current_time = time.time()
        run_time = current_time - self.start_time

        async def _get_stats():
            async with self._lock:
                # Calculate rates
                receive_rate = self.received_count / run_time if run_time > 0 else 0
                process_rate = self.processed_count / run_time if run_time > 0 else 0

                # Calculate latency percentiles (queue wait)
                if self.latencies:
                    sorted_latencies = sorted(self.latencies)
                    n = len(sorted_latencies)
                    p50 = sorted_latencies[int(n * 0.5)] if n > 0 else 0
                    p90 = sorted_latencies[int(n * 0.9)] if n > 1 else 0
                    p99 = sorted_latencies[int(n * 0.99)] if n > 2 else 0
                    p999 = sorted_latencies[int(n * 0.999)] if n > 3 else 0
                else:
                    p50 = p90 = p99 = p999 = 0

                # E2E latency percentiles (Binance E → mark_processed)
                if self.e2e_latencies:
                    sorted_e2e = sorted(self.e2e_latencies)
                    ne = len(sorted_e2e)
                    e2e_p50 = sorted_e2e[int(ne * 0.5)] if ne > 0 else 0
                    e2e_p90 = sorted_e2e[int(ne * 0.9)] if ne > 1 else 0
                    e2e_p99 = sorted_e2e[int(ne * 0.99)] if ne > 2 else 0
                    e2e_p999 = sorted_e2e[int(ne * 0.999)] if ne > 3 else 0
                else:
                    e2e_p50 = e2e_p90 = e2e_p99 = e2e_p999 = 0

                return {
                    "queue_size": self.queue.qsize(),
                    "max_size": self.max_size,
                    "received_count": self.received_count,
                    "processed_count": self.processed_count,
                    "dropped_count": self.dropped_count,
                    "receive_rate": receive_rate,
                    "process_rate": process_rate,
                    "backlog": self.received_count - self.processed_count,
                    "run_time": run_time,
                    "latency_p50": p50,
                    "latency_p90": p90,
                    "latency_p99": p99,
                    "latency_p999": p999,
                    "latency_e2e_p50": e2e_p50,
                    "latency_e2e_p90": e2e_p90,
                    "latency_e2e_p99": e2e_p99,
                    "latency_e2e_p999": e2e_p999,
                    "symbol_stats": dict(self.symbol_stats),
                }

        # Since this is called from async context, we need to run it
        import asyncio
        return asyncio.create_task(_get_stats())

    def reset_stats(self):
        """Reset performance statistics."""
        async def _reset():
            async with self._lock:
                self.received_count = 0
                self.processed_count = 0
                self.dropped_count = 0
                self.latencies.clear()
                self.e2e_latencies.clear()
                self.symbol_stats.clear()
                self.start_time = time.time()

        return asyncio.create_task(_reset())