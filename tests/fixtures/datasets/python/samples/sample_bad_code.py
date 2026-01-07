"""
Sample bad code with intentional code smells for valknut testing.
This file contains various code quality issues to test detection capabilities.
"""

import os
import sys
import json
import datetime
import random


# Code Smell 1: God Class - Too many responsibilities
class DataProcessorManagerHandlerController:
    """A class that does way too many things - classic God Class smell."""
    
    def __init__(self):
        self.data = []
        self.processed_data = []
        self.config = {}
        self.logger = None
        self.database_connection = None
        self.file_handler = None
        self.email_service = None
        self.cache = {}
        self.statistics = {}
        self.user_preferences = {}
        self.security_settings = {}
        
    def load_data_from_file(self, filename):
        # File handling responsibility
        with open(filename, 'r') as f:
            self.data = json.load(f)
            
    def connect_to_database(self, host, port, username, password):
        # Database responsibility
        self.database_connection = f"Connected to {host}:{port}"
        
    def authenticate_user(self, username, password):
        # Authentication responsibility
        return username == "admin" and password == "123456"  # Bad security!
        
    def process_data(self):
        # Data processing responsibility
        for item in self.data:
            processed_item = self.complex_processing_logic(item)
            self.processed_data.append(processed_item)
            
    def complex_processing_logic(self, item):
        # Long method with high cyclomatic complexity
        result = {}
        
        if item.get('type') == 'A':
            if item.get('status') == 'active':
                if item.get('priority') > 5:
                    if item.get('category') == 'urgent':
                        result = {'processed': True, 'score': 100}
                    elif item.get('category') == 'normal':
                        result = {'processed': True, 'score': 80}
                    else:
                        result = {'processed': True, 'score': 60}
                elif item.get('priority') > 3:
                    if item.get('category') == 'urgent':
                        result = {'processed': True, 'score': 70}
                    else:
                        result = {'processed': True, 'score': 50}
                else:
                    result = {'processed': True, 'score': 30}
            elif item.get('status') == 'pending':
                if item.get('priority') > 7:
                    result = {'processed': True, 'score': 40}
                else:
                    result = {'processed': True, 'score': 20}
            else:
                result = {'processed': False, 'score': 0}
        elif item.get('type') == 'B':
            if item.get('status') == 'active':
                result = {'processed': True, 'score': 45}
            else:
                result = {'processed': False, 'score': 10}
        else:
            result = {'processed': False, 'score': 0}
            
        return result
        
    def send_email_notification(self, recipient, message):
        # Email responsibility
        print(f"Sending email to {recipient}: {message}")
        
    def log_activity(self, activity):
        # Logging responsibility
        print(f"[{datetime.datetime.now()}] {activity}")
        
    def cache_result(self, key, value):
        # Caching responsibility
        self.cache[key] = value
        
    def generate_report(self):
        # Reporting responsibility
        report = "Data Processing Report\n"
        report += "=" * 50 + "\n"
        report += f"Total items processed: {len(self.processed_data)}\n"
        return report


# Code Smell 2: Long Parameter List
def create_user_account(first_name, last_name, email, phone, address, city, state, 
                       zip_code, country, birth_date, gender, occupation, company, 
                       department, manager, salary, start_date, emergency_contact, 
                       emergency_phone, medical_conditions, dietary_restrictions):
    """Function with way too many parameters."""
    user = {
        'first_name': first_name,
        'last_name': last_name,
        'email': email,
        'phone': phone,
        'address': address,
        'city': city,
        'state': state,
        'zip_code': zip_code,
        'country': country,
        'birth_date': birth_date,
        'gender': gender,
        'occupation': occupation,
        'company': company,
        'department': department,
        'manager': manager,
        'salary': salary,
        'start_date': start_date,
        'emergency_contact': emergency_contact,
        'emergency_phone': emergency_phone,
        'medical_conditions': medical_conditions,
        'dietary_restrictions': dietary_restrictions
    }
    return user


