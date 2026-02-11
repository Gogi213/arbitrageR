import re
import sys
import os
from dataclasses import dataclass
from typing import Dict, Tuple, List
from collections import defaultdict

@dataclass
class PathScores:
    hot: int = 0
    warm: int = 0
    cold: int = 0
    
    def total(self) -> int:
        return self.hot + self.warm + self.cold
    
    def risk_level(self) -> str:
        """Определяет уровень риска на основе hot path score"""
        if self.hot == 0:
            return "✅ EXCELLENT"
        elif self.hot < 10:
            return "✅ GOOD"
        elif self.hot < 30:
            return "⚠️  MODERATE"
        elif self.hot < 50:
            return "❌ HIGH RISK"
        else:
            return "💀 CRITICAL"

@dataclass
class ResponsibilityMetrics:
    """Метрики ответственности и слоёв"""
    total_lines: int = 0
    method_count: int = 0
    public_method_count: int = 0
    field_count: int = 0
    has_hot_path: bool = False
    has_warm_path: bool = False
    has_cold_path: bool = False
    responsibilities: List[str] = None  # type: ignore
    
    def __post_init__(self):
        if self.responsibilities is None:
            self.responsibilities = []
    
    def is_god_object(self) -> bool:
        """God Object: >500 LOC ИЛИ >15 методов ИЛИ смешивание всех трёх слоёв"""
        return (
            self.total_lines > 500 or 
            self.method_count > 15 or
            (self.has_hot_path and self.has_warm_path and self.has_cold_path)
        )
    
    def get_architecture_smell(self) -> Tuple[str, str]:
        """Возвращает (severity, description) для архитектурных smell'ов"""
        smells = []
        severity = "✅ CLEAN"
        
        # God Object Detection
        if self.total_lines > 500:
            smells.append(f"God Object ({self.total_lines} LOC)")
            severity = "❌ CRITICAL"
        elif self.total_lines > 300:
            smells.append(f"Large Class ({self.total_lines} LOC)")
            severity = "⚠️  MODERATE"
            
        if self.method_count > 15:
            smells.append(f"Too Many Methods ({self.method_count})")
            severity = max_severity(severity, "⚠️  MODERATE")
        elif self.method_count > 10:
            smells.append(f"High Method Count ({self.method_count})")
        
        # Layer Mixing Detection
        layers_present = sum([self.has_hot_path, self.has_warm_path, self.has_cold_path])
        
        if layers_present == 3:
            smells.append("Layer Mixing: Hot+Warm+Cold (God Object pattern)")
            severity = "❌ CRITICAL"
        elif layers_present == 2:
            if self.has_hot_path and self.has_cold_path:
                smells.append("Layer Mixing: Hot+Cold (cohesion violation)")
                severity = max_severity(severity, "⚠️  MODERATE")
            elif self.has_hot_path:
                smells.append("Layer Mixing: Hot+Warm (acceptable but watch)")
                severity = max_severity(severity, "✅ GOOD")
        
        # Public API Surface
        if self.public_method_count > 12:
            smells.append(f"Large Public API ({self.public_method_count} methods)")
            severity = max_severity(severity, "⚠️  MODERATE")
        
        if not smells:
            return ("✅ CLEAN", "Good cohesion and layer separation")
        
        return (severity, "; ".join(smells))

