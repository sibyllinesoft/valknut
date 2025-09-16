"""Complex algorithms for performance testing."""
import time
import random
from typing import List, Dict, Any
from collections import defaultdict

class PerformanceTestSuite:
    """Collection of performance-intensive algorithms."""
    
    def __init__(self):
        self.cache = {}
        self.stats = defaultdict(int)
    
    def fibonacci_recursive(self, n: int) -> int:
        """Inefficient recursive Fibonacci for complexity testing."""
        if n in self.cache:
            return self.cache[n]
        
        if n <= 1:
            result = n
        else:
            result = self.fibonacci_recursive(n-1) + self.fibonacci_recursive(n-2)
        
        self.cache[n] = result
        return result
    
    def bubble_sort(self, arr: List[int]) -> List[int]:
        """Bubble sort implementation for complexity analysis."""
        arr = arr.copy()
        n = len(arr)
        
        for i in range(n):
            swapped = False
            for j in range(0, n - i - 1):
                if arr[j] > arr[j + 1]:
                    arr[j], arr[j + 1] = arr[j + 1], arr[j]
                    swapped = True
                    self.stats['swaps'] += 1
            
            if not swapped:
                break
            
            self.stats['iterations'] += 1
        
        return arr
    
    def nested_loops_complexity(self, n: int) -> Dict[str, Any]:
        """Multiple nested loops for complexity testing."""
        results = {
            'single_loop': [],
            'double_loop': [],
            'triple_loop': []
        }
        
        # O(n) complexity
        for i in range(n):
            results['single_loop'].append(i * 2)
        
        # O(n²) complexity
        for i in range(n):
            for j in range(n):
                if i != j:
                    results['double_loop'].append((i, j))
        
        # O(n³) complexity  
        for i in range(min(n, 10)):  # Limit to prevent timeout
            for j in range(min(n, 10)):
                for k in range(min(n, 10)):
                    if i + j + k == n:
                        results['triple_loop'].append((i, j, k))
        
        return results
    
    def recursive_tree_traversal(self, depth: int, branching_factor: int = 3) -> int:
        """Recursive tree traversal for complexity analysis."""
        if depth <= 0:
            return 1
        
        total = 0
        for _ in range(branching_factor):
            total += self.recursive_tree_traversal(depth - 1, branching_factor)
        
        return total
    
    def memory_intensive_operation(self, size: int) -> List[List[int]]:
        """Memory-intensive operation for resource testing."""
        matrix = []
        for i in range(size):
            row = []
            for j in range(size):
                # Complex calculation to stress both CPU and memory
                value = (i * j) % 1000
                if value % 2 == 0:
                    value = value ** 2
                else:
                    value = value * 3 + 1
                row.append(value)
            matrix.append(row)
        
        return matrix
    
    def string_operations_complexity(self, text: str, operations: int) -> Dict[str, Any]:
        """String operation complexity testing."""
        results = {
            'concatenations': '',
            'replacements': text,
            'searches': []
        }
        
        # String concatenation in loop (inefficient)
        for i in range(operations):
            results['concatenations'] += f"operation_{i}_"
        
        # Multiple string replacements
        for i in range(operations):
            old_char = chr(ord('a') + (i % 26))
            new_char = chr(ord('A') + (i % 26))
            results['replacements'] = results['replacements'].replace(old_char, new_char)
        
        # String searching
        for i in range(operations):
            search_term = f"search_{i}"
            if search_term in text:
                results['searches'].append(i)
        
        return results
    
    def graph_traversal_complexity(self, nodes: int) -> Dict[str, Any]:
        """Graph traversal algorithms for complexity testing."""
        # Create a random graph
        graph = defaultdict(list)
        for i in range(nodes):
            # Each node connects to random other nodes
            connections = random.randint(1, min(nodes - 1, 5))
            for _ in range(connections):
                target = random.randint(0, nodes - 1)
                if target != i:
                    graph[i].append(target)
        
        # Depth-First Search
        visited_dfs = set()
        def dfs(node):
            if node in visited_dfs:
                return 0
            visited_dfs.add(node)
            count = 1
            for neighbor in graph[node]:
                count += dfs(neighbor)
            return count
        
        dfs_nodes = dfs(0) if nodes > 0 else 0
        
        # Breadth-First Search
        visited_bfs = set()
        queue = [0] if nodes > 0 else []
        bfs_nodes = 0
        
        while queue:
            node = queue.pop(0)
            if node not in visited_bfs:
                visited_bfs.add(node)
                bfs_nodes += 1
                for neighbor in graph[node]:
                    if neighbor not in visited_bfs:
                        queue.append(neighbor)
        
        return {
            'graph_size': nodes,
            'dfs_visited': dfs_nodes,
            'bfs_visited': bfs_nodes,
            'graph_density': sum(len(neighbors) for neighbors in graph.values()) / nodes if nodes > 0 else 0
        }

def run_performance_benchmark():
    """Run a comprehensive performance benchmark."""
    suite = PerformanceTestSuite()
    
    print("Running performance benchmark...")
    start_time = time.time()
    
    # Test different complexity scenarios
    results = {}
    
    # Fibonacci test
    results['fibonacci'] = suite.fibonacci_recursive(25)
    
    # Sorting test
    test_array = [random.randint(1, 1000) for _ in range(100)]
    results['sorted_array'] = suite.bubble_sort(test_array)
    
    # Nested loops test
    results['nested_loops'] = suite.nested_loops_complexity(20)
    
    # Recursive tree test
    results['tree_traversal'] = suite.recursive_tree_traversal(8)
    
    # Memory test
    results['memory_matrix'] = len(suite.memory_intensive_operation(50))
    
    # String operations test
    test_text = "The quick brown fox jumps over the lazy dog. " * 100
    results['string_ops'] = suite.string_operations_complexity(test_text, 50)
    
    # Graph traversal test
    results['graph_traversal'] = suite.graph_traversal_complexity(100)
    
    end_time = time.time()
    
    print(f"Benchmark completed in {end_time - start_time:.2f} seconds")
    print(f"Cache hits: {len(suite.cache)}")
    print(f"Operations stats: {dict(suite.stats)}")
    
    return results

if __name__ == "__main__":
    run_performance_benchmark()
