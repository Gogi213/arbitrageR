import os
import re

def analyze_file(filepath):
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            lines = f.readlines()
    except Exception as e:
        return None

    control_flow_keywords = ['if', 'else', 'switch', 'for', 'foreach', 'while', 'do', 'catch']
    logical_operators = ['&&', '||']
    
    control_flow_count = 0
    logical_ops_count = 0
    max_nesting = 0
    current_nesting = 0
    loc = 0
    
    keyword_patterns = [re.compile(r'\b' + k + r'\b') for k in control_flow_keywords]
    
    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith('//'):
            continue
            
        loc += 1
        
        for pattern in keyword_patterns:
            if pattern.search(line):
                control_flow_count += 1
                
        for op in logical_operators:
            logical_ops_count += line.count(op)
            
        open_braces = line.count('{')
        close_braces = line.count('}')
        
        current_nesting += open_braces
        current_nesting -= close_braces
        
        if current_nesting > max_nesting:
            max_nesting = current_nesting

    score = (control_flow_count * 2) + (logical_ops_count * 1) + (max_nesting * 3) + (loc * 0.05)
    
    return {
        'path': filepath,
        'score': score,
        'control_flow': control_flow_count,
        'logical_ops': logical_ops_count,
        'max_nesting': max_nesting,
        'loc': loc
    }

def main():
    root_dir = '/root/screener123'
    results = []
    
    for dirpath, _, filenames in os.walk(root_dir):
        for filename in filenames:
            if filename.endswith('.cs'):
                filepath = os.path.join(dirpath, filename)
                analysis = analyze_file(filepath)
                if analysis:
                    results.append(analysis)
    
    results.sort(key=lambda x: x['score'], reverse=True)
    
    print(f"{'Score':<10} {'File Path':<60}")
    print("-" * 80)
    
    for i, res in enumerate(results[:10]):
        # Fixed formatting: width 10, 2 decimal places
        print(f"{res['score']:<10.2f} {res['path']}")
        
        reasons = []
        if res['control_flow'] > 20: reasons.append(f"high control flow ({res['control_flow']})")
        if res['logical_ops'] > 10: reasons.append(f"complex logic ({res['logical_ops']} ops)")
        if res['max_nesting'] > 5: reasons.append(f"deep nesting ({res['max_nesting']})")
        if res['loc'] > 200: reasons.append(f"large file ({res['loc']} LOC)")
        
        print(f"   Reason: {', '.join(reasons)}")
        print()

if __name__ == '__main__':
    main()
