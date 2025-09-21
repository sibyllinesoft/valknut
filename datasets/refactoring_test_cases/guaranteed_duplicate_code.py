#!/usr/bin/env python3
"""
Test file with guaranteed duplicate code detection.
DUPLICATE_MIN_TOKEN_COUNT = 10 and DUPLICATE_MIN_LINE_COUNT = 4
These functions are designed to be near-identical to trigger duplication detection.
"""

def data_processor_version_a(input_list, config_dict):
    """
    First version of data processing function - contains duplicated logic.
    This has 15+ lines and 25+ tokens to ensure detection.
    """
    result_items = []
    error_count = 0
    warning_count = 0
    
    # This exact logic block is duplicated below
    for item in input_list:
        try:
            if item is None:
                error_count += 1
                continue
            
            processed_item = str(item).strip().lower()
            if len(processed_item) < config_dict.get('min_length', 3):
                warning_count += 1
                continue
            
            if config_dict.get('validate_numeric', False):
                try:
                    float(processed_item)
                    processed_item = f"num_{processed_item}"
                except ValueError:
                    processed_item = f"str_{processed_item}"
            
            result_items.append(processed_item)
        except Exception as e:
            error_count += 1
            print(f"Processing error: {e}")
    
    return {
        'items': result_items,
        'errors': error_count,
        'warnings': warning_count,
        'total_processed': len(result_items)
    }

def data_processor_version_b(input_list, config_dict):
    """
    Second version of data processing function - nearly identical to version_a.
    This should be detected as duplicate code needing consolidation.
    """
    result_items = []
    error_count = 0
    warning_count = 0
    
    # This exact logic block is duplicated from above
    for item in input_list:
        try:
            if item is None:
                error_count += 1
                continue
            
            processed_item = str(item).strip().lower()
            if len(processed_item) < config_dict.get('min_length', 3):
                warning_count += 1
                continue
            
            if config_dict.get('validate_numeric', False):
                try:
                    float(processed_item)
                    processed_item = f"num_{processed_item}"
                except ValueError:
                    processed_item = f"str_{processed_item}"
            
            result_items.append(processed_item)
        except Exception as e:
            error_count += 1
            print(f"Processing error: {e}")
    
    return {
        'items': result_items,
        'errors': error_count,
        'warnings': warning_count,
        'total_processed': len(result_items)
    }

def validation_helper_alpha(data_entry, validation_rules):
    """
    First validation helper - contains duplicated validation logic.
    """
    validation_results = []
    passed_count = 0
    failed_count = 0
    
    # Duplicated validation logic block
    for rule in validation_rules:
        rule_name = rule.get('name', 'unnamed')
        rule_type = rule.get('type', 'string')
        rule_required = rule.get('required', False)
        
        try:
            if rule_type == 'string':
                if isinstance(data_entry, str) and len(data_entry) > 0:
                    passed_count += 1
                    validation_results.append(f"{rule_name}: PASS")
                else:
                    failed_count += 1
                    validation_results.append(f"{rule_name}: FAIL - not valid string")
            elif rule_type == 'numeric':
                try:
                    float(data_entry)
                    passed_count += 1
                    validation_results.append(f"{rule_name}: PASS")
                except (ValueError, TypeError):
                    failed_count += 1
                    validation_results.append(f"{rule_name}: FAIL - not numeric")
            elif rule_type == 'required':
                if data_entry is not None and str(data_entry).strip() != '':
                    passed_count += 1
                    validation_results.append(f"{rule_name}: PASS")
                else:
                    failed_count += 1
                    validation_results.append(f"{rule_name}: FAIL - required field empty")
        except Exception as e:
            failed_count += 1
            validation_results.append(f"{rule_name}: ERROR - {e}")
    
    return {
        'results': validation_results,
        'passed': passed_count,
        'failed': failed_count,
        'success_rate': passed_count / max(1, passed_count + failed_count)
    }

