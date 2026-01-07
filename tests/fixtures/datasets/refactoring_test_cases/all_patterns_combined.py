#!/usr/bin/env python3
"""
Comprehensive refactoring test file combining ALL patterns:
- Long methods (50+ lines)
- Complex conditionals (4+ logical operators)
- Duplicate code (10+ tokens, 4+ lines)
- Large classes (200+ lines, 12+ methods)

This file should trigger EVERY refactoring recommendation type.
"""

class ComprehensiveTestClass:
    """
    Massive class with 25+ methods and 300+ lines combining all refactoring patterns.
    Contains long methods, complex conditionals, and duplicate code.
    """
    
    def __init__(self, config, database, cache, validator, logger, metrics_collector):
        self.config = config
        self.database = database
        self.cache = cache
        self.validator = validator
        self.logger = logger
        self.metrics = metrics_collector
        self.processed_items = 0
        self.error_count = 0
        self.warning_count = 0
        self.performance_data = {}
        self.validation_rules = []
        self.transformation_pipeline = []
    
    def extremely_long_method_with_complex_conditionals(self, input_data, processing_config, validation_settings, output_options):
        """
        COMBINES: Long method + Complex conditionals + Duplicate logic
        This method is 80+ lines with 8+ logical operators and contains duplicated code.
        """
        # Input validation with complex conditionals (8+ operators)
        if ((input_data is not None and len(input_data) > 0) and
            (processing_config.get('enabled', True) or processing_config.get('force_process', False)) and
            ((validation_settings.get('strict_mode', False) and validation_settings.get('validate_all', True)) or
             (validation_settings.get('loose_mode', True) and validation_settings.get('skip_basic_validation', False))) and
            (output_options.get('format', '') in ['json', 'xml', 'csv'] or output_options.get('allow_custom', False)) and
            ((self.config.get('processing_enabled', True) and self.config.get('system_ready', False)) or
             (self.config.get('emergency_mode', False) and self.config.get('bypass_checks', True)))):
            
            # Data preprocessing phase (lines 15-35)
            processed_items = []
            validation_errors = []
            transformation_errors = []
            
            for item in input_data:
                try:
                    # Item validation
                    if item is None:
                        self.error_count += 1
                        continue
                    
                    normalized_item = str(item).strip().lower()
                    if len(normalized_item) < processing_config.get('min_item_length', 2):
                        self.warning_count += 1
                        continue
                    
                    # Type detection and conversion
                    if processing_config.get('detect_types', True):
                        try:
                            if normalized_item.isdigit():
                                normalized_item = f"int_{normalized_item}"
                            elif '.' in normalized_item and all(c.isdigit() or c == '.' for c in normalized_item):
                                normalized_item = f"float_{normalized_item}"
                            else:
                                normalized_item = f"str_{normalized_item}"
                        except Exception as type_error:
                            transformation_errors.append(str(type_error))
                    
                    processed_items.append(normalized_item)
                    self.processed_items += 1
                    
                except Exception as item_error:
                    self.error_count += 1
                    validation_errors.append(str(item_error))
            
            # Validation phase with complex conditionals (lines 40-60)
            validated_items = []
            for item in processed_items:
                # Another complex conditional block (6+ operators)
                if ((validation_settings.get('require_prefix', False) and item.startswith(validation_settings.get('required_prefix', ''))) or
                    (validation_settings.get('allow_no_prefix', True) and validation_settings.get('flexible_validation', False)) and
                    ((len(item) >= validation_settings.get('min_length', 1) and len(item) <= validation_settings.get('max_length', 100)) or
                     (validation_settings.get('ignore_length', False) and validation_settings.get('length_not_critical', True))) and
                    (validation_settings.get('custom_validator', None) is None or self.validate_with_custom(item, validation_settings))):
                    validated_items.append(item)
                else:
                    validation_errors.append(f"Validation failed for item: {item}")
            
            # Output formatting phase (lines 65-85)
            formatted_results = []
            formatting_errors = []
            
            output_format = output_options.get('format', 'json')
            include_metadata = output_options.get('include_metadata', False)
            include_stats = output_options.get('include_statistics', True)
            
            for item in validated_items:
                try:
                    if output_format == 'json':
                        formatted_item = {'data': item, 'type': 'json_formatted'}
                    elif output_format == 'xml':
                        formatted_item = f"<item type='processed'>{item}</item>"
                    elif output_format == 'csv':
                        formatted_item = f'"{item}","processed"'
                    else:
                        formatted_item = f"CUSTOM:{item}"
                    
                    if include_metadata:
                        timestamp = str(__import__('datetime').datetime.now())
                        if isinstance(formatted_item, dict):
                            formatted_item['metadata'] = {'timestamp': timestamp, 'processor': 'long_method'}
                        else:
                            formatted_item = f"{formatted_item}|META:{timestamp}"
                    
                    formatted_results.append(formatted_item)
                except Exception as format_error:
                    formatting_errors.append(str(format_error))
            
            # Statistics compilation (lines 86-95)
            statistics = {
                'total_input': len(input_data),
                'processed': len(processed_items),
                'validated': len(validated_items),
                'formatted': len(formatted_results),
                'errors': self.error_count,
                'warnings': self.warning_count,
                'validation_errors': len(validation_errors),
                'formatting_errors': len(formatting_errors),
                'success_rate': len(formatted_results) / max(1, len(input_data))
            }
            
            return {
                'results': formatted_results,
                'statistics': statistics,
                'errors': validation_errors + formatting_errors
            }
        
        else:
            # Fallback processing (still part of long method)
            self.logger.warning("Processing conditions not met, using fallback")
            return {
                'results': [],
                'statistics': {'total_input': len(input_data) if input_data else 0, 'processed': 0},
                'errors': ['Processing conditions not satisfied']
            }
    
    def validate_with_custom(self, item, settings):
        """Helper method for custom validation."""
        custom_validator = settings.get('custom_validator')
        if callable(custom_validator):
            return custom_validator(item)
        return True
    
    def duplicate_data_processor_first(self, data_list, config_settings):
        """
        DUPLICATE CODE PATTERN #1
        This method contains logic that will be duplicated in the next method.
        15+ lines, 25+ tokens to ensure duplicate detection.
        """
        results = []
        errors = []
        processed_count = 0
        
        # This exact logic block is duplicated below
        for data_item in data_list:
            try:
                if data_item is None:
                    errors.append("Null data item encountered")
                    continue
                
                processed_item = str(data_item).strip()
                if len(processed_item) < config_settings.get('min_length', 3):
                    errors.append(f"Item too short: {processed_item}")
                    continue
                
                if config_settings.get('uppercase', False):
                    processed_item = processed_item.upper()
                
                if config_settings.get('add_prefix', False):
                    prefix = config_settings.get('prefix_value', 'PROC_')
                    processed_item = prefix + processed_item
                
                if config_settings.get('validate_format', True):
                    if not self.validate_item_format(processed_item, config_settings):
                        errors.append(f"Format validation failed: {processed_item}")
                        continue
                
                results.append(processed_item)
                processed_count += 1
                
            except Exception as e:
                errors.append(f"Processing error for {data_item}: {e}")
        
        return {
            'processed_data': results,
            'processing_errors': errors,
            'total_processed': processed_count,
            'success_rate': processed_count / max(1, len(data_list))
        }
    
    def duplicate_data_processor_second(self, data_list, config_settings):
        """
        DUPLICATE CODE PATTERN #2
        This method is nearly identical to duplicate_data_processor_first.
        Should be detected as requiring consolidation.
        """
        results = []
        errors = []
        processed_count = 0
        
        # This exact logic block is duplicated from above
        for data_item in data_list:
            try:
                if data_item is None:
                    errors.append("Null data item encountered")
                    continue
                
                processed_item = str(data_item).strip()
                if len(processed_item) < config_settings.get('min_length', 3):
                    errors.append(f"Item too short: {processed_item}")
                    continue
                
                if config_settings.get('uppercase', False):
                    processed_item = processed_item.upper()
                
                if config_settings.get('add_prefix', False):
                    prefix = config_settings.get('prefix_value', 'PROC_')
                    processed_item = prefix + processed_item
                
                if config_settings.get('validate_format', True):
                    if not self.validate_item_format(processed_item, config_settings):
                        errors.append(f"Format validation failed: {processed_item}")
                        continue
                
                results.append(processed_item)
                processed_count += 1
                
            except Exception as e:
                errors.append(f"Processing error for {data_item}: {e}")
        
        return {
            'processed_data': results,
            'processing_errors': errors,
            'total_processed': processed_count,
            'success_rate': processed_count / max(1, len(data_list))
        }
    
    def validate_item_format(self, item, config):
        """Helper method for format validation."""
        return len(item) > 0 and item.isalnum()
    
    def authorization_method_with_complex_conditionals(self, user, resource, action, context, policies):
        """
        COMPLEX CONDITIONALS PATTERN
        Method with 10+ logical operators in authorization logic.
        """
        # Authorization with extremely complex conditionals (12+ operators)
        if (((user.get('active', False) and user.get('verified', True)) or 
             (user.get('temp_access', False) and user.get('emergency_authorized', False))) and
            ((resource.get('public', False) and action in ['read', 'view', 'list']) or
             (resource.get('restricted', False) and user.get('clearance', 0) >= resource.get('min_clearance', 5)) or
             (resource.get('owner_id', None) == user.get('id', None) and action in ['read', 'write', 'update', 'delete'])) and
            ((context.get('secure_session', False) and context.get('encrypted_channel', True)) or
             (context.get('internal_network', False) and context.get('trusted_source', False))) and
            ((policies.get('allow_action', {}).get(action, False) and policies.get('resource_access', {}).get(resource.get('type', ''), False)) or
             (policies.get('admin_override', False) and user.get('role', '') in ['admin', 'superuser', 'root']))):
            return self.grant_access_with_audit(user, resource, action, context)
        else:
            return self.deny_access_with_logging(user, resource, action, context)
    
    def grant_access_with_audit(self, user, resource, action, context):
        """Grant access and log the decision."""
        self.logger.info(f"Access granted: {user.get('id')} -> {resource.get('id')} ({action})")
        return {'access': 'granted', 'user': user.get('id'), 'resource': resource.get('id'), 'action': action}
    
    def deny_access_with_logging(self, user, resource, action, context):
        """Deny access and log the decision."""
        self.logger.warning(f"Access denied: {user.get('id')} -> {resource.get('id')} ({action})")
        return {'access': 'denied', 'user': user.get('id'), 'resource': resource.get('id'), 'action': action}
    
    # Additional methods to reach the large class threshold (12+ methods)
    def method_08_cache_management(self, key, data):
        """Cache management functionality."""
        try:
            self.cache.set(key, data, ttl=3600)
            return True
        except Exception as e:
            self.error_count += 1
            return False
    
    def method_09_database_operations(self, operation, data):
        """Database operation wrapper."""
        try:
            if operation == 'insert':
                return self.database.insert(data)
            elif operation == 'update':
                return self.database.update(data)
            elif operation == 'delete':
                return self.database.delete(data)
            else:
                return None
        except Exception as e:
            self.error_count += 1
            return False
    
    def method_10_metrics_collection(self, metric_name, value):
        """Collect performance metrics."""
        if metric_name not in self.performance_data:
            self.performance_data[metric_name] = []
        self.performance_data[metric_name].append(value)
    
    def method_11_validation_rules_management(self, rule):
        """Manage validation rules."""
        self.validation_rules.append(rule)
    
    def method_12_transformation_pipeline(self, transformer):
        """Add transformer to pipeline."""
        self.transformation_pipeline.append(transformer)
    
    def method_13_error_reporting(self):
        """Generate error report."""
        return {
            'error_count': self.error_count,
            'warning_count': self.warning_count,
            'processed_items': self.processed_items
        }
    
    def method_14_performance_analysis(self):
        """Analyze performance metrics."""
        analysis = {}
        for metric, values in self.performance_data.items():
            analysis[metric] = {
                'average': sum(values) / len(values),
                'min': min(values),
                'max': max(values),
                'count': len(values)
            }
        return analysis
    
    def method_15_system_health_check(self):
        """Check system component health."""
        health = {
            'database': self.database is not None,
            'cache': self.cache is not None,
            'validator': self.validator is not None,
            'logger': self.logger is not None
        }
        return all(health.values())
    
    def method_16_cleanup_resources(self):
        """Clean up all allocated resources."""
        self.processed_items = 0
        self.error_count = 0
        self.warning_count = 0
        self.performance_data.clear()
        self.validation_rules.clear()
        self.transformation_pipeline.clear()
    
    def method_17_export_configuration(self):
        """Export current configuration."""
        return {
            'config': self.config,
            'validation_rules': len(self.validation_rules),
            'transformation_pipeline': len(self.transformation_pipeline),
            'performance_metrics': list(self.performance_data.keys())
        }
    
    def method_18_import_configuration(self, config_data):
        """Import configuration from data."""
        if 'config' in config_data:
            self.config.update(config_data['config'])
        return True
    
    def method_19_backup_state(self):
        """Backup current processing state."""
        return {
            'processed_items': self.processed_items,
            'error_count': self.error_count,
            'warning_count': self.warning_count,
            'performance_data': self.performance_data.copy()
        }
    
    def method_20_restore_state(self, state_data):
        """Restore processing state from backup."""
        self.processed_items = state_data.get('processed_items', 0)
        self.error_count = state_data.get('error_count', 0)
        self.warning_count = state_data.get('warning_count', 0)
        self.performance_data = state_data.get('performance_data', {})


