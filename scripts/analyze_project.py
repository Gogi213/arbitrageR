#!/usr/bin/env python3
"""
Массовый анализ проекта с приоритизацией
"""

import os
import sys
import subprocess
from pathlib import Path
from dataclasses import dataclass
from typing import List, Tuple
import re

@dataclass
class FileAnalysis:
    file_path: str
    filename: str
    hot_score: int = 0
    warm_score: int = 0
    cold_score: int = 0
    total_score: int = 0
    type_compliance: int = 100
    is_god_object: bool = False
    has_critical_issues: bool = False
    has_high_issues: bool = False
    has_moderate_issues: bool = False
    architecture_severity: str = "✅ CLEAN"
    architecture_issues: str = ""
    priority_actions: List[str] = None
    loc: int = 0
    method_count: int = 0
    
    def __post_init__(self):
        if self.priority_actions is None:
            self.priority_actions = []
    
    def severity_level(self) -> int:
        """Возвращает числовой уровень серьёзности (0-3, где 3 = критично)"""
        if self.has_critical_issues:
            return 3
        elif self.has_high_issues:
            return 2
        elif self.has_moderate_issues:
            return 1
        return 0
    
    def priority_score(self) -> int:
        """Общий приоритет для сортировки (выше = хуже)"""
        score = 0
        score += self.severity_level() * 1000
        score += min(self.hot_score, 500)
        score += self.total_score // 10
        if self.is_god_object:
            score += 2000
        return score

def find_cs_files(root_dir: str) -> List[str]:
    """Находит все релевантные C# файлы"""
    files = []
    for root, dirs, filenames in os.walk(root_dir):
        # Пропускаем автогенерированные директории
        if '/obj/' in root or '/bin/' in root:
            continue
        
        for filename in filenames:
            if filename.endswith('.cs'):
                # Пропускаем автогенерированные файлы
                if 'AssemblyInfo.cs' in filename or \
                   'GlobalUsings.g.cs' in filename or \
                   'AssemblyAttributes.cs' in filename:
                    continue
                
                full_path = os.path.join(root, filename)
                files.append(full_path)
    
    return sorted(files)

def analyze_file(file_path: str) -> FileAnalysis:
    """Анализирует один файл с помощью скрипта"""
    script_path = "/root/screener123/collections/scripts/calculate_allocation_score.py"
    
    try:
        result = subprocess.run(
            [sys.executable, script_path, file_path],
            capture_output=True,
            text=True,
            timeout=30
        )
        
        output = result.stdout
        
        # Парсим результат
        analysis = FileAnalysis(
            file_path=file_path,
            filename=os.path.basename(file_path)
        )
        
        # Извлекаем scores
        hot_match = re.search(r'Hot Path Score:\s+(\d+)', output)
        warm_match = re.search(r'Warm Path Score:\s+(\d+)', output)
        cold_match = re.search(r'Cold Path Score:\s+(\d+)', output)
        total_match = re.search(r'TOTAL:\s+(\d+)', output)
        type_match = re.search(r'Type Compliance:\s+(\d+)/100', output)
        
        if hot_match:
            analysis.hot_score = int(hot_match.group(1))
        if warm_match:
            analysis.warm_score = int(warm_match.group(1))
        if cold_match:
            analysis.cold_score = int(cold_match.group(1))
        if total_match:
            analysis.total_score = int(total_match.group(1))
        if type_match:
            analysis.type_compliance = int(type_match.group(1))
        
        # Извлекаем метрики
        loc_match = re.search(r'File Size:\s+(\d+)\s+LOC', output)
        methods_match = re.search(r'Methods:\s+(\d+)\s+total', output)
        
        if loc_match:
            analysis.loc = int(loc_match.group(1))
        if methods_match:
            analysis.method_count = int(methods_match.group(1))
        
        # Проверяем на God Object
        if 'God Object' in output:
            analysis.is_god_object = True
        
        # Определяем severity
        analysis.has_critical_issues = '🔴 CRITICAL' in output or '💀 CRITICAL' in output
        analysis.has_high_issues = '❌ HIGH RISK' in output or '🟠 HIGH' in output
        analysis.has_moderate_issues = '⚠️  MODERATE' in output or '🟠 WARNING' in output
        
        # Извлекаем architecture status
        arch_match = re.search(r'Status:\s+(.*?)$', output, re.MULTILINE)
        if arch_match:
            analysis.architecture_severity = arch_match.group(1).strip()
        
        arch_issues_match = re.search(r'Issues:\s+(.*?)$', output, re.MULTILINE)
        if arch_issues_match:
            analysis.architecture_issues = arch_issues_match.group(1).strip()
        
        # Извлекаем priority actions
        in_actions = False
        for line in output.split('\n'):
            if 'PRIORITY ACTIONS:' in line:
                in_actions = True
                continue
            if in_actions:
                if line.strip().startswith('💡 RECOMMENDATIONS:') or line.strip().startswith('==='):
                    break
                if line.strip() and (line.strip().startswith('🔴') or 
                                    line.strip().startswith('🟠') or 
                                    line.strip().startswith('🟡') or
                                    line.strip().startswith('✅')):
                    analysis.priority_actions.append(line.strip())
        
        return analysis
        
    except Exception as e:
        print(f"Error analyzing {file_path}: {e}", file=sys.stderr)
        return FileAnalysis(file_path=file_path, filename=os.path.basename(file_path))

