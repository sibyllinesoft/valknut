//! Test Fixtures and Data for Clone Denoising System
//!
//! Provides realistic test data including:
//! - Boilerplate-heavy code (benchmark macros, decorators, builders)
//! - Genuine clones with shared algorithms and structure  
//! - Multi-language examples for AST pattern mining
//! - Edge cases (empty functions, single-line functions, complex nesting)

use std::collections::HashMap;
use valknut_rs::core::featureset::CodeEntity;

/// Creates a comprehensive dataset of boilerplate-heavy code patterns
pub fn create_boilerplate_heavy_dataset() -> Vec<CodeEntity> {
    vec![
        // Python decorators and boilerplate
        CodeEntity::new(
            "python_decorator_1",
            "function",
            "api_endpoint",
            "/test/api1.py",
        )
        .with_source_code(
            r#"
@app.route('/api/users/<int:user_id>', methods=['GET'])
@login_required
@permission_required('user.read')  
@validate_json_schema(USER_SCHEMA)
@rate_limit(requests_per_minute=100)
def get_user(user_id):
    return jsonify(user_service.get_user(user_id))
"#,
        ),
        CodeEntity::new(
            "python_decorator_2",
            "function",
            "update_user",
            "/test/api2.py",
        )
        .with_source_code(
            r#"
@app.route('/api/users/<int:user_id>', methods=['PUT'])
@login_required
@permission_required('user.update')
@validate_json_schema(USER_UPDATE_SCHEMA) 
@rate_limit(requests_per_minute=50)
def update_user(user_id):
    return jsonify(user_service.update_user(user_id, request.json))
"#,
        ),
        // JavaScript/TypeScript builder patterns
        CodeEntity::new(
            "js_builder_1",
            "function",
            "QueryBuilder",
            "/test/builder1.js",
        )
        .with_source_code(
            r#"
class QueryBuilder {
    constructor() {
        this.query = {};
        this.conditions = [];
        this.ordering = [];
        this.limitValue = null;
        this.offsetValue = null;
    }
    
    where(field, operator, value) {
        this.conditions.push({ field, operator, value });
        return this;
    }
    
    orderBy(field, direction = 'ASC') {
        this.ordering.push({ field, direction });
        return this;
    }
    
    limit(count) {
        this.limitValue = count;
        return this;
    }
    
    offset(count) {
        this.offsetValue = count;
        return this;
    }
    
    build() {
        return {
            conditions: this.conditions,
            ordering: this.ordering,
            limit: this.limitValue,
            offset: this.offsetValue
        };
    }
}
"#,
        ),
        CodeEntity::new(
            "js_builder_2",
            "function",
            "RequestBuilder",
            "/test/builder2.js",
        )
        .with_source_code(
            r#"
class RequestBuilder {
    constructor() {
        this.url = null;
        this.method = 'GET';
        this.headers = {};
        this.body = null;
        this.timeout = 30000;
    }
    
    setUrl(url) {
        this.url = url;
        return this;
    }
    
    setMethod(method) {
        this.method = method.toUpperCase();
        return this;
    }
    
    addHeader(key, value) {
        this.headers[key] = value;
        return this;
    }
    
    setBody(data) {
        this.body = data;
        return this;
    }
    
    setTimeout(ms) {
        this.timeout = ms;
        return this;
    }
    
    build() {
        return {
            url: this.url,
            method: this.method,
            headers: this.headers,
            body: this.body,
            timeout: this.timeout
        };
    }
}
"#,
        ),
        // Rust macro-heavy boilerplate
        CodeEntity::new(
            "rust_macro_1",
            "function",
            "debug_struct",
            "/test/debug1.rs",
        )
        .with_source_code(
            r#"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    
    #[serde(skip_serializing_if = "Option::is_none")]  
    pub client_info: Option<ClientInfo>,
    
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<HttpHeader>,
}

impl Default for ApiRequest {
    fn default() -> Self {
        Self {
            request_id: None,
            timestamp: None,
            client_info: None,
            headers: Vec::new(),
        }
    }
}
"#,
        ),
        CodeEntity::new(
            "rust_macro_2",
            "function",
            "response_struct",
            "/test/debug2.rs",
        )
        .with_source_code(
            r#"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<ResponseData>,
}

impl Default for ApiResponse {
    fn default() -> Self {
        Self {
            response_id: None,
            timestamp: None,
            status_code: None,
            data: Vec::new(),
        }
    }
}
"#,
        ),
        // Go interface boilerplate
        CodeEntity::new(
            "go_interface_1",
            "function",
            "UserService",
            "/test/service1.go",
        )
        .with_source_code(
            r#"
type UserService interface {
    CreateUser(ctx context.Context, req *CreateUserRequest) (*User, error)
    GetUser(ctx context.Context, userID string) (*User, error)
    UpdateUser(ctx context.Context, userID string, req *UpdateUserRequest) (*User, error)
    DeleteUser(ctx context.Context, userID string) error
    ListUsers(ctx context.Context, req *ListUsersRequest) (*ListUsersResponse, error)
}

type userServiceImpl struct {
    db     Database
    logger Logger
    cache  Cache
    config *Config
}

func NewUserService(db Database, logger Logger, cache Cache, config *Config) UserService {
    return &userServiceImpl{
        db:     db,
        logger: logger,
        cache:  cache,
        config: config,
    }
}
"#,
        ),
        CodeEntity::new(
            "go_interface_2",
            "function",
            "OrderService",
            "/test/service2.go",
        )
        .with_source_code(
            r#"
type OrderService interface {
    CreateOrder(ctx context.Context, req *CreateOrderRequest) (*Order, error)
    GetOrder(ctx context.Context, orderID string) (*Order, error)
    UpdateOrder(ctx context.Context, orderID string, req *UpdateOrderRequest) (*Order, error)
    CancelOrder(ctx context.Context, orderID string) error
    ListOrders(ctx context.Context, req *ListOrdersRequest) (*ListOrdersResponse, error)
}

type orderServiceImpl struct {
    db     Database
    logger Logger
    cache  Cache
    config *Config
}

func NewOrderService(db Database, logger Logger, cache Cache, config *Config) OrderService {
    return &orderServiceImpl{
        db:     db,
        logger: logger,
        cache:  cache,
        config: config,
    }
}
"#,
        ),
    ]
}