def standalone_long_function_with_complex_logic(data_source, processing_rules, output_config, audit_settings):
    """
    STANDALONE LONG METHOD + COMPLEX CONDITIONALS
    Function with 70+ lines and complex conditional logic.
    Not part of a class but should still trigger long method refactoring.
    """
    # Complex initial validation (8+ operators)
    if ((data_source is not None and len(data_source) > 0) and
        (processing_rules.get('enabled', True) or processing_rules.get('force_execution', False)) and
        ((output_config.get('format', '') in ['json', 'xml', 'csv'] and output_config.get('valid_format', True)) or
         (output_config.get('custom_format', False) and output_config.get('allow_custom', True))) and
        (audit_settings.get('log_processing', True) and audit_settings.get('audit_enabled', False)) and
        ((processing_rules.get('strict_mode', False) and processing_rules.get('validate_all', True)) or
         (processing_rules.get('lenient_mode', True) and processing_rules.get('skip_validation', False)))):
        
        # Phase 1: Data collection and preprocessing (lines 10-25)
        collected_data = []
        collection_errors = []
        preprocessing_warnings = []
        
        for source_item in data_source:
            try:
                if source_item is None:
                    collection_errors.append("Encountered null source item")
                    continue
                
                item_str = str(source_item).strip()
                if len(item_str) == 0:
                    preprocessing_warnings.append("Empty string after strip operation")
                    continue
                
                # Item type analysis
                item_metadata = {
                    'original': source_item,
                    'string_repr': item_str,
                    'length': len(item_str),
                    'type': type(source_item).__name__
                }
                
                collected_data.append(item_metadata)
                
            except Exception as e:
                collection_errors.append(f"Collection error: {e}")
        
        # Phase 2: Rule application and validation (lines 30-50)
        processed_data = []
        rule_violations = []
        
        for item in collected_data:
            rule_results = {'item': item, 'passed_rules': [], 'failed_rules': []}
            
            for rule in processing_rules.get('validation_rules', []):
                rule_name = rule.get('name', 'unnamed_rule')
                rule_type = rule.get('type', 'string')
                rule_params = rule.get('parameters', {})
                
                try:
                    if rule_type == 'length':
                        min_len = rule_params.get('min', 0)
                        max_len = rule_params.get('max', 1000)
                        if min_len <= item['length'] <= max_len:
                            rule_results['passed_rules'].append(rule_name)
                        else:
                            rule_results['failed_rules'].append(rule_name)
                    elif rule_type == 'type_check':
                        expected_type = rule_params.get('expected', 'str')
                        if item['type'] == expected_type:
                            rule_results['passed_rules'].append(rule_name)
                        else:
                            rule_results['failed_rules'].append(rule_name)
                    elif rule_type == 'pattern':
                        pattern = rule_params.get('pattern', '')
                        if pattern in item['string_repr']:
                            rule_results['passed_rules'].append(rule_name)
                        else:
                            rule_results['failed_rules'].append(rule_name)
                except Exception as e:
                    rule_violations.append(f"Rule {rule_name} execution error: {e}")
            
            processed_data.append(rule_results)
        
        # Phase 3: Output formatting and finalization (lines 55-75)
        final_output = []
        formatting_errors = []
        
        output_format = output_config.get('format', 'json')
        include_metadata = output_config.get('include_metadata', True)
        include_rule_results = output_config.get('include_rule_results', False)
        
        for processed_item in processed_data:
            try:
                if output_format == 'json':
                    output_item = {
                        'data': processed_item['item']['string_repr'],
                        'original_type': processed_item['item']['type']
                    }
                elif output_format == 'xml':
                    data_value = processed_item['item']['string_repr']
                    output_item = f"<item type='{processed_item['item']['type']}'>{data_value}</item>"
                elif output_format == 'csv':
                    data_value = processed_item['item']['string_repr']
                    type_value = processed_item['item']['type']
                    output_item = f'"{data_value}","{type_value}"'
                else:
                    output_item = str(processed_item['item']['string_repr'])
                
                if include_metadata and isinstance(output_item, dict):
                    output_item['metadata'] = {
                        'length': processed_item['item']['length'],
                        'processing_timestamp': str(__import__('datetime').datetime.now())
                    }
                
                if include_rule_results and isinstance(output_item, dict):
                    output_item['rule_results'] = {
                        'passed': processed_item['passed_rules'],
                        'failed': processed_item['failed_rules']
                    }
                
                final_output.append(output_item)
                
            except Exception as e:
                formatting_errors.append(f"Formatting error: {e}")
        
        # Generate comprehensive results
        return {
            'output': final_output,
            'statistics': {
                'total_input': len(data_source),
                'collected': len(collected_data),
                'processed': len(processed_data),
                'formatted': len(final_output),
                'collection_errors': len(collection_errors),
                'rule_violations': len(rule_violations),
                'formatting_errors': len(formatting_errors)
            },
            'errors': collection_errors + rule_violations + formatting_errors
        }
    
    else:
        # Return minimal result if conditions not met
        return {
            'output': [],
            'statistics': {'total_input': len(data_source) if data_source else 0},
            'errors': ['Processing conditions not satisfied']
        }