# Code Smell 3: Magic Numbers and Strings
def calculate_discount(customer_type, order_amount, items):
    """Function full of magic numbers and strings."""
    discount = 0
    
    if customer_type == "PREMIUM_GOLD_VIP":  # Magic string
        if order_amount > 1000:  # Magic number
            discount = 0.15  # Magic number
        elif order_amount > 500:  # Magic number
            discount = 0.12  # Magic number
        else:
            discount = 0.08  # Magic number
    elif customer_type == "REGULAR_CUSTOMER":  # Magic string
        if order_amount > 750:  # Magic number
            discount = 0.10  # Magic number
        elif order_amount > 300:  # Magic number
            discount = 0.05  # Magic number
        else:
            discount = 0.02  # Magic number
    
    if len(items) > 10:  # Magic number
        discount += 0.03  # Magic number
    elif len(items) > 5:  # Magic number
        discount += 0.01  # Magic number
    
    return min(discount, 0.25)  # Magic number


# Code Smell 4: Duplicate Code
def process_order_payment_credit_card(order_id, amount, card_number, expiry, cvv):
    """Process credit card payment - lots of duplicate validation logic."""
    # Validate order
    if not order_id:
        raise ValueError("Order ID is required")
    if len(str(order_id)) < 5:
        raise ValueError("Invalid order ID format")
    if amount <= 0:
        raise ValueError("Amount must be positive")
    if amount > 10000:
        raise ValueError("Amount too large")
    
    # Validate card
    if not card_number or len(card_number) != 16:
        raise ValueError("Invalid card number")
    
    # Process payment
    print(f"Processing credit card payment: ${amount}")
    return {"status": "success", "transaction_id": f"cc_{order_id}"}


def process_order_payment_debit_card(order_id, amount, card_number, pin):
    """Process debit card payment - duplicate validation logic again."""
    # Validate order (DUPLICATE CODE!)
    if not order_id:
        raise ValueError("Order ID is required")
    if len(str(order_id)) < 5:
        raise ValueError("Invalid order ID format")
    if amount <= 0:
        raise ValueError("Amount must be positive")
    if amount > 10000:
        raise ValueError("Amount too large")
    
    # Validate card (DUPLICATE CODE!)
    if not card_number or len(card_number) != 16:
        raise ValueError("Invalid card number")
    
    # Process payment
    print(f"Processing debit card payment: ${amount}")
    return {"status": "success", "transaction_id": f"dc_{order_id}"}


def process_order_payment_paypal(order_id, amount, paypal_email):
    """Process PayPal payment - yet more duplicate validation."""
    # Validate order (DUPLICATE CODE AGAIN!)
    if not order_id:
        raise ValueError("Order ID is required")
    if len(str(order_id)) < 5:
        raise ValueError("Invalid order ID format")
    if amount <= 0:
        raise ValueError("Amount must be positive")
    if amount > 10000:
        raise ValueError("Amount too large")
    
    # Process payment
    print(f"Processing PayPal payment: ${amount}")
    return {"status": "success", "transaction_id": f"pp_{order_id}"}


# Code Smell 5: Large Class with poor cohesion
class UtilityHelper:
    """A utility class that groups unrelated functions together."""
    
    @staticmethod
    def format_currency(amount, currency="USD"):
        """Format currency amount."""
        return f"{currency} {amount:.2f}"
        
    @staticmethod
    def send_sms(phone_number, message):
        """Send SMS message."""
        print(f"SMS to {phone_number}: {message}")
        
    @staticmethod
    def compress_file(filename):
        """Compress a file."""
        return f"{filename}.gz"
        
    @staticmethod
    def validate_email(email):
        """Validate email format."""
        return "@" in email and "." in email
        
    @staticmethod
    def calculate_distance(lat1, lon1, lat2, lon2):
        """Calculate distance between coordinates."""
        return ((lat2 - lat1) ** 2 + (lon2 - lon1) ** 2) ** 0.5
        
    @staticmethod
    def hash_password(password):
        """Hash password (insecurely)."""
        return str(hash(password))  # Very bad security practice!
        
    @staticmethod
    def generate_random_color():
        """Generate random hex color."""
        return f"#{random.randint(0, 16777215):06x}"
        
    @staticmethod
    def parse_csv_line(line):
        """Parse CSV line."""
        return line.strip().split(',')


# Code Smell 6: Inappropriate Intimacy - Classes that know too much about each other
class BankAccount:
    def __init__(self, account_number, balance):
        self.account_number = account_number
        self.balance = balance
        self.transaction_history = []