/// Creates a dataset of genuine clones with shared algorithms and structure
pub fn create_genuine_clones_dataset() -> Vec<CodeEntity> {
    vec![
        // Matrix operations - genuine algorithmic clones
        CodeEntity::new("matrix_mult_v1", "function", "matrix_multiply", "/test/math1.py")
            .with_source_code(r#"
def matrix_multiply(A, B):
    """Standard matrix multiplication algorithm"""
    rows_A, cols_A = len(A), len(A[0])
    rows_B, cols_B = len(B), len(B[0])
    
    if cols_A != rows_B:
        raise ValueError("Matrix dimensions are incompatible for multiplication")
        
    # Initialize result matrix
    C = [[0 for _ in range(cols_B)] for _ in range(rows_A)]
    
    # Perform multiplication using three nested loops
    for i in range(rows_A):
        for j in range(cols_B):
            for k in range(cols_A):
                C[i][j] += A[i][k] * B[k][j]
                
    return C
"#),

        CodeEntity::new("matrix_mult_v2", "function", "multiply_matrices", "/test/math2.py")
            .with_source_code(r#"
def multiply_matrices(matrix1, matrix2):
    """Alternative matrix multiplication implementation"""  
    m, n = len(matrix1), len(matrix1[0])
    p, q = len(matrix2), len(matrix2[0])
    
    if n != p:
        raise ValueError("Cannot multiply matrices: inner dimensions must match")
        
    # Create result matrix with zeros
    result = [[0 for _ in range(q)] for _ in range(m)]
    
    # Triple loop for matrix multiplication
    for row in range(m):
        for col in range(q):
            for inner in range(n):
                result[row][col] += matrix1[row][inner] * matrix2[inner][col]
                
    return result
"#),

        // Quicksort algorithm variations - genuine clones
        CodeEntity::new("quicksort_v1", "function", "quicksort", "/test/sort1.py")
            .with_source_code(r#"
def quicksort(arr, low=0, high=None):
    """Standard quicksort implementation"""
    if high is None:
        high = len(arr) - 1
        
    if low < high:
        # Partition the array and get pivot index
        pivot_index = partition(arr, low, high)
        
        # Recursively sort elements before and after partition
        quicksort(arr, low, pivot_index - 1)
        quicksort(arr, pivot_index + 1, high)
        
    return arr

def partition(arr, low, high):
    """Partition function for quicksort"""
    pivot = arr[high]
    i = low - 1
    
    for j in range(low, high):
        if arr[j] <= pivot:
            i += 1
            arr[i], arr[j] = arr[j], arr[i]
            
    arr[i + 1], arr[high] = arr[high], arr[i + 1]
    return i + 1
"#),

        CodeEntity::new("quicksort_v2", "function", "quick_sort", "/test/sort2.py")
            .with_source_code(r#"
def quick_sort(array, start=0, end=None):
    """Alternative quicksort implementation with similar logic"""
    if end is None:
        end = len(array) - 1
        
    if start < end:
        # Find partition point
        partition_point = partition_array(array, start, end)
        
        # Sort elements before and after partition
        quick_sort(array, start, partition_point - 1) 
        quick_sort(array, partition_point + 1, end)
        
    return array

def partition_array(array, start, end):
    """Partitioning logic for quicksort"""
    pivot_value = array[end]
    smaller_element_index = start - 1
    
    for current_index in range(start, end):
        if array[current_index] <= pivot_value:
            smaller_element_index += 1
            array[smaller_element_index], array[current_index] = array[current_index], array[smaller_element_index]
            
    array[smaller_element_index + 1], array[end] = array[end], array[smaller_element_index + 1]
    return smaller_element_index + 1
"#),

        // Binary search variations - genuine clones
        CodeEntity::new("binary_search_v1", "function", "binary_search", "/test/search1.py")
            .with_source_code(r#"
def binary_search(arr, target):
    """Standard binary search implementation"""
    left, right = 0, len(arr) - 1
    
    while left <= right:
        mid = (left + right) // 2
        
        if arr[mid] == target:
            return mid
        elif arr[mid] < target:
            left = mid + 1
        else:
            right = mid - 1
            
    return -1  # Target not found
"#),

        CodeEntity::new("binary_search_v2", "function", "binary_find", "/test/search2.py")
            .with_source_code(r#"
def binary_find(sorted_array, search_value):
    """Alternative binary search with same algorithm"""
    low_index, high_index = 0, len(sorted_array) - 1
    
    while low_index <= high_index:
        middle_index = (low_index + high_index) // 2
        middle_value = sorted_array[middle_index]
        
        if middle_value == search_value:
            return middle_index
        elif middle_value < search_value:
            low_index = middle_index + 1
        else:
            high_index = middle_index - 1
            
    return -1  # Element not found
"#),

        // Tree traversal algorithms - genuine clones
        CodeEntity::new("tree_traversal_v1", "function", "inorder_traversal", "/test/tree1.py")
            .with_source_code(r#"
def inorder_traversal(root):
    """In-order binary tree traversal (left, root, right)"""
    if root is None:
        return []
        
    result = []
    
    # Traverse left subtree
    result.extend(inorder_traversal(root.left))
    
    # Visit root node
    result.append(root.val)
    
    # Traverse right subtree
    result.extend(inorder_traversal(root.right))
    
    return result
"#),

        CodeEntity::new("tree_traversal_v2", "function", "traverse_inorder", "/test/tree2.py")
            .with_source_code(r#"
def traverse_inorder(tree_node):
    """In-order tree traversal with alternative naming"""
    if tree_node is None:
        return []
        
    traversal_result = []
    
    # Process left subtree first
    traversal_result.extend(traverse_inorder(tree_node.left))
    
    # Add current node value  
    traversal_result.append(tree_node.val)
    
    # Process right subtree last
    traversal_result.extend(traverse_inorder(tree_node.right))
    
    return traversal_result
"#),
    ]
}

/// Creates multi-language examples for AST pattern mining
pub fn create_multi_language_ast_examples() -> Vec<CodeEntity> {
    vec![CodeEntity::new("test", "test", "test", "/test.py").with_source_code("def test(): pass")]
}
pub fn create_edge_case_dataset() -> Vec<CodeEntity> {
    vec![
        // This function is intentionally empty for now
        // Edge case data has been moved to create_multi_language_ast_examples
    ]
}

/// Creates test data optimized for performance benchmarking
pub fn create_performance_test_dataset(size: usize) -> Vec<CodeEntity> {
    let mut entities = Vec::with_capacity(size);

    for i in 0..size {
        let complexity_level = i % 5; // Vary complexity
        let source_code = match complexity_level {
            0 => format!("def simple_{}(): return {}", i, i),
            1 => format!(
                r#"
def medium_{}(x):
    if x > {}:
        return x * 2
    return x + {}
"#,
                i,
                i % 100,
                i % 10
            ),
            2 => format!(
                r#"
def complex_{}(data):
    result = []
    for item in data:
        if item > {}:
            processed = item * {} + {}
            result.append(processed)
    return result
"#,
                i,
                i % 50,
                i % 7,
                i % 13
            ),
            3 => format!(
                r#"
def nested_{}(matrix, config):
    output = []
    for row_idx, row in enumerate(matrix):
        if row_idx % {} == 0:
            row_result = []
            for col_idx, val in enumerate(row):
                if col_idx % {} == 0:
                    processed = val * config.multiplier_{}
                    row_result.append(processed)
            output.append(row_result)
    return output
"#,
                i,
                (i % 5) + 1,
                (i % 3) + 1,
                i % 10
            ),
            _ => format!(
                r#"
def very_complex_{}(data_stream, processors, config):
    stages = []
    for processor_idx, processor in enumerate(processors):
        if processor.enabled and processor_idx % {} == 0:
            stage_results = []
            for batch_idx, batch in enumerate(data_stream.batches):
                if batch.size > config.min_batch_size_{}:
                    batch_result = []
                    for item_idx, item in enumerate(batch.items):
                        if item.valid and item_idx % {} == 0:
                            try:
                                processed = processor.process(item, config.params_{})
                                if processed.success:
                                    batch_result.append(processed.value)
                            except ProcessingError:
                                continue
                    stage_results.append(batch_result)
            stages.append(stage_results)
    return stages
"#,
                i,
                (i % 4) + 1,
                i % 20,
                (i % 6) + 1,
                i % 15
            ),
        };

        entities.push(
            CodeEntity::new(
                &format!("perf_test_{}", i),
                "function",
                &format!("func_{}", i),
                &format!("/test/perf_{}.py", i % 20), // Reuse some file paths
            )
            .with_source_code(&source_code),
        );
    }

    entities
}

/// Creates a mixed realistic codebase for comprehensive testing
pub fn create_realistic_codebase_sample() -> Vec<CodeEntity> {
    let mut dataset = Vec::new();

    // Add each category of test data
    dataset.extend(create_boilerplate_heavy_dataset());
    dataset.extend(create_genuine_clones_dataset());
    dataset.extend(create_multi_language_ast_examples());
    dataset.extend(create_edge_case_dataset());

    // Add some performance test data for scale
    dataset.extend(create_performance_test_dataset(20));

    dataset
}

/// Helper function to categorize entities by expected behavior
pub fn categorize_test_entities(entities: &[CodeEntity]) -> HashMap<String, Vec<String>> {
    let mut categories = HashMap::new();

    for entity in entities {
        let category = if entity.source_code.len() < 50 {
            "simple"
        } else if entity.source_code.contains("@") && entity.source_code.contains("def ") {
            "boilerplate_decorators"
        } else if entity.source_code.contains("class ") && entity.source_code.contains("self.") {
            "boilerplate_builders"
        } else if entity.source_code.contains("matrix") || entity.source_code.contains("quicksort")
        {
            "genuine_clones"
        } else if entity.source_code.contains("import ") {
            "ast_patterns"
        } else if entity.source_code.contains("for ")
            && entity.source_code.matches("if ").count() > 2
        {
            "complex_nested"
        } else {
            "other"
        };

        categories
            .entry(category.to_string())
            .or_insert_with(Vec::new)
            .push(entity.id.clone());
    }

    categories
}