@dataclass
class NumericTypeMetrics:
    """Метрики использования числовых типов"""
    long_count: int = 0
    double_count: int = 0
    decimal_count: int = 0
    int128_count: int = 0
    
    long_in_hot: int = 0
    double_in_hot: int = 0
    decimal_in_hot: int = 0
    int128_in_hot: int = 0
    
    def get_recommendations(self) -> List[str]:
        """Возвращает рекомендации по типам чисел"""
        recs = []
        
        # Проблема: decimal в hot path
        if self.decimal_in_hot > 0:
            recs.append(f"🔴 CRITICAL: decimal in hot path ({self.decimal_in_hot}x) - MIGRATE TO long (FixedPoint8)")
        
        # Проблема: double в hot path
        if self.double_in_hot > 0:
            recs.append(f"🟠 WARNING: double in hot path ({self.double_in_hot}x) - CONSIDER long for price storage")
        
        # Хорошо: long в hot path
        if self.long_in_hot > 0:
            recs.append(f"✅ GOOD: long (FixedPoint8) in hot path ({self.long_in_hot}x)")
        
        # Int128 в hot path - нормально для math safety
        if self.int128_in_hot > 0:
            recs.append(f"✅ OK: Int128 in hot path ({self.int128_in_hot}x) - Math safety")
        
        return recs
    
    def get_type_compliance_score(self) -> int:
        """Score 0-100 по соответствию типов best practices"""
        score = 100
        
        # Штрафы
        if self.decimal_in_hot > 0:
            score -= min(50, self.decimal_in_hot * 10)  # -10 за каждый decimal в hot path
        
        if self.double_in_hot > 5:
            score -= min(20, (self.double_in_hot - 5) * 2)  # -2 за каждый лишний double
        
        if self.long_in_hot == 0 and (self.double_in_hot > 0 or self.decimal_in_hot > 0):
            score -= 15  # Нет long в hot path где он нужен
        
        return max(0, score)

def max_severity(s1: str, s2: str) -> str:
    """Возвращает более высокий уровень серьёзности"""
    order = ["💀 CRITICAL", "❌ CRITICAL", "❌ HIGH RISK", "⚠️  MODERATE", "⚠️  WARNING", "✅ GOOD", "✅ CLEAN", "✅ EXCELLENT"]
    try:
        idx1 = order.index(s1) if s1 in order else 999
        idx2 = order.index(s2) if s2 in order else 999
        return order[min(idx1, idx2)]
    except:
        return s2 if "❌" in s2 or "💀" in s2 else s1

def detect_method_type(method_signature: str, method_body: str) -> str:
    """
    Определяет тип пути (hot/warm/cold) на основе сигнатуры и содержимого метода
    """
    sig_lower = method_signature.lower()
    body_lower = method_body.lower()
    
    # HOT PATH паттерны (критичная latency, zero tolerance)
    hot_patterns = [
        r'func\s*\<[^\>]*,\s*task\s*\>',  # Callback'и с Func\<T, Task\>
        r'action\s*\<[^\>]*\>',         # Action callback'и
        r'subscribe',          # WebSocket subscribe методы
        r'onmessage',          # Message handlers
        r'ondata',             # Data handlers
        r'process',            # Process methods
        r'handle',             # Handle methods
        r'while\s*\(',         # Loops (receive loops)
        r'receive',            # Receive methods
        r'callback',           # Callback methods
    ]
    
    for pattern in hot_patterns:
        if re.search(pattern, sig_lower) or re.search(pattern, body_lower):
            return 'hot'
    
    # COLD PATH паттерны (логирование, инициализация, I/O)
    cold_patterns = [
        r'void\s+log',         # Log методы
        r'configure',          # Configuration
        r'initialize',         # Initialization
        r'setup',              # Setup
        r'file\.\w+',          # File I/O
        r'console\.\w+',       # Console I/O
        r'disposable',         # Dispose pattern
        r'tostring',           # ToString methods
        r'gethashcode',        # GetHashCode
        r'equals',             # Equals
        r'save',               # Save operations
        r'load',               # Load operations
        r'dashboard',          # Dashboard/UI
        r'csv',                # CSV generation
        r'export',             # Export
    ]
    
    for pattern in cold_patterns:
        if re.search(pattern, sig_lower) or re.search(pattern, body_lower):
            return 'cold'
    
    # WARM PATH - всё остальное (валидация, кэш, трансформация)
    warm_patterns = [
        r'get\w*async',        # Get методы
        r'validate',           # Validation
        r'normalize',          # Normalization
        r'convert',            # Conversion
        r'transform',          # Transformation
        r'cache',              # Cache operations
        r'tryget',             # Dictionary lookups
        r'update',             # Updates
        r'refresh',            # Refresh
        r'calculate',          # Calculations
    ]
    
    for pattern in warm_patterns:
        if re.search(pattern, sig_lower) or re.search(pattern, body_lower):
            return 'warm'
    
    # По умолчанию - warm path
    return 'warm'

