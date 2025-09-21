#!/usr/bin/env python3
"""
Test file with guaranteed large class detection.
LARGE_CLASS_LINE_THRESHOLD = 200 and LARGE_CLASS_MEMBER_THRESHOLD = 12
This class is designed to exceed both thresholds.
"""

class MassiveDataProcessingClass:
    """
    Intentionally large class with 20+ methods and 250+ lines.
    Should trigger ExtractClass refactoring recommendation.
    """
    
    def __init__(self, config, database_connection, cache_manager, logger):
        self.config = config
        self.database = database_connection
        self.cache = cache_manager
        self.logger = logger
        self.processed_count = 0
        self.error_count = 0
        self.warning_count = 0
        self.performance_metrics = {}
    
    def method_01_validate_input(self, input_data):
        """Input validation method."""
        if input_data is None:
            self.error_count += 1
            return False
        if not isinstance(input_data, (dict, list, str)):
            self.error_count += 1
            return False
        return True
    
    def method_02_preprocess_data(self, raw_data):
        """Data preprocessing method."""
        processed = []
        for item in raw_data:
            if self.method_01_validate_input(item):
                processed.append(str(item).strip().lower())
        return processed
    
    def method_03_apply_filters(self, data, filter_config):
        """Apply filtering rules to data."""
        filtered = []
        for item in data:
            if self.method_04_check_filter_match(item, filter_config):
                filtered.append(item)
        return filtered
    
    def method_04_check_filter_match(self, item, filter_config):
        """Check if item matches filter criteria."""
        for filter_rule in filter_config:
            if filter_rule.get('pattern', '') in item:
                return filter_rule.get('include', True)
        return True
    
    def method_05_transform_data(self, data, transformation_rules):
        """Apply transformation rules to data."""
        transformed = []
        for item in data:
            result = self.method_06_apply_single_transformation(item, transformation_rules)
            transformed.append(result)
        return transformed
    
    def method_06_apply_single_transformation(self, item, rules):
        """Apply single transformation to an item."""
        result = item
        for rule in rules:
            if rule['type'] == 'uppercase':
                result = result.upper()
            elif rule['type'] == 'prefix':
                result = rule['value'] + result
            elif rule['type'] == 'suffix':
                result = result + rule['value']
        return result
    
    def method_07_store_to_database(self, data):
        """Store processed data to database."""
        try:
            for item in data:
                self.database.insert(item)
                self.processed_count += 1
            return True
        except Exception as e:
            self.error_count += 1
            self.logger.error(f"Database error: {e}")
            return False
    
    def method_08_cache_results(self, key, data):
        """Cache processing results."""
        try:
            self.cache.set(key, data, ttl=3600)
            return True
        except Exception as e:
            self.warning_count += 1
            self.logger.warning(f"Cache error: {e}")
            return False
    
    def method_09_retrieve_from_cache(self, key):
        """Retrieve data from cache."""
        try:
            return self.cache.get(key)
        except Exception as e:
            self.warning_count += 1
            self.logger.warning(f"Cache retrieval error: {e}")
            return None
    
    def method_10_generate_report(self, data):
        """Generate processing report."""
        report = {
            'total_items': len(data),
            'processed_count': self.processed_count,
            'error_count': self.error_count,
            'warning_count': self.warning_count,
            'success_rate': self.processed_count / max(1, len(data))
        }
        return report
    
    def method_11_validate_report(self, report):
        """Validate generated report."""
        required_fields = ['total_items', 'processed_count', 'error_count', 'success_rate']
        for field in required_fields:
            if field not in report:
                return False
        return True
    
    def method_12_export_to_json(self, data):
        """Export data to JSON format."""
        import json
        try:
            return json.dumps(data, indent=2)
        except Exception as e:
            self.error_count += 1
            self.logger.error(f"JSON export error: {e}")
            return None
    
    def method_13_export_to_csv(self, data):
        """Export data to CSV format."""
        csv_lines = ['item']
        for item in data:
            csv_lines.append(f'"{item}"')
        return '\n'.join(csv_lines)
    
    def method_14_export_to_xml(self, data):
        """Export data to XML format."""
        xml_content = ['<data>']
        for item in data:
            xml_content.append(f'  <item>{item}</item>')
        xml_content.append('</data>')
        return '\n'.join(xml_content)
    
    def method_15_log_performance(self, operation, duration):
        """Log performance metrics."""
        if operation not in self.performance_metrics:
            self.performance_metrics[operation] = []
        self.performance_metrics[operation].append(duration)
        self.logger.info(f"{operation} took {duration:.3f}s")
    
    def method_16_get_average_performance(self, operation):
        """Get average performance for operation."""
        if operation in self.performance_metrics:
            metrics = self.performance_metrics[operation]
            return sum(metrics) / len(metrics)
        return 0.0
    
    def method_17_reset_counters(self):
        """Reset all processing counters."""
        self.processed_count = 0
        self.error_count = 0
        self.warning_count = 0
        self.performance_metrics = {}
    
    def method_18_backup_data(self, data, backup_location):
        """Backup processed data."""
        try:
            import pickle
            with open(backup_location, 'wb') as f:
                pickle.dump(data, f)
            return True
        except Exception as e:
            self.error_count += 1
            self.logger.error(f"Backup error: {e}")
            return False
    
    def method_19_restore_data(self, backup_location):
        """Restore data from backup."""
        try:
            import pickle
            with open(backup_location, 'rb') as f:
                return pickle.load(f)
        except Exception as e:
            self.error_count += 1
            self.logger.error(f"Restore error: {e}")
            return None
    
    def method_20_cleanup_resources(self):
        """Cleanup all allocated resources."""
        try:
            if self.database:
                self.database.close()
            if self.cache:
                self.cache.clear()
            self.method_17_reset_counters()
            return True
        except Exception as e:
            self.logger.error(f"Cleanup error: {e}")
            return False