class Transaction:
    def __init__(self, account):
        self.account = account
        
    def transfer_money(self, target_account, amount):
        """This method knows too much about BankAccount internals."""
        # Directly accessing and modifying account internals
        if self.account.balance >= amount:
            self.account.balance -= amount  # Direct manipulation
            target_account.balance += amount  # Direct manipulation
            
            # Direct manipulation of internal data structures
            self.account.transaction_history.append(f"Transfer out: ${amount}")
            target_account.transaction_history.append(f"Transfer in: ${amount}")
            
            return True
        return False


# Code Smell 7: Dead Code
def unused_function_that_nobody_calls():
    """This function is never called anywhere."""
    print("This code will never execute")
    return "dead code"


# More unused code
UNUSED_CONSTANT = 42
ANOTHER_UNUSED_CONSTANT = "never used"


def another_dead_function(param1, param2, param3):
    """Another function that's never used."""
    result = param1 + param2 * param3
    return result / 2


# Code Smell 8: Feature Envy - Method that uses another class's data more than its own
class Customer:
    def __init__(self, name, email, phone):
        self.name = name
        self.email = email
        self.phone = phone


class Order:
    def __init__(self, customer, items, total):
        self.customer = customer
        self.items = items
        self.total = total
        
    def print_customer_info(self):
        """This method is more interested in Customer than in Order."""
        # Uses customer data extensively but barely uses own data
        print(f"Customer Name: {self.customer.name}")
        print(f"Customer Email: {self.customer.email}")  
        print(f"Customer Phone: {self.customer.phone}")
        print(f"Customer Name Length: {len(self.customer.name)}")
        print(f"Customer Email Domain: {self.customer.email.split('@')[1]}")
        print(f"Customer Phone Area Code: {self.customer.phone[:3]}")
        # Only uses own data minimally
        print(f"Order Total: {self.total}")


# Code Smell 9: Shotgun Surgery - Changes require modifications in many places
# These global variables are used everywhere, making changes difficult
GLOBAL_TAX_RATE = 0.08
GLOBAL_SHIPPING_COST = 15.99
GLOBAL_DISCOUNT_THRESHOLD = 100


def calculate_item_price(base_price):
    """Uses global state."""
    return base_price * (1 + GLOBAL_TAX_RATE)


def calculate_shipping(order_total):
    """Uses global state."""
    if order_total > GLOBAL_DISCOUNT_THRESHOLD:
        return 0
    return GLOBAL_SHIPPING_COST


def calculate_total_with_tax(subtotal):
    """Uses global state."""
    return subtotal * (1 + GLOBAL_TAX_RATE) + calculate_shipping(subtotal)


# Code Smell 10: Refused Bequest - Inheriting but not using parent functionality
class Animal:
    def __init__(self, name):
        self.name = name
        
    def move(self):
        return f"{self.name} is moving"
        
    def make_sound(self):
        return f"{self.name} makes a sound"
        
    def eat(self):
        return f"{self.name} is eating"


class Fish(Animal):
    """Fish class that refuses most Animal behaviors."""
    
    def __init__(self, name):
        super().__init__(name)
        
    def swim(self):
        return f"{self.name} is swimming"
        
    # Refuses the parent's move method by overriding with different semantics
    def move(self):
        return self.swim()  # Completely different behavior
        
    # Refuses make_sound - fish don't make sounds like other animals
    def make_sound(self):
        return ""  # Fish are silent
        
    # Fish eating is very different from general animals
    def eat(self):
        return f"{self.name} is filtering water for food"


if __name__ == "__main__":
    # Some basic usage to make the code "runnable"
    processor = DataProcessorManagerHandlerController()
    
    # Create sample data
    sample_data = [
        {"type": "A", "status": "active", "priority": 8, "category": "urgent"},
        {"type": "B", "status": "pending", "priority": 5, "category": "normal"},
        {"type": "C", "status": "inactive", "priority": 2, "category": "low"}
    ]
    
    processor.data = sample_data
    processor.process_data()
    
    print("Sample bad code executed successfully!")
    print(f"Processed {len(processor.processed_data)} items")