def analyze_numeric_types(content: str) -> NumericTypeMetrics:
    """Анализирует использование числовых типов"""
    metrics = NumericTypeMetrics()
    
    # Подсчет общего использования типов
    metrics.long_count = len(re.findall(r'\blong\b', content))
    metrics.double_count = len(re.findall(r'\bdouble\b', content))
    metrics.decimal_count = len(re.findall(r'\bdecimal\b', content))
    metrics.int128_count = len(re.findall(r'\bInt128\b', content))
    
    # Разбиваем на методы и анализируем hot path
    method_pattern = r'((?:public|private|protected|internal)\s+(?:async\s+)?(?:override\s+)?(?:virtual\s+)?(?:static\s+)?(?:void|task|valuetask|[^\s]+)\s+(\w+)\s*\([^)]*\)\s*)\{([^}]*)\}'
    methods = re.findall(method_pattern, content, re.DOTALL | re.IGNORECASE)
    
    for sig, name, body in methods:
        path_type = detect_method_type(sig, body)
        
        if path_type == 'hot':
            metrics.long_in_hot += len(re.findall(r'\blong\b', body))
            metrics.double_in_hot += len(re.findall(r'\bdouble\b', body))
            metrics.decimal_in_hot += len(re.findall(r'\bdecimal\b', body))
            metrics.int128_in_hot += len(re.findall(r'\bInt128\b', body))
    
    return metrics

def analyze_responsibilities(content: str) -> ResponsibilityMetrics:
    """Анализирует ответственности и архитектуру файла"""
    metrics = ResponsibilityMetrics()
    
    # Подсчёт строк (без пустых и комментариев)
    lines = [l for l in content.split('\n') if l.strip() and not l.strip().startswith('//')]
    metrics.total_lines = len(lines)
    
    # Подсчёт методов
    method_pattern = r'((?:public|private|protected|internal)\s+(?:async\s+)?(?:override\s+)?(?:virtual\s+)?(?:static\s+)?(?:void|task|valuetask|[^\s]+)\s+(\w+)\s*\([^)]*\)\s*)\{'
    methods = re.findall(method_pattern, content, re.DOTALL | re.IGNORECASE)
    metrics.method_count = len(methods)
    
    # Подсчёт public методов
    public_methods = re.findall(r'public\s+(?:async\s+)?(?:override\s+)?(?:virtual\s+)?(?:static\s+)?(?:void|task|valuetask|[^\s]+)\s+(\w+)\s*\(', content, re.IGNORECASE)
    metrics.public_method_count = len(public_methods)
    
    # Подсчёт полей
    field_pattern = r'(private|protected|public)\s+(?:readonly\s+)?(?:static\s+)?[A-Z]\w+\s+_\w+'
    metrics.field_count = len(re.findall(field_pattern, content))
    
    # Определение слоёв (hot/warm/cold)
    method_blocks = re.findall(
        r'((?:public|private|protected|internal)\s+(?:async\s+)?(?:override\s+)?(?:virtual\s+)?(?:static\s+)?(?:void|task|valuetask|[^\s]+)\s+(\w+)\s*\([^)]*\)\s*)\{([^}]*)\}',
        content,
        re.DOTALL | re.IGNORECASE
    )
    
    for sig, name, body in method_blocks:
        path_type = detect_method_type(sig, body)
        if path_type == 'hot':
            metrics.has_hot_path = True
        elif path_type == 'warm':
            metrics.has_warm_path = True
        elif path_type == 'cold':
            metrics.has_cold_path = True
    
    # Определяем ответственности
    resp_patterns = {
        'WebSocket': r'(websocket|subscribe|onmessage|receive)',
        'Trading': r'(trade|order|position|execution)',
        'MarketData': r'(ticker|price|book|spread)',
        'Analysis': r'(analyze|calculate|indicator|signal)',
        'Persistence': r'(save|load|persist|storage)',
        'Configuration': r'(config|setting|option)',
        'Logging': r'(log|diagnostic|metric)',
        'UI/Dashboard': r'(dashboard|display|view|ui)',
        'API/REST': r'(http|api|rest|getasync|postasync)',
    }
    
    content_lower = content.lower()
    for resp_name, pattern in resp_patterns.items():
        if re.search(pattern, content_lower):
            metrics.responsibilities.append(resp_name)
    
    return metrics

