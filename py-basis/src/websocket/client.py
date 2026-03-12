"""
WebSocket client for Binance spot and futures markets.
"""

import asyncio
import time
from typing import Dict, Any, Union
import websockets
import orjson
from loguru import logger

from ..config import Config


class WebSocketClient:
    """WebSocket client for Binance markets."""

    def __init__(
        self,
        config: Config,
        symbol: str,
        queue: Union[asyncio.Queue, "QueueManager"],
        market_type: str = "spot",
    ):
        """
        Initialize WebSocket client.

        Args:
            config: Application configuration
            symbol: Trading symbol (e.g., "solusdt")
            queue: Message queue or QueueManager (use QueueManager for correct receive_rate)
            market_type: "spot" or "futures"
        """
        from ..queue.manager import QueueManager

        self.config = config
        self.symbol = symbol
        self.queue_manager = queue if isinstance(queue, QueueManager) else None
        self.queue = queue.queue if isinstance(queue, QueueManager) else queue
        self.market_type = market_type
        self.running = False
        self.reconnect_count = 0

        # Get appropriate WebSocket URL
        if market_type == "spot":
            self.ws_url = config.get_spot_ws_url(symbol)
        else:
            self.ws_url = config.get_futures_ws_url(symbol)

    async def connect(self):
        """Connect to WebSocket and start listening."""
        self.running = True
        while self.running and self.reconnect_count < self.config.ws_max_retries:
            try:
                logger.info(
                    f"Connecting to {self.market_type} WebSocket for {self.symbol.upper()}: {self.ws_url}"
                )
                async with websockets.connect(
                    self.ws_url, ping_interval=20, ping_timeout=10
                ) as websocket:
                    self.reconnect_count = 0
                    await self._listen(websocket)

            except websockets.exceptions.ConnectionClosed:
                logger.warning(
                    f"{self.market_type} WebSocket connection closed for {self.symbol.upper()}"
                )
            except Exception as e:
                logger.error(
                    f"Error in {self.market_type} WebSocket for {self.symbol.upper()}: {e}"
                )

            # Reconnect logic
            if self.running:
                self.reconnect_count += 1
                wait_time = self.config.ws_reconnect_interval * self.reconnect_count
                logger.info(
                    f"Reconnecting {self.market_type} for {self.symbol.upper()} in {wait_time}s (attempt {self.reconnect_count})"
                )
                await asyncio.sleep(wait_time)

    async def _listen(self, websocket):
        """Listen for messages from WebSocket."""
        async for message in websocket:
            if not self.running:
                break

            try:
                # Parse message with orjson (fast C extension)
                data = orjson.loads(message)

                # Normalize Binance bookTicker format: b/a -> bestBidPrice/bestAskPrice
                if "b" in data and "bestBidPrice" not in data:
                    data["bestBidPrice"] = data["b"]
                if "a" in data and "bestAskPrice" not in data:
                    data["bestAskPrice"] = data["a"]

                # Add metadata
                data["market_type"] = self.market_type
                data["symbol"] = self.symbol
                data["received_timestamp"] = time.time()

                # Put message: use queue_manager.put() for receive_rate/queue_entry_timestamp
                if self.queue_manager is not None:
                    await self.queue_manager.put(data)
                else:
                    data["queue_entry_timestamp"] = time.time()
                    await self.queue.put(data)

            except orjson.JSONDecodeError as e:
                logger.error(f"Failed to parse JSON message: {e}, message: {message[:100]}")
            except asyncio.QueueFull:
                logger.warning(f"Queue full, dropping {self.market_type} message for {self.symbol.upper()}")
            except Exception as e:
                logger.error(f"Error processing {self.market_type} message for {self.symbol.upper()}: {e}")

    async def stop(self):
        """Stop the WebSocket client."""
        self.running = False
        logger.info(f"Stopping {self.market_type} WebSocket client for {self.symbol.upper()}")


async def start_websocket_client(
    config: Config, symbol: str, queue, market_type: str = "spot"
):
    """
    Start a WebSocket client for the given symbol and market type.

    Args:
        config: Application configuration
        symbol: Trading symbol
        queue: Message queue
        market_type: "spot" or "futures"

    Returns:
        WebSocketClient instance
    """
    client = WebSocketClient(config, symbol, queue, market_type)
    await client.connect()
    return client