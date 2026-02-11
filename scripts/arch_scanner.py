#!/usr/bin/env python3
"""
Architecture Scanner v3.0 — AI-Optimized Deep Architecture Analyzer

Generates comprehensive documentation for AI consumption:
- AUTO_ARCH_SERVER.md — Detailed server analysis with code snippets
- AUTO_ARCH_CLIENT.md — Detailed client analysis  
- AI_BRIEF.md — Unified brief for quick understanding

Style: HFT_CSHARP_ULTIMATE_GUIDE.md format with ❌/✅ markers, code examples, contextual explanations.

Usage:
  python arch_scanner.py                    # Generate all reports
  python arch_scanner.py --client-only      # Client only
"""

import os
import re
import json
import argparse
from collections import defaultdict, Counter
from dataclasses import dataclass, field
from typing import List, Dict, Set, Optional, Tuple
from pathlib import Path
from datetime import datetime

# --- Configuration ---
PROJECT_ROOT = Path(__file__).parent.parent
SERVER_ROOT = PROJECT_ROOT / "src"
CLIENT_ROOT = PROJECT_ROOT / "src" / "SpreadAggregator.Presentation" / "wwwroot"
OUTPUT_DIR = PROJECT_ROOT / "docs" / "arch"

# Ensure output directory exists
os.makedirs(OUTPUT_DIR, exist_ok=True)

# Layer definitions for C# project
LAYER_MAP = {
    "Infrastructure": ["SpreadAggregator.Infrastructure"],
    "Application": ["SpreadAggregator.Application"],
    "Domain": ["SpreadAggregator.Domain"],
    "Presentation": ["SpreadAggregator.Presentation"],
}

# --- Latency Cost Model (nanoseconds) ---
LATENCY_COSTS = {
    "lock": 150,
    "linq": 100,
    "allocation": 50,
    "dictionary_lookup": 80,
    "datetime_utcnow": 100,
    "decimal_op": 20,
    "task_run": 300,
    "string_interpolation": 80,
}

# --- Regex Patterns ---
RX_NAMESPACE = re.compile(r'^\s*namespace\s+([\w\.]+)')
RX_CLASS = re.compile(r'^\s*(?:public|internal)\s+(?:sealed\s+|abstract\s+|static\s+|partial\s+|readonly\s+)*(class|struct|interface|record)\s+(\w+)')
RX_CTOR = re.compile(r'^\s*public\s+(\w+)\s*\((.*)\)')
RX_METHOD = re.compile(r'^\s*(?:public|private|protected|internal)\s+(?:static\s+|virtual\s+|override\s+|async\s+)*(?:[\w<>\[\]?,\s]+)\s+(\w+)\s*\(')
RX_SUMMARY = re.compile(r'///\s*<summary>\s*\n(.*?)\s*///\s*</summary>', re.DOTALL)
RX_SUMMARY_SINGLE = re.compile(r'///\s*<summary>(.*?)</summary>')
RX_FIELD = re.compile(r'^\s*(?:private|public|protected|internal)\s+(?:static\s+)?(?:readonly\s+)?(?:const\s+)?(.+?)\s+(_?\w+)\s*[=;]')

# HFT Anti-Pattern Detection
RX_LOCK = re.compile(r'\block\s*\(')
RX_LINQ = re.compile(r'(?<!Math)\.(?:Where|Select|ToList|ToArray|OrderBy|OrderByDescending|GroupBy|FirstOrDefault|LastOrDefault|Any|Count|Sum|Average|Min|Max)\s*\(')
RX_ALLOC = re.compile(r'\bnew\s+(?:List|Dictionary|ConcurrentDictionary|StringBuilder|Queue|Stack|HashSet)\s*[<\(]')
RX_TASK_RUN = re.compile(r'\bTask\.Run\b')
RX_UTC_NOW = re.compile(r'\bDateTime\.(?:UtcNow|Now)\b')
RX_DECIMAL = re.compile(r'\bdecimal\b')
RX_STRING_INTERP = re.compile(r'\$"[^"]*\{')
RX_DICT_ACCESS = re.compile(r'\.(?:TryGetValue|GetOrAdd|AddOrUpdate|ContainsKey)\s*\(')

# Good Pattern Detection
RX_SPAN = re.compile(r'\b(?:Span|ReadOnlySpan|Memory|ReadOnlyMemory)<')
RX_STRUCT = re.compile(r'^\s*(?:public|internal)\s+(?:readonly\s+)?struct\s+')
RX_RINGBUFFER = re.compile(r'\bRingBuffer\b')
RX_INTERLOCKED = re.compile(r'\bInterlocked\.')
RX_CHANNEL = re.compile(r'\bChannel<')
RX_MESSAGEPACK = re.compile(r'\bMessagePack')
RX_CONCURRENT_DICT = re.compile(r'\bConcurrentDictionary<')  # NEW: lock-free collection
RX_STOPWATCH = re.compile(r'\bStopwatch\.GetTimestamp')  # NEW: monotonic time

# Data Pipeline Architecture Detection
RX_POLLING = re.compile(r'\b(?:PeriodicTimer|WaitForNextTickAsync|Timer\.Elapsed)\b')
RX_EVENT_DRIVEN = re.compile(r'\bOn(?:Ticker|BookTicker|Trade)(?:Update|Received)\b')
RX_THROTTLE = re.compile(r'\b(?:THROTTLE_MS|_lastBroadcastTicks|elapsedMs)\b')
RX_BUSY_SPIN = re.compile(r'\b(?:SpinWait|SpinOnce|Thread\.SpinWait)\b')
RX_CONFIGURE_HFT = re.compile(r'\bConfigureHFT\b')

# Hot Path Indicators
HOT_PATH_INDICATORS = [
    "RingBuffer", "TickerUpdate", "IExchangeClient", "WebSocket",
    "OnBookTicker", "OnTrade", "ProcessTicker", "ParseMessage",
    "Broadcast", "Deviation", "Handler"
]

# NEW: Lock Context Classification
# Cold path methods - locks here are acceptable
COLD_PATH_METHODS = {
    'StartAsync', 'StopAsync', 'Initialize', 'Dispose', 'Configure',
    'GetExchangeHealth', 'EnsureCacheUpdated', 'EvictOldestSymbol',
    'constructor', 'ctor'
}
# Hot path methods - locks here are CRITICAL
HOT_PATH_METHOD_PATTERNS = {
    'OnTicker', 'OnTrade', 'OnBookTicker', 'Process', 'Calculate',
    'Broadcast', 'Handler', 'Update', 'Parse', 'Receive'
}
# Fine-grained lock patterns (acceptable)
RX_FINE_GRAINED_LOCK = re.compile(r'SymbolLockManager\.GetLock|GetLock\s*\(')
RX_CACHE_UPDATE_LOCK = re.compile(r'lock\s*\(\s*_\w*[Cc]ache')

# Client-side patterns
RX_JS_DATE_NOW = re.compile(r'(new\s+Date\s*\(|Date\.now\s*\()')
RX_JS_CONSOLE = re.compile(r'console\.(log|warn|error|debug)')
RX_JS_WEBSOCKET = re.compile(r'new\s+WebSocket\s*\(')
RX_SERVER_TIMESTAMP = re.compile(r'(msg|data|dev|d|message)\.\w*timestamp', re.IGNORECASE)
RX_MSGPACK_DECODE = re.compile(r'msgpack\.decode|MessagePack')
RX_JS_RINGBUFFER = re.compile(r'class\s+RingBuffer|new\s+RingBuffer')
RX_TIMESTAMP_CONVERT = re.compile(r'timestamp\s*/\s*1000')
RX_FLOAT64_ARRAY = re.compile(r'new\s+Float64Array')


@dataclass
class CodeIssue:
    """Contextual issue with explanation"""
    issue_type: str
    severity: str  # CRITICAL, WARNING, INFO
    file_path: str
    line_num: int
    code_snippet: str
    explanation: str
    fix_suggestion: str
    latency_impact: str = ""


@dataclass
class CodeStrength:
    """Positive pattern found in code"""
    pattern_type: str
    file_path: str
    line_num: int
    code_snippet: str
    explanation: str


