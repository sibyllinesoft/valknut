#!/usr/bin/env python3
"""
Test file with guaranteed long method detection.
LONG_METHOD_LINE_THRESHOLD = 50, so this method is designed to exceed that.
"""

def extremely_long_processing_function(input_data, config_options, validation_rules):
    """
    This function is intentionally very long to trigger ExtractMethod refactoring.
    It has 75+ lines of actual code, well above the 50-line threshold.
    """
    # Step 1: Input validation (lines 1-15)
    if input_data is None:
        raise ValueError("Input data cannot be None")
    
    if not isinstance(input_data, (list, dict, str)):
        raise TypeError("Input data must be list, dict, or string")
    
    if config_options is None:
        config_options = {}
    
    if validation_rules is None:
        validation_rules = []
    
    if not isinstance(config_options, dict):
        raise TypeError("Config options must be a dictionary")
    
    if not isinstance(validation_rules, list):
        raise TypeError("Validation rules must be a list")
    
    # Step 2: Data preprocessing (lines 16-30)
    processed_data = []
    error_count = 0
    warning_count = 0
    total_items = 0
    
    if isinstance(input_data, str):
        input_data = input_data.split('\n')
    elif isinstance(input_data, dict):
        input_data = list(input_data.values())
    
    for item in input_data:
        total_items += 1
        if item is None:
            error_count += 1
            continue
        
        try:
            normalized_item = str(item).strip().lower()
            if len(normalized_item) == 0:
                warning_count += 1
                continue
            processed_data.append(normalized_item)
        except Exception as e:
            error_count += 1
            print(f"Error processing item {item}: {e}")
    
    # Step 3: Validation processing (lines 31-50)
    validated_data = []
    validation_errors = []
    
    for rule in validation_rules:
        rule_name = rule.get('name', 'unnamed_rule')
        rule_pattern = rule.get('pattern', '')
        rule_required = rule.get('required', False)
        
        matching_items = []
        for item in processed_data:
            if rule_pattern in item:
                matching_items.append(item)
        
        if rule_required and len(matching_items) == 0:
            validation_errors.append(f"Required rule '{rule_name}' matched no items")
        
        for item in matching_items:
            if item not in validated_data:
                validated_data.append(item)
    
    # Step 4: Configuration application (lines 51-65)
    max_items = config_options.get('max_items', 1000)
    sort_order = config_options.get('sort_order', 'asc')
    include_metadata = config_options.get('include_metadata', False)
    
    if len(validated_data) > max_items:
        validated_data = validated_data[:max_items]
    
    if sort_order == 'asc':
        validated_data.sort()
    elif sort_order == 'desc':
        validated_data.sort(reverse=True)
    
    # Step 5: Result compilation (lines 66-80)
    result = {
        'data': validated_data,
        'stats': {
            'total_input_items': total_items,
            'processed_items': len(processed_data),
            'validated_items': len(validated_data),
            'error_count': error_count,
            'warning_count': warning_count,
            'validation_errors': validation_errors
        }
    }
    
    if include_metadata:
        result['metadata'] = {
            'config_applied': config_options,
            'rules_applied': len(validation_rules),
            'processing_timestamp': str(__import__('datetime').datetime.now())
        }
    
    return result

def another_extremely_long_function(data_source, transformation_config):
    """
    Another intentionally long function for testing.
    This one focuses on data transformation with 60+ lines.
    """
    # Input processing
    if not data_source:
        return None
    
    transformations = transformation_config.get('transformations', [])
    output_format = transformation_config.get('output_format', 'json')
    
    # Data collection phase
    collected_data = []
    collection_errors = []
    
    for source_item in data_source:
        try:
            item_type = type(source_item).__name__
            item_value = str(source_item)
            item_length = len(item_value)
            
            collected_data.append({
                'type': item_type,
                'value': item_value,
                'length': item_length,
                'original': source_item
            })
        except Exception as e:
            collection_errors.append(str(e))
    
    # Transformation phase
    transformed_data = []
    transformation_errors = []
    
    for item in collected_data:
        for transform in transformations:
            transform_type = transform.get('type', 'none')
            transform_params = transform.get('params', {})
            
            try:
                if transform_type == 'uppercase':
                    item['value'] = item['value'].upper()
                elif transform_type == 'lowercase':
                    item['value'] = item['value'].lower()
                elif transform_type == 'prefix':
                    prefix = transform_params.get('prefix', '')
                    item['value'] = prefix + item['value']
                elif transform_type == 'suffix':
                    suffix = transform_params.get('suffix', '')
                    item['value'] = item['value'] + suffix
                elif transform_type == 'replace':
                    old_val = transform_params.get('old', '')
                    new_val = transform_params.get('new', '')
                    item['value'] = item['value'].replace(old_val, new_val)
            except Exception as e:
                transformation_errors.append(f"Transform {transform_type}: {e}")
        
        transformed_data.append(item)
    
    # Output formatting phase
    if output_format == 'json':
        result = {
            'data': transformed_data,
            'errors': {
                'collection': collection_errors,
                'transformation': transformation_errors
            }
        }
    elif output_format == 'csv':
        result = "type,value,length\n"
        for item in transformed_data:
            result += f"{item['type']},{item['value']},{item['length']}\n"
    else:
        result = str(transformed_data)
    
    return result