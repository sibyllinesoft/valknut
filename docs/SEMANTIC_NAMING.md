# Semantic Naming Analysis

## Overview

The Valknut semantic naming analyzer uses **Qwen3-Embedding-0.6B** model to detect mismatches between function names and their actual behavior. This system provides deterministic, actionable recommendations for improving code readability through better naming practices.

## Key Features

### 🧠 **Behavior Signature Analysis**
- **Side Effects Detection**: Identifies I/O operations, database access, file system calls, network requests
- **Mutation Analysis**: Detects parameter modifications, global state changes
- **Execution Patterns**: Distinguishes sync vs async operations
- **Return Type Analysis**: Analyzes optionality, cardinality, and type categories

### 🎯 **Semantic Mismatch Detection**
- **Cosine Similarity**: Uses Qwen embeddings to measure semantic distance between name and behavior
- **Rule-Based Analysis**: Applies deterministic rules for common mismatch patterns
- **Confidence Scoring**: Provides confidence levels for all recommendations
- **Threshold Gating**: Configurable thresholds prevent noise

### 📋 **Deterministic Name Generation**
- **Verb Selection**: Maps observed effects to appropriate verbs (get/create/update/delete)
- **Noun Extraction**: Derives nouns from return types, parameters, and domain context
- **Abbreviation Handling**: Expands abbreviations and maintains allowed abbreviations list
- **Convention Compliance**: Follows language-specific naming conventions

## Analysis Types

### Rename Packs
Generated when function names don't match their behavior:

```rust
get_user() // but actually creates/updates user
→ Suggests: create_user(), update_user(), upsert_user()
```

### Contract Mismatch Packs  
Generated when names imply different contracts than implementation:

```rust
find_user() // but returns User (not Optional<User>)
→ Suggests: get_user() or make return type optional
```

### Consistency Issues
Project-wide naming inconsistencies:
- Synonym detection (`user`/`member`/`account`)
- Abbreviation inconsistencies
- Convention violations

## Configuration

### Basic Settings
```yaml
names:
  enabled: true
  embedding_model: "Qwen/Qwen3-Embedding-0.6B-GGUF"
  min_mismatch: 0.65      # Mismatch threshold (0.0-1.0)
  min_impact: 3           # Minimum external references
  protect_public_api: true # Protect public functions
```

### Abbreviation Handling
```yaml
names:
  abbrev_map:
    usr: user
    cfg: config
    mgr: manager
  allowed_abbrevs:
    - id
    - url
    - api
    - db
```

### I/O Library Detection
```yaml
names:
  io_libs:
    python:
      - requests
      - sqlalchemy
      - boto3
    rust:
      - reqwest
      - sqlx
      - tokio
```

## Command Line Usage

### Basic Analysis
```bash
# Analyze current directory
valknut names .

# Analyze specific languages  
valknut names . -e rs,py,ts

# Focus on specific issue types
valknut names . --renames-only
valknut names . --contracts-only
```

### Advanced Options
```bash
# Lower mismatch threshold for more sensitive analysis
valknut names . --min-mismatch 0.5

# Include public API functions
valknut names . --include-public-api

# Limit results
valknut names . -n 10
```

### Output Formats
```bash
# Human-readable format
valknut names . -f pretty

# JSON for tooling integration
valknut names . -f json

# YAML format
valknut names . -f yaml
```

## Mismatch Scoring Formula

The system uses a weighted formula from the TODO.md specification:

```
mismatch_score = 0.5*(1 - cosine_similarity) + 
                 0.2*effect_mismatch + 
                 0.1*cardinality_mismatch + 
                 0.1*optionality_mismatch + 
                 0.1*async_mismatch
```

### Confidence Dampers
- **-0.15** for weak behavior inference (dynamic calls, complex control flow)
- **-0.1** for short names (<2 tokens)

## Golden Test Cases

### Effect Mismatch
```python
def get_user(id):  # Name implies read-only
    user = db.find(id)
    user.last_seen = now()  # Actually mutates!
    db.save(user)
    return user

→ EffectMismatch: Expected read-only, got mutating
→ Suggestion: update_user_last_seen()
```

### Cardinality Mismatch  
```python
def user():  # Singular name
    return User.objects.all()  # Returns collection

→ CardinalityMismatch: Expected single item, got collection  
→ Suggestion: users(), list_users(), iter_users()
```

### Optionality Mismatch
```python
def find_user(id) -> User:  # find_ implies optional
    return User.objects.get(id)  # Throws if not found

→ OptionalityMismatch: Expected optional, got guaranteed
→ Solutions: 
  • Rename to get_user()
  • Change return type to Optional[User]
```

## Integration with Qwen3-Embedding-0.6B

### Model Requirements
- **Model**: Qwen3-Embedding-0.6B-GGUF Q4_K_M quantization (~395 MB)
- **CPU-only**: No GPU required
- **Offline**: Model cached locally at `~/.refactor_rank/cache/`

