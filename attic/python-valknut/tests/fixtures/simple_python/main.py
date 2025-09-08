"""
Simple Python test fixture with various refactoring opportunities.
"""

import os
import sys
from typing import List, Dict, Optional


class DataProcessor:
    """A class with some refactoring opportunities."""
    
    def __init__(self, name: str):
        self.name = name
        self.data = []
        self.cache = {}
    
    def process_data(self, input_data: List[Dict]) -> List[Dict]:
        """Complex method that could be refactored."""
        results = []
        
        # Complex nested logic - potential for extract method
        for item in input_data:
            if item.get('type') == 'A':
                if item.get('status') == 'active':
                    processed_item = {
                        'id': item.get('id'),
                        'value': item.get('value', 0) * 2,
                        'category': 'processed_A',
                        'timestamp': item.get('timestamp')
                    }
                    results.append(processed_item)
                elif item.get('status') == 'pending':
                    processed_item = {
                        'id': item.get('id'),
                        'value': item.get('value', 0) * 1.5,
                        'category': 'pending_A',
                        'timestamp': item.get('timestamp')
                    }
                    results.append(processed_item)
            elif item.get('type') == 'B':
                if item.get('status') == 'active':
                    processed_item = {
                        'id': item.get('id'),
                        'value': item.get('value', 0) * 3,
                        'category': 'processed_B',
                        'timestamp': item.get('timestamp')
                    }
                    results.append(processed_item)
        
        return results
    
    def calculate_metrics(self, data: List[Dict]) -> Dict:
        """Another complex method with duplicated logic."""
        total = 0
        count = 0
        categories = {}
        
        # Duplicated calculation pattern
        for item in data:
            if item.get('category') == 'processed_A':
                total += item.get('value', 0)
                count += 1
                if 'processed_A' not in categories:
                    categories['processed_A'] = {'total': 0, 'count': 0}
                categories['processed_A']['total'] += item.get('value', 0)
                categories['processed_A']['count'] += 1
            elif item.get('category') == 'pending_A':
                total += item.get('value', 0)
                count += 1
                if 'pending_A' not in categories:
                    categories['pending_A'] = {'total': 0, 'count': 0}
                categories['pending_A']['total'] += item.get('value', 0)
                categories['pending_A']['count'] += 1
            elif item.get('category') == 'processed_B':
                total += item.get('value', 0)
                count += 1
                if 'processed_B' not in categories:
                    categories['processed_B'] = {'total': 0, 'count': 0}
                categories['processed_B']['total'] += item.get('value', 0)
                categories['processed_B']['count'] += 1
        
        return {
            'total': total,
            'count': count,
            'average': total / count if count > 0 else 0,
            'categories': categories
        }


def validate_config(config_path: str) -> bool:
    """Function with high cyclomatic complexity."""
    if not os.path.exists(config_path):
        return False
    
    try:
        with open(config_path, 'r') as f:
            content = f.read()
            
        if not content:
            return False
            
        if content.startswith('{'):
            # JSON validation logic
            import json
            try:
                data = json.loads(content)
                if 'name' not in data:
                    return False
                if 'version' not in data:
                    return False
                if 'settings' not in data:
                    return False
                if not isinstance(data['settings'], dict):
                    return False
                if 'database' in data['settings']:
                    db_config = data['settings']['database']
                    if 'host' not in db_config:
                        return False
                    if 'port' not in db_config:
                        return False
                    if 'name' not in db_config:
                        return False
                return True
            except json.JSONDecodeError:
                return False
        elif content.startswith('['):
            # YAML validation logic
            try:
                import yaml
                data = yaml.safe_load(content)
                if not isinstance(data, dict):
                    return False
                # Similar validation as JSON
                return True
            except Exception:
                return False
        else:
            return False
            
    except Exception:
        return False


# Global functions with code smells
def process_file_data(file_path: str, processor_name: str = "default"):
    """Long function with multiple responsibilities."""
    if not os.path.exists(file_path):
        raise FileNotFoundError(f"File not found: {file_path}")
    
    # File reading logic
    with open(file_path, 'r') as f:
        raw_data = f.read()
    
    # Data parsing logic  
    lines = raw_data.split('\n')
    parsed_data = []
    for line in lines:
        if line.strip():
            parts = line.split(',')
            if len(parts) >= 3:
                parsed_data.append({
                    'id': parts[0].strip(),
                    'value': float(parts[1].strip()) if parts[1].strip().replace('.', '').isdigit() else 0,
                    'type': parts[2].strip(),
                    'status': parts[3].strip() if len(parts) > 3 else 'unknown'
                })
    
    # Processing logic
    processor = DataProcessor(processor_name)
    processed_data = processor.process_data(parsed_data)
    
    # Metrics calculation logic
    metrics = processor.calculate_metrics(processed_data)
    
    # Output logic
    output_file = file_path.replace('.txt', '_processed.json')
    import json
    with open(output_file, 'w') as f:
        json.dump({
            'data': processed_data,
            'metrics': metrics,
            'processor': processor_name
        }, f, indent=2)
    
    return output_file


if __name__ == "__main__":
    # Main execution with hardcoded values
    if len(sys.argv) > 1:
        result = process_file_data(sys.argv[1])
        print(f"Processed data saved to: {result}")
    else:
        print("Usage: python main.py <input_file>")