@dataclass
class ServiceAnalysis:
    """Deep analysis of a service/class"""
    name: str
    kind: str  # class, struct, interface
    namespace: str
    layer: str  # Infrastructure, Application, Domain, Presentation
    file_path: str
    summary: str  # From XML docs
    dependencies: List[str] = field(default_factory=list)
    public_methods: List[str] = field(default_factory=list)
    key_code_snippet: str = ""
    issues: List[CodeIssue] = field(default_factory=list)
    strengths: List[CodeStrength] = field(default_factory=list)
    is_hot_path: bool = False
    health_score: int = 100
    lines: int = 0
    
    # Metrics
    lock_count: int = 0
    linq_count: int = 0
    alloc_count: int = 0
    span_count: int = 0
    
    # Pipeline Architecture Metrics
    polling_count: int = 0
    event_driven_count: int = 0
    throttle_count: int = 0
    ringbuffer_count: int = 0
    busy_spin_count: int = 0
    
    # Class fields
    class_fields: List[str] = field(default_factory=list)


@dataclass
class ClientAnalysis:
    """Analysis of frontend file"""
    name: str
    file_path: str
    file_type: str  # html, js
    summary: str
    issues: List[CodeIssue] = field(default_factory=list)
    strengths: List[CodeStrength] = field(default_factory=list)
    health_score: int = 100
    lines: int = 0
    
    # Specifics
    date_now_count: int = 0
    server_timestamp_count: int = 0
    websocket_count: int = 0
    console_log_count: int = 0
    msgpack_count: int = 0  # Track msgpack usage for deduplication


def get_layer(namespace: str) -> str:
    """Determine architectural layer from namespace"""
    for layer, patterns in LAYER_MAP.items():
        for pattern in patterns:
            if pattern in namespace:
                return layer
    return "Unknown"


def extract_summary(content: str, class_line: int) -> str:
    """Extract XML summary comment before class definition"""
    lines = content.split('\n')
    summary_lines = []
    
    # Look backwards from class line
    for i in range(class_line - 2, max(-1, class_line - 15), -1):
        if i < 0:
            break
        line = lines[i].strip()
        if line.startswith('///'):
            # Extract content
            text = line[3:].strip()
            text = re.sub(r'</?summary>', '', text).strip()
            if text:
                summary_lines.insert(0, text)
        elif not line or line.startswith('['):
            continue
        else:
            break
    
    return ' '.join(summary_lines) if summary_lines else ""


def extract_key_code(content: str, class_name: str, start_line: int) -> str:
    """Extract key method code (first public method with logic)"""
    lines = content.split('\n')
    in_method = False
    brace_count = 0
    method_lines = []
    
    for i, line in enumerate(lines[start_line:], start=start_line + 1):
        stripped = line.strip()
        
        # Find first meaningful public method
        if not in_method and RX_METHOD.match(stripped):
            if any(skip in stripped for skip in ['get;', 'set;', '=>']):
                continue
            if 'public' in stripped or 'internal' in stripped:
                in_method = True
                method_lines = [line.rstrip()]
                brace_count = stripped.count('{') - stripped.count('}')
                continue
        
        if in_method:
            method_lines.append(line.rstrip())
            brace_count += stripped.count('{') - stripped.count('}')
            
            if brace_count <= 0 or len(method_lines) > 25:
                break
    
    if method_lines and len(method_lines) > 3:
        return '\n'.join(method_lines[:20])
    return ""


def analyze_server_file(file_path: Path) -> List[ServiceAnalysis]:
    """Analyze a C# file and extract services with deep analysis"""
    services = []
    
    try:
        content = file_path.read_text(encoding='utf-8-sig', errors='ignore')
        lines = content.split('\n')
    except Exception as e:
        print(f"Error reading {file_path}: {e}")
        return []
    
    current_namespace = ""
    
    # Find namespace
    for line in lines:
        if m := RX_NAMESPACE.match(line):
            current_namespace = m.group(1)
            break
    
    # Find classes/structs
    for line_num, line in enumerate(lines):
        if m := RX_CLASS.match(line):
            kind = m.group(1)
            name = m.group(2)
            
            layer = get_layer(current_namespace)
            summary = extract_summary(content, line_num)
            key_code = extract_key_code(content, name, line_num)
            
            service = ServiceAnalysis(
                name=name,
                kind=kind,
                namespace=current_namespace,
                layer=layer,
                file_path=str(file_path.relative_to(PROJECT_ROOT)),
                summary=summary,
                key_code_snippet=key_code,
                lines=len(lines),
                is_hot_path=any(ind in name for ind in HOT_PATH_INDICATORS)
            )
            
            # Analyze content for issues and strengths
            analyze_service_content(service, content, lines, line_num)
            
            # Extract dependencies from constructor
            extract_dependencies(service, lines, line_num, name)
            
            # Extract class fields
            extract_class_fields(service, lines, line_num)
            
            services.append(service)
    
    return services


