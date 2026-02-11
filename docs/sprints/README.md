# HFT Arbitrage Bot - Sprint Index

## Overview
Ultra-low latency arbitrage bot between Binance and Bybit using Rust.

## Sprint Roadmap

### Sprint 1: Foundation & Core Types
**Status**: 🔄 NOT STARTED  
**Goal**: Zero-allocation core types and project infrastructure

- Phase 1.1: Cargo Project Setup
- Phase 1.2: Fixed-Point Arithmetic
- Phase 1.3: Symbol Interning
- Phase 1.4: TickerData & TradeData Types
- Phase 1.5: Object Pooling Infrastructure

### Sprint 2: WebSocket Infrastructure
**Status**: 🔄 NOT STARTED  
**Goal**: Ultra-low latency WebSocket connections

- Phase 2.1: WebSocket Connection Core
- Phase 2.2: Message Router
- Phase 2.3: Connection Pool
- Phase 2.4: Subscription Manager
- Phase 2.5: Ping/Pong Handler

### Sprint 3: Exchange Implementations
**Status**: 🔄 NOT STARTED  
**Goal**: Binance and Bybit specific implementations

- Phase 3.1: Binance Native WebSocket
- Phase 3.2: Bybit Native WebSocket
- Phase 3.3: Zero-Copy JSON Parsers
- Phase 3.4: Exchange Abstractions
- Phase 3.5: Symbol Mapping

### Sprint 4: REST API & Order Management
**Status**: 🔄 NOT STARTED  
**Goal**: Order placement and account management

- Phase 4.1: REST Client Foundation
- Phase 4.2: Request Signing
- Phase 4.3: Order Types & Structures
- Phase 4.4: Order Placement
- Phase 4.5: Position Tracking
- Phase 4.6: Account State

### Sprint 5: Arbitrage Engine
**Status**: 🔄 NOT STARTED  
**Goal**: Detect and execute arbitrage opportunities

- Phase 5.1: Spread Calculator
- Phase 5.2: Opportunity Detector
- Phase 5.3: Risk Manager
- Phase 5.4: Execution Engine
- Phase 5.5: PnL Tracker

### Sprint 6: Infrastructure & Observability
**Status**: 🔄 NOT STARTED  
**Goal**: Monitoring, metrics, and operational tooling

- Phase 6.1: Metrics Collection
- Phase 6.2: Structured Logging
- Phase 6.3: Health Monitoring
- Phase 6.4: Configuration Management
- Phase 6.5: Graceful Shutdown

## Getting Started

1. Start with Sprint 1, Phase 1.1
2. Say: **"Start Phase 1.1"** to begin
3. Follow rust-sprint-workflow skill exactly
4. One commit per phase
5. Wait for trigger before next phase

## HFT Principles

- **HOT PATH**: Zero allocation, no panic, no locks
- **WARM PATH**: Minimal allocation, fast paths
- **COLD PATH**: Standard Rust, logging, I/O

## Project Structure

```
/root/arbitrageR/
├── docs/sprints/          # Sprint documents
├── rust-hft/
│   ├── src/
│   │   ├── core/         # Fixed-point, types
│   │   ├── hot_path/     # Zero-alloc logic
│   │   ├── exchanges/    # Binance/Bybit
│   │   ├── ws/          # WebSocket clients
│   │   ├── rest/        # REST clients
│   │   └── infrastructure/ # Cold path
│   ├── config/          # Configuration files
│   ├── benches/         # Benchmarks
│   └── tests/          # Integration tests
├── src/exchanges/       # C# reference files
├── wwwroot/            # HTML dashboards
├── scripts/            # Python/C# tools
└── appsettings.json    # Reference config
```

## Reference Materials

- C# source files in `/root/arbitrageR/src/exchanges/`
- Original project: `/root/screener123/collections/`
- Binance API docs: https://binance-docs.github.io/
- Bybit API docs: https://bybit-exchange.github.io/
