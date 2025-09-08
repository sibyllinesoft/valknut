#!/usr/bin/env python3
"""
Create test files for structure analyzer testing.
"""
from pathlib import Path

def create_overcrowded_directory():
    """Create a directory with too many small files."""
    crowded_dir = Path("overcrowded")
    crowded_dir.mkdir(exist_ok=True)
    
    # Create 40 tiny files - should trigger branch reorganization
    for i in range(40):
        file_path = crowded_dir / f"tiny_file_{i:02d}.py"
        content = f"""# Tiny file {i}
class TinyClass{i}:
    def method_{i}(self):
        return {i}

def utility_function_{i}():
    return "utility_{i}"
"""
        file_path.write_text(content)
    
    print(f"Created {len(list(crowded_dir.glob('*.py')))} files in {crowded_dir}")

def create_imbalanced_directory():
    """Create directory with one huge file and many small ones."""
    imbalanced_dir = Path("imbalanced") 
    imbalanced_dir.mkdir(exist_ok=True)
    
    # One massive 2000-LOC file
    huge_content = """# Huge file with everything mixed together
import os
import sys
import json
from typing import Dict, List, Optional

"""
    
    # Add classes from different domains
    for i in range(30):
        huge_content += f"""
class DatabaseModel{i}:
    def __init__(self):
        self.id = {i}
        self.name = "model_{i}"
        
    def save(self):
        pass
        
    def load(self):
        pass
        
    def validate(self):
        return True
"""

    for i in range(20):
        huge_content += f"""
class UIComponent{i}:
    def render(self):
        return f"<div>Component {i}</div>"
        
    def handle_click(self):
        print("Clicked {i}")
"""

    for i in range(25):
        huge_content += f"""
def utility_function_{i}(data):
    # Complex processing
    result = data + {i}
    if result > 100:
        return result * 2
    return result

def test_function_{i}():
    assert utility_function_{i}(50) > 0
"""

    huge_file = imbalanced_dir / "everything.py"
    huge_file.write_text(huge_content)
    
    # Many small files (50-LOC each)
    for i in range(15):
        small_content = f"""# Small file {i}
def small_function_{i}():
    return {i} * 2

class SmallClass{i}:
    value = {i}
"""
        small_file = imbalanced_dir / f"small_{i:02d}.py"
        small_file.write_text(small_content)
    
    print(f"Created huge file ({len(huge_content.splitlines())} lines) and {len(list(imbalanced_dir.glob('small_*.py')))} small files")