def analyze_service_content(service: ServiceAnalysis, content: str, lines: List[str], start_line: int):
    """Analyze service for issues and strengths"""
    
    # NEW: Track current method for context-aware analysis
    current_method = ""
    brace_depth = 0
    
    for line_num, line in enumerate(lines[start_line:], start=start_line + 1):
        stripped = line.strip()
        
        # IMPORTANT: Remove C# comments to avoid false positives
        # Remove single-line comments (//...)
        code_only = stripped
        if '//' in code_only:
            code_only = code_only[:code_only.index('//')].strip()
        
        # NEW: Track method context
        if m := RX_METHOD.match(stripped):
            method_name = m.group(1) if m.groups() else ""
            for part in stripped.split():
                if '(' in part:
                    current_method = part.split('(')[0]
                    break
        
        # Track brace depth to know when we exit a method
        brace_depth += stripped.count('{') - stripped.count('}')
        if brace_depth <= 0:
            current_method = ""
            brace_depth = 0
        
        # === ISSUES ===
        
        # Lock detection with CONTEXT
        if RX_LOCK.search(code_only):
            service.lock_count += 1
            
            # NEW: Determine lock severity based on context
            is_cold_method = any(cold in current_method for cold in COLD_PATH_METHODS)
            is_fine_grained = RX_FINE_GRAINED_LOCK.search(code_only)
            is_cache_lock = RX_CACHE_UPDATE_LOCK.search(code_only)
            
            # Classification logic
            if is_cold_method:
                # Cold path locks are acceptable
                severity = "INFO"
                explanation = "Lock in cold path (startup/shutdown) — acceptable, does not affect runtime latency."
                fix_suggestion = "Keep as is — this is not a hot path."
            elif is_fine_grained:
                # Fine-grained locks are acceptable
                severity = "INFO"
                explanation = "Fine-grained lock (per-symbol) — no contention between different symbols."
                fix_suggestion = "Keep as is — this is a correct pattern."
            elif is_cache_lock:
                # Cache update locks are acceptable (infrequent)
                severity = "INFO"
                explanation = "Cache update lock — executes rarely (once every N seconds)."
                fix_suggestion = "Keep as is — rare cache updates are acceptable."
            elif service.is_hot_path:
                # Hot path global lock = CRITICAL
                severity = "CRITICAL"
                explanation = "🔴 GLOBAL LOCK in hot path! Blocks all threads under high load."
                fix_suggestion = "Replace with ConcurrentDictionary, Interlocked, or RingBuffer."
            else:
                # Non-hot-path global lock = WARNING
                severity = "WARNING"
                explanation = "Lock may cause contention under high load."
                fix_suggestion = "Consider ConcurrentDictionary if call frequency is high."
            
            service.issues.append(CodeIssue(
                issue_type="LOCK_CONTENTION",
                severity=severity,
                file_path=service.file_path,
                line_num=line_num,
                code_snippet=stripped,
                explanation=explanation,
                fix_suggestion=fix_suggestion,
                latency_impact=f"{LATENCY_COSTS['lock']}ns per lock" if severity == "CRITICAL" else ""
            ))
        
        # LINQ detection
        if RX_LINQ.search(code_only):
            service.linq_count += 1
            if service.linq_count <= 3:  # Limit reported
                service.issues.append(CodeIssue(
                    issue_type="LINQ_IN_PATH",
                    severity="WARNING" if service.is_hot_path else "INFO",
                    file_path=service.file_path,
                    line_num=line_num,
                    code_snippet=stripped,
                    explanation="LINQ creates iterator objects and delegate allocations. Causes GC pressure in hot paths.",
                    fix_suggestion="Replace with for-loop using Span<T> or pre-allocated buffer.",
                    latency_impact=f"{LATENCY_COSTS['linq']}ns per call"
                ))
        
        # Allocation detection
        if RX_ALLOC.search(code_only):
            service.alloc_count += 1
            if service.alloc_count <= 2:
                service.issues.append(CodeIssue(
                    issue_type="HEAP_ALLOCATION",
                    severity="WARNING" if service.is_hot_path else "INFO",
                    file_path=service.file_path,
                    line_num=line_num,
                    code_snippet=stripped,
                    explanation="Creating collections in hot path increases Gen0 GC pressure.",
                    fix_suggestion="Use ArrayPool<T>.Shared or pre-allocated structures.",
                    latency_impact=f"{LATENCY_COSTS['allocation']}ns per allocation"
                ))
        
        # DateTime.Now detection (CRITICAL FIX: check code_only to ignore comments)
        if RX_UTC_NOW.search(code_only):
            service.issues.append(CodeIssue(
                issue_type="DATETIME_SYSCALL",
                severity="CRITICAL" if service.is_hot_path else "WARNING",
                file_path=service.file_path,
                line_num=line_num,
                code_snippet=stripped,
                explanation="DateTime.UtcNow makes a system call (~100ns). Violates 'Trust Transaction Time' principle.",
                fix_suggestion="Use Stopwatch.GetTimestamp() or exchange timestamp.",
                latency_impact=f"{LATENCY_COSTS['datetime_utcnow']}ns per call"
            ))
        
        # === STRENGTHS ===
        
        # Span usage
        if RX_SPAN.search(line):
            service.span_count += 1
            if service.span_count == 1:
                service.strengths.append(CodeStrength(
                    pattern_type="ZERO_ALLOC_SPAN",
                    file_path=service.file_path,
                    line_num=line_num,
                    code_snippet=stripped,
                    explanation="Span<T> usage ensures zero-allocation memory operations. Excellent HFT pattern!"
                ))
        
        # RingBuffer usage
        if RX_RINGBUFFER.search(line):
            service.strengths.append(CodeStrength(
                pattern_type="RING_BUFFER",
                file_path=service.file_path,
                line_num=line_num,
                code_snippet=stripped,
                explanation="RingBuffer — lock-free structure with O(1) operations. Ideal for high-frequency updates."
            ))
        
        # Interlocked usage
        if RX_INTERLOCKED.search(line):
            service.strengths.append(CodeStrength(
                pattern_type="LOCK_FREE_ATOMIC",
                file_path=service.file_path,
                line_num=line_num,
                code_snippet=stripped,
                explanation="Interlocked operations — atomic without lock contention. Correct approach for multithreading."
            ))
        # MessagePack usage
        if RX_MESSAGEPACK.search(line):
            service.strengths.append(CodeStrength(
                pattern_type="BINARY_SERIALIZATION",
                file_path=service.file_path,
                line_num=line_num,
                code_snippet=stripped,
                explanation="MessagePack — fast binary serialization. 2-10x faster than JSON."
            ))
        
        # NEW: ConcurrentDictionary detection (lock-free)
        if RX_CONCURRENT_DICT.search(line):
            service.strengths.append(CodeStrength(
                pattern_type="LOCK_FREE_COLLECTION",
                file_path=service.file_path,
                line_num=line_num,
                code_snippet=stripped,
                explanation="ConcurrentDictionary — lock-free collection. Correct choice for shared state in HFT."
            ))
        
        # NEW: Stopwatch.GetTimestamp detection (monotonic time)
        if RX_STOPWATCH.search(line):
            service.strengths.append(CodeStrength(
                pattern_type="MONOTONIC_TIME",
                file_path=service.file_path,
                line_num=line_num,
                code_snippet=stripped,
                explanation="Stopwatch.GetTimestamp — monotonic clock without DateTime.UtcNow overhead."
            ))
        
        # === PIPELINE ARCHITECTURE DETECTION ===
        
        # Polling detection
        if RX_POLLING.search(line):
            service.polling_count += 1
            if service.polling_count == 1:
                service.issues.append(CodeIssue(
                    issue_type="POLLING_PATTERN",
                    severity="INFO",
                    file_path=service.file_path,
                    line_num=line_num,
                    code_snippet=stripped,
                    explanation="Polling pattern: service wakes up on timer. Can add up to 50ms latency.",
                    fix_suggestion="Consider event-driven approach with OnTickerUpdate callback + throttle."
                ))
        
        # Event-driven detection
        if RX_EVENT_DRIVEN.search(line):
            service.event_driven_count += 1
            if service.event_driven_count == 1:
                service.strengths.append(CodeStrength(
                    pattern_type="EVENT_DRIVEN",
                    file_path=service.file_path,
                    line_num=line_num,
                    code_snippet=stripped,
                    explanation="Event-driven architecture: ~0ms latency on initial signal. Ideal HFT pattern!"
                ))
        
        # Throttle detection
        if RX_THROTTLE.search(line):
            service.throttle_count += 1
            if service.throttle_count == 1:
                service.strengths.append(CodeStrength(
                    pattern_type="THROTTLED_BROADCAST",
                    file_path=service.file_path,
                    line_num=line_num,
                    code_snippet=stripped,
                    explanation="Throttle broadcasts: controlled emission rate without flooding client."
                ))
        
        # RingBuffer count (for pipeline metrics)
        if RX_RINGBUFFER.search(line):
            service.ringbuffer_count += 1
        
        # Busy spin detection
        if RX_BUSY_SPIN.search(line):
            service.busy_spin_count += 1
            if service.busy_spin_count == 1:
                service.strengths.append(CodeStrength(
                    pattern_type="BUSY_SPIN",
                    file_path=service.file_path,
                    line_num=line_num,
                    code_snippet=stripped,
                    explanation="Busy spin: lowest possible latency at the cost of 100% single-core CPU usage."
                ))
    
    # Struct bonus
    if service.kind == 'struct':
        service.strengths.append(CodeStrength(
            pattern_type="VALUE_TYPE_DTO",
            file_path=service.file_path,
            line_num=0,
            code_snippet=f"public struct {service.name}",
            explanation="Struct is stack-allocated, avoids GC. Excellent choice for HFT DTOs."
        ))
    
    # Calculate health score
    calculate_health_score(service)


def calculate_health_score(service: ServiceAnalysis):
    """Calculate health score based on issues and strengths - IMPROVED VERSION"""
    score = 100
    
    # NEW: Count issues by severity (not just total locks)
    critical_issues = [i for i in service.issues if i.severity == "CRITICAL"]
    warning_issues = [i for i in service.issues if i.severity == "WARNING"]
    info_issues = [i for i in service.issues if i.severity == "INFO"]
    
    # Penalties based on SEVERITY, not raw counts
    if service.is_hot_path:
        # Hot path - critical issues are very bad
        score -= len(critical_issues) * 20
        score -= len(warning_issues) * 5
        # INFO issues = 0 penalty (cold path locks, fine-grained locks)
    else:
        # Cold path - minimal penalties
        score -= len(critical_issues) * 10
        score -= len(warning_issues) * 2
        # INFO = 0 penalty
    
    # LINQ penalty (still relevant but reduced)
    score -= int(min(service.linq_count, 10) * 2)
    
    # Bonuses (increased to balance)
    if service.kind == 'struct':
        score += 10
    score += min(service.span_count * 3, 15)
    score += min(len(service.strengths) * 2, 20)  # NEW: Bonus for strengths
    
    service.health_score = max(0, min(100, score))


def extract_dependencies(service: ServiceAnalysis, lines: List[str], start_line: int, class_name: str):
    """Extract constructor dependencies"""
    for i in range(start_line, min(start_line + 50, len(lines))):
        line = lines[i].strip()
        if m := RX_CTOR.match(line):
            if m.group(1) == class_name:
                args = m.group(2)
                deps = []
                for arg in args.split(','):
                    arg = arg.strip()
                    if arg:
                        parts = arg.split()
                        if parts:
                            dep_type = parts[0].split('<')[0].replace('?', '')
                            if dep_type not in ['int', 'string', 'bool', 'double', 'decimal', 'long', 'ILogger']:
                                deps.append(dep_type)
                service.dependencies = deps
                break