def generate_report(analyses: List[FileAnalysis], output_file: str):
    """Генерирует итоговый отчет с приоритизацией"""
    
    # Сортируем по priority score (убывание)
    sorted_analyses = sorted(analyses, key=lambda a: a.priority_score(), reverse=True)
    
    # Группируем по severity
    critical = [a for a in sorted_analyses if a.severity_level() == 3]
    high = [a for a in sorted_analyses if a.severity_level() == 2]
    moderate = [a for a in sorted_analyses if a.severity_level() == 1]
    good = [a for a in sorted_analyses if a.severity_level() == 0]
    
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write("=" * 100 + "\n")
        f.write("HFT ALLOCATION & ARCHITECTURE ANALYSIS - EXECUTIVE SUMMARY\n")
        f.write("=" * 100 + "\n\n")
        
        f.write(f"Total files analyzed: {len(analyses)}\n\n")
        
        f.write("STATISTICS:\n")
        f.write(f"  🔴 Critical Issues:  {len(critical):3d} files\n")
        f.write(f"  🟠 High Risk:        {len(high):3d} files\n")
        f.write(f"  ⚠️  Moderate:         {len(moderate):3d} files\n")
        f.write(f"  ✅ Good:             {len(good):3d} files\n")
        f.write("\n")
        
        # TOP 10 WORST FILES
        f.write("=" * 100 + "\n")
        f.write("🔴 TOP 10 FILES REQUIRING IMMEDIATE ATTENTION\n")
        f.write("=" * 100 + "\n\n")
        
        for i, analysis in enumerate(sorted_analyses[:10], 1):
            f.write(f"{i}. {analysis.filename}\n")
            f.write(f"   Path: {analysis.file_path}\n")
            f.write(f"   Priority Score: {analysis.priority_score()}\n")
            f.write(f"   Hot Path Score: {analysis.hot_score} | Type Compliance: {analysis.type_compliance}/100\n")
            f.write(f"   Size: {analysis.loc} LOC | Methods: {analysis.method_count}\n")
            if analysis.is_god_object:
                f.write(f"   ⚠️  God Object detected!\n")
            f.write(f"   Architecture: {analysis.architecture_severity}\n")
            if analysis.architecture_issues:
                f.write(f"   Issues: {analysis.architecture_issues}\n")
            if analysis.priority_actions:
                f.write(f"   Actions:\n")
                for action in analysis.priority_actions[:3]:  # Top 3 actions
                    f.write(f"      {action}\n")
            f.write("\n")
        
        # CRITICAL FILES (все)
        if critical:
            f.write("=" * 100 + "\n")
            f.write(f"🔴 CRITICAL FILES ({len(critical)} files)\n")
            f.write("=" * 100 + "\n\n")
            
            for analysis in critical:
                f.write(f"• {analysis.filename}\n")
                f.write(f"  {analysis.file_path}\n")
                f.write(f"  Hot: {analysis.hot_score} | Warm: {analysis.warm_score} | Type Compliance: {analysis.type_compliance}/100\n")
                if analysis.priority_actions:
                    for action in analysis.priority_actions[:2]:
                        f.write(f"  {action}\n")
                f.write("\n")
        
        # HIGH RISK FILES
        if high:
            f.write("=" * 100 + "\n")
            f.write(f"🟠 HIGH RISK FILES ({len(high)} files)\n")
            f.write("=" * 100 + "\n\n")
            
            for analysis in high:
                f.write(f"• {analysis.filename}\n")
                f.write(f"  {analysis.file_path}\n")
                f.write(f"  Hot: {analysis.hot_score} | Warm: {analysis.warm_score} | Type Compliance: {analysis.type_compliance}/100\n")
                f.write("\n")
        
        # MODERATE FILES (только список)
        if moderate:
            f.write("=" * 100 + "\n")
            f.write(f"⚠️  MODERATE ISSUES ({len(moderate)} files)\n")
            f.write("=" * 100 + "\n\n")
            
            for analysis in moderate:
                f.write(f"• {analysis.filename} - Hot: {analysis.hot_score}, Type Compliance: {analysis.type_compliance}/100\n")
            f.write("\n")
        
        # GOOD FILES (только количество)
        if good:
            f.write("=" * 100 + "\n")
            f.write(f"✅ GOOD FILES ({len(good)} files)\n")
            f.write("=" * 100 + "\n\n")
            f.write("These files have no critical issues.\n\n")
        
        # ПРИОРИТЕТЫ ОПТИМИЗАЦИИ
        f.write("=" * 100 + "\n")
        f.write("📋 OPTIMIZATION PRIORITIES\n")
        f.write("=" * 100 + "\n\n")
        
        f.write("PHASE 1 - CRITICAL (Immediate action required):\n")
        f.write("-" * 100 + "\n")
        
        # God Objects
        god_objects = [a for a in sorted_analyses if a.is_god_object]
        if god_objects:
            f.write(f"\n1. 🏗️  SPLIT GOD OBJECTS ({len(god_objects)} files):\n")
            for go in god_objects[:5]:  # Top 5
                f.write(f"   • {go.filename} ({go.loc} LOC, {go.method_count} methods)\n")
            if len(god_objects) > 5:
                f.write(f"   ... and {len(god_objects) - 5} more\n")
        
        # Hot path critical
        hot_critical = [a for a in sorted_analyses if a.hot_score > 50]
        if hot_critical:
            f.write(f"\n2. 🔥 CRITICAL HOT PATH ALLOCATIONS ({len(hot_critical)} files):\n")
            for hc in hot_critical[:5]:
                f.write(f"   • {hc.filename} (Hot Score: {hc.hot_score})\n")
            if len(hot_critical) > 5:
                f.write(f"   ... and {len(hot_critical) - 5} more\n")
        
        # Type compliance critical
        type_critical = [a for a in sorted_analyses if a.type_compliance < 70]
        if type_critical:
            f.write(f"\n3. 📊 NUMERIC TYPE ISSUES ({len(type_critical)} files):\n")
            for tc in type_critical[:5]:
                f.write(f"   • {tc.filename} (Compliance: {tc.type_compliance}/100)\n")
            if len(type_critical) > 5:
                f.write(f"   ... and {len(type_critical) - 5} more\n")
        
        f.write("\n")
        f.write("PHASE 2 - HIGH PRIORITY (Next sprint):\n")
        f.write("-" * 100 + "\n")
        
        hot_high = [a for a in sorted_analyses if 20 < a.hot_score <= 50]
        if hot_high:
            f.write(f"\n1. 🔥 Hot Path Optimization ({len(hot_high)} files)\n")
        
        warm_high = [a for a in sorted_analyses if a.warm_score > 100]
        if warm_high:
            f.write(f"2. 🌡️  Warm Path Optimization ({len(warm_high)} files)\n")
        
        f.write("\n")
        f.write("PHASE 3 - MODERATE (Technical debt cleanup):\n")
        f.write("-" * 100 + "\n")
        f.write(f"1. Architecture improvements for {len(moderate)} files\n")
        f.write(f"2. General code quality improvements\n")
        
        f.write("\n")
        f.write("=" * 100 + "\n")
        f.write("END OF REPORT\n")
        f.write("=" * 100 + "\n")

