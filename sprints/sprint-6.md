# Sprint 6: Dashboard & Metrics Stabilization

## Sprint Goal
Исправить критические проблемы, блокирующие работу dashboard и метрик. Устранить архитектурные костыли перенесенные из C#.

---

## Phase 6.1: Fix Dashboard API (CRITICAL)

**Goal:** Удалить устаревший paper endpoint, создать единый dashboard API

### Tasks
- [ ] Удалить `/api/paper/stats` endpoint из api.rs
- [ ] Создать `/api/dashboard/stats` endpoint
- [ ] Объединить screener + system метрики в один ответ
- [ ] Обновить store.js на новый endpoint
- [ ] Проверить SystemDashboard.js получает данные

### Acceptance Criteria
- [ ] Dashboard показывает актуальные метрики
- [ ] Нет 404 ошибок в browser console
- [ ] Данные обновляются каждую секунду

---

## Phase 6.2: Implement Metrics Collection (CRITICAL)

**Goal:** Реализовать infrastructure/metrics.rs для сбора системных метрик

### Tasks
- [ ] Создать MetricsCollector struct
- [ ] Добавить метрики: connection status, message rate, latency
- [ ] Интегрировать в AppEngine
- [ ] Экспорт через API endpoint
- [ ] Добавить тесты

### Acceptance Criteria
- [ ] metrics.rs содержит рабочий код
- [ ] API отдает системные метрики
- [ ] Frontend видит connection status

---

## Phase 6.3: Fix Range2M Calculation (HIGH)

**Goal:** Исправить вычисление range2m согласно требованиям dashboard

### Требования Dashboard:
- **range2m** = |min| + max за скользящее окно 2 минуты
- Если min и max имеют одинаковый знак → N/A (нет арбитража)
- **spread** = фактический реалтайм спред между биржами (не среднее)

### Tasks
- [ ] Заменить RingBuffer на TimeWindowBuffer (окно 2 минуты, не 1200 тиков)
- [ ] Реализовать min/max за временное окно
- [ ] Вычислять range2m = |min| + max
- [ ] isSpreadNA = true когда min/max одинаковый знак
- [ ] Исправить ThresholdTracker.get_all_stats() filter (AND вместо OR)
- [ ] Добавить unit тесты

### Acceptance Criteria
- [ ] range2m считается за 2 минуты (не за количество тиков)
- [ ] N/A когда нет арбитража (один знак)
- [ ] В dashboard только пары с обеих бирж
- [ ] Фактический спред обновляется в реалтайме

---

## Phase 6.4: Build Performance (MEDIUM)

**Goal:** Ускорить цикл сборки

### Tasks
- [ ] Добавить profile.dev-fast без LTO
- [ ] Убрать unused tower-http features
- [ ] Удалить мертвый код (закомментированные строки)
- [ ] Оптимизировать codegen-units для dev

### Acceptance Criteria
- [ ] Сборка debug < 10 секунд
- [ ] Нет предупреждений о неиспользуемых imports
- [ ] Код чище

---

## Phase 6.5: Code Architecture (MEDIUM)

**Goal:** Устранить дублирование и костыли

### Tasks
- [ ] Вынести common symbols list в константу
- [ ] Упростить ExchangeClient enum dispatch
- [ ] Унифицировать error types
- [ ] Убрать hardcoded пути из api.rs

### Acceptance Criteria
- [ ] Нет дублирования symbol lists
- [ ] Единый путь к static files
- [ ] Consistent error handling

---

## Sprint Checklist

**Before Start:**
- [ ] Все фазы понятны
- [ ] Порядок выполнения согласован

**During:**
- [ ] Каждая фаза = один коммит
- [ ] Тесты проходят перед коммитом
- [ ] Документация обновлена

**Complete When:**
- [ ] Dashboard показывает данные
- [ ] Метрики работают
- [ ] Сборка быстрая
- [ ] Нет критических bugs

---

## Current Status

| Phase | Status | Commit |
|-------|--------|--------|
| 6.1 | COMPLETE | 668a02b |
| 6.2 | COMPLETE | 3a85a5a |
| 6.3 | PENDING | - |
| 6.4 | PENDING | - |
| 6.5 | PENDING | - |

**Last Updated:** 2026-02-12
