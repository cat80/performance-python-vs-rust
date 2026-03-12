"""
Combined WebSocket client for Binance - multiple symbols in one connection.
"""

import asyncio
import time
from typing import Union
import websockets
import orjson
from loguru import logger

from ..config import Config
from ..queue.manager import QueueManager


async def run_combined_client(
    url: str,
    market_type: str,
    queue_manager: QueueManager,
):
    """
    Run a combined WebSocket client (multiple symbols per connection, max 120).
    """
    reconnect_count = 0
    max_retries = 10

    while reconnect_count < max_retries:
        try:
            logger.info(f"Connecting to {market_type} combined WebSocket")
            async with websockets.connect(
                url, ping_interval=20, ping_timeout=10
            ) as websocket:
                reconnect_count = 0
                logger.info(f"{market_type} combined WebSocket connected")
                async for message in websocket:
                    try:
                        # Combined format: {"stream":"btcusdt@bookTicker","data":{...}}
                        msg = orjson.loads(message)
                        data = msg.get("data", msg)
                        stream = msg.get("stream", "")

                        # Extract symbol from stream "btcusdt@bookTicker" or data["s"]
                        symbol = (
                            stream.split("@")[0]
                            if stream
                            else data.get("s", "unknown")
                        )
                        if isinstance(symbol, str):
                            symbol = symbol.lower()

                        # Normalize b/a -> bestBidPrice/bestAskPrice
                        if "b" in data and "bestBidPrice" not in data:
                            data["bestBidPrice"] = data["b"]
                        if "a" in data and "bestAskPrice" not in data:
                            data["bestAskPrice"] = data["a"]

                        data["market_type"] = market_type
                        data["symbol"] = symbol
                        data["received_timestamp"] = time.time()

                        await queue_manager.put(data)

                    except orjson.JSONDecodeError as e:
                        logger.error(f"JSON decode error: {e}, msg: {message[:100]}")
                    except asyncio.QueueFull:
                        logger.warning(f"Queue full, dropping {market_type} message")
                    except Exception as e:
                        logger.error(f"Error processing {market_type} message: {e}")

        except websockets.exceptions.ConnectionClosed:
            logger.warning(f"{market_type} combined WebSocket connection closed")
        except Exception as e:
            logger.error(f"Error in {market_type} combined WebSocket: {e}")

        reconnect_count += 1
        if reconnect_count < max_retries:
            wait = 1 * reconnect_count
            logger.info(f"Reconnecting {market_type} in {wait}s (attempt {reconnect_count})")
            await asyncio.sleep(wait)