def main():
    root_dir = "/root/screener123"
    output_file = "/root/screener123/OPTIMIZATION_PRIORITIES.txt"
    
    print("🔍 Finding C# files...")
    files = find_cs_files(root_dir)
    print(f"Found {len(files)} files to analyze\n")
    
    print("🚀 Starting analysis...")
    analyses = []
    
    for i, file_path in enumerate(files, 1):
        print(f"[{i}/{len(files)}] Analyzing: {os.path.basename(file_path)}", end='')
        analysis = analyze_file(file_path)
        analyses.append(analysis)
        
        # Показываем краткий статус
        if analysis.has_critical_issues:
            print(" 🔴 CRITICAL")
        elif analysis.has_high_issues:
            print(" 🟠 HIGH")
        elif analysis.has_moderate_issues:
            print(" ⚠️  MODERATE")
        else:
            print(" ✅ OK")
    
    print("\n📊 Generating report...")
    generate_report(analyses, output_file)
    
    print(f"\n✅ Analysis complete!")
    print(f"Report saved to: {output_file}")
    
    # Краткая статистика в консоль
    critical = len([a for a in analyses if a.severity_level() == 3])
    high = len([a for a in analyses if a.severity_level() == 2])
    moderate = len([a for a in analyses if a.severity_level() == 1])
    good = len([a for a in analyses if a.severity_level() == 0])
    
    print("\nSUMMARY:")
    print(f"  🔴 Critical: {critical}")
    print(f"  🟠 High:     {high}")
    print(f"  ⚠️  Moderate: {moderate}")
    print(f"  ✅ Good:     {good}")

if __name__ == "__main__":
    main()
