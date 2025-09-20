// Test file with intentionally complex code to trigger refactoring candidates

pub fn overly_complex_function(x: i32, y: i32, z: i32, a: i32, b: i32) -> i32 {
    let mut result = 0;
    
    // Deeply nested conditionals
    if x > 0 {
        if y > 0 {
            if z > 0 {
                if a > 0 {
                    if b > 0 {
                        result = x + y + z + a + b;
                        if result > 100 {
                            result = result * 2;
                            if result > 200 {
                                result = result - 50;
                                if result > 150 {
                                    result = result / 2;
                                    if result > 75 {
                                        result = result + 25;
                                    }
                                }
                            }
                        }
                    } else {
                        result = x + y + z + a;
                        if result > 50 {
                            result = result * 3;
                        }
                    }
                } else {
                    result = x + y + z;
                    if result > 25 {
                        result = result + 10;
                    }
                }
            } else {
                result = x + y;
                if result > 10 {
                    result = result * 2;
                }
            }
        } else {
            result = x;
            if result > 5 {
                result = result + 1;
            }
        }
    } else {
        result = 0;
    }
    
    // More complexity
    for i in 0..10 {
        if i % 2 == 0 {
            if i % 4 == 0 {
                result += i;
            } else {
                result -= i;
            }
        } else {
            if i % 3 == 0 {
                result *= 2;
            } else {
                result /= 2;
            }
        }
    }
    
    // Duplicate code patterns
    let temp1 = result + 10;
    let final1 = temp1 * 2;
    
    let temp2 = result + 10;
    let final2 = temp2 * 2;
    
    let temp3 = result + 10; 
    let final3 = temp3 * 2;
    
    result + final1 + final2 + final3
}

pub fn another_complex_function(data: Vec<i32>) -> Vec<i32> {
    let mut output = Vec::new();
    
    for item in data {
        if item > 0 {
            if item % 2 == 0 {
                if item % 4 == 0 {
                    if item % 8 == 0 {
                        output.push(item * 8);
                    } else {
                        output.push(item * 4);
                    }
                } else {
                    output.push(item * 2);
                }
            } else {
                if item % 3 == 0 {
                    if item % 9 == 0 {
                        output.push(item * 9);
                    } else {
                        output.push(item * 3);
                    }
                } else {
                    output.push(item);
                }
            }
        } else if item < 0 {
            if item % 2 == 0 {
                output.push(item.abs());
            } else {
                output.push(item.abs() * 2);
            }
        } else {
            output.push(0);
        }
    }
    
    output
}