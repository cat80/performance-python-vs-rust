"""
Basis calculation for spot and futures prices.
"""

import time
from typing import Dict, Any, List, Optional, Tuple
from dataclasses import dataclass
from collections import defaultdict
from loguru import logger


@dataclass
class BasisData:
    """Data class for basis calculation results."""
    symbol: str
    timestamp: float
    spot_price: float
    futures_price: float
    basis: float  # (future - spot) / spot
    spot_mid: float
    futures_mid: float


class BasisCalculator:
    """Calculator for basis between spot and futures prices."""

    def __init__(self, window_interval: int = 60):
        """
        Initialize basis calculator.

        Args:
            window_interval: Time window interval in seconds
        """
        self.window_interval = window_interval

        # Store basis data for time window aggregation
        self.basis_history: Dict[str, List[BasisData]] = defaultdict(list)

        # Current time window data
        self.window_data: Dict[str, List[BasisData]] = defaultdict(list)
        self.window_start_time: Dict[str, float] = {}

    def calculate_basis(
        self,
        spot_data: Dict[str, Any],
        futures_data: Dict[str, Any]
    ) -> Optional[BasisData]:
        """
        Calculate basis from spot and futures data.

        Args:
            spot_data: Spot market data
            futures_data: Futures market data

        Returns:
            BasisData if calculation successful, None otherwise
        """
        try:
            symbol = spot_data["symbol"].upper()
            timestamp = time.time()

            # Get bid/ask (Binance bookTicker uses b/a, some APIs use bestBidPrice/bestAskPrice)
            def _get_bid_ask(data: Dict[str, Any]) -> Optional[Tuple[float, float]]:
                bid = data.get("bestBidPrice") or data.get("b")
                ask = data.get("bestAskPrice") or data.get("a")
                if bid is None or ask is None:
                    return None
                return float(bid), float(ask)

            spot_bid_ask = _get_bid_ask(spot_data)
            futures_bid_ask = _get_bid_ask(futures_data)
            if spot_bid_ask is None or futures_bid_ask is None:
                return None  # Skip silently - data format issue

            spot_bid, spot_ask = spot_bid_ask
            spot_mid = (spot_bid + spot_ask) / 2

            futures_bid, futures_ask = futures_bid_ask
            futures_mid = (futures_bid + futures_ask) / 2

            # Calculate basis (future - spot) / spot
            if spot_mid > 0:
                basis = (futures_mid - spot_mid) / spot_mid
            else:
                logger.error(f"Zero spot price for {symbol}")
                return None

            # Create basis data
            basis_data = BasisData(
                symbol=symbol,
                timestamp=timestamp,
                spot_price=spot_mid,
                futures_price=futures_mid,
                basis=basis,
                spot_mid=spot_mid,
                futures_mid=futures_mid,
            )

            # Store in window
            self._add_to_window(symbol, basis_data)

            return basis_data

        except (KeyError, ValueError, TypeError) as e:
            logger.error(f"Error calculating basis: {e}")
            return None

    def _add_to_window(self, symbol: str, basis_data: BasisData):
        """
        Add basis data to current time window.

        Args:
            symbol: Trading symbol
            basis_data: Basis calculation result
        """
        # Initialize window if needed
        if symbol not in self.window_start_time:
            self.window_start_time[symbol] = basis_data.timestamp

        # Check if window has expired
        window_age = basis_data.timestamp - self.window_start_time[symbol]
        if window_age >= self.window_interval:
            # Window expired, aggregate and move to history
            self._aggregate_window(symbol)
            self.window_start_time[symbol] = basis_data.timestamp
            self.window_data[symbol].clear()

        # Add to current window
        self.window_data[symbol].append(basis_data)

    def _aggregate_window(self, symbol: str):
        """Aggregate data in the current window and add to history."""
        if not self.window_data[symbol]:
            return

        # Calculate window statistics
        basis_values = [d.basis for d in self.window_data[symbol]]
        spot_prices = [d.spot_price for d in self.window_data[symbol]]
        futures_prices = [d.futures_price for d in self.window_data[symbol]]

        if basis_values:
            # Create aggregated data point (use average)
            avg_basis = sum(basis_values) / len(basis_values)
            avg_spot = sum(spot_prices) / len(spot_prices)
            avg_futures = sum(futures_prices) / len(futures_prices)

            aggregated_data = BasisData(
                symbol=symbol,
                timestamp=self.window_start_time[symbol] + self.window_interval / 2,
                spot_price=avg_spot,
                futures_price=avg_futures,
                basis=avg_basis,
                spot_mid=avg_spot,
                futures_mid=avg_futures,
            )

            # Add to history
            self.basis_history[symbol].append(aggregated_data)

            # Keep only last N windows in history (e.g., last 1000)
            max_history = 1000
            if len(self.basis_history[symbol]) > max_history:
                self.basis_history[symbol] = self.basis_history[symbol][-max_history:]

            logger.debug(
                f"Aggregated window for {symbol}: "
                f"basis={avg_basis:.6%}, "
                f"samples={len(basis_values)}"
            )

    def get_recent_basis_data(self, symbol: str, count: int = 100) -> List[BasisData]:
        """
        Get recent basis data for a symbol.

        Args:
            symbol: Trading symbol
            count: Number of recent data points to return

        Returns:
            List of recent BasisData
        """
        symbol_key = symbol.upper()
        if symbol_key in self.basis_history:
            return self.basis_history[symbol_key][-count:]
        return []

    def get_current_window_data(self, symbol: str) -> List[BasisData]:
        """
        Get current window data for a symbol.

        Args:
            symbol: Trading symbol

        Returns:
            List of BasisData in current window
        """
        symbol_key = symbol.upper()
        return self.window_data.get(symbol_key, [])

    def get_window_stats(self, symbol: str) -> Dict[str, Any]:
        """
        Get statistics for current window.

        Args:
            symbol: Trading symbol

        Returns:
            Dictionary with window statistics
        """
        symbol_key = symbol.upper()
        current_data = self.window_data.get(symbol_key, [])

        if not current_data:
            return {
                "sample_count": 0,
                "current_basis": 0.0,
                "avg_basis": 0.0,
                "window_progress": 0.0,
            }

        basis_values = [d.basis for d in current_data]
        current_basis = basis_values[-1] if basis_values else 0.0
        avg_basis = sum(basis_values) / len(basis_values) if basis_values else 0.0

        # Calculate window progress
        if symbol_key in self.window_start_time:
            window_age = time.time() - self.window_start_time[symbol_key]
            window_progress = min(window_age / self.window_interval, 1.0)
        else:
            window_progress = 0.0

        return {
            "sample_count": len(current_data),
            "current_basis": current_basis,
            "avg_basis": avg_basis,
            "window_progress": window_progress,
            "window_start": self.window_start_time.get(symbol_key, 0),
            "window_end": self.window_start_time.get(symbol_key, 0) + self.window_interval,
        }