#!/usr/bin/env python3
"""
Extreme Complexity Benchmark Dataset for Valknut Testing

This file contains functions with dramatically different complexity characteristics
to test whether the Bayesian normalization system can handle real variance properly.

Includes:
- Simple functions (cyclomatic=1, params=0-1)  
- Complex functions (cyclomatic=15+, params=8+)
- God class with high centrality
- Deeply nested functions (nesting=6+)
- Functions with extreme parameter counts
"""

import random
import time
import json
from typing import Dict, List, Optional, Union, Tuple, Any
from dataclasses import dataclass
from enum import Enum


# === SIMPLE FUNCTIONS (Low Complexity) ===

def simple_getter():
    """Trivial getter - cyclomatic=1, params=0"""
    return 42


def simple_setter(value):
    """Trivial setter - cyclomatic=1, params=1"""
    global global_value
    global_value = value


def basic_add(a, b):
    """Basic arithmetic - cyclomatic=1, params=2"""
    return a + b


# === MODERATELY COMPLEX FUNCTIONS ===

def moderate_logic(x, y, z):
    """Moderate branching - cyclomatic=4, params=3"""
    if x > 0:
        if y > 0:
            return x + y + z
        else:
            return x - y + z
    else:
        if z > 0:
            return y + z
        else:
            return 0


def parameter_heavy_function(a, b, c, d, e, f):
    """Many parameters but simple logic - cyclomatic=2, params=6"""
    if a > b:
        return a + b + c + d + e + f
    else:
        return a * b * c * d * e * f


# === EXTREMELY COMPLEX FUNCTIONS ===

def extremely_complex_business_logic(
    user_id: int,
    account_type: str, 
    transaction_amount: float,
    currency: str,
    risk_level: int,
    compliance_flags: Dict[str, bool],
    region_code: str,
    payment_method: str
):
    """
    Extremely complex business logic with high cyclomatic complexity
    cyclomatic complexity = ~20, params=8
    """
    result = {}
    
    # Primary account validation (3 branches)
    if account_type == "premium":
        base_limit = 100000.0
        fee_rate = 0.01
    elif account_type == "business":
        base_limit = 500000.0
        fee_rate = 0.005
    else:
        base_limit = 10000.0
        fee_rate = 0.02
    
    # Transaction amount validation (4 branches)
    if transaction_amount < 0:
        return {"error": "negative_amount", "code": 400}
    elif transaction_amount == 0:
        return {"error": "zero_amount", "code": 400}
    elif transaction_amount > base_limit:
        if account_type == "premium" and transaction_amount < base_limit * 2:
            # Allow premium accounts 2x limit
            pass
        else:
            return {"error": "limit_exceeded", "code": 403}
    
    # Currency handling (5 branches)
    if currency == "USD":
        exchange_rate = 1.0
    elif currency == "EUR":
        exchange_rate = 1.1
    elif currency == "GBP":
        exchange_rate = 1.25
    elif currency == "JPY":
        exchange_rate = 0.0067
    else:
        return {"error": "unsupported_currency", "code": 400}
    
    # Risk assessment (6 branches)  
    if risk_level >= 9:
        return {"error": "high_risk_blocked", "code": 403}
    elif risk_level >= 7:
        if not compliance_flags.get("manual_review", False):
            return {"error": "manual_review_required", "code": 202}
    elif risk_level >= 5:
        if not compliance_flags.get("identity_verified", False):
            return {"error": "identity_verification_required", "code": 202}
    elif risk_level >= 3:
        if region_code in ["XX", "YY", "ZZ"]:  # High risk regions
            return {"error": "region_restricted", "code": 403}
    
    # Payment method validation (4 branches)
    if payment_method == "credit_card":
        processing_fee = transaction_amount * 0.029
    elif payment_method == "bank_transfer":
        processing_fee = min(transaction_amount * 0.001, 25.0)
    elif payment_method == "crypto":
        if region_code in ["US", "UK"]:
            processing_fee = transaction_amount * 0.015
        else:
            return {"error": "crypto_not_supported", "code": 400}
    else:
        return {"error": "invalid_payment_method", "code": 400}
    
    # Final calculations
    usd_amount = transaction_amount * exchange_rate
    total_fee = (usd_amount * fee_rate) + processing_fee
    net_amount = usd_amount - total_fee
    
    return {
        "success": True,
        "transaction_id": f"TXN_{user_id}_{int(time.time())}",
        "usd_amount": usd_amount,
        "total_fee": total_fee,
        "net_amount": net_amount,
        "risk_level": risk_level,
        "processed_at": time.time()
    }