def extract_class_fields(service: ServiceAnalysis, lines: List[str], start_line: int):
    """Extract class fields for 'Current State' section"""
    fields = []
    brace_count = 0
    in_class = False
    
    for i in range(start_line, min(start_line + 1000, len(lines))):
        line = lines[i]
        stripped = line.strip()
        
        # Track class body
        brace_count += stripped.count('{') - stripped.count('}')
        if '{' in stripped and not in_class:
            in_class = True
            continue
        
        if brace_count <= 0 and in_class:
            break  # End of class
        
        # Skip methods, properties with bodies (but not new() initializers)
        if '=>' in stripped:
            continue
        if '(' in stripped and 'new()' not in stripped and 'new (' not in stripped:
            continue
        
        # Match field declarations
        if m := RX_FIELD.match(line):
            field_type = m.group(1).strip()
            field_name = m.group(2).strip()
            
            # Skip backing fields for properties
            if field_name.startswith('_') or 'static' in line.lower() or 'readonly' in line.lower():
                # Extract value if present
                if '=' in stripped:
                    value_part = stripped.split('=', 1)[1].strip().rstrip(';')
                    if len(value_part) > 50:
                        value_part = value_part[:47] + "..."
                    fields.append(f"`{field_name}`: `{field_type}` = {value_part}")
                else:
                    fields.append(f"`{field_name}`: `{field_type}`")
    
    service.class_fields = fields[:10]  # Limit to 10 fields


def analyze_client_file(file_path: Path) -> Optional[ClientAnalysis]:
    """Analyze HTML/JS file"""
    try:
        content = file_path.read_text(encoding='utf-8', errors='ignore')
        lines = content.split('\n')
    except Exception:
        return None
    
    name = file_path.name
    file_type = file_path.suffix[1:]
    
    # Determine summary based on filename
    summary_map = {
        "index.html": "Screener main page — real-time table with all ticker data",
        "dashboard.html": "Trading Bot Dashboard — trading bot monitoring",
        "worker.js": "Web Worker for background data processing",
        "screener.js": "Screener table logic with WebSocket connection",
    }
    summary = summary_map.get(name, f"{file_type.upper()} client file")
    
    analysis = ClientAnalysis(
        name=name,
        file_path=str(file_path.relative_to(PROJECT_ROOT)),
        file_type=file_type,
        summary=summary,
        lines=len(lines)
    )
    
    for line_num, line in enumerate(lines, start=1):
        # Issues
        if RX_JS_DATE_NOW.search(line):
            analysis.date_now_count += 1
            if analysis.date_now_count <= 2:
                analysis.issues.append(CodeIssue(
                    issue_type="CLIENT_TIMESTAMP",
                    severity="CRITICAL",
                    file_path=analysis.file_path,
                    line_num=line_num,
                    code_snippet=line.strip()[:80],
                    explanation="Date.now() uses client time, which may drift from server time by seconds.",
                    fix_suggestion="Use timestamp from WebSocket message (server time).",
                    latency_impact="Clock skew risk"
                ))
        
        if RX_JS_CONSOLE.search(line):
            analysis.console_log_count += 1
        
        # Strengths
        if RX_SERVER_TIMESTAMP.search(line):
            analysis.server_timestamp_count += 1
            if analysis.server_timestamp_count == 1:
                analysis.strengths.append(CodeStrength(
                    pattern_type="SERVER_TIMESTAMP",
                    file_path=analysis.file_path,
                    line_num=line_num,
                    code_snippet=line.strip()[:80],
                    explanation="Uses server timestamp — correct approach for time synchronization."
                ))
        
        if RX_JS_WEBSOCKET.search(line):
            analysis.websocket_count += 1
            if analysis.websocket_count == 1:  # Only add strength once
                analysis.strengths.append(CodeStrength(
                    pattern_type="WEBSOCKET_REALTIME",
                    file_path=analysis.file_path,
                    line_num=line_num,
                    code_snippet=line.strip()[:80],
                    explanation="WebSocket connection for real-time updates."
                ))
        
        if RX_MSGPACK_DECODE.search(line):
            analysis.msgpack_count += 1
            if analysis.msgpack_count == 1:  # Only add strength once
                analysis.strengths.append(CodeStrength(
                    pattern_type="MSGPACK_DECODE",
                    file_path=analysis.file_path,
                    line_num=line_num,
                    code_snippet=line.strip()[:80],
                    explanation="MessagePack decoding — faster than JSON parsing."
                ))
        
        # HFT JS Patterns
        if RX_JS_RINGBUFFER.search(line):
            analysis.strengths.append(CodeStrength(
                pattern_type="JS_RINGBUFFER",
                file_path=analysis.file_path,
                line_num=line_num,
                code_snippet=line.strip()[:80],
                explanation="JS RingBuffer — O(1) push/eviction, avoids GC overhead from Array.shift()."
            ))
        
        if RX_TIMESTAMP_CONVERT.search(line):
            analysis.strengths.append(CodeStrength(
                pattern_type="TIMESTAMP_CONVERSION",
                file_path=analysis.file_path,
                line_num=line_num,
                code_snippet=line.strip()[:80],
                explanation="Server timestamp conversion (ms → sec) for uPlot — proper HFT approach."
            ))
        
        if RX_FLOAT64_ARRAY.search(line):
            analysis.strengths.append(CodeStrength(
                pattern_type="TYPED_ARRAY",
                file_path=analysis.file_path,
                line_num=line_num,
                code_snippet=line.strip()[:80],
                explanation="Float64Array — zero-allocation typed array for numeric data."
            ))
    
    # Console.log warning
    if analysis.console_log_count > 5:
        analysis.issues.append(CodeIssue(
            issue_type="CONSOLE_LOG_SPAM",
            severity="WARNING",
            file_path=analysis.file_path,
            line_num=0,
            code_snippet=f"{analysis.console_log_count} console.log calls",
            explanation="Multiple console.log calls slow down execution in production.",
            fix_suggestion="Remove or wrap in DEBUG condition."
        ))
    
    # Health score
    score = 100
    score -= analysis.date_now_count * 20
    score -= min(analysis.console_log_count, 10) * 2
    score += analysis.server_timestamp_count * 10
    analysis.health_score = max(0, min(100, score))
    
    return analysis


def scan_server() -> List[ServiceAnalysis]:
    """Scan all server C# files"""
    services = []
    
    for file_path in SERVER_ROOT.rglob("*.cs"):
        # Skip generated files
        if any(skip in str(file_path) for skip in ['obj', 'bin', 'Debug', 'Release']):
            continue
        
        file_services = analyze_server_file(file_path)
        services.extend(file_services)
    
    # Propagate hot path through dependencies
    propagate_hot_path(services)
    
    return services


def scan_client() -> List[ClientAnalysis]:
    """Scan client files"""
    clients = []
    
    if not CLIENT_ROOT.exists():
        return clients
    
    for ext in ['*.html', '*.js']:
        for file_path in CLIENT_ROOT.rglob(ext):
            if analysis := analyze_client_file(file_path):
                clients.append(analysis)
    
    return clients