### Performance Profile
- **Embedding Generation**: ~5-20k texts/minute (CPU-dependent)
- **Memory Usage**: ≤1 GB typical (model + batches)
- **Cache Hit Rate**: ~80-90% on repeated analysis

### Model Download
The first run will require manual model download:
```bash
# Download instructions will be provided automatically
# Place qwen3-embedding-0.6b-q4_k_m.gguf in:
# ~/.refactor_rank/cache/qwen3-embedding-0.6b-q4_k_m.gguf
```

## Advanced Features

### Project Lexicon Building
The analyzer builds a project-specific lexicon:
- **Domain Nouns**: Extracted from types, directories, schemas
- **Verb Patterns**: Common operation patterns in the codebase  
- **Synonym Detection**: Related terms that should be consistent
- **Convention Analysis**: Project-specific naming patterns

### Priority Ranking
Recommendations are ranked by value/effort ratio:
```
rename_priority = mismatch_score * log(1 + external_refs) / effort
contract_priority = (mismatch + penalties) / (api_multiplier * effort)
```

### Batch Processing
Optimized for large codebases:
- **Parallel Analysis**: Multi-threaded function processing
- **Embedding Batching**: Efficient batch embedding generation
- **Incremental Caching**: Only re-analyze changed functions

## Output Examples

### Pretty Format
```
🏷️  Semantic Naming Analysis Results
=====================================

📝 Rename Recommendations (3 found):
─────────────────────────────────────

1. get_user → update_user_last_seen
   File: src/user.rs:42
   Mismatch Score: 0.78
   Priority: 2.31
   Impact: 12 external references, 5 files affected
   Rationale: Based on update operation returning User
   Issues: Effect mismatch (expected read-only, got mutating)

📊 Summary:
   • 3 rename recommendations
   • 1 contract mismatches
   • 0 consistency issues
   • 4 total naming issues found
```

### JSON Format
```json
{
  "rename_packs": [
    {
      "function_id": "user.rs::get_user",
      "current_name": "get_user", 
      "file_path": "src/user.rs",
      "line_number": 42,
      "proposals": [
        {
          "name": "update_user_last_seen",
          "rationale": "Based on update operation returning User",
          "confidence": 0.85,
          "components": {
            "verb": "update",
            "noun": "user", 
            "qualifiers": ["last_seen"]
          }
        }
      ],
      "mismatch": {
        "cosine_similarity": 0.32,
        "mismatch_score": 0.78,
        "mismatch_types": [
          {
            "EffectMismatch": {
              "expected": "read-only operation",
              "actual": "modifies state"
            }
          }
        ]
      }
    }
  ]
}
```

## Best Practices

### 1. **Start with High-Priority Issues**
Focus on critical and high-priority recommendations first.

### 2. **Batch Related Changes**
Group related naming changes to maintain consistency.

### 3. **Review Public API Changes Carefully**  
Public API changes have broader impact - consider deprecation strategies.

### 4. **Use Configuration Customization**
Tailor abbreviation maps and I/O libraries to your project's conventions.

### 5. **Iterative Improvement**
Run analysis periodically to catch new naming issues as code evolves.

## Troubleshooting

### Model Download Issues
```bash
# Check cache directory
ls -la ~/.refactor_rank/cache/

# Verify model file size (~395 MB for Q4_K_M)
ls -lh ~/.refactor_rank/cache/qwen3-embedding-0.6b-q4_k_m.gguf
```

### Low Confidence Results
- Check if functions have sufficient complexity for analysis
- Verify I/O library patterns are configured for your tech stack
- Consider lowering `min_mismatch` threshold for more sensitive analysis

### Performance Issues
- Enable caching: `io.enable_caching: true`
- Adjust batch size: `performance.batch_size: 50`
- Limit concurrent analysis: `performance.max_threads: 4`

## Technical Implementation

### Architecture
- **BehaviorExtractor**: AST-based static analysis for effect detection
- **EmbeddingBackend**: Qwen3-Embedding integration with local caching
- **NameGenerator**: Deterministic name construction using verb/noun rules
- **SemanticMatcher**: Cosine similarity + rule-based mismatch detection

### Language Support
Currently supports behavior analysis for:
- **Rust**: Full support with `sqlx`, `reqwest`, `tokio` patterns  
- **Python**: Full support with `requests`, `sqlalchemy`, `asyncio` patterns
- **TypeScript/JavaScript**: Full support with `fetch`, `fs`, database libs
- **Go**: Basic support with standard library patterns

### Extensibility
The system is designed for easy extension:
- **New Languages**: Add tree-sitter grammars and I/O library patterns
- **Custom Rules**: Extend mismatch detection with domain-specific rules  
- **Alternative Models**: Replace Qwen with other embedding models
- **Integration**: JSON API enables tooling integration

This comprehensive semantic naming analysis helps maintain high-quality, readable codebases by ensuring function names accurately reflect their behavior and maintaining consistency across projects.