def deeply_nested_algorithm(data: List[Dict[str, Any]], threshold: float):
    """
    Deeply nested processing algorithm - high nesting depth (7+ levels)
    cyclomatic complexity = ~12, high nesting
    """
    results = []
    
    for item in data:  # Level 1
        if "values" in item:  # Level 2
            for category, values in item["values"].items():  # Level 3
                if isinstance(values, list):  # Level 4
                    for i, value in enumerate(values):  # Level 5
                        if isinstance(value, dict):  # Level 6
                            for key, metric in value.items():  # Level 7
                                if isinstance(metric, (int, float)):  # Level 8
                                    if metric > threshold:  # Level 9
                                        results.append({
                                            "item_id": item.get("id", i),
                                            "category": category,
                                            "index": i,
                                            "key": key,
                                            "metric": metric,
                                            "threshold_ratio": metric / threshold
                                        })
    
    return results


def massive_parameter_function(
    p1, p2, p3, p4, p5, p6, p7, p8, p9, p10,
    p11, p12, p13, p14, p15, p16, p17, p18, p19, p20,
    *args, **kwargs
):
    """
    Function with massive parameter count to test param_count feature
    20+ explicit parameters plus varargs
    """
    params = [p1, p2, p3, p4, p5, p6, p7, p8, p9, p10,
              p11, p12, p13, p14, p15, p16, p17, p18, p19, p20]
    
    total = sum(p for p in params if isinstance(p, (int, float)))
    total += sum(a for a in args if isinstance(a, (int, float)))
    
    for key, value in kwargs.items():
        if isinstance(value, (int, float)):
            total += value
    
    return {
        "total": total,
        "param_count": len(params),
        "args_count": len(args),
        "kwargs_count": len(kwargs)
    }


# === GOD CLASS (High Centrality) ===

class GodClass:
    """
    God class that does everything - high centrality, fan-in/fan-out
    This should have high betweenness, fan_in, fan_out metrics
    """
    
    def __init__(self):
        self.data = {}
        self.cache = {}
        self.connections = {}
        self.state = "initialized"
    
    def process_data(self, data):
        """Called by many other functions"""
        self.data.update(data)
        return self._internal_process()
    
    def _internal_process(self):
        """Internal processing that calls many other methods"""
        self._validate_data()
        self._transform_data()
        self._cache_results()
        self._update_connections()
        self._notify_observers()
        return self._generate_output()
    
    def _validate_data(self):
        """Data validation"""
        for key, value in self.data.items():
            if not self._is_valid(key, value):
                raise ValueError(f"Invalid data: {key}={value}")
    
    def _is_valid(self, key, value):
        """Validation logic"""
        return value is not None and str(key).strip() != ""
    
    def _transform_data(self):
        """Data transformation"""  
        for key in list(self.data.keys()):
            self.data[key] = self._transform_value(self.data[key])
    
    def _transform_value(self, value):
        """Value transformation"""
        if isinstance(value, str):
            return value.upper()
        elif isinstance(value, (int, float)):
            return value * 1.1
        else:
            return str(value)
    
    def _cache_results(self):
        """Caching logic"""
        cache_key = hash(str(sorted(self.data.items())))
        self.cache[cache_key] = dict(self.data)
    
    def _update_connections(self):
        """Update connection graph"""
        for key in self.data.keys():
            if key not in self.connections:
                self.connections[key] = set()
            # Create connections between all keys
            for other_key in self.data.keys():
                if other_key != key:
                    self.connections[key].add(other_key)
    
    def _notify_observers(self):
        """Notify all observers"""
        # This would call external observers in real code
        self.state = "processed"
    
    def _generate_output(self):
        """Generate final output"""
        return {
            "processed_data": dict(self.data),
            "connection_count": sum(len(conns) for conns in self.connections.values()),
            "cache_size": len(self.cache),
            "state": self.state
        }
    
    def get_stats(self):
        """Get statistics - called by external code"""
        return self._generate_output()
    
    def clear_cache(self):
        """Clear cache - called by external code"""
        self.cache.clear()
    
    def reset(self):
        """Reset everything - called by external code"""
        self.data.clear()
        self.cache.clear()
        self.connections.clear()
        self.state = "reset"