def find_dead_css() -> List[Dict]:
    """Find CSS classes not used in any HTML file"""
    dead_css = []
    
    css_path = CLIENT_ROOT / "css" / "styles.css"
    if not css_path.exists():
        return dead_css
    
    try:
        css_content = css_path.read_text(encoding='utf-8', errors='ignore')
    except Exception:
        return dead_css
    
    # Extract CSS class definitions (e.g., .classname, .class-name)
    RX_CSS_CLASS_DEF = re.compile(r'\.([a-zA-Z_][a-zA-Z0-9_-]*)\s*[,{:]')
    css_classes = set(RX_CSS_CLASS_DEF.findall(css_content))
    
    # Find all HTML files and extract used classes
    used_classes = set()
    for html_path in CLIENT_ROOT.rglob("*.html"):
        try:
            html_content = html_path.read_text(encoding='utf-8', errors='ignore')
            
            # Extract class="..." and class='...' from HTML
            class_matches = re.findall(r'class=["\']([^"\']+)["\']', html_content)
            for match in class_matches:
                used_classes.update(match.split())
            
            # Extract inline JavaScript from <script> tags
            script_blocks = re.findall(r'<script[^>]*>(.*?)</script>', html_content, re.DOTALL)
            for script in script_blocks:
                # Multiple JS patterns for class usage
                patterns = [
                    r'className\s*=\s*["\']([^"\']+)["\']',  # className = 'foo'
                    r'classList\.(?:add|remove|toggle|contains)\s*\(\s*["\']([^"\']+)["\']',  # classList methods
                    r'\.className\s*=\s*["\']([^"\']+)["\']',  # element.className = 'foo'
                    r'setAttribute\s*\(\s*["\']class["\']\s*,\s*["\']([^"\']+)["\']',  # setAttribute('class', 'foo')
                ]
                for pattern in patterns:
                    matches = re.findall(pattern, script)
                    for match in matches:
                        used_classes.update(match.split())
        except Exception:
            continue
    
    # Also check standalone JS files for dynamic class usage
    for js_path in CLIENT_ROOT.rglob("*.js"):
        try:
            js_content = js_path.read_text(encoding='utf-8', errors='ignore')
            # Same patterns as above
            patterns = [
                r'className\s*=\s*["\']([^"\']+)["\']',
                r'classList\.(?:add|remove|toggle|contains)\s*\(\s*["\']([^"\']+)["\']',
                r'\.className\s*=\s*["\']([^"\']+)["\']',
                r'setAttribute\s*\(\s*["\']class["\']\s*,\s*["\']([^"\']+)["\']',
            ]
            for pattern in patterns:
                matches = re.findall(pattern, js_content)
                for match in matches:
                    used_classes.update(match.split())
        except Exception:
            continue
    
    # Exclude pseudo-classes, state modifiers, and library classes
    EXCLUDE_PREFIXES = {'hover', 'active', 'focus', 'disabled', 'checked', 'before', 'after'}
    EXCLUDE_EXACT = {'uplot', 'hidden'}  # Library/utility classes that are standard
    
    # Find unused classes
    for css_class in css_classes:
        # Skip pseudo-classes and state modifiers
        if any(css_class.startswith(prefix) for prefix in EXCLUDE_PREFIXES):
            continue
        
        # Skip library/utility classes
        if css_class in EXCLUDE_EXACT:
            continue
        
        # Check if base class name is used
        base_class = css_class.split('.')[0]
        if base_class in used_classes:
            continue
        
        # Find line number in CSS
        line_num = 0
        for i, line in enumerate(css_content.split('\n'), 1):
            if f'.{css_class}' in line:
                line_num = i
                break
        
        dead_css.append({
            'class': css_class,
            'line': line_num,
            'file': str(css_path.relative_to(PROJECT_ROOT))
        })
    
    return dead_css


def propagate_hot_path(services: List[ServiceAnalysis]):

    """Propagate hot path status through dependencies"""
    name_to_service = {s.name: s for s in services}
    
    changed = True
    iterations = 0
    while changed and iterations < 10:
        changed = False
        iterations += 1
        
        for service in services:
            if service.is_hot_path:
                for dep_name in service.dependencies:
                    clean_name = dep_name.split('<')[0]
                    if clean_name in name_to_service:
                        if not name_to_service[clean_name].is_hot_path:
                            name_to_service[clean_name].is_hot_path = True
                            changed = True


def _generate_service_detail(service: ServiceAnalysis) -> List[str]:
    """Generate detailed section for a single service"""
    lines = []
    
    hot_marker = " 🔥" if service.is_hot_path else ""
    health_emoji = "🟢" if service.health_score >= 80 else ("🟡" if service.health_score >= 50 else "🔴")
    
    lines.append(f"### {service.name}{hot_marker} [{health_emoji} {service.health_score}/100]\n")
    
    lines.append(f"**Type:** `{service.kind}` | **Namespace:** `{service.namespace}`")
    lines.append(f"**File:** `{service.file_path}`\n")
    
    if service.summary:
        lines.append(f"**Purpose:** {service.summary}\n")
    
    if service.dependencies:
        lines.append(f"**Dependencies:** {', '.join(service.dependencies)}\n")
    
    # Class fields (Current State)
    if service.class_fields:
        lines.append("**📋 Current State (Fields):**")
        for f in service.class_fields[:5]:
            lines.append(f"- {f}")
        lines.append("")
    
    # Key code
    if service.key_code_snippet:
        lines.append("**Key Code:**")
        lines.append("```csharp")
        lines.append(service.key_code_snippet)
        lines.append("```\n")
    
    # Strengths
    if service.strengths:
        lines.append("**✅ Strengths:**")
        for s in service.strengths[:3]:
            lines.append(f"- **{s.pattern_type}:** {s.explanation}")
        lines.append("")
    
    # Issues
    if service.issues:
        lines.append("**🔴 Issues:**")
        for issue in service.issues[:3]:
            lines.append(f"\n**{issue.issue_type}** (line {issue.line_num}):")
            lines.append(f"```csharp\n{issue.code_snippet}\n```")
            lines.append(f"- *Issue:* {issue.explanation}")
            lines.append(f"- *Fix:* {issue.fix_suggestion}")
            if issue.latency_impact:
                lines.append(f"- *Latency:* {issue.latency_impact}")
        lines.append("")
    
    return lines


def generate_summary_report(services: List[ServiceAnalysis]) -> str:
    """Generate concise summary report with table and critical issues"""
    lines = []
    
    # Header
    lines.append("# 📊 Server Architecture Summary\n")
    lines.append(f"> **Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M')}")
    lines.append(f"> **Components:** {len(services)} | **Hot Path:** {len([s for s in services if s.is_hot_path])}")
    
    total_health = sum(s.health_score for s in services) // max(len(services), 1)
    lines.append(f"> **Overall Health:** {total_health}/100\n")
    
    # Health by layer
    lines.append("## 🏗️ Health by Layer\n")
    lines.append("| Layer | Components | Avg Health |")
    lines.append("|-------|------------|------------|")
    
    by_layer = defaultdict(list)
    for s in services:
        by_layer[s.layer].append(s.health_score)
    
    for layer in ["Infrastructure", "Application", "Domain", "Presentation"]:
        if layer in by_layer:
            scores = by_layer[layer]
            avg = sum(scores) // len(scores)
            emoji = "🟢" if avg >= 80 else ("🟡" if avg >= 50 else "🔴")
            lines.append(f"| {layer} | {len(scores)} | {emoji} {avg}/100 |")
    
    lines.append("")
    
    # Critical issues (red components)
    critical = [s for s in services if s.health_score < 50]
    if critical:
        lines.append("## 🔴 Critical Components (Health < 50)\n")
        for s in sorted(critical, key=lambda x: x.health_score):
            hot = " 🔥" if s.is_hot_path else ""
            issues_summary = ", ".join([i.issue_type for i in s.issues[:3]])
            lines.append(f"- **{s.name}**{hot} [{s.health_score}/100] — {issues_summary}")
        lines.append("")
    
    # Component Summary table
    lines.append("## 📋 Component Summary\n")
    lines.append("| Component | Type | Layer | Hot | Health | Locks | LINQ | Span |")
    lines.append("|-----------|------|-------|-----|--------|-------|------|------|")
    
    for s in sorted(services, key=lambda x: (not x.is_hot_path, -x.health_score)):
        hot = "🔥" if s.is_hot_path else ""
        health = f"{'🟢' if s.health_score >= 80 else ('🟡' if s.health_score >= 50 else '🔴')}{s.health_score}"
        lines.append(f"| {s.name} | `{s.kind}` | {s.layer} | {hot} | {health} | {s.lock_count} | {s.linq_count} | {s.span_count} |")
    
    lines.append("")
    
    # NEW: Actionable Optimizations section
    actionable = []
    for s in services:
        for issue in s.issues:
            if issue.severity == "CRITICAL":
                priority = "⭐⭐⭐" if s.is_hot_path else "⭐⭐"
                actionable.append((priority, s.name, issue.issue_type, issue.line_num, issue.fix_suggestion[:50]))
    
    if actionable:
        lines.append("## 🎯 Actionable Optimizations\n")
        lines.append("> Only CRITICAL issues in hot path components are shown here.\n")
        lines.append("| Priority | Component | Issue | Line | Suggestion |")
        lines.append("|----------|-----------|-------|------|------------|")
        for p, name, issue_type, line, fix in sorted(actionable, key=lambda x: x[0], reverse=True)[:10]:
            lines.append(f"| {p} | {name} | {issue_type} | L{line} | {fix}... |")
        lines.append("")
    else:
        lines.append("## 🎯 Actionable Optimizations\n")
        lines.append("> ✅ No critical hot path issues found! Architecture is optimized.\n")
    
    lines.append("\n---\n")
    lines.append("> See `AUTO_ARCH_HOTPATH.md` for hot path details, `AUTO_ARCH_DETAILS.md` for full component analysis.")
    
    return "\n".join(lines)


