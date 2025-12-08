# Unitrade

**Unitrade** is a generic, high-performance, type-safe trading system architecture for Rust. It provides a robust foundation for building trading bots, market makers, and exchange connectors.

## Features

- **Generic Architecture**: Decoupled design with `Exchange`, `Feed`, `Gateway`, and `Connector` components.
- **Type-Safe DSL**: A comprehensive Domain Specific Language (DSL) for trading operations, ensuring compile-time safety for orders, positions, and market data.
- **Actor-Based**: Built on `tokio` and `async-trait`, treating every component as an Actor for high concurrency.
- **Trading Complete**: Supports advanced features like:
    - Batch Orders (Atomic support)
    - Iceberg Orders (`visible_quantity`)
    - Advanced Order Attributes (`TimeInForce`, `PostOnly`, `ReduceOnly`)
    - Multi-Asset Support (Spot, Futures, Options)
- **Protocol Agnostic**: Define your logic once, run it against any exchange (Binance, OKX, etc.) by implementing the Connector traits.

## Architecture

The system is built around the "Exchange Triad":

1.  **Connector**: The network layer. Handles WebSocket/REST connections and raw protocol translation.
2.  **Feed**: The perception layer. Manages `OrderBook`, `TradeFlow`, and `Kline` data.
3.  **Gateway**: The execution layer. Manages `Orders`, `Positions`, and `Balances`.

These are orchestrated by the `Exchange` facade, which provides a unified interface for the application.

## Usage

Add `unitrade` to your `Cargo.toml`:

```toml
[dependencies]
unitrade = { git = "https://github.com/eternamaze/unitrade" }
```

Implement the traits for your specific exchange (see `mmg-tradengine` for a reference implementation).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