def analyze_allocations(content: str, resp_metrics: ResponsibilityMetrics) -> Dict[str, PathScores]:
    """
    Анализирует аллокации с учетом путей
    """
    hot_score = PathScores()
    warm_score = PathScores()
    cold_score = PathScores()
    
    # Разбиваем на методы - ищем сигнатуру и тело отдельно
    method_pattern = r'((?:public|private|protected|internal)\s+(?:async\s+)?(?:override\s+)?(?:virtual\s+)?(?:static\s+)?(?:void|task|valuetask|[^\s]+)\s+(\w+)\s*\([^)]*\)\s*)\{'
    
    # Находим все методы с их позициями
    method_matches = list(re.finditer(method_pattern, content, re.DOTALL | re.IGNORECASE))
    methods = []
    
    for match in method_matches:
        sig = match.group(1)
        name = match.group(2)
        start_pos = match.end() - 1  # Позиция открывающей {
        
        # Найдем закрывающую } с учетом вложенности
        brace_count = 1
        pos = start_pos + 1
        while brace_count > 0 and pos < len(content):
            if content[pos] == '{':
                brace_count += 1
            elif content[pos] == '}':
                brace_count -= 1
            pos += 1
        
        body = content[start_pos:pos]
        methods.append((sig, name, body))
    
    for sig, name, body in methods:
        path_type = detect_method_type(sig, body)
        
        # Считаем аллокации в этом методе
        # 1. String interpolations
        string_interps = len(re.findall(r'\$"[^"]*"', body))
        string_concats = len(re.findall(r'\+\s*["\']', body))
        
        # 2. Lambdas (исключаем => в switch expressions)
        lambdas = len(re.findall(r'=>', body))
        
        # 3. new (исключаем new string[], new int и т.д. - value types)
        new_objects = len(re.findall(r'\bnew\s+(?!string\[|int\[|bool\[|byte\[|char\[|long\[|float\[|double\[)[A-Z]', body))
        
        # 4. LINQ
        linq_methods = ['.Where(', '.Select(', '.ToList(', '.ToArray(', 
                       '.FirstOrDefault(', '.Any(', '.Count(', '.OrderBy(']
        linq_count = sum(body.count(m) for m in linq_methods)
        
        # Применяем веса в зависимости от пути
        if path_type == 'hot':
            hot_score.hot += (string_interps * 10) + (string_concats * 5)
            hot_score.hot += lambdas * 10
            hot_score.hot += new_objects * 5
            hot_score.hot += linq_count * 20
        elif path_type == 'cold':
            cold_score.cold += (string_interps * 1) + (string_concats * 1)
            cold_score.cold += lambdas * 2
            cold_score.cold += new_objects * 1
            cold_score.cold += linq_count * 2
        else:  # warm
            warm_score.warm += (string_interps * 3) + (string_concats * 2)
            warm_score.warm += lambdas * 5
            warm_score.warm += new_objects * 2
            warm_score.warm += linq_count * 10
    
    return {
        'hot': hot_score,
        'warm': warm_score,
        'cold': cold_score
    }

