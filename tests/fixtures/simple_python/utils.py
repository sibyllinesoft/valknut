"""
Utility functions with various code quality issues.
"""

from typing import Any, Dict, List, Optional
import re
import hashlib


class StringUtils:
    """String utility class with code duplication."""
    
    @staticmethod
    def clean_email(email: str) -> Optional[str]:
        """Clean and validate email address."""
        if not email:
            return None
        
        # Remove whitespace
        email = email.strip().lower()
        
        # Basic validation
        if '@' not in email:
            return None
        
        # Split and validate parts
        parts = email.split('@')
        if len(parts) != 2:
            return None
        
        local, domain = parts
        if not local or not domain:
            return None
        
        # More validation
        if '.' not in domain:
            return None
        
        # Pattern validation
        pattern = r'^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$'
        if not re.match(pattern, email):
            return None
        
        return email
    
    @staticmethod
    def clean_phone(phone: str) -> Optional[str]:
        """Clean and validate phone number - similar pattern to email."""
        if not phone:
            return None
        
        # Remove whitespace
        phone = phone.strip()
        
        # Remove common separators
        phone = re.sub(r'[^\d+]', '', phone)
        
        # Basic validation
        if len(phone) < 10:
            return None
        
        # US format validation
        if phone.startswith('+1'):
            phone = phone[2:]
        
        if len(phone) != 10:
            return None
        
        # Pattern validation
        pattern = r'^\d{10}$'
        if not re.match(pattern, phone):
            return None
        
        return phone
    
    @staticmethod
    def clean_username(username: str) -> Optional[str]:
        """Clean and validate username - another similar pattern."""
        if not username:
            return None
        
        # Remove whitespace
        username = username.strip().lower()
        
        # Length validation
        if len(username) < 3 or len(username) > 20:
            return None
        
        # Character validation
        if not username.replace('_', '').replace('-', '').isalnum():
            return None
        
        # Pattern validation
        pattern = r'^[a-zA-Z0-9_-]+$'
        if not re.match(pattern, username):
            return None
        
        return username


def hash_data(data: Any, algorithm: str = 'sha256') -> str:
    """Function with complex conditional logic."""
    if data is None:
        return ""
    
    # Convert data to string
    if isinstance(data, str):
        str_data = data
    elif isinstance(data, int):
        str_data = str(data)
    elif isinstance(data, float):
        str_data = str(data)
    elif isinstance(data, bool):
        str_data = 'true' if data else 'false'
    elif isinstance(data, list):
        str_data = ','.join(str(item) for item in data)
    elif isinstance(data, dict):
        str_data = ','.join(f"{k}:{v}" for k, v in sorted(data.items()))
    else:
        str_data = str(data)
    
    # Choose algorithm
    if algorithm == 'md5':
        hasher = hashlib.md5()
    elif algorithm == 'sha1':
        hasher = hashlib.sha1()
    elif algorithm == 'sha256':
        hasher = hashlib.sha256()
    elif algorithm == 'sha512':
        hasher = hashlib.sha512()
    else:
        raise ValueError(f"Unsupported algorithm: {algorithm}")
    
    hasher.update(str_data.encode('utf-8'))
    return hasher.hexdigest()


