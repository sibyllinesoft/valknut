#!/usr/bin/env python3
"""
IRREFUTABLE complexity test - if valknut can't differentiate these functions,
something is fundamentally broken.
"""

def trivial():
    """Absolutely trivial function - should be lowest complexity"""
    return 1

def simple_conditional(x):
    """Simple if statement - should be low complexity"""
    if x > 0:
        return x
    return 0

def nested_loops_and_conditions(data):
    """Complex function with nested loops and conditions - should be HIGH complexity"""
    result = []
    count = 0
    
    for i in range(len(data)):
        if data[i] is not None:
            for j in range(i):
                if data[j] > data[i]:
                    if j % 2 == 0:
                        if data[j] > 10:
                            result.append(data[j] * 2)
                            count += 1
                        else:
                            result.append(data[j])
                    else:
                        if data[i] < 5:
                            result.append(data[i] + data[j])
                        else:
                            for k in range(data[i]):
                                if k % 3 == 0:
                                    result.append(k)
                                elif k % 3 == 1:
                                    result.append(k * 2)
                                else:
                                    result.append(k * 3)
                                    
    if count > 5:
        result.sort()
    elif count > 2:
        result.reverse()
    else:
        result = [x * 2 for x in result if x > 0]
        
    return result

def massive_branching_function(a, b, c, d, e, f, g, h, i, j):
    """Function with massive branching - should be HIGHEST complexity"""
    result = 0
    
    if a > 0:
        if b > 0:
            if c > 0:
                if d > 0:
                    if e > 0:
                        if f > 0:
                            if g > 0:
                                if h > 0:
                                    if i > 0:
                                        if j > 0:
                                            result = a + b + c + d + e + f + g + h + i + j
                                        else:
                                            result = a * b * c * d * e * f * g * h * i
                                    else:
                                        result = a + b + c + d + e + f + g + h
                                else:
                                    result = a * b * c * d * e * f * g
                            else:
                                result = a + b + c + d + e + f
                        else:
                            result = a * b * c * d * e
                    else:
                        result = a + b + c + d
                else:
                    result = a * b * c
            else:
                result = a + b
        else:
            result = a
    else:
        if b < 0:
            if c < 0:
                if d < 0:
                    if e < 0:
                        result = -1000
                    else:
                        result = -100
                else:
                    result = -10
            else:
                result = -1
        else:
            result = 0
    
    # More branching
    if result > 100:
        if result > 1000:
            if result > 10000:
                return result * 10
            else:
                return result * 5
        else:
            return result * 2
    elif result > 10:
        return result + 100
    elif result > 0:
        return result + 10
    elif result == 0:
        return 1
    else:
        if result < -100:
            return result / 10
        else:
            return result / 2

# If these four functions get the same cyclomatic complexity score,
# valknut's complexity analysis is completely broken