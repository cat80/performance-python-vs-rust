"""
Statistical indicators calculation (MA, EMA, Z-Score).
"""

import numpy as np
from typing import List, Dict, Any, Optional
from dataclasses import dataclass
from loguru import logger

# Try to import polars, but have fallback
try:
    import polars as pl
    POLARS_AVAILABLE = True
except ImportError:
    POLARS_AVAILABLE = False
    logger.warning("Polars not available, using NumPy for calculations")


@dataclass
class IndicatorResult:
    """Result of indicator calculations."""
    symbol: str
    timestamp: float
    ma_price: float  # Moving average of price
    ma_basis: float  # Moving average of basis
    ema_basis: float  # Exponential moving average of basis
    z_score: float   # Z-Score of basis


class IndicatorCalculator:
    """Calculator for statistical indicators."""

    def __init__(self, window_size: int = 30):
        """
        Initialize indicator calculator.

        Args:
            window_size: Window size for MA/EMA calculations
        """
        self.window_size = window_size

        # Store basis history per symbol (using native lists for performance)
        self.basis_history: Dict[str, List[float]] = {}
        self.price_history: Dict[str, List[float]] = {}

        # EMA smoothing factor
        self.alpha = 2 / (window_size + 1)

    def add_data(self, symbol: str, basis: float, price: float):
        """
        Add basis and price data for a symbol.

        Args:
            symbol: Trading symbol
            basis: Basis value
            price: Price value
        """
        symbol_key = symbol.upper()

        # Initialize lists if needed
        if symbol_key not in self.basis_history:
            self.basis_history[symbol_key] = []
            self.price_history[symbol_key] = []

        # Add data
        self.basis_history[symbol_key].append(basis)
        self.price_history[symbol_key].append(price)

        # Keep only window_size most recent values
        if len(self.basis_history[symbol_key]) > self.window_size:
            self.basis_history[symbol_key] = self.basis_history[symbol_key][-self.window_size:]
            self.price_history[symbol_key] = self.price_history[symbol_key][-self.window_size:]

    def calculate_indicators(self, symbol: str) -> Optional[IndicatorResult]:
        """
        Calculate indicators for a symbol.

        Args:
            symbol: Trading symbol

        Returns:
            IndicatorResult if enough data, None otherwise
        """
        symbol_key = symbol.upper()

        # Check if we have enough data
        if (symbol_key not in self.basis_history or
            len(self.basis_history[symbol_key]) < self.window_size):
            return None

        try:
            basis_data = self.basis_history[symbol_key]
            price_data = self.price_history[symbol_key]

            # Use Polars if available for optimized calculations
            if POLARS_AVAILABLE and len(basis_data) >= self.window_size:
                return self._calculate_with_polars(symbol_key, basis_data, price_data)
            else:
                return self._calculate_with_numpy(symbol_key, basis_data, price_data)

        except Exception as e:
            logger.error(f"Error calculating indicators for {symbol}: {e}")
            return None

    def _calculate_with_polars(self, symbol: str, basis_data: List[float], price_data: List[float]) -> IndicatorResult:
        """Calculate indicators using Polars (optimized)."""
        # Convert to Polars Series only when needed (performance optimization)
        basis_series = pl.Series("basis", basis_data)
        price_series = pl.Series("price", price_data)

        # Calculate MA (simple moving average)
        ma_basis = basis_series.tail(self.window_size).mean()
        ma_price = price_series.tail(self.window_size).mean()

        # Calculate EMA (exponential moving average)
        # Polars doesn't have built-in EMA for Series, so we calculate manually
        ema_basis = self._calculate_ema_numpy(basis_data)

        # Calculate Z-Score: (current - MA) / StdDev
        current_basis = basis_data[-1] if basis_data else 0
        std_basis = basis_series.tail(self.window_size).std()

        if std_basis > 0:
            z_score = (current_basis - ma_basis) / std_basis
        else:
            z_score = 0.0

        return IndicatorResult(
            symbol=symbol,
            timestamp=np.float64(time.time()),
            ma_price=float(ma_price),
            ma_basis=float(ma_basis),
            ema_basis=float(ema_basis),
            z_score=float(z_score),
        )

    def _calculate_with_numpy(self, symbol: str, basis_data: List[float], price_data: List[float]) -> IndicatorResult:
        """Calculate indicators using NumPy (fallback)."""
        import numpy as np
        import time

        # Convert to numpy arrays for efficient calculation
        basis_array = np.array(basis_data[-self.window_size:])
        price_array = np.array(price_data[-self.window_size:])

        # Calculate MA (simple moving average)
        ma_basis = np.mean(basis_array)
        ma_price = np.mean(price_array)

        # Calculate EMA (exponential moving average)
        ema_basis = self._calculate_ema_numpy(basis_data)

        # Calculate Z-Score: (current - MA) / StdDev
        current_basis = basis_data[-1] if basis_data else 0
        std_basis = np.std(basis_array) if len(basis_array) > 1 else 0

        if std_basis > 0:
            z_score = (current_basis - ma_basis) / std_basis
        else:
            z_score = 0.0

        return IndicatorResult(
            symbol=symbol,
            timestamp=time.time(),
            ma_price=float(ma_price),
            ma_basis=float(ma_basis),
            ema_basis=float(ema_basis),
            z_score=float(z_score),
        )

    def _calculate_ema_numpy(self, data: List[float]) -> float:
        """
        Calculate EMA using NumPy.

        Args:
            data: List of values

        Returns:
            EMA value
        """
        import numpy as np

        if not data:
            return 0.0

        # Use the last window_size values
        window_data = data[-self.window_size:] if len(data) >= self.window_size else data

        # Calculate EMA manually
        weights = np.exp(np.linspace(-1, 0, len(window_data)))
        weights /= weights.sum()

        return np.dot(window_data, weights)

    def get_indicator_history(self, symbol: str, count: int = 100) -> List[IndicatorResult]:
        """
        Get historical indicator data.

        Note: This is inefficient for real-time use - only for occasional queries.
        For real-time, store indicator results as they're calculated.

        Args:
            symbol: Trading symbol
            count: Number of historical points

        Returns:
            List of historical IndicatorResult
        """
        # In a real implementation, you would store calculated indicator results
        # For now, we'll just return empty list
        return []

    def calculate_all_indicators(self) -> Dict[str, IndicatorResult]:
        """
        Calculate indicators for all symbols with sufficient data.

        Returns:
            Dictionary mapping symbol to IndicatorResult
        """
        results = {}
        for symbol in list(self.basis_history.keys()):
            result = self.calculate_indicators(symbol)
            if result:
                results[symbol] = result
        return results


# Import time here to avoid circular import
import time