class DataValidator:
    """Class with god object tendencies."""
    
    def __init__(self):
        self.rules = {}
        self.errors = []
        self.warnings = []
        self.info = []
    
    def validate_user_data(self, user_data: Dict[str, Any]) -> bool:
        """Massive method that does too much."""
        self.errors.clear()
        self.warnings.clear()
        self.info.clear()
        
        # Email validation
        if 'email' in user_data:
            email = user_data['email']
            if not email:
                self.errors.append("Email is required")
            elif not isinstance(email, str):
                self.errors.append("Email must be a string")
            else:
                cleaned_email = StringUtils.clean_email(email)
                if not cleaned_email:
                    self.errors.append("Invalid email format")
                else:
                    user_data['email'] = cleaned_email
        
        # Phone validation
        if 'phone' in user_data:
            phone = user_data['phone']
            if phone:  # Phone is optional
                if not isinstance(phone, str):
                    self.errors.append("Phone must be a string")
                else:
                    cleaned_phone = StringUtils.clean_phone(phone)
                    if not cleaned_phone:
                        self.warnings.append("Invalid phone format")
                    else:
                        user_data['phone'] = cleaned_phone
        
        # Username validation
        if 'username' in user_data:
            username = user_data['username']
            if not username:
                self.errors.append("Username is required")
            elif not isinstance(username, str):
                self.errors.append("Username must be a string")
            else:
                cleaned_username = StringUtils.clean_username(username)
                if not cleaned_username:
                    self.errors.append("Invalid username format")
                else:
                    user_data['username'] = cleaned_username
        
        # Age validation
        if 'age' in user_data:
            age = user_data['age']
            if not isinstance(age, (int, float)):
                try:
                    age = int(age)
                    user_data['age'] = age
                except (ValueError, TypeError):
                    self.errors.append("Age must be a number")
            else:
                if age < 0:
                    self.errors.append("Age cannot be negative")
                elif age > 150:
                    self.warnings.append("Age seems unusually high")
                elif age < 13:
                    self.warnings.append("User may be under minimum age")
        
        # Profile data validation
        if 'profile' in user_data:
            profile = user_data['profile']
            if not isinstance(profile, dict):
                self.errors.append("Profile must be an object")
            else:
                # Validate profile fields
                if 'bio' in profile:
                    bio = profile['bio']
                    if bio and len(bio) > 500:
                        self.warnings.append("Bio is very long")
                
                if 'interests' in profile:
                    interests = profile['interests']
                    if not isinstance(interests, list):
                        self.errors.append("Interests must be a list")
                    elif len(interests) > 20:
                        self.warnings.append("Too many interests listed")
        
        return len(self.errors) == 0
    
    def get_validation_report(self) -> Dict[str, Any]:
        """Return validation results."""
        return {
            'valid': len(self.errors) == 0,
            'errors': self.errors.copy(),
            'warnings': self.warnings.copy(),
            'info': self.info.copy()
        }


# Utility functions with high complexity
def process_batch_data(batch_data: List[Dict[str, Any]], options: Dict[str, Any] = None) -> Dict[str, Any]:
    """Complex function that processes batch data."""
    if not batch_data:
        return {'processed': 0, 'errors': 0, 'results': []}
    
    options = options or {}
    validator = DataValidator()
    results = []
    error_count = 0
    processed_count = 0
    
    # Process each item
    for i, item in enumerate(batch_data):
        try:
            # Validate item
            if not isinstance(item, dict):
                error_count += 1
                continue
            
            # Apply transformations based on options
            if options.get('clean_strings', True):
                for key, value in item.items():
                    if isinstance(value, str):
                        item[key] = value.strip()
            
            if options.get('validate_users', False):
                if not validator.validate_user_data(item):
                    error_count += 1
                    continue
            
            # Add metadata
            if options.get('add_metadata', True):
                item['_processed_at'] = hash_data(f"{i}:{item}")
                item['_batch_index'] = i
            
            # Apply filters
            if options.get('filters'):
                filters = options['filters']
                skip_item = False
                
                for filter_key, filter_value in filters.items():
                    if filter_key in item:
                        if isinstance(filter_value, list):
                            if item[filter_key] not in filter_value:
                                skip_item = True
                                break
                        elif item[filter_key] != filter_value:
                            skip_item = True
                            break
                
                if skip_item:
                    continue
            
            results.append(item)
            processed_count += 1
            
        except Exception as e:
            error_count += 1
            if options.get('debug', False):
                print(f"Error processing item {i}: {e}")
    
    return {
        'processed': processed_count,
        'errors': error_count,
        'results': results,
        'total_input': len(batch_data)
    }