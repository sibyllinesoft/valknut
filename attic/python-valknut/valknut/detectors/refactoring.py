"""
Refactoring suggestion analyzer with specific, actionable recommendations.

Provides concrete refactoring suggestions with before/after code examples
based on complexity metrics and AST analysis.
"""

import re
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Set, Tuple, Any
from enum import Enum

from valknut.core.featureset import BaseFeatureExtractor
from valknut.lang.common_ast import Entity, EntityKind, ParseIndex


class RefactoringType(Enum):
    """Types of refactoring suggestions."""
    EXTRACT_METHOD = "extract_method"
    EXTRACT_CLASS = "extract_class"
    SPLIT_FUNCTION = "split_function"
    CONSOLIDATE_CONDITIONALS = "consolidate_conditionals"
    REPLACE_MAGIC_NUMBERS = "replace_magic_numbers"
    REDUCE_PARAMETERS = "reduce_parameters"
    SIMPLIFY_BOOLEAN = "simplify_boolean"
    EXTRACT_VARIABLE = "extract_variable"
    INLINE_TEMP = "inline_temp"
    MOVE_METHOD = "move_method"


@dataclass
class RefactoringSuggestion:
    """A specific refactoring suggestion with examples."""
    
    type: RefactoringType
    severity: str  # "high", "medium", "low"
    title: str
    description: str
    rationale: str
    before_code: str
    after_code: str
    benefits: List[str] = field(default_factory=list)
    effort: str = "medium"  # "low", "medium", "high"
    location: Optional[str] = None
    line_range: Optional[Tuple[int, int]] = None


@dataclass
class CodePattern:
    """Represents a detected code pattern for refactoring."""
    
    pattern_type: str
    confidence: float
    location: Tuple[int, int]  # start_line, end_line
    context: Dict[str, Any] = field(default_factory=dict)


