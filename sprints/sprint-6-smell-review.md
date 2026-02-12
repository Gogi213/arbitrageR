# Code Smell Review Report

## Executive Summary

Проект rust-hft содержит **архитектурные костыли** перенесенные из C# и **недоделанные компоненты**, блокирующие работу dashboard.

---

## Critical Issues (Блокирующие)

### 1. Пустой metrics.rs (Line 1-4)
**Файл:** `src/infrastructure/metrics.rs`
```rust
//! Placeholder for metrics collection
//!
//! Will implement lock-free metrics counters and export
```
**Проблема:** Полностью пустой файл, хотя dashboard ждет метрик. Метрики не собираются вообще.
**Impact:** Dashboard не видит системные метрики (latency, connections, throughput).

### 2. Архаичный /api/paper/stats endpoint
**Файл:** `src/infrastructure/api.rs:66, 102-105`
```rust
.route("/api/paper/stats", get(get_paper_stats)) // Stub for store.js compatibility

async fn get_paper_stats() -> Json<Vec<()>> {
    Json(vec![]) // Empty bots list
}
```
**Проблема:** 
- "Paper" - архаизм из C# (paper trading)
- Возвращает пустой массив
- Frontend ждет там данные, получает []
**Impact:** Dashboard показывает 0 ботов всегда.

### 3. Дублирование путей к static files
**Файл:** `src/infrastructure/api.rs:61`
```rust
let static_files = ServeDir::new("/root/arbitrageR/reference/frontend");
```
**Файл:** `src/infrastructure/config.rs:80`
```rust
fn default_static_path() -> PathBuf {
    PathBuf::from("./reference/frontend")
}
```
**Проблема:** Hardcoded абсолютный путь в коде + конфигурация игнорируется.

---

## Architecture Issues

### 4. Неправильный min_max() в RingBuffer
**Файл:** `src/infrastructure/ring_buffer.rs:96-131`
```rust
pub fn min_max(&self) -> (FixedPoint8, FixedPoint8) {
    // ...
    let idx = if self.count < N {
        i
    } else {
        i  // BUG: Не учитывает wraparound ring buffer!
    };
```
**Проблема:** Индексация не учитывает, что ring buffer циклический. После переполнения читает неправильные данные.
**Impact:** spread_range метрика некорректна.

### 5. ExchangeClient enum dispatch избыточен
**Файл:** `src/exchanges/mod.rs:17-51`
```rust
pub enum ExchangeClient {
    Binance(BinanceWsClient),
    Bybit(BybitWsClient),
}

impl ExchangeClient {
    pub async fn connect(&mut self) -> Result<()> {
        match self {
            Self::Binance(c) => c.connect().await,
            Self::Bybit(c) => c.connect(false).await,  // Inconsistent!
        }
    }
```
**Проблема:** 
- Ручной dispatch для каждого метода
- `BybitWsClient::connect()` принимает testnet flag, Binance - нет (inconsistent API)
- Нет нужды в enum dispatch когда есть WebSocketExchange trait

### 6. Дублирование проверки common symbols
**Файл:** `src/core/symbol.rs:49-62` и `src/core/registry.rs:111-117`
Оба места дублируют список hardcoded символов:
```rust
match bytes {
    b"BTCUSDT" => return Some(Symbol::BTCUSDT),
    b"ETHUSDT" => return Some(Symbol::ETHUSDT),
    // ... одинаковый список в двух файлах
}
```

### 7. Metrics::get_all_stats() фильтрует слишком много
**Файл:** `src/hot_path/tracker.rs:129-136`
```rust
pub fn get_all_stats(&self) -> Vec<ScreenerStats> {
    self.states
        .iter()
        .filter_map(|s| s.as_ref())
        .filter(|s| s.last_binance.is_some() || s.last_bybit.is_some())  // OR!
        .map(|s| s.get_stats())
        .collect()
}
```
**Проблема:** `||` (OR) вместо `&&` (AND). Показывает пары где есть данные только с одной биржи.
**Impact:** В dashboard отображаются пары без реального спреда (is_spread_na=true).

---

## Code Quality Issues

### 8. Мертвый код в ws/connection.rs
**Файл:** `src/ws/connection.rs:162-184`
```rust
pub async fn recv(&mut self) -> Result<Option<Message>> {
    match self.stream.next().await {
        Some(Ok(msg)) => {
            // tracing::debug!("WS Recv: {:?}", msg);  // Закомментировано
            self.last_activity = Instant::now();
            
            // ...  // Пустой комментарий
            
            Ok(Some(msg))
        }
```

### 9. Неиспользуемые imports
**Файл:** `src/core/discovery.rs:1-11`
```rust
use std::time::Duration;  // Используется только в одном месте
```

### 10. Inconsistent error handling
В одних местах `HftError`, в других `ExchangeError`, в третьих строки.

---

## Performance Issues

### 11. Сборка долгая из-за LTO
**Файл:** `Cargo.toml:81-87`
```toml
[profile.release]
lto = "fat"
codegen-units = 1
```
**Проблема:** LTO = Link Time Optimization делает линковку очень долгой. Для dev-цикла избыточно.
**Solution:** Добавить profile для быстрой dev-сборки.

### 12. Tower-http features слишком много
**Файл:** `Cargo.toml:57`
```rust
tower-http = { version = "0.5", features = ["fs", "cors", "trace"] }
```
**Проблема:** `trace` feature не используется, но компилируется.

---

## Frontend Issues

### 13. Рассинхронизация API и frontend
**Файл:** `reference/frontend/js/store.js:3`
```javascript
const API_URL = '/api/paper/stats';  // Устаревший endpoint
```
**Проблема:** Frontend ожидает структуру из C#, но backend выдает Rust-структуру.

### 14. Dashboard ожидает bots[], но backend дает screener[]
**Файл:** `reference/frontend/js/components/SystemDashboard.js:37`
```javascript
<tr v-for="s in store.screener" :key="s.symbol">
```
**Но:** `store.js` использует `API_URL = '/api/paper/stats'` который возвращает `bots` (пустой).

---

## Summary

| Category | Count | Critical |
|----------|-------|----------|
| Missing Implementation | 1 | metrics.rs empty |
| API Design | 2 | paper endpoint, path duplication |
| Logic Bugs | 2 | min_max, get_all_stats filter |
| Code Duplication | 2 | symbol lists, dispatch |
| Performance | 2 | LTO, unused features |
| Frontend Sync | 2 | endpoint mismatch, data structure |

**Total Critical:** 8 блокирующих issues
**Total Warnings:** 6 технических долгов

---

## Recommended Sprint Structure

### Phase 6.1: Fix Dashboard API
- Удалить /api/paper/stats
- Создать /api/dashboard/stats с полными данными
- Обновить frontend

### Phase 6.2: Fix Metrics Collection
- Реализовать metrics.rs
- Добавить системные метрики (latency, connections)
- Интегрировать с API

### Phase 6.3: Fix Data Consistency
- Исправить RingBuffer.min_max()
- Исправить get_all_stats() filter
- Добавить тесты

### Phase 6.4: Build Performance
- Добавить dev profile без LTO
- Очистить unused features
- Убрать мертвый код

### Phase 6.5: Code Cleanup
- Убрать дублирование symbol lists
- Упростить ExchangeClient dispatch
- Унифицировать error handling