def generate_hotpath_report(services: List[ServiceAnalysis]) -> str:
    """Generate detailed report for hot path components only"""
    lines = []
    
    hot_services = [s for s in services if s.is_hot_path]
    
    # Header
    lines.append("# 🔥 Hot Path Components Analysis\n")
    lines.append(f"> **Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M')}")
    lines.append(f"> **Hot Path Components:** {len(hot_services)}")
    
    avg_health = sum(s.health_score for s in hot_services) // max(len(hot_services), 1)
    lines.append(f"> **Average Health:** {avg_health}/100\n")
    
    lines.append("> ⚠️ These components are on the critical data path. Performance issues here directly impact latency.\n")
    
    # Group by layer
    by_layer = defaultdict(list)
    for s in hot_services:
        by_layer[s.layer].append(s)
    
    layer_order = ["Infrastructure", "Application", "Domain", "Presentation"]
    
    for layer in layer_order:
        layer_services = by_layer.get(layer, [])
        if not layer_services:
            continue
        
        lines.append(f"\n---\n\n## 🏗️ {layer} Layer\n")
        
        # Sort by health (worst first for hot path - to prioritize fixes)
        layer_services.sort(key=lambda s: s.health_score)
        
        for service in layer_services:
            lines.extend(_generate_service_detail(service))
    
    return "\n".join(lines)


def generate_details_report(services: List[ServiceAnalysis]) -> str:
    """Generate detailed report for non-hot-path components"""
    lines = []
    
    other_services = [s for s in services if not s.is_hot_path]
    
    # Header
    lines.append("# 📁 Component Details (Non-Hot-Path)\n")
    lines.append(f"> **Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M')}")
    lines.append(f"> **Components:** {len(other_services)}\n")
    
    lines.append("> For hot path components (🔥), see `AUTO_ARCH_HOTPATH.md`\n")
    
    # Group by layer
    by_layer = defaultdict(list)
    for s in other_services:
        by_layer[s.layer].append(s)
    
    layer_order = ["Infrastructure", "Application", "Domain", "Presentation", "Unknown"]
    
    for layer in layer_order:
        layer_services = by_layer.get(layer, [])
        if not layer_services:
            continue
        
        lines.append(f"\n---\n\n## 🏗️ {layer} Layer\n")
        
        # Sort by health
        layer_services.sort(key=lambda s: s.health_score)
        
        for service in layer_services:
            lines.extend(_generate_service_detail(service))
    
    return "\n".join(lines)


def generate_client_report(clients: List[ClientAnalysis]) -> str:
    """Generate detailed client architecture report"""
    lines = []
    
    lines.append("# 🌐 Client Architecture Analysis\n")
    lines.append(f"> **Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M')}")
    lines.append(f"> **Components:** {len(clients)}")
    
    total_health = sum(c.health_score for c in clients) // max(len(clients), 1)
    lines.append(f"> **Overall Health:** {total_health}/100\n")
    
    lines.append("---\n")
    
    for client in sorted(clients, key=lambda c: c.health_score):
        health_emoji = "🟢" if client.health_score >= 80 else ("🟡" if client.health_score >= 50 else "🔴")
        
        lines.append(f"## {client.name} [{health_emoji} {client.health_score}/100]\n")
        lines.append(f"**Type:** `{client.file_type}` | **Lines:** {client.lines}")
        lines.append(f"**File:** `{client.file_path}`\n")
        lines.append(f"**Purpose:** {client.summary}\n")
        
        # Metrics
        lines.append("**Metrics:**")
        lines.append(f"- WebSocket connections: {client.websocket_count}")
        lines.append(f"- Server timestamp usage: {client.server_timestamp_count}")
        lines.append(f"- Date.now() calls: {client.date_now_count}")
        lines.append(f"- Console.log calls: {client.console_log_count}\n")
        
        # Strengths
        if client.strengths:
            lines.append("**✅ Strengths:**")
            for s in client.strengths:
                lines.append(f"- **{s.pattern_type}:** {s.explanation}")
            lines.append("")
        
        # Issues
        if client.issues:
            lines.append("**🔴 Issues:**")
            for issue in client.issues:
                lines.append(f"\n**{issue.issue_type}** (line {issue.line_num}):")
                if issue.code_snippet and issue.line_num > 0:
                    lines.append(f"```javascript\n{issue.code_snippet}\n```")
                lines.append(f"- *Issue:* {issue.explanation}")
                lines.append(f"- *Fix:* {issue.fix_suggestion}")
            lines.append("")
        
        lines.append("---\n")
    
    # Timestamp flow section
    lines.append("## ⏱️ Timestamp Flow Analysis\n")
    lines.append("| Component | Date.now() | Server TS | Verdict |")
    lines.append("|-----------|------------|-----------|---------|")
    
    for c in clients:
        if c.server_timestamp_count > 0 and c.date_now_count == 0:
            verdict = "✅ SERVER TS"
        elif c.date_now_count > 0:
            verdict = "🔴 CLIENT TS"
        else:
            verdict = "ℹ️ N/A"
        lines.append(f"| {c.name} | {c.date_now_count} | {c.server_timestamp_count} | {verdict} |")
    
    # Dead CSS section
    dead_css = find_dead_css()
    if dead_css:
        lines.append("\n## 🗑️ Dead CSS Classes\n")
        lines.append("> CSS classes defined but not used in any HTML/JS file.\n")
        lines.append("| Class | Line | File |")
        lines.append("|-------|------|------|")
        for item in sorted(dead_css, key=lambda x: x['line']):
            lines.append(f"| `.{item['class']}` | L{item['line']} | `{item['file']}` |")
        lines.append(f"\n**Total:** {len(dead_css)} unused classes\n")
    else:
        lines.append("\n## 🗑️ Dead CSS Classes\n")
        lines.append("> ✅ No unused CSS classes detected!\n")
    
    return "\n".join(lines)


def generate_ai_brief(services: List[ServiceAnalysis], clients: List[ClientAnalysis]) -> str:
    """Generate unified AI brief for quick understanding"""
    lines = []
    
    lines.append("# 🤖 AI Architecture Brief\n")
    lines.append("> Concise overview for quick architecture understanding by other AIs.\n")
    
    # System Identity
    lines.append("## 📋 System Identity\n")
    lines.append("| Attribute | Value |")
    lines.append("|-----------|-------|")
    lines.append("| **Purpose** | Real-time crypto futures screener with HFT deviation analysis |")
    lines.append("| **Tech Stack** | C# .NET 9, WebSocket, MessagePack, HTML/JS |")
    lines.append("| **Architecture** | Clean Architecture (Domain → Application → Infrastructure → Presentation) |")
    lines.append("| **Exchanges** | Binance, Bybit (enabled) | MEXC, Gate.io, OKX (disabled) |")
    lines.append("| **Output** | Web dashboard (dashboard.html), Screener (index.html) |\n")
    
    # Data Flow
    lines.append("## 🔄 Data Flow\n")
    lines.append("```")
    lines.append("1. Exchange WebSocket (Binance/Bybit) отправляет BookTicker/Trade JSON")
    lines.append("2. *WebSocketClient парсит в TickerUpdate struct (zero-allocation)")
    lines.append("3. OrchestrationService получает callback, обновляет TickerData")
    lines.append("4. BroadcastCoordinatorService сериализует в MessagePack/JSON")
    lines.append("5. FleckWebSocketServer отправляет клиентам через WebSocket")
    lines.append("6. dashboard.html декодирует MessagePack, обновляет графики/стаканы")
    lines.append("7. index.html парсит JSON, обновляет таблицу screener")
    lines.append("```\n")
    
    # Key Components
    lines.append("## 🧩 Key Components\n")
    
    hot_services = [s for s in services if s.is_hot_path][:5]
    for s in hot_services:
        desc = s.summary if s.summary else f"{s.kind} в {s.layer} layer"
        lines.append(f"- **{s.name}** 🔥 — {desc}")
    
    lines.append("")
    
    # Strengths
    all_strengths = []
    for s in services:
        all_strengths.extend(s.strengths)
    for c in clients:
        all_strengths.extend(c.strengths)
    
    lines.append("## ✅ Strengths\n")
    strength_types = set()
    for st in all_strengths:
        if st.pattern_type not in strength_types:
            strength_types.add(st.pattern_type)
            lines.append(f"- **{st.pattern_type}:** {st.explanation}")
    lines.append("")
    
    # Weaknesses
    all_issues = []
    for s in services:
        if s.is_hot_path:
            all_issues.extend(s.issues)
    for c in clients:
        all_issues.extend(c.issues)
    
    critical_issues = [i for i in all_issues if i.severity == "CRITICAL"]
    
    lines.append("## ⚠️ Weaknesses\n")
    issue_types = set()
    for issue in critical_issues[:5]:
        if issue.issue_type not in issue_types:
            issue_types.add(issue.issue_type)
            lines.append(f"- **{issue.issue_type}** in `{issue.file_path}`: {issue.explanation[:100]}...")
    
    if not critical_issues:
        lines.append("- No critical issues detected! 🎉")
    lines.append("")
    
    # Health Summary
    lines.append("## 📊 Health Summary\n")
    lines.append("| Layer | Components | Avg Health |")
    lines.append("|-------|------------|------------|")
    
    by_layer = defaultdict(list)
    for s in services:
        by_layer[s.layer].append(s.health_score)
    
    for layer, scores in by_layer.items():
        avg = sum(scores) // len(scores)
        emoji = "🟢" if avg >= 80 else ("🟡" if avg >= 50 else "🔴")
        lines.append(f"| {layer} | {len(scores)} | {emoji} {avg}/100 |")
    
    if clients:
        avg = sum(c.health_score for c in clients) // len(clients)
        emoji = "🟢" if avg >= 80 else ("🟡" if avg >= 50 else "🔴")
        lines.append(f"| Client | {len(clients)} | {emoji} {avg}/100 |")
    
    return "\n".join(lines)