# === FUNCTIONS THAT CALL THE GOD CLASS (Creates fan-in) ===

god_instance = GodClass()

def user_service_process(user_data):
    """User service that uses god class"""
    return god_instance.process_data({"user": user_data})

def order_service_process(order_data):  
    """Order service that uses god class"""
    return god_instance.process_data({"order": order_data})

def payment_service_process(payment_data):
    """Payment service that uses god class"""  
    return god_instance.process_data({"payment": payment_data})

def notification_service_process(notification_data):
    """Notification service that uses god class"""
    return god_instance.process_data({"notification": notification_data})

def analytics_service_process(analytics_data):
    """Analytics service that uses god class"""
    return god_instance.process_data({"analytics": analytics_data})


# === UTILITY FUNCTIONS (Create more variance) ===

def quick_sort(arr):
    """Recursive function with moderate complexity"""
    if len(arr) <= 1:
        return arr
    
    pivot = arr[len(arr) // 2]
    left = [x for x in arr if x < pivot]
    middle = [x for x in arr if x == pivot]
    right = [x for x in arr if x > pivot]
    
    return quick_sort(left) + middle + quick_sort(right)


def fibonacci_recursive(n):
    """Classic recursive function"""
    if n <= 1:
        return n
    else:
        return fibonacci_recursive(n-1) + fibonacci_recursive(n-2)


def main():
    """Main function that orchestrates everything"""
    # Test simple functions
    simple_getter()
    simple_setter(100)
    basic_add(1, 2)
    
    # Test moderate complexity
    moderate_logic(1, 2, 3)
    parameter_heavy_function(1, 2, 3, 4, 5, 6)
    
    # Test extreme complexity
    extremely_complex_business_logic(
        user_id=123,
        account_type="premium", 
        transaction_amount=50000.0,
        currency="USD",
        risk_level=3,
        compliance_flags={"identity_verified": True, "manual_review": False},
        region_code="US",
        payment_method="credit_card"
    )
    
    # Test deep nesting
    test_data = [
        {"id": 1, "values": {"metrics": [{"score": 85.5}, {"rating": 92.1}]}},
        {"id": 2, "values": {"performance": [{"latency": 120.0}, {"throughput": 1500.0}]}}
    ]
    deeply_nested_algorithm(test_data, 100.0)
    
    # Test massive parameters
    massive_parameter_function(1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
                              11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                              21, 22, 23, extra=24, bonus=25)
    
    # Test god class usage
    user_service_process({"name": "John", "age": 30})
    order_service_process({"id": "ORD123", "amount": 99.99})
    payment_service_process({"method": "card", "amount": 99.99})
    
    # Test utilities
    quick_sort([64, 34, 25, 12, 22, 11, 90])
    fibonacci_recursive(10)
    
    print("Complexity benchmark completed - should show varied cyclomatic complexity!")


if __name__ == "__main__":
    main()