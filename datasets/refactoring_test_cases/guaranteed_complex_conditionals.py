#!/usr/bin/env python3
"""
Test file with guaranteed complex conditional detection.
COMPLEX_CONDITIONAL_THRESHOLD = 4, so these functions exceed that.
"""

def extremely_complex_condition_checker(user_role, permissions, resource_type, action, context):
    """
    Function with 8+ logical operators - well above the threshold of 4.
    """
    if ((user_role == 'admin' or user_role == 'superuser') and 
        (permissions.get('read', False) and permissions.get('write', False)) or
        (resource_type == 'public' and action == 'read') or
        (user_role == 'moderator' and resource_type == 'forum' and action in ['read', 'write']) and
        (context.get('authenticated', False) and context.get('session_valid', False))):
        return True
    return False

def another_complex_conditional_function(data, filters, options, metadata):
    """
    Another function with many logical operators for testing.
    """
    # This conditional has 10+ logical operators
    if ((data is not None and len(data) > 0) and
        (filters.get('enabled', True) or filters.get('force_enabled', False)) and
        ((options.get('strict_mode', False) and options.get('validate_all', True)) or
         (options.get('loose_mode', True) and options.get('skip_validation', False))) and
        (metadata.get('version', 0) >= 2 or metadata.get('legacy_support', False)) and
        ((metadata.get('source', '') == 'trusted' and metadata.get('verified', False)) or
         (metadata.get('source', '') == 'internal' and metadata.get('system_generated', True)))):
        return process_complex_data(data, filters, options, metadata)
    elif ((data is None or len(data) == 0) and
          (options.get('allow_empty', False) or options.get('default_empty_behavior', True)) and
          (filters.get('handle_empty', True) and filters.get('empty_is_valid', False))):
        return handle_empty_case(filters, options, metadata)
    else:
        return None

def process_complex_data(data, filters, options, metadata):
    """Helper function to avoid making the main function even longer."""
    return f"Processed {len(data)} items with filters and options"

def handle_empty_case(filters, options, metadata):
    """Helper function for empty data case."""
    return "Handled empty data case"

def validation_function_with_complex_logic(input_value, validation_config, business_rules, audit_context):
    """
    Function with nested complex conditionals that exceed threshold.
    """
    # First complex conditional block
    if ((input_value is not None and str(input_value).strip() != '') and
        (validation_config.get('required', False) or validation_config.get('enforce_presence', True)) and
        ((business_rules.get('allow_null', False) and business_rules.get('null_is_valid', True)) or
         (business_rules.get('strict_validation', True) and business_rules.get('no_empty_strings', False))) and
        (audit_context.get('log_validation', True) and audit_context.get('track_decisions', False))):
        
        # Second complex conditional block  
        if ((len(str(input_value)) >= validation_config.get('min_length', 0)) and
            (len(str(input_value)) <= validation_config.get('max_length', 1000)) and
            ((business_rules.get('allow_special_chars', False) and business_rules.get('unicode_allowed', True)) or
             (business_rules.get('alphanumeric_only', True) and str(input_value).isalnum())) and
            (audit_context.get('detailed_logging', False) or audit_context.get('simple_audit', True))):
            return {"valid": True, "value": input_value}
    
    return {"valid": False, "value": None, "error": "Complex validation failed"}

def authorization_checker(user, resource, action, environment, policies):
    """
    Authorization function with extremely complex conditional logic.
    """
    # Master authorization conditional with 12+ operators
    if (((user.get('active', False) and user.get('verified', False)) or
         (user.get('temporary_access', False) and user.get('emergency_override', False))) and
        ((resource.get('public', False) and action in ['read', 'view']) or
         (resource.get('restricted', False) and user.get('clearance_level', 0) >= resource.get('required_clearance', 10)) or
         (resource.get('owner_id', None) == user.get('id', None) and action in ['read', 'write', 'delete'])) and
        ((environment.get('secure_connection', False) and environment.get('trusted_network', False)) or
         (environment.get('internal_access', False) and environment.get('vpn_connected', True))) and
        ((policies.get('allow_action', {}).get(action, False) and policies.get('resource_permissions', {}).get(resource.get('type', ''), False)) or
         (policies.get('admin_override', False) and user.get('role', '') in ['admin', 'superuser']))):
        return grant_access(user, resource, action, environment, policies)
    else:
        return deny_access(user, resource, action, environment, policies)

def grant_access(user, resource, action, environment, policies):
    """Helper function for access grant."""
    return {"access": "granted", "user": user.get('id'), "resource": resource.get('id'), "action": action}

def deny_access(user, resource, action, environment, policies):
    """Helper function for access denial."""
    return {"access": "denied", "user": user.get('id'), "resource": resource.get('id'), "action": action}