def generate_conception_report() -> str:
    """Generate architecture conception and math explanation"""
    lines = []
    lines.append("# 🧠 Architecture Conception & Mathematics\n")
    lines.append(f"> **Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M')}\n")
    lines.append("> **Purpose:** Explanation of core HFT algorithms and architectural decisions.\n\n")

    lines.append("## 1. HFT Core Philosophy\n")
    lines.append("- **Zero Allocation:** Hot paths use `Span<T>`, `ArrayPool`, and `ref struct` to avoid GC pauses.\n")
    lines.append("- **Lock-Free Concurrency:** Uses `Interlocked` operations and `ConcurrentDictionary` for shared state.\n")
    lines.append("- **Event-Driven:** No polling. Updates propagate immediately from socket to UI via `Channel<T>` and `MessagePack`.\n")
    lines.append("- **Monotonic Time:** Uses `Stopwatch.GetTimestamp()` for duration measurements to avoid system clock skew.\n\n")

    lines.append("## 2. Mathematical Model\n")
    
    lines.append("### 2.1. Ratio Calculation (Price Parity)\n")
    lines.append("> **Goal:** Measure how much one exchange deviates from another relative to price equality.\n")
    lines.append("```math\n")
    lines.append("Ratio = (Mid1 - Mid2) / Mid2\n")
    lines.append("Mid = (Bid + Ask) / 2\n")
    lines.append("```\n")
    lines.append("**Interpretation:**\n")
    lines.append("- `0.0000` = Perfect parity (prices are equal)\n")
    lines.append("- `+0.0020` = Exchange 1 is 0.2% more expensive\n")
    lines.append("- `-0.0015` = Exchange 1 is 0.15% cheaper\n\n")

    lines.append("### 2.2. Signal Normalization (EMA)\n")
    lines.append("> **Goal:** Remove persistent spreads (drift) to isolate pure volatility spikes.\n")
    lines.append("```math\n")
    lines.append("Signal = Ratio - EMA[t-1]\n")
    lines.append("EMA[t] = Alpha * Ratio + (1 - Alpha) * EMA[t-1]\n")
    lines.append("Alpha = 2 / (Period + 1)  (Period = 7)\n")
    lines.append("```\n")
    lines.append("**Why?** If Binance is consistently $10 higher than Bybit, we don't want a constant signal. We only want to know when it *suddenly* becomes $20 higher.\n\n")

    lines.append("### 2.3. Scoring Model (Weighted Frequency)\n")
    lines.append("> **Goal:** Rank pairs by opportunity quality (Frequency + Magnitude).\n")
    lines.append("```math\n")
    lines.append("Score = Frequency * AvgSpikeSize\n")
    lines.append("Frequency = 3600 / EMA(IntervalSeconds)\n")
    lines.append("AvgSpikeSize = EMA(SpikePeak)\n")
    lines.append("```\n")
    lines.append("- **Focus:** High score identify pairs that spike *often* with *large* moves.\n\n")

    lines.append("## 3. Data Flow Architecture\n")
    lines.append("```mermaid\n")
    lines.append("graph TD\n")
    lines.append("    A[Exchange Socket] -->|Zero-Alloc Parsing| B(TickerData)\n")
    lines.append("    B -->|Channel| C{DeviationAnalysis}\n")
    lines.append("    C -->|Calculate| D[RatioService]\n")
    lines.append("    D -->|Update| E[RingBuffer]\n")
    lines.append("    E -->|Snapshot| F[BroadcastCoordinator]\n")
    lines.append("    F -->|MessagePack| G((Web Clients))\n")
    lines.append("```\n")

    return "\n".join(lines)


def find_dead_js() -> List[Dict]:
    """Find unused JavaScript functions"""
    dead_js = []
    
    # 1. Collect all function definitions
    definitions = {} # name -> {file, line}
    defined_names = set()
    
    # Regex for function definitions
    rx_func = re.compile(r'function\s+(\w+)\s*\(')
    rx_arrow = re.compile(r'(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s*)?(?:\([^)]*\)|[^=]+)\s*=>')
    rx_assign = re.compile(r'(?:const|let|var)\s+(\w+)\s*=\s*function')
    
    # Files to check
    js_files = list(CLIENT_ROOT.rglob("*.js")) + list(CLIENT_ROOT.rglob("*.html"))
    
    for path in js_files:
        try:
            content = path.read_text(encoding='utf-8', errors='ignore')
            
            # Find definitions
            for i, line in enumerate(content.split('\n'), 1):
                matches = []
                matches.extend(rx_func.findall(line))
                matches.extend(rx_arrow.findall(line))
                matches.extend(rx_assign.findall(line))
                
                for name in matches:
                    if name not in defined_names:
                        definitions[name] = {
                            'file': str(path.relative_to(PROJECT_ROOT)),
                            'line': i
                        }
                        defined_names.add(name)
        except: continue
        
    # 2. Check for usages
    if not defined_names:
        return dead_js
        
    used_names = set()
    
    # Scanning all client files for usage
    for path in js_files:
        try:
            content = path.read_text(encoding='utf-8', errors='ignore')
            
            # Simple word boundary check (may have false positives, but safe)
            words = set(re.findall(r'\b(\w+)\b', content))
            used_names.update(words.intersection(defined_names))
            
        except: continue
        
    # 3. Determine dead code
    # Exclude standard lifecycle or lib names
    EXCLUDES = {'init', 'setup', 'update', 'render', 'constructor', 'connect', 'onmessage', 'onerror', 'onopen', 'onclose', 'getServerTime'}
    
    name_counts = Counter()
    for path in js_files:
        try:
            content = path.read_text(encoding='utf-8', errors='ignore')
            words = re.findall(r'\b(\w+)\b', content)
            names_in_file = [w for w in words if w in defined_names]
            name_counts.update(names_in_file)
        except: continue
            
    for name, info in definitions.items():
        if name in EXCLUDES: continue
        
        # If count <= 1, it's likely only defined and never called
        if name_counts[name] <= 1:
            dead_js.append({
                'name': name,
                'file': info['file'],
                'line': info['line']
            })
            
    return dead_js