class AnotherOversizedClass:
    """
    Another large class to ensure detection works with multiple classes.
    This class has 15+ methods and 220+ lines.
    """
    
    def __init__(self, service_config):
        self.config = service_config
        self.active_connections = {}
        self.request_queue = []
        self.response_cache = {}
        self.metrics = {}
    
    def service_method_01(self, request_data):
        """Handle incoming service request."""
        request_id = self.service_method_02_generate_id()
        self.request_queue.append({'id': request_id, 'data': request_data})
        return request_id
    
    def service_method_02_generate_id(self):
        """Generate unique request ID."""
        import uuid
        return str(uuid.uuid4())
    
    def service_method_03_process_queue(self):
        """Process all queued requests."""
        processed = []
        while self.request_queue:
            request = self.request_queue.pop(0)
            result = self.service_method_04_handle_request(request)
            processed.append(result)
        return processed
    
    def service_method_04_handle_request(self, request):
        """Handle individual request."""
        try:
            response = self.service_method_05_generate_response(request['data'])
            self.service_method_06_cache_response(request['id'], response)
            return {'id': request['id'], 'status': 'success', 'response': response}
        except Exception as e:
            return {'id': request['id'], 'status': 'error', 'error': str(e)}
    
    def service_method_05_generate_response(self, request_data):
        """Generate response for request data."""
        return f"Processed: {request_data}"
    
    def service_method_06_cache_response(self, request_id, response):
        """Cache response for future use."""
        self.response_cache[request_id] = response
    
    def service_method_07_get_cached_response(self, request_id):
        """Retrieve cached response."""
        return self.response_cache.get(request_id)
    
    def service_method_08_clear_cache(self):
        """Clear response cache."""
        self.response_cache.clear()
    
    def service_method_09_add_connection(self, connection_id, connection_info):
        """Add new active connection."""
        self.active_connections[connection_id] = connection_info
    
    def service_method_10_remove_connection(self, connection_id):
        """Remove active connection."""
        if connection_id in self.active_connections:
            del self.active_connections[connection_id]
    
    def service_method_11_get_connection_count(self):
        """Get number of active connections."""
        return len(self.active_connections)
    
    def service_method_12_record_metric(self, metric_name, value):
        """Record performance metric."""
        if metric_name not in self.metrics:
            self.metrics[metric_name] = []
        self.metrics[metric_name].append(value)
    
    def service_method_13_get_metric_average(self, metric_name):
        """Get average value for metric."""
        if metric_name in self.metrics:
            values = self.metrics[metric_name]
            return sum(values) / len(values)
        return 0.0
    
    def service_method_14_export_metrics(self):
        """Export all metrics data."""
        return {
            'metrics': self.metrics,
            'active_connections': len(self.active_connections),
            'cached_responses': len(self.response_cache),
            'queue_size': len(self.request_queue)
        }
    
    def service_method_15_shutdown(self):
        """Shutdown service and cleanup."""
        self.request_queue.clear()
        self.active_connections.clear()
        self.response_cache.clear()
        self.metrics.clear()
        return True