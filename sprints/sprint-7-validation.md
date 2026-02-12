# Sprint 7: Code Validation & Cleanup

## Sprint Goal
Провести полную валидацию кодовой базы на предмет бессмысленных костылей, fallback-ов, dead code и дублирования. Устранить все найденные проблемы.

---

## Phase 7.1: Remove All Hardcoded Values & Fallback Logic (CRITICAL)

**Problem:** main.rs содержит бессмысленный fallback на 10 хардкодных символов, который скрывает реальные ошибки. Также в коде разбросаны хардкод значения.

**Found in:** 
- `src/main.rs:87-140` - fallback blocks
- `src/infrastructure/api.rs:80` - hardcoded path
- Различные magic numbers по всему коду

### Tasks
- [ ] Удалить все 3 fallback блока (vec![Symbol::BTCUSDT, ...])
- [ ] Сделать так чтобы бот падал при ошибке инициализации
- [ ] Добавить правильную обработку ошибок вместо fallback
- [ ] Найти и вынести в config все хардкод значения:
  - Threshold 0.25% (250_000)
  - Window duration 2 minutes
  - API port 5000
  - Пути к static files

### Acceptance Criteria
- [ ] Нет fallback списка из 10 символов
- [ ] Нет хардкод путей
- [ ] Нет magic numbers
- [ ] Все конфигурируемые значения в Config
- [ ] При ошибке discovery бот останавливается с понятной ошибкой
- [ ] 277 символов из discovery используются всегда

---

## Phase 7.2: Remove Dead Code (HIGH)

**Problems Found:**
1. `src/exchanges/binance/mod.rs:191` - `parse_message` never used
2. `src/exchanges/bybit/mod.rs:285` - `parse_message` never used  
3. `src/ws/ping.rs:234` - `unhealthy_threshold` field never read
4. `src/ws/pool.rs:63` - `id` field never read
5. `src/core/symbol.rs:200` - `STATIC_SYMBOL_COUNT` never used

### Tasks
- [ ] Удалить неиспользуемые методы `parse_message`
- [ ] Удалить неиспользуемые поля `unhealthy_threshold`, `id`
- [ ] Удалить неиспользуемую константу `STATIC_SYMBOL_COUNT`
- [ ] Убрать unused imports по всему проекту

### Acceptance Criteria
- [ ] Нет предупреждений `dead_code` от rustc
- [ ] Нет предупреждений `unused` от rustc
- [ ] Все тесты проходят

---

## Phase 7.3: Consolidate Symbol Lists (HIGH)

**Problem:** Дублирование списков символов в разных местах

**Found:**
- `src/core/symbol.rs` - статические символы
- `src/core/registry.rs` - возможно дублирование
- Fallback list в main.rs (будет удален в 7.1)

### Tasks
- [ ] Найти все места где есть списки символов
- [ ] Вынести в единое место (constants или config)
- [ ] Убрать дублирование

### Acceptance Criteria
- [ ] Один источник правды для списков символов
- [ ] Нет copy-paste списков в разных файлах

---

## Phase 7.4: Fix Hardcoded Paths (MEDIUM)

**Problem:** Абсолютный путь в API сервере

**Found:** `src/infrastructure/api.rs:80`
```rust
let static_files = ServeDir::new("/root/arbitrageR/reference/frontend");
```

### Tasks
- [ ] Убрать hardcoded путь из api.rs
- [ ] Использовать относительный путь или config
- [ ] Добавить проверку существования пути

### Acceptance Criteria
- [ ] Путь не содержит /root/arbitrageR
- [ ] Работает из любой директории

---

## Phase 7.5: Unify Error Types (MEDIUM)

**Problem:** Разные типы ошибок в разных местах

**Found:**
- `HftError` в lib.rs
- `ExchangeError` в exchanges/traits.rs
- String errors в некоторых местах

### Tasks
- [ ] Аудит всех error types
- [ ] Унифицировать в один иерархический тип
- [ ] Убрать String errors

### Acceptance Criteria
- [ ] Единый Error enum для всего проекта
- [ ] Нет `Result<T, String>`

---

## Sprint Checklist

**Complete When:**
- [ ] Нет fallback логики
- [ ] Нет dead code
- [ ] Нет дублирования
- [ ] Нет hardcoded путей
- [ ] Единая система ошибок
- [ ] `cargo build` без предупреждений
- [ ] Все тесты проходят

**Last Updated:** 2026-02-12