def find_dead_files() -> List[Dict]:
    """Find files in wwwroot that are likely unused"""
    dead_files = []
    
    # Track all client files
    all_files = list(CLIENT_ROOT.rglob("*.*"))
    all_files = [f for f in all_files if f.is_file()]
    
    # Known entry points
    ENTRY_POINTS = {'index.html', 'ratio_chart.html', 'favicon.ico'} 
    
    referenced_files = set()
    
    # Scan content for filenames
    for scanner_path in all_files:
        try:
            content = scanner_path.read_text(encoding='utf-8', errors='ignore')
            for candidate in all_files:
                if candidate == scanner_path: continue # Don't count self
                
                # Check for filename reference (simple text search)
                name = candidate.name
                if name in content:
                    referenced_files.add(candidate)
        except: continue
        
    for f in all_files:
        if f.name in ENTRY_POINTS: continue
        if f in referenced_files: continue
        
        dead_files.append({
            'file': str(f.relative_to(PROJECT_ROOT)),
            'type': f.suffix
        })
        
    return dead_files

def find_dead_csharp() -> List[Dict]:
    """Find unused C# classes/methods (Best Effort)"""
    dead_csharp = []
    
    # 1. Collect definitions
    definitions = {} # name -> {type, file, line}
    defined_names = set()
    
    src_root = PROJECT_ROOT / "src"
    cs_files = list(src_root.rglob("*.cs"))
    
    rx_class = re.compile(r'(?:class|interface|record|struct)\s+(\w+)')
    rx_method = re.compile(r'(?:void|Task|Task<.*?>|int|string|bool|double|float)\s+(\w+)\s*\(')
    
    for path in cs_files:
        try:
            content = path.read_text(encoding='utf-8', errors='ignore')
            for i, line in enumerate(content.split('\n'), 1):
                # Classes
                for m in rx_class.findall(line):
                    if m not in defined_names:
                        definitions[m] = {'type': 'Class', 'file': str(path.relative_to(PROJECT_ROOT)), 'line': i}
                        defined_names.add(m)
                # Methods
                for m in rx_method.findall(line):
                    # Filter common keywords that regex might mistake
                    if m in {'if', 'while', 'for', 'foreach', 'switch', 'using', 'catch'}: continue
                    if m not in defined_names:
                        definitions[m] = {'type': 'Method', 'file': str(path.relative_to(PROJECT_ROOT)), 'line': i}
                        defined_names.add(m)
        except: continue
        
    # 2. Count usages
    name_counts = Counter()
    for path in cs_files:
        try:
            content = path.read_text(encoding='utf-8', errors='ignore')
            words = re.findall(r'\b(\w+)\b', content)
            names_in_file = [w for w in words if w in defined_names]
            name_counts.update(names_in_file)
        except: continue
        
    # 3. Filter
    EXCLUDES = {
        'Main', 'Program', 'Startup', 
        'StartAsync', 'StopAsync', 'ExecuteAsync', 
        'ToString', 'GetHashCode', 'Equals', 'Dispose',
        'ConfigureServices', 'Configure' # ASP.NET Core
    }
    
    for name, count in name_counts.items():
        if count <= 1: # Only defined
            if name in EXCLUDES: continue
            if name not in definitions: continue # Should be there
            
            info = definitions[name]
            
            # If it's a class and ends in "Service", skip warning to reduce noise
            if info['type'] == 'Class' and name.endswith('Service'): continue
            
            dead_csharp.append({
                'name': name,
                'type': info['type'],
                'file': info['file'],
                'line': info['line']
            })
            
    return dead_csharp

def generate_dead_code_report():
    """Generates a dedicated report for dead code analysis"""
    
    print("[SERVER] Analyzing dead code (Experimental)...")
    
    dead_css = find_dead_css()
    dead_js = find_dead_js()
    dead_files = find_dead_files()
    dead_csharp = find_dead_csharp()
    
    report_path = OUTPUT_DIR / "AUTO_DEAD_CODE.md"
    
    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("# 💀 Dead Code Analysis Report\n\n")
        f.write(f"> **Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M')}\n")
        f.write("> **Note:** This is a static analysis. Dynamic references (Reflection, DI strings) are not detected.\n")
        f.write("> **Action:** Verify manually before deleting.\n\n")
        
        # Dead CSS
        f.write(f"## 🎨 Unused CSS Classes ({len(dead_css)})\n")
        if dead_css:
            f.write("| Class | File | Line |\n")
            f.write("|-------|------|------|\n")
            for item in dead_css:
                f.write(f"| `{item['class']}` | `{item['file']}` | L{item['line']} |\n")
        else:
            f.write("> ✅ Clean\n")
        f.write("\n")
            
        # Dead JS
        f.write(f"## 📜 Unused JavaScript ({len(dead_js)})\n")
        if dead_js:
            f.write("| Name | File | Line |\n")
            f.write("|------|------|------|\n")
            for item in dead_js:
                f.write(f"| `{item['name']}` | `{item['file']}` | L{item['line']} |\n")
        else:
            f.write("> ✅ Clean\n")
        f.write("\n")

        # Dead Files
        f.write(f"## 📁 Unused Files ({len(dead_files)})\n")
        if dead_files:
            f.write("| File | Type |\n")
            f.write("|------|------|\n")
            for item in dead_files:
                f.write(f"| `{item['file']}` | {item['type']} |\n")
        else:
            f.write("> ✅ Clean\n")
        f.write("\n")
        
        # Dead C#
        f.write(f"## 🔷 Unused C# Symbols (Experimental) ({len(dead_csharp)})\n")
        if dead_csharp:
            f.write("| Type | Name | File | Line |\n")
            f.write("|------|------|------|------|\n")
            for item in dead_csharp:
                f.write(f"| {item['type']} | `{item['name']}` | `{item['file']}` | L{item['line']} |\n")
        else:
            f.write("> ✅ Clean\n")
        f.write("\n")
        
    print(f"[REPORT] ✅ {report_path}")

def main():
    parser = argparse.ArgumentParser(description="Architecture Scanner v3.0 — AI Deep Analyzer")
    parser.add_argument("--server-only", action="store_true", help="Scan server only")
    parser.add_argument("--client-only", action="store_true", help="Scan client only")
    args = parser.parse_args()
    
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    
    services = []
    clients = []
    
    # Server scan
    if not args.client_only:
        print(f"\n[SERVER] Scanning {SERVER_ROOT}...")
        services = scan_server()
        print(f"[SERVER] Found {len(services)} components")
        print(f"[SERVER] Hot Path: {len([s for s in services if s.is_hot_path])} components")
        
        # Generate 3 separate reports
        summary_report = generate_summary_report(services)
        output_path = OUTPUT_DIR / "AUTO_ARCH_SUMMARY.md"
        output_path.write_text(summary_report, encoding='utf-8')
        print(f"[SERVER] ✅ {output_path}")
        
        hotpath_report = generate_hotpath_report(services)
        output_path = OUTPUT_DIR / "AUTO_ARCH_HOTPATH.md"
        output_path.write_text(hotpath_report, encoding='utf-8')
        print(f"[SERVER] ✅ {output_path}")
        
        details_report = generate_details_report(services)
        output_path = OUTPUT_DIR / "AUTO_ARCH_DETAILS.md"
        output_path.write_text(details_report, encoding='utf-8')
        print(f"[SERVER] ✅ {output_path}")
    
    # Client scan
    if not args.server_only:
        print(f"\n[CLIENT] Scanning {CLIENT_ROOT}...")
        clients = scan_client()
        print(f"[CLIENT] Found {len(clients)} components")
        
        if clients:
            report = generate_client_report(clients)
            output_path = OUTPUT_DIR / "AUTO_ARCH_CLIENT.md"
            output_path.write_text(report, encoding='utf-8')
            print(f"[CLIENT] ✅ {output_path}")
    
    # AI Brief
    if services or clients:
        brief = generate_ai_brief(services, clients)
        output_path = OUTPUT_DIR / "AI_BRIEF.md"
        output_path.write_text(brief, encoding='utf-8')
        print(f"\n[BRIEF] ✅ {output_path}")
    # Conception Report
    conception = generate_conception_report()
    output_path = OUTPUT_DIR / "AUTO_ARCH_CONCEPTION.md"
    output_path.write_text(conception, encoding='utf-8')
    print(f"[BRIEF] ✅ {output_path}")

    # Dead Code Report
    generate_dead_code_report()
    
    print("\n[DONE] Architecture scan complete!")


if __name__ == "__main__":
    main()