class RefactoringAnalyzer(BaseFeatureExtractor):
    """Analyzes code for specific refactoring opportunities."""
    
    @property
    def name(self) -> str:
        return "refactoring"
    
    def _initialize_features(self) -> None:
        """Initialize refactoring suggestion features."""
        self._add_feature(
            "refactoring_urgency",
            "Urgency of refactoring (0-100)",
            min_value=0.0,
            max_value=100.0,
            default_value=0.0,
        )
        self._add_feature(
            "suggestion_count", 
            "Number of refactoring suggestions",
            min_value=0.0,
            max_value=50.0,
            default_value=0.0,
        )
        self._add_feature(
            "high_severity_suggestions",
            "Number of high-severity suggestions",
            min_value=0.0,
            max_value=20.0,
            default_value=0.0,
        )
    
    def supports_entity(self, entity: Entity) -> bool:
        """Support functions, methods, classes, and files."""
        return entity.kind in {
            EntityKind.FUNCTION,
            EntityKind.METHOD,
            EntityKind.CLASS,
            EntityKind.FILE,
        }
    
    def extract(self, entity: Entity, index: ParseIndex) -> Dict[str, float]:
        """Extract refactoring features and generate suggestions."""
        features = {}
        
        # Generate refactoring suggestions
        suggestions = self.analyze_refactoring_opportunities(entity, index)
        
        # Store suggestions in entity metadata
        if not hasattr(entity, 'refactoring_suggestions'):
            entity.refactoring_suggestions = suggestions
        
        # Calculate features based on suggestions
        high_severity_count = len([s for s in suggestions if s.severity == "high"])
        total_suggestions = len(suggestions)
        
        # Calculate urgency based on complexity and suggestion severity
        urgency = min(100.0, high_severity_count * 20 + total_suggestions * 5)
        
        features["refactoring_urgency"] = urgency
        features["suggestion_count"] = float(total_suggestions)
        features["high_severity_suggestions"] = float(high_severity_count)
        
        return features
    
    def analyze_refactoring_opportunities(
        self, 
        entity: Entity, 
        index: ParseIndex
    ) -> List[RefactoringSuggestion]:
        """Analyze entity for refactoring opportunities."""
        suggestions = []
        
        if not entity.raw_text:
            return suggestions
        
        # Extract patterns from code
        patterns = self._detect_code_patterns(entity)
        
        # Generate suggestions based on patterns and complexity
        suggestions.extend(self._suggest_extract_method(entity, patterns))
        suggestions.extend(self._suggest_split_function(entity, patterns))
        suggestions.extend(self._suggest_extract_class(entity, patterns, index))
        suggestions.extend(self._suggest_consolidate_conditionals(entity, patterns))
        suggestions.extend(self._suggest_replace_magic_numbers(entity, patterns))
        suggestions.extend(self._suggest_reduce_parameters(entity, patterns))
        suggestions.extend(self._suggest_language_specific(entity, patterns))
        
        return suggestions
    
    def _detect_code_patterns(self, entity: Entity) -> List[CodePattern]:
        """Detect code patterns for refactoring analysis."""
        patterns = []
        source = entity.raw_text
        lines = source.split('\n')
        
        # Detect long methods/functions
        if len(lines) > 20:
            patterns.append(CodePattern(
                pattern_type="long_method",
                confidence=min(1.0, len(lines) / 50.0),
                location=(1, len(lines)),
                context={"line_count": len(lines)}
            ))
        
        # Detect high parameter count
        if len(entity.parameters) > 3:
            patterns.append(CodePattern(
                pattern_type="long_parameter_list",
                confidence=min(1.0, len(entity.parameters) / 8.0),
                location=(1, 1),
                context={"param_count": len(entity.parameters)}
            ))
        
        # Detect repeated code blocks
        patterns.extend(self._detect_code_duplication(lines))
        
        # Detect complex conditionals
        patterns.extend(self._detect_complex_conditionals(lines))
        
        # Detect magic numbers
        patterns.extend(self._detect_magic_numbers(lines))
        
        # Detect data clumps
        patterns.extend(self._detect_data_clumps(entity, lines))
        
        return patterns
    
    def _detect_code_duplication(self, lines: List[str]) -> List[CodePattern]:
        """Detect duplicated code blocks."""
        patterns = []
        
        # Look for repeated sequences of 3+ similar lines
        for i in range(len(lines) - 3):
            block = lines[i:i+3]
            normalized_block = [line.strip() for line in block if line.strip()]
            
            if len(normalized_block) < 3:
                continue
            
            # Look for similar blocks later in the code
            for j in range(i + 3, len(lines) - 2):
                compare_block = lines[j:j+3]
                normalized_compare = [line.strip() for line in compare_block if line.strip()]
                
                if len(normalized_compare) < 3:
                    continue
                
                similarity = self._calculate_similarity(normalized_block, normalized_compare)
                if similarity > 0.7:
                    patterns.append(CodePattern(
                        pattern_type="duplicate_code",
                        confidence=similarity,
                        location=(i+1, i+4),
                        context={
                            "duplicate_at": (j+1, j+4),
                            "similarity": similarity
                        }
                    ))
                    break
        
        return patterns
    
    def _detect_complex_conditionals(self, lines: List[str]) -> List[CodePattern]:
        """Detect overly complex conditional statements."""
        patterns = []
        
        for i, line in enumerate(lines):
            stripped = line.strip()
            
            # Count logical operators in conditionals
            if_match = re.search(r'\b(if|elif|while)\b.*', stripped)
            if if_match:
                condition = if_match.group(0)
                logical_ops = len(re.findall(r'\b(and|or|\&\&|\|\|)\b', condition))
                
                if logical_ops >= 2:
                    patterns.append(CodePattern(
                        pattern_type="complex_conditional",
                        confidence=min(1.0, logical_ops / 5.0),
                        location=(i+1, i+1),
                        context={
                            "logical_operators": logical_ops,
                            "condition": condition
                        }
                    ))
        
        return patterns
    
    def _detect_magic_numbers(self, lines: List[str]) -> List[CodePattern]:
        """Detect magic numbers that should be constants."""
        patterns = []
        
        # Common magic number patterns (excluding 0, 1, -1)
        magic_pattern = re.compile(r'\b(?<![\w.])((?:[2-9]|\d{2,}|0\.\d+|\d+\.\d+))(?![\w.])\b')
        
        for i, line in enumerate(lines):
            stripped = line.strip()
            
            # Skip comments and string literals
            if stripped.startswith('#') or stripped.startswith('//'):
                continue
            
            matches = magic_pattern.findall(stripped)
            if matches:
                patterns.append(CodePattern(
                    pattern_type="magic_numbers",
                    confidence=len(matches) / 5.0,
                    location=(i+1, i+1),
                    context={
                        "numbers": matches,
                        "line": stripped
                    }
                ))
        
        return patterns
    
    def _detect_data_clumps(self, entity: Entity, lines: List[str]) -> List[CodePattern]:
        """Detect groups of data that travel together."""
        patterns = []
        
        # Look for repeated parameter patterns
        if len(entity.parameters) > 2:
            # This is simplified - in practice, we'd analyze parameter usage patterns
            param_types = {}
            for param in entity.parameters:
                # Extract potential type information
                if ':' in param:
                    param_type = param.split(':')[-1].strip()
                    param_types[param_type] = param_types.get(param_type, 0) + 1
            
            # If we have 3+ parameters of similar types, suggest extraction
            for param_type, count in param_types.items():
                if count >= 3:
                    patterns.append(CodePattern(
                        pattern_type="data_clump",
                        confidence=min(1.0, count / 5.0),
                        location=(1, 1),
                        context={
                            "type": param_type,
                            "count": count,
                            "parameters": entity.parameters
                        }
                    ))
        
        return patterns
    
    def _suggest_extract_method(
        self, 
        entity: Entity, 
        patterns: List[CodePattern]
    ) -> List[RefactoringSuggestion]:
        """Suggest extract method refactoring."""
        suggestions = []
        
        # Check for long methods
        long_method_patterns = [p for p in patterns if p.pattern_type == "long_method"]
        
        for pattern in long_method_patterns:
            line_count = pattern.context.get("line_count", 0)
            
            if line_count > 30:
                severity = "high"
            elif line_count > 20:
                severity = "medium"
            else:
                continue
            
            # Generate language-specific example
            before_example, after_example = self._generate_extract_method_example(entity.language, entity.raw_text)
            
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.EXTRACT_METHOD,
                severity=severity,
                title=f"Extract smaller methods from {entity.name}",
                description=f"This {entity.kind.value.lower()} has {line_count} lines and could benefit from extraction.",
                rationale=f"Functions with more than 20 lines are harder to understand, test, and maintain. Breaking this into smaller, focused methods will improve readability and testability.",
                before_code=before_example,
                after_code=after_example,
                benefits=[
                    "Improved readability",
                    "Easier testing of individual components",
                    "Better code reuse",
                    "Reduced complexity",
                    "Clearer separation of concerns"
                ],
                effort="medium",
                location=entity.name,
                line_range=pattern.location
            ))
        
        # Check for duplicate code
        duplicate_patterns = [p for p in patterns if p.pattern_type == "duplicate_code"]
        for pattern in duplicate_patterns:
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.EXTRACT_METHOD,
                severity="medium",
                title="Extract common functionality",
                description="Duplicate code blocks detected that could be extracted into a reusable method.",
                rationale="DRY principle violation. Duplicate code increases maintenance burden and bug risk.",
                before_code=self._generate_duplicate_code_example(entity.language),
                after_code=self._generate_extracted_duplicate_example(entity.language),
                benefits=[
                    "Eliminates code duplication",
                    "Single point of change for logic",
                    "Reduced maintenance burden",
                    "Consistent behavior across usages"
                ],
                effort="low"
            ))
        
        return suggestions
    
    def _suggest_split_function(
        self,
        entity: Entity,
        patterns: List[CodePattern]
    ) -> List[RefactoringSuggestion]:
        """Suggest splitting complex functions."""
        suggestions = []
        
        # Check complexity from entity metrics
        complexity = entity.metrics.get("cyclomatic", 1)
        
        if complexity > 10:
            severity = "high" if complexity > 15 else "medium"
            
            before_example, after_example = self._generate_split_function_example(
                entity.language, 
                entity.raw_text,
                complexity
            )
            
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.SPLIT_FUNCTION,
                severity=severity,
                title=f"Split complex function (complexity: {complexity})",
                description=f"Cyclomatic complexity of {complexity} indicates this function does too much.",
                rationale="Functions with high cyclomatic complexity (>10) are harder to understand, test, and maintain. Consider the Single Responsibility Principle.",
                before_code=before_example,
                after_code=after_example,
                benefits=[
                    "Reduced cognitive load",
                    "Easier to test individual responsibilities", 
                    "Better adherence to SRP",
                    "Improved maintainability",
                    f"Complexity reduction from {complexity} to ~3-5 per function"
                ],
                effort="medium",
                location=entity.name
            ))
        
        return suggestions
    
    def _suggest_extract_class(
        self,
        entity: Entity,
        patterns: List[CodePattern],
        index: ParseIndex
    ) -> List[RefactoringSuggestion]:
        """Suggest extract class refactoring."""
        suggestions = []
        
        # Check for data clumps
        data_clump_patterns = [p for p in patterns if p.pattern_type == "data_clump"]
        
        for pattern in data_clump_patterns:
            count = pattern.context.get("count", 0)
            param_type = pattern.context.get("type", "unknown")
            
            if count >= 3:
                before_example, after_example = self._generate_extract_class_example(
                    entity.language, 
                    pattern.context.get("parameters", [])
                )
                
                suggestions.append(RefactoringSuggestion(
                    type=RefactoringType.EXTRACT_CLASS,
                    severity="medium",
                    title="Extract parameter object",
                    description=f"Function has {count} parameters of type {param_type} that could be grouped.",
                    rationale="Long parameter lists are hard to remember and use. Grouping related parameters into a class improves encapsulation and reduces coupling.",
                    before_code=before_example,
                    after_code=after_example,
                    benefits=[
                        "Reduced parameter list complexity",
                        "Better encapsulation of related data", 
                        "Easier to extend with new related fields",
                        "Improved method signatures"
                    ],
                    effort="medium"
                ))
        
        return suggestions
    
    def _suggest_consolidate_conditionals(
        self,
        entity: Entity, 
        patterns: List[CodePattern]
    ) -> List[RefactoringSuggestion]:
        """Suggest consolidating complex conditionals."""
        suggestions = []
        
        complex_conditional_patterns = [p for p in patterns if p.pattern_type == "complex_conditional"]
        
        for pattern in complex_conditional_patterns:
            logical_ops = pattern.context.get("logical_operators", 0)
            condition = pattern.context.get("condition", "")
            
            if logical_ops >= 2:
                before_example, after_example = self._generate_conditional_example(
                    entity.language,
                    condition
                )
                
                suggestions.append(RefactoringSuggestion(
                    type=RefactoringType.CONSOLIDATE_CONDITIONALS,
                    severity="medium",
                    title="Simplify complex conditional",
                    description=f"Conditional with {logical_ops} logical operators could be simplified.",
                    rationale="Complex conditionals reduce readability and increase the chance of logical errors. Extract meaningful boolean methods.",
                    before_code=before_example,
                    after_code=after_example,
                    benefits=[
                        "Improved readability",
                        "Self-documenting code",
                        "Easier to test boolean logic",
                        "Reduced cognitive complexity"
                    ],
                    effort="low",
                    line_range=(pattern.location[0], pattern.location[1])
                ))
        
        return suggestions
    
    def _suggest_replace_magic_numbers(
        self,
        entity: Entity,
        patterns: List[CodePattern]
    ) -> List[RefactoringSuggestion]:
        """Suggest replacing magic numbers with constants."""
        suggestions = []
        
        magic_number_patterns = [p for p in patterns if p.pattern_type == "magic_numbers"]
        
        if magic_number_patterns:
            # Collect all magic numbers
            all_numbers = []
            for pattern in magic_number_patterns:
                all_numbers.extend(pattern.context.get("numbers", []))
            
            if all_numbers:
                before_example, after_example = self._generate_magic_number_example(
                    entity.language,
                    all_numbers
                )
                
                suggestions.append(RefactoringSuggestion(
                    type=RefactoringType.REPLACE_MAGIC_NUMBERS,
                    severity="low",
                    title="Replace magic numbers with named constants",
                    description=f"Found {len(all_numbers)} magic numbers that could be named constants.",
                    rationale="Magic numbers make code harder to understand and maintain. Named constants provide context and make changes easier.",
                    before_code=before_example,
                    after_code=after_example,
                    benefits=[
                        "Self-documenting code",
                        "Easier to modify values",
                        "Reduced risk of typos",
                        "Better maintainability"
                    ],
                    effort="low"
                ))
        
        return suggestions
    
    def _suggest_reduce_parameters(
        self,
        entity: Entity,
        patterns: List[CodePattern]
    ) -> List[RefactoringSuggestion]:
        """Suggest reducing parameter count."""
        suggestions = []
        
        param_patterns = [p for p in patterns if p.pattern_type == "long_parameter_list"]
        
        for pattern in param_patterns:
            param_count = pattern.context.get("param_count", 0)
            
            if param_count > 5:
                severity = "high"
            elif param_count > 3:
                severity = "medium"
            else:
                continue
            
            before_example, after_example = self._generate_parameter_reduction_example(
                entity.language,
                entity.parameters
            )
            
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.REDUCE_PARAMETERS,
                severity=severity,
                title=f"Reduce parameter count ({param_count} parameters)",
                description=f"Function has {param_count} parameters, consider parameter object or builder pattern.",
                rationale="Functions with many parameters are hard to use and understand. Consider grouping related parameters.",
                before_code=before_example,
                after_code=after_example,
                benefits=[
                    "Easier to call and remember",
                    "Better encapsulation",
                    "More extensible interface",
                    "Reduced coupling"
                ],
                effort="medium"
            ))
        
        return suggestions
    
    def _suggest_language_specific(
        self,
        entity: Entity,
        patterns: List[CodePattern]
    ) -> List[RefactoringSuggestion]:
        """Generate language-specific refactoring suggestions."""
        suggestions = []
        
        if entity.language == "python":
            suggestions.extend(self._suggest_python_specific(entity))
        elif entity.language in ["typescript", "javascript"]:
            suggestions.extend(self._suggest_typescript_specific(entity))
        elif entity.language == "rust":
            suggestions.extend(self._suggest_rust_specific(entity))
        elif entity.language == "go":
            suggestions.extend(self._suggest_go_specific(entity))
        
        return suggestions
    
    def _suggest_python_specific(self, entity: Entity) -> List[RefactoringSuggestion]:
        """Python-specific refactoring suggestions."""
        suggestions = []
        source = entity.raw_text
        
        # Check for string concatenation that could use f-strings
        if re.search(r'["\'].*["\'].*\+.*["\']', source):
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.EXTRACT_VARIABLE,
                severity="low",
                title="Use f-strings for string formatting",
                description="String concatenation found that could use more readable f-string syntax.",
                rationale="f-strings are more readable, faster, and less error-prone than string concatenation.",
                before_code='''# Before
name = "John"
age = 30
message = "Hello " + name + ", you are " + str(age) + " years old"''',
                after_code='''# After
name = "John"
age = 30
message = f"Hello {name}, you are {age} years old"''',
                benefits=["Improved readability", "Better performance", "Less error-prone"],
                effort="low"
            ))
        
        # Check for loop patterns that could be list comprehensions
        if re.search(r'for\s+\w+\s+in.*:\s*\n\s*.*\.append\(', source, re.MULTILINE):
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.SIMPLIFY_BOOLEAN,
                severity="low", 
                title="Consider list comprehensions",
                description="Found append loops that could be more pythonic list comprehensions.",
                rationale="List comprehensions are more readable and often faster than manual loops with append.",
                before_code='''# Before
result = []
for item in items:
    if item > 0:
        result.append(item * 2)''',
                after_code='''# After
result = [item * 2 for item in items if item > 0]''',
                benefits=["More Pythonic", "Often faster", "More concise"],
                effort="low"
            ))
        
        return suggestions
    
    def _suggest_typescript_specific(self, entity: Entity) -> List[RefactoringSuggestion]:
        """TypeScript/JavaScript-specific suggestions."""
        suggestions = []
        source = entity.raw_text
        
        # Check for any types
        if re.search(r':\s*any\b', source):
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.EXTRACT_VARIABLE,
                severity="medium",
                title="Replace 'any' with specific types",
                description="Found 'any' types that could be more specific.",
                rationale="Specific types provide better IDE support, catch errors at compile-time, and improve documentation.",
                before_code='''// Before
function processData(data: any): any {
    return data.map((item: any) => item.value);
}''',
                after_code='''// After
interface DataItem {
    value: string;
}

function processData(data: DataItem[]): string[] {
    return data.map((item: DataItem) => item.value);
}''',
                benefits=["Type safety", "Better IDE support", "Self-documenting code"],
                effort="medium"
            ))
        
        # Check for traditional function syntax that could use arrow functions
        if re.search(r'function\s+\w+\s*\([^)]*\)\s*{[^}]*}', source):
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.SIMPLIFY_BOOLEAN,
                severity="low",
                title="Consider arrow functions for callbacks",
                description="Traditional function syntax found that could use more concise arrow functions.",
                rationale="Arrow functions are more concise and have lexical 'this' binding.",
                before_code='''// Before
items.filter(function(item) {
    return item.active;
}).map(function(item) {
    return item.name;
});''',
                after_code='''// After
items
    .filter(item => item.active)
    .map(item => item.name);''',
                benefits=["More concise", "Lexical this binding", "Functional style"],
                effort="low"
            ))
        
        return suggestions
    
    def _suggest_rust_specific(self, entity: Entity) -> List[RefactoringSuggestion]:
        """Rust-specific suggestions."""
        suggestions = []
        source = entity.raw_text
        
        # Check for unwrap() usage
        if re.search(r'\.unwrap\(\)', source):
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.EXTRACT_VARIABLE,
                severity="medium", 
                title="Replace .unwrap() with proper error handling",
                description="Found .unwrap() calls that could use more robust error handling.",
                rationale="unwrap() can cause panics. Use match, if let, or ? operator for better error handling.",
                before_code='''// Before
let value = maybe_value.unwrap();
let result = risky_operation().unwrap();''',
                after_code='''// After
let value = maybe_value?;
let result = match risky_operation() {
    Ok(val) => val,
    Err(e) => return Err(e.into()),
};''',
                benefits=["No panics", "Explicit error handling", "More robust code"],
                effort="medium"
            ))
        
        return suggestions
    
    def _suggest_go_specific(self, entity: Entity) -> List[RefactoringSuggestion]:
        """Go-specific suggestions.""" 
        suggestions = []
        source = entity.raw_text
        
        # Check for error handling patterns
        if source.count('if err != nil') > 3:
            suggestions.append(RefactoringSuggestion(
                type=RefactoringType.EXTRACT_METHOD,
                severity="low",
                title="Consider extracting error handling helpers",
                description="Repeated error handling patterns found.",
                rationale="Extracting common error handling reduces boilerplate and improves consistency.",
                before_code='''// Before
result1, err := operation1()
if err != nil {
    return nil, fmt.Errorf("operation1 failed: %w", err)
}

result2, err := operation2()
if err != nil {
    return nil, fmt.Errorf("operation2 failed: %w", err)
}''',
                after_code='''// After
func must(result interface{}, err error, operation string) interface{} {
    if err != nil {
        return nil, fmt.Errorf("%s failed: %w", operation, err)
    }
    return result
}

result1 := must(operation1(), "operation1")
result2 := must(operation2(), "operation2")''',
                benefits=["Less boilerplate", "Consistent error handling", "More readable"],
                effort="low"
            ))
        
        return suggestions
    
    def _calculate_similarity(self, block1: List[str], block2: List[str]) -> float:
        """Calculate similarity between two code blocks."""
        if len(block1) != len(block2):
            return 0.0
        
        matches = 0
        for line1, line2 in zip(block1, block2):
            # Normalize by removing extra whitespace and comparing tokens
            tokens1 = line1.split()
            tokens2 = line2.split()
            
            if tokens1 == tokens2:
                matches += 1
            elif len(tokens1) == len(tokens2):
                # Check token similarity
                token_matches = sum(1 for t1, t2 in zip(tokens1, tokens2) if t1 == t2)
                matches += token_matches / len(tokens1)
        
        return matches / len(block1)
    
    # Example generation methods
    def _generate_extract_method_example(self, language: str, source: str) -> Tuple[str, str]:
        """Generate before/after example for extract method refactoring."""
        if language == "python":
            before = '''def process_user_data(user_data):
    # Validate input data
    if not user_data:
        raise ValueError("User data is required")
    if not user_data.get("email"):
        raise ValueError("Email is required")
    if "@" not in user_data["email"]:
        raise ValueError("Invalid email format")
    
    # Transform data
    normalized_email = user_data["email"].lower().strip()
    full_name = f"{user_data.get('first_name', '')} {user_data.get('last_name', '')}"
    user_data["full_name"] = full_name.strip()
    user_data["email"] = normalized_email
    
    # Save to database
    try:
        db.users.insert_one(user_data)
        logger.info(f"User {normalized_email} created successfully")
    except Exception as e:
        logger.error(f"Failed to create user: {e}")
        raise
    
    # Send welcome email
    email_content = f"Welcome {full_name}! Thanks for joining."
    send_email(normalized_email, "Welcome", email_content)
    
    return user_data'''
            
            after = '''def process_user_data(user_data):
    validated_data = _validate_user_data(user_data)
    normalized_data = _transform_user_data(validated_data)
    saved_user = _save_user_to_db(normalized_data)
    _send_welcome_email(saved_user)
    return saved_user

def _validate_user_data(user_data):
    if not user_data:
        raise ValueError("User data is required")
    if not user_data.get("email"):
        raise ValueError("Email is required")
    if "@" not in user_data["email"]:
        raise ValueError("Invalid email format")
    return user_data

def _transform_user_data(user_data):
    normalized_email = user_data["email"].lower().strip()
    full_name = f"{user_data.get('first_name', '')} {user_data.get('last_name', '')}"
    user_data["full_name"] = full_name.strip()
    user_data["email"] = normalized_email
    return user_data

def _save_user_to_db(user_data):
    try:
        db.users.insert_one(user_data)
        logger.info(f"User {user_data['email']} created successfully")
        return user_data
    except Exception as e:
        logger.error(f"Failed to create user: {e}")
        raise

def _send_welcome_email(user_data):
    email_content = f"Welcome {user_data['full_name']}! Thanks for joining."
    send_email(user_data["email"], "Welcome", email_content)'''
        
        elif language in ["typescript", "javascript"]:
            before = '''function processOrderData(orderData) {
    // Validate order
    if (!orderData || !orderData.items || orderData.items.length === 0) {
        throw new Error("Order must have items");
    }
    
    // Calculate totals
    let subtotal = 0;
    for (const item of orderData.items) {
        if (!item.price || item.price < 0) {
            throw new Error("Invalid item price");
        }
        subtotal += item.price * (item.quantity || 1);
    }
    const tax = subtotal * 0.08;
    const total = subtotal + tax;
    
    // Apply discounts
    let discount = 0;
    if (orderData.couponCode) {
        const coupon = validateCoupon(orderData.couponCode);
        if (coupon) {
            discount = coupon.type === "percentage" 
                ? subtotal * (coupon.value / 100)
                : coupon.value;
        }
    }
    
    const finalTotal = total - discount;
    
    return {
        ...orderData,
        subtotal,
        tax,
        discount,
        total: finalTotal
    };
}'''
            
            after = '''function processOrderData(orderData) {
    validateOrder(orderData);
    const totals = calculateOrderTotals(orderData);
    const discount = calculateDiscount(orderData, totals.subtotal);
    
    return {
        ...orderData,
        ...totals,
        discount,
        total: totals.total - discount
    };
}

function validateOrder(orderData) {
    if (!orderData || !orderData.items || orderData.items.length === 0) {
        throw new Error("Order must have items");
    }
    
    for (const item of orderData.items) {
        if (!item.price || item.price < 0) {
            throw new Error("Invalid item price");
        }
    }
}

function calculateOrderTotals(orderData) {
    const subtotal = orderData.items.reduce((sum, item) => 
        sum + (item.price * (item.quantity || 1)), 0
    );
    const tax = subtotal * 0.08;
    
    return { subtotal, tax, total: subtotal + tax };
}

function calculateDiscount(orderData, subtotal) {
    if (!orderData.couponCode) return 0;
    
    const coupon = validateCoupon(orderData.couponCode);
    if (!coupon) return 0;
    
    return coupon.type === "percentage" 
        ? subtotal * (coupon.value / 100)
        : coupon.value;
}'''
        
        else:
            # Generic example
            before = "// Long method with multiple responsibilities"
            after = "// Extracted into focused, single-purpose methods"
        
        return before, after
    
    def _generate_split_function_example(self, language: str, source: str, complexity: int) -> Tuple[str, str]:
        """Generate example for function splitting."""
        if language == "python":
            before = '''def process_file_upload(file_data, user_id, options=None):
    # Complex function with high cyclomatic complexity
    if not file_data:
        return {"error": "No file provided"}
    
    if options and options.get("validate_size"):
        if len(file_data) > 10_000_000:
            return {"error": "File too large"}
    
    if options and options.get("validate_type"):
        allowed_types = options.get("allowed_types", [".jpg", ".png"])
        file_ext = file_data.name.split(".")[-1].lower()
        if f".{file_ext}" not in allowed_types:
            return {"error": "Invalid file type"}
    
    # Process based on file type
    if file_ext in ["jpg", "jpeg", "png"]:
        # Image processing
        if options and options.get("resize"):
            # Complex image resizing logic
            width = options.get("width", 800)
            height = options.get("height", 600)
            if width > 2000 or height > 2000:
                return {"error": "Resize dimensions too large"}
            # ... more image processing
        return {"status": "image_processed"}
    elif file_ext in ["pdf", "doc", "docx"]:
        # Document processing
        if options and options.get("extract_text"):
            # Text extraction logic
            pass
        return {"status": "document_processed"}
    else:
        return {"error": "Unsupported file type"}'''
        
            after = '''def process_file_upload(file_data, user_id, options=None):
    """Main orchestrator for file upload processing."""
    validation_result = validate_file_upload(file_data, options)
    if validation_result.get("error"):
        return validation_result
    
    return process_file_by_type(file_data, options)

def validate_file_upload(file_data, options):
    """Validate file upload constraints."""
    if not file_data:
        return {"error": "No file provided"}
    
    if not validate_file_size(file_data, options):
        return {"error": "File too large"}
    
    if not validate_file_type(file_data, options):
        return {"error": "Invalid file type"}
    
    return {"valid": True}

def validate_file_size(file_data, options):
    """Check if file size is within limits."""
    if not (options and options.get("validate_size")):
        return True
    return len(file_data) <= 10_000_000

def validate_file_type(file_data, options):
    """Check if file type is allowed."""
    if not (options and options.get("validate_type")):
        return True
    
    allowed_types = options.get("allowed_types", [".jpg", ".png"])
    file_ext = file_data.name.split(".")[-1].lower()
    return f".{file_ext}" in allowed_types

def process_file_by_type(file_data, options):
    """Process file based on its type."""
    file_ext = file_data.name.split(".")[-1].lower()
    
    if file_ext in ["jpg", "jpeg", "png"]:
        return process_image_file(file_data, options)
    elif file_ext in ["pdf", "doc", "docx"]:
        return process_document_file(file_data, options)
    else:
        return {"error": "Unsupported file type"}

def process_image_file(file_data, options):
    """Handle image file processing."""
    if options and options.get("resize"):
        return resize_image(file_data, options)
    return {"status": "image_processed"}

def process_document_file(file_data, options):
    """Handle document file processing."""
    if options and options.get("extract_text"):
        return extract_text_from_document(file_data)
    return {"status": "document_processed"}'''
        
        else:
            before = f"// Complex function with cyclomatic complexity: {complexity}"
            after = "// Split into focused functions with complexity ~3-5 each"
        
        return before, after
    
    def _generate_extract_class_example(self, language: str, parameters: List[str]) -> Tuple[str, str]:
        """Generate example for extract class refactoring."""
        if language == "python":
            before = '''def create_user_profile(first_name, last_name, email, 
                       address_line1, address_line2, city, state, zip_code,
                       phone_number, emergency_contact_name, emergency_contact_phone):
    # Function with too many parameters - data clump detected
    user = User(
        name=f"{first_name} {last_name}",
        email=email,
        address=f"{address_line1} {address_line2}, {city}, {state} {zip_code}",
        phone=phone_number,
        emergency_contact=f"{emergency_contact_name}: {emergency_contact_phone}"
    )
    return user.save()'''
        
            after = '''@dataclass
class Address:
    line1: str
    line2: str = ""
    city: str = ""
    state: str = ""
    zip_code: str = ""
    
    def __str__(self):
        return f"{self.line1} {self.line2}, {self.city}, {self.state} {self.zip_code}"

@dataclass
class EmergencyContact:
    name: str
    phone: str
    
    def __str__(self):
        return f"{self.name}: {self.phone}"

def create_user_profile(first_name, last_name, email, 
                       address: Address, phone_number: str,
                       emergency_contact: EmergencyContact):
    # Much cleaner interface with grouped data
    user = User(
        name=f"{first_name} {last_name}",
        email=email,
        address=str(address),
        phone=phone_number,
        emergency_contact=str(emergency_contact)
    )
    return user.save()

# Usage becomes much clearer:
# address = Address("123 Main St", "", "City", "ST", "12345")
# emergency = EmergencyContact("Jane Doe", "555-0123")
# create_user_profile("John", "Doe", "john@example.com", address, "555-0123", emergency)'''
        
        else:
            before = "// Function with many related parameters"
            after = "// Grouped related parameters into cohesive classes"
        
        return before, after
    
    def _generate_conditional_example(self, language: str, condition: str) -> Tuple[str, str]:
        """Generate example for conditional consolidation."""
        if language == "python":
            before = '''def is_valid_user_for_premium(user, subscription, payment_method):
    if user.age >= 18 and user.verified and user.country in ["US", "CA", "UK"] and \
       subscription.type == "monthly" and subscription.active and not subscription.cancelled and \
       payment_method.valid and payment_method.type in ["credit_card", "paypal"] and \
       payment_method.expires_at > datetime.now():
        return True
    return False'''
        
            after = '''def is_valid_user_for_premium(user, subscription, payment_method):
    return (is_eligible_user(user) and 
            has_active_subscription(subscription) and 
            has_valid_payment_method(payment_method))

def is_eligible_user(user):
    return (user.age >= 18 and 
            user.verified and 
            user.country in ["US", "CA", "UK"])

def has_active_subscription(subscription):
    return (subscription.type == "monthly" and 
            subscription.active and 
            not subscription.cancelled)

def has_valid_payment_method(payment_method):
    return (payment_method.valid and 
            payment_method.type in ["credit_card", "paypal"] and 
            payment_method.expires_at > datetime.now())'''
        
        else:
            before = "// Complex conditional with multiple logical operators"
            after = "// Extracted meaningful boolean methods"
        
        return before, after
    
    def _generate_magic_number_example(self, language: str, numbers: List[str]) -> Tuple[str, str]:
        """Generate example for magic number replacement."""
        if language == "python":
            before = '''def calculate_shipping_cost(weight, distance):
    base_cost = 5.99
    if weight > 10:
        base_cost += 2.50 * (weight - 10)
    
    if distance > 100:
        base_cost *= 1.25
    elif distance > 50:
        base_cost *= 1.15
    
    if base_cost > 25.00:
        base_cost = 25.00  # Cap at maximum
    
    return round(base_cost, 2)'''
        
            after = '''# Constants make the business logic clear
BASE_SHIPPING_COST = 5.99
WEIGHT_THRESHOLD = 10  # kg
EXTRA_WEIGHT_COST = 2.50  # per kg over threshold
LONG_DISTANCE_THRESHOLD = 100  # km
MEDIUM_DISTANCE_THRESHOLD = 50  # km
LONG_DISTANCE_MULTIPLIER = 1.25
MEDIUM_DISTANCE_MULTIPLIER = 1.15
MAX_SHIPPING_COST = 25.00

def calculate_shipping_cost(weight, distance):
    base_cost = BASE_SHIPPING_COST
    
    if weight > WEIGHT_THRESHOLD:
        base_cost += EXTRA_WEIGHT_COST * (weight - WEIGHT_THRESHOLD)
    
    if distance > LONG_DISTANCE_THRESHOLD:
        base_cost *= LONG_DISTANCE_MULTIPLIER
    elif distance > MEDIUM_DISTANCE_THRESHOLD:
        base_cost *= MEDIUM_DISTANCE_MULTIPLIER
    
    if base_cost > MAX_SHIPPING_COST:
        base_cost = MAX_SHIPPING_COST
    
    return round(base_cost, 2)'''
        
        else:
            before = "// Code with magic numbers"
            after = "// Named constants provide context"
        
        return before, after
    
    def _generate_parameter_reduction_example(self, language: str, parameters: List[str]) -> Tuple[str, str]:
        """Generate example for parameter reduction."""
        if language == "python":
            before = '''def send_notification(user_id, title, message, notification_type, 
                       priority, send_email, send_sms, send_push,
                       schedule_time, retry_count, retry_interval):
    # Too many parameters make this function hard to use
    notification = Notification(
        user_id=user_id,
        title=title,
        message=message,
        type=notification_type,
        priority=priority,
        channels={
            "email": send_email,
            "sms": send_sms, 
            "push": send_push
        },
        schedule_time=schedule_time,
        retry_policy={
            "count": retry_count,
            "interval": retry_interval
        }
    )
    return notification.send()'''
        
            after = '''@dataclass
class NotificationChannels:
    email: bool = True
    sms: bool = False
    push: bool = True

@dataclass 
class RetryPolicy:
    count: int = 3
    interval: int = 60  # seconds

@dataclass
class NotificationConfig:
    priority: str = "normal"
    channels: NotificationChannels = field(default_factory=NotificationChannels)
    schedule_time: Optional[datetime] = None
    retry_policy: RetryPolicy = field(default_factory=RetryPolicy)

def send_notification(user_id: str, title: str, message: str, 
                     notification_type: str = "info",
                     config: NotificationConfig = None):
    # Much cleaner interface with sensible defaults
    if config is None:
        config = NotificationConfig()
    
    notification = Notification(
        user_id=user_id,
        title=title,
        message=message,
        type=notification_type,
        config=config
    )
    return notification.send()

# Usage becomes much simpler:
# send_notification("user123", "Welcome", "Thanks for joining!")
# 
# Or with custom config:
# config = NotificationConfig(
#     channels=NotificationChannels(email=False, sms=True),
#     priority="high"
# )
# send_notification("user123", "Alert", "Important message", config=config)'''
        
        else:
            before = "// Function with too many parameters"  
            after = "// Parameter object pattern with sensible defaults"
        
        return before, after
    
    def _generate_duplicate_code_example(self, language: str) -> str:
        """Generate before example for duplicate code."""
        if language == "python":
            return '''def process_user_registration(user_data):
    if not user_data.get("email"):
        logger.error("Missing email in user registration")
        return {"error": "Email required", "code": 400}
    
    if not user_data.get("password"):
        logger.error("Missing password in user registration")  
        return {"error": "Password required", "code": 400}
    
    # Registration logic here
    return {"success": True}

def process_user_update(user_data):
    if not user_data.get("email"):
        logger.error("Missing email in user update")
        return {"error": "Email required", "code": 400}
    
    if not user_data.get("id"):
        logger.error("Missing ID in user update")
        return {"error": "ID required", "code": 400}
    
    # Update logic here
    return {"success": True}'''
        else:
            return "// Duplicate validation logic across methods"
    
    def _generate_extracted_duplicate_example(self, language: str) -> str:
        """Generate after example for duplicate code extraction."""
        if language == "python":
            return '''def validate_required_field(user_data, field_name, context="operation"):
    """Reusable validation with consistent error handling."""
    if not user_data.get(field_name):
        logger.error(f"Missing {field_name} in {context}")
        return {"error": f"{field_name.title()} required", "code": 400}
    return None

def process_user_registration(user_data):
    # Validate required fields
    error = validate_required_field(user_data, "email", "user registration")
    if error:
        return error
    
    error = validate_required_field(user_data, "password", "user registration")
    if error:
        return error
    
    # Registration logic here
    return {"success": True}

def process_user_update(user_data):
    # Same validation, no duplication
    error = validate_required_field(user_data, "email", "user update")
    if error:
        return error
    
    error = validate_required_field(user_data, "id", "user update")
    if error:
        return error
    
    # Update logic here
    return {"success": True}'''
        else:
            return "// Extracted common validation logic"