def validation_helper_beta(data_entry, validation_rules):
    """
    Second validation helper - nearly identical to validation_helper_alpha.
    This duplicated code should be detected and consolidated.
    """
    validation_results = []
    passed_count = 0
    failed_count = 0
    
    # Duplicated validation logic block (identical to above)
    for rule in validation_rules:
        rule_name = rule.get('name', 'unnamed')
        rule_type = rule.get('type', 'string')
        rule_required = rule.get('required', False)
        
        try:
            if rule_type == 'string':
                if isinstance(data_entry, str) and len(data_entry) > 0:
                    passed_count += 1
                    validation_results.append(f"{rule_name}: PASS")
                else:
                    failed_count += 1
                    validation_results.append(f"{rule_name}: FAIL - not valid string")
            elif rule_type == 'numeric':
                try:
                    float(data_entry)
                    passed_count += 1
                    validation_results.append(f"{rule_name}: PASS")
                except (ValueError, TypeError):
                    failed_count += 1
                    validation_results.append(f"{rule_name}: FAIL - not numeric")
            elif rule_type == 'required':
                if data_entry is not None and str(data_entry).strip() != '':
                    passed_count += 1
                    validation_results.append(f"{rule_name}: PASS")
                else:
                    failed_count += 1
                    validation_results.append(f"{rule_name}: FAIL - required field empty")
        except Exception as e:
            failed_count += 1
            validation_results.append(f"{rule_name}: ERROR - {e}")
    
    return {
        'results': validation_results,
        'passed': passed_count,
        'failed': failed_count,
        'success_rate': passed_count / max(1, passed_count + failed_count)
    }

def format_output_method_one(processed_data, output_config):
    """
    First output formatting method - contains duplicated formatting logic.
    """
    formatted_output = []
    formatting_errors = []
    
    # Duplicated formatting logic
    for data_item in processed_data:
        try:
            output_format = output_config.get('format', 'json')
            include_metadata = output_config.get('include_metadata', False)
            timestamp_format = output_config.get('timestamp_format', 'iso')
            
            if output_format == 'json':
                formatted_item = {
                    'data': data_item,
                    'format': 'json'
                }
            elif output_format == 'xml':
                formatted_item = f"<item>{data_item}</item>"
            elif output_format == 'csv':
                formatted_item = f'"{data_item}"'
            else:
                formatted_item = str(data_item)
            
            if include_metadata:
                if isinstance(formatted_item, dict):
                    formatted_item['metadata'] = {
                        'timestamp': str(__import__('datetime').datetime.now()),
                        'processor': 'method_one'
                    }
                else:
                    formatted_item = f"{formatted_item} [method_one]"
            
            formatted_output.append(formatted_item)
        except Exception as e:
            formatting_errors.append(str(e))
    
    return {
        'output': formatted_output,
        'errors': formatting_errors,
        'count': len(formatted_output)
    }

def format_output_method_two(processed_data, output_config):
    """
    Second output formatting method - nearly identical to method_one.
    This duplicated formatting logic should be detected and refactored.
    """
    formatted_output = []
    formatting_errors = []
    
    # Duplicated formatting logic (identical to above)
    for data_item in processed_data:
        try:
            output_format = output_config.get('format', 'json')
            include_metadata = output_config.get('include_metadata', False)
            timestamp_format = output_config.get('timestamp_format', 'iso')
            
            if output_format == 'json':
                formatted_item = {
                    'data': data_item,
                    'format': 'json'
                }
            elif output_format == 'xml':
                formatted_item = f"<item>{data_item}</item>"
            elif output_format == 'csv':
                formatted_item = f'"{data_item}"'
            else:
                formatted_item = str(data_item)
            
            if include_metadata:
                if isinstance(formatted_item, dict):
                    formatted_item['metadata'] = {
                        'timestamp': str(__import__('datetime').datetime.now()),
                        'processor': 'method_two'
                    }
                else:
                    formatted_item = f"{formatted_item} [method_two]"
            
            formatted_output.append(formatted_item)
        except Exception as e:
            formatting_errors.append(str(e))
    
    return {
        'output': formatted_output,
        'errors': formatting_errors,
        'count': len(formatted_output)
    }