def calculate_hft_score(file_path: str) -> None:
    """
    Основная функция расчета HFT-aware allocation score с анализом архитектуры и числовых типов
    """
    with open(file_path, 'r') as f:
        content = f.read()
    
    # Анализ ответственностей и архитектуры
    resp_metrics = analyze_responsibilities(content)
    
    # Анализ аллокаций
    scores = analyze_allocations(content, resp_metrics)
    
    # Анализ числовых типов
    numeric_metrics = analyze_numeric_types(content)
    
    # Общая статистика
    total_hot = scores['hot'].hot
    total_warm = scores['warm'].warm
    total_cold = scores['cold'].cold
    grand_total = total_hot + total_warm + total_cold
    
    # Определяем архитектурный smell
    arch_severity, arch_description = resp_metrics.get_architecture_smell()
    
    # Тип compliance score
    type_compliance = numeric_metrics.get_type_compliance_score()
    
    # Вывод результатов
    filename = os.path.basename(file_path)
    print(f"\n{'='*70}")
    print(f"HFT ALLOCATION & ARCHITECTURE ANALYSIS: {filename}")
    print(f"{'='*70}")
    
    # Секция: Архитектура и ответственности
    print(f"\n🏗️  ARCHITECTURE ANALYSIS:")
    print(f"   File Size: {resp_metrics.total_lines} LOC")
    print(f"   Methods: {resp_metrics.method_count} total ({resp_metrics.public_method_count} public)")
    print(f"   Fields: {resp_metrics.field_count}")
    
    if resp_metrics.responsibilities:
        print(f"   Responsibilities: {', '.join(resp_metrics.responsibilities)}")
    
    print(f"\n   Layer Distribution:")
    print(f"      🔥 Hot Path:   {'✅' if resp_metrics.has_hot_path else '❌'}")
    print(f"      🌡️  Warm Path:  {'✅' if resp_metrics.has_warm_path else '❌'}")
    print(f"      ❄️  Cold Path:  {'✅' if resp_metrics.has_cold_path else '❌'}")
    
    print(f"\n   Architecture Health:")
    print(f"      Status: {arch_severity}")
    print(f"      Issues: {arch_description}")
    
    # Секция: Числовые типы
    print(f"\n{'─'*70}")
    print(f"NUMERIC TYPE ANALYSIS (by Path):")
    print(f"{'─'*70}")
    
    print(f"\n📊 Type Usage (Total):")
    print(f"   long (FixedPoint8):  {numeric_metrics.long_count:3d}  ✅ Hot Path & Storage")
    print(f"   double (Math):       {numeric_metrics.double_count:3d}  🧮 Warm Path & Calculations")
    print(f"   decimal (API):       {numeric_metrics.decimal_count:3d}  🧊 Cold Path & Public API")
    print(f"   Int128 (Safety):     {numeric_metrics.int128_count:3d}  🛡 Math Safety")
    
    if resp_metrics.has_hot_path:
        print(f"\n🔥 HOT PATH Type Usage:")
        print(f"   long:    {numeric_metrics.long_in_hot:3d}  {'✅' if numeric_metrics.long_in_hot > 0 else '❌'}")
        print(f"   double:  {numeric_metrics.double_in_hot:3d}  {'⚠️' if numeric_metrics.double_in_hot > 0 else '✅'}")
        print(f"   decimal: {numeric_metrics.decimal_in_hot:3d}  {'❌' if numeric_metrics.decimal_in_hot > 0 else '✅'}")
        print(f"   Int128:  {numeric_metrics.int128_in_hot:3d}  {'✅' if numeric_metrics.int128_in_hot > 0 else '✓'}")
    
    print(f"\n   Type Compliance Score: {type_compliance}/100")
    
    # Секция: Аллокации
    print(f"\n{'─'*70}")
    print(f"ALLOCATION ANALYSIS (by Path):")
    print(f"{'─'*70}")
    
    print(f"\n🔥 HOT PATH (Zero tolerance): {total_hot} points")
    if total_hot > 0:
        print(f"   Risk: {scores['hot'].risk_level()}")
    else:
        print(f"   ✅ ZERO ALLOCATIONS - EXCELLENT")
    
    print(f"\n🌡️  WARM PATH (Minimal tolerance): {total_warm} points")
    if total_warm > 50:
        print(f"   ⚠️  High allocation rate")
    else:
        print(f"   ✅ Acceptable for throughput-critical code")
    
    print(f"\n❄️  COLD PATH (Standard tolerance): {total_cold} points")
    print(f"   ✅ Standard C# patterns OK here")
    
    print(f"\n{'='*70}")
    print(f"SCORES:")
    print(f"   Hot Path Score:       {total_hot:4d} (Critical)")
    print(f"   Warm Path Score:      {total_warm:4d} (Warning if >50)")
    print(f"   Cold Path Score:      {total_cold:4d} (Informational)")
    print(f"   Type Compliance:      {type_compliance:4d}/100")
    print(f"   ─────────────────────────")
    print(f"   TOTAL:                {grand_total:4d}")
    print(f"{'='*70}")
    
    # Приоритеты действий
    print(f"\n📋 PRIORITY ACTIONS:")
    
    actions = []
    
    # Архитектурные проблемы
    if resp_metrics.is_god_object():
        actions.append(("🔴 CRITICAL", "Split God Object into separate Hot/Warm/Cold classes"))
    elif arch_severity.startswith("❌") or arch_severity.startswith("💀"):
        actions.append(("🔴 CRITICAL", f"Architecture: {arch_description}"))
    elif arch_severity.startswith("⚠️"):
        actions.append(("🟠 WARNING", f"Architecture: {arch_description}"))
    
    # Типы чисел
    if numeric_metrics.decimal_in_hot > 0:
        actions.append(("🔴 CRITICAL", f"Migrate {numeric_metrics.decimal_in_hot} decimal → long (FixedPoint8) in hot path"))
    
    if numeric_metrics.double_in_hot > 3:
        actions.append(("🟠 WARNING", f"Consider migrating {numeric_metrics.double_in_hot} double → long in hot path"))
    
    # Аллокации
    if total_hot > 50:
        actions.append(("🔴 CRITICAL", "Remove ALL allocations from hot path immediately"))
    elif total_hot > 20:
        actions.append(("🟠 HIGH", "Optimize hot path allocations"))
    elif total_hot > 0:
        actions.append(("🟡 MEDIUM", "Minor hot path improvements possible"))
    
    if total_warm > 100 and not resp_metrics.is_god_object():
        actions.append(("🟠 HIGH", "Consider ObjectPool\u003cT\u003e for warm path"))
    
    if not actions:
        actions.append(("✅ CLEAN", "No critical actions needed"))
    
    for priority, action in actions:
        print(f"   {priority}: {action}")
    
    # Рекомендации
    print(f"\n💡 RECOMMENDATIONS:")
    if resp_metrics.is_god_object():
        print(f"   1. 🏗️  ARCHITECTURE: Split by Hot/Warm/Cold layers")
        print(f"      - Extract HotPathProcessor (WebSocket callbacks)")
        print(f"      - Extract ColdPathServices (logging, persistence)")
        print(f"      - Keep WarmPath in main class")
    
    # Рекомендации по типам
    type_recs = numeric_metrics.get_recommendations()
    if type_recs:
        print(f"\n   2. 📊 NUMERIC TYPES:")
        for rec in type_recs:
            print(f"      {rec}")
    
    if total_hot > 0:
        print(f"\n   3. 🔥 HOT PATH:")
        print(f"      - Remove string formatting from callbacks")
        print(f"      - Replace LINQ with manual loops")
        print(f"      - Use ObjectPool\u003cT\u003e for DTOs")
        print(f"      - Convert classes to structs")
    
    if total_warm > 50:
        print(f"\n   4. 🌡️  WARM PATH:")
        print(f"      - Use ArrayPool\u003cT\u003e for temporary buffers")
        print(f"      - Cache frequently accessed data")
    
    print()

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python calculate_allocation_score.py \u003cfile_path\u003e")
        print("\nHFT-Aware Analyzer with Architecture & Numeric Type Detection:")
        print("  🔥 HOT:   WebSocket callbacks (zero tolerance)")
        print("  🌡️  WARM: Validation, cache (minimal tolerance)")
        print("  ❄️  COLD: Logging, config (standard tolerance)")
        print("\nNumeric Types Best Practices:")
        print("  ✅ long (FixedPoint8): Hot path, storage, atomics")
        print("  🧮 double: Warm path, math, SIMD calculations")
        print("  🧊 decimal: Cold path, public API, JSON only")
        print("  🛡 Int128: Math safety (intermediate calculations)")
        print("\nArchitecture Detection:")
        print("  🏗️  God Object: \u003e500 LOC OR \u003e15 methods OR Hot+Warm+Cold mixed")
        print("  ⚠️  Layer Mixing: Hot+Cold in same class")
        print()
        sys.exit(1)
    
    calculate_hft_score(sys.argv[1])