def create_cohesive_large_file():
    """Create a large file with 3 distinct cohesive clusters."""
    large_file = Path("cohesive_large.py")
    
    content = """# Large file with 3 cohesive communities that should be split
from typing import Dict, List, Optional
import json

# === USER MANAGEMENT CLUSTER ===
class User:
    def __init__(self, user_id: str, name: str, email: str):
        self.user_id = user_id
        self.name = name
        self.email = email
        
    def authenticate(self, password: str) -> bool:
        # Simulate authentication
        return len(password) > 8
        
    def update_profile(self, name: str = None, email: str = None):
        if name:
            self.name = name
        if email:
            self.email = email
            
    def get_preferences(self) -> Dict:
        return {"theme": "dark", "notifications": True}

class UserManager:
    def __init__(self):
        self.users = {}
        
    def create_user(self, name: str, email: str) -> User:
        user_id = f"user_{len(self.users)}"
        user = User(user_id, name, email)
        self.users[user_id] = user
        return user
        
    def find_user(self, user_id: str) -> Optional[User]:
        return self.users.get(user_id)
        
    def list_users(self) -> List[User]:
        return list(self.users.values())

def validate_email(email: str) -> bool:
    return "@" in email and "." in email.split("@")[1]

def hash_password(password: str) -> str:
    # Simplified hashing
    return f"hashed_{password[::-1]}"

# === DATA PROCESSING CLUSTER ===
class DataProcessor:
    def __init__(self):
        self.cache = {}
        
    def process_json(self, data: str) -> Dict:
        try:
            return json.loads(data)
        except json.JSONDecodeError:
            return {}
            
    def validate_data(self, data: Dict) -> bool:
        required_fields = ["id", "type", "value"]
        return all(field in data for field in required_fields)
        
    def transform_data(self, data: Dict) -> Dict:
        transformed = data.copy()
        if "value" in transformed:
            transformed["value"] = str(transformed["value"]).upper()
        return transformed
        
    def cache_result(self, key: str, value: Dict):
        self.cache[key] = value
        
    def get_cached(self, key: str) -> Optional[Dict]:
        return self.cache.get(key)

class DataExporter:
    def export_to_json(self, data: List[Dict]) -> str:
        return json.dumps(data, indent=2)
        
    def export_to_csv(self, data: List[Dict]) -> str:
        if not data:
            return ""
        headers = list(data[0].keys())
        csv_lines = [",".join(headers)]
        for row in data:
            csv_lines.append(",".join(str(row.get(h, "")) for h in headers))
        return "\\n".join(csv_lines)
        
    def export_to_xml(self, data: List[Dict]) -> str:
        xml_lines = ["<data>"]
        for item in data:
            xml_lines.append("  <item>")
            for key, value in item.items():
                xml_lines.append(f"    <{key}>{value}</{key}>")
            xml_lines.append("  </item>")
        xml_lines.append("</data>")
        return "\\n".join(xml_lines)

def sanitize_input(input_str: str) -> str:
    # Remove potentially dangerous characters
    dangerous = ["<", ">", "&", "script", "eval"]
    cleaned = input_str
    for char in dangerous:
        cleaned = cleaned.replace(char, "")
    return cleaned

# === API INTERFACE CLUSTER ===
class APIResponse:
    def __init__(self, status: int, data: Dict = None, error: str = None):
        self.status = status
        self.data = data or {}
        self.error = error
        
    def to_dict(self) -> Dict:
        result = {"status": self.status}
        if self.data:
            result["data"] = self.data
        if self.error:
            result["error"] = self.error
        return result
        
    def is_success(self) -> bool:
        return 200 <= self.status < 300

class APIHandler:
    def __init__(self, user_manager: UserManager, processor: DataProcessor):
        self.user_manager = user_manager
        self.processor = processor
        
    def handle_user_request(self, request: Dict) -> APIResponse:
        action = request.get("action")
        
        if action == "create_user":
            name = request.get("name", "")
            email = request.get("email", "")
            if not validate_email(email):
                return APIResponse(400, error="Invalid email")
            user = self.user_manager.create_user(name, email)
            return APIResponse(200, {"user_id": user.user_id})
            
        elif action == "get_user":
            user_id = request.get("user_id", "")
            user = self.user_manager.find_user(user_id)
            if not user:
                return APIResponse(404, error="User not found")
            return APIResponse(200, {"user": user.__dict__})
            
        return APIResponse(400, error="Unknown action")
        
    def handle_data_request(self, request: Dict) -> APIResponse:
        action = request.get("action")
        
        if action == "process":
            raw_data = request.get("data", "")
            data = self.processor.process_json(raw_data)
            if self.processor.validate_data(data):
                transformed = self.processor.transform_data(data)
                return APIResponse(200, {"result": transformed})
            return APIResponse(400, error="Invalid data format")
            
        return APIResponse(400, error="Unknown data action")

def create_api_error(message: str, status: int = 500) -> APIResponse:
    return APIResponse(status, error=message)

def log_api_request(request: Dict, response: APIResponse):
    print(f"API Request: {request.get('action', 'unknown')} -> Status: {response.status}")

# Usage example showing how clusters interact
if __name__ == "__main__":
    # User management
    user_mgr = UserManager()
    user = user_mgr.create_user("John Doe", "john@example.com")
    
    # Data processing  
    processor = DataProcessor()
    sample_data = '{"id": 1, "type": "test", "value": "hello"}'
    processed = processor.process_json(sample_data)
    
    # API handling
    api = APIHandler(user_mgr, processor)
    response = api.handle_user_request({"action": "create_user", "name": "Jane", "email": "jane@test.com"})
    print(response.to_dict())
"""
    
    large_file.write_text(content)
    print(f"Created large cohesive file ({len(content.splitlines())} lines)")

if __name__ == "__main__":
    create_overcrowded_directory()
    create_imbalanced_directory()  
    create_cohesive_large_file()
    print("Test fixtures created successfully!")