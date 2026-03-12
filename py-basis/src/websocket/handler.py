"""
Message handler for WebSocket data.
"""

import time
from typing import Dict, Any, Optional, Tuple
from loguru import logger


class MessageHandler:
    """Handler for processing WebSocket messages."""

    def __init__(self):
        """Initialize message handler."""
        self.spot_prices: Dict[str, Dict[str, Any]] = {}
        self.futures_prices: Dict[str, Dict[str, Any]] = {}
        self.last_match_time: Dict[str, float] = {}

    def process_message(self, data: Dict[str, Any]) -> Optional[Tuple[Dict[str, Any], Dict[str, Any]]]:
        """
        Process a WebSocket message and match spot/futures prices.

        Args:
            data: WebSocket message data

        Returns:
            Tuple of (spot_data, futures_data) if matched, None otherwise
        """
        symbol = data["symbol"]
        market_type = data["market_type"]

        # Store price based on market type
        if market_type == "spot":
            self.spot_prices[symbol] = data
        else:  # futures
            self.futures_prices[symbol] = data

        # Check if we have both spot and futures prices
        if symbol in self.spot_prices and symbol in self.futures_prices:
            spot_data = self.spot_prices[symbol]
            futures_data = self.futures_prices[symbol]

            # Calculate time difference
            spot_time = spot_data["received_timestamp"]
            futures_time = futures_data["received_timestamp"]
            time_diff = abs(spot_time - futures_time)

            # Only process if prices are reasonably close in time (within 1 second)
            if time_diff < 1.0:
                # Normalize Binance bookTicker format (b/a -> bestBidPrice/bestAskPrice)
                for d in (spot_data, futures_data):
                    if "b" in d:
                        d["bestBidPrice"] = d["b"]
                    if "a" in d:
                        d["bestAskPrice"] = d["a"]

                # Clear stored prices to avoid reusing old data
                self.spot_prices.pop(symbol, None)
                self.futures_prices.pop(symbol, None)

                # Update last match time
                self.last_match_time[symbol] = time.time()

                return spot_data, futures_data
            elif time_diff > 5.0:
                # Prices are too stale, clear them
                logger.warning(f"Price mismatch for {symbol.upper()}: time diff {time_diff:.2f}s")
                self.spot_prices.pop(symbol, None)
                self.futures_prices.pop(symbol, None)

        return None

    def get_mid_price(self, data: Dict[str, Any]) -> float:
        """
        Calculate mid price from bookTicker data.

        Args:
            data: WebSocket message data

        Returns:
            Mid price
        """
        try:
            bid = data.get("bestBidPrice") or data.get("b")
            ask = data.get("bestAskPrice") or data.get("a")
            if bid is None or ask is None:
                raise KeyError("bestBidPrice" if bid is None else "bestAskPrice")
            return (float(bid) + float(ask)) / 2
        except (KeyError, ValueError) as e:
            logger.error(f"Error calculating mid price: {e}, data: {data}")
            return 0.0

    def cleanup_stale_prices(self, max_age: float = 10.0):
        """
        Clean up stale price data.

        Args:
            max_age: Maximum age in seconds
        """
        current_time = time.time()

        # Clean spot prices
        stale_symbols = []
        for symbol, data in self.spot_prices.items():
            if current_time - data["received_timestamp"] > max_age:
                stale_symbols.append(symbol)

        for symbol in stale_symbols:
            self.spot_prices.pop(symbol, None)
            logger.debug(f"Cleaned stale spot price for {symbol.upper()}")

        # Clean futures prices
        stale_symbols = []
        for symbol, data in self.futures_prices.items():
            if current_time - data["received_timestamp"] > max_age:
                stale_symbols.append(symbol)

        for symbol in stale_symbols:
            self.futures_prices.pop(symbol, None)
            logger.debug(f"Cleaned stale futures price for {symbol.upper()}")