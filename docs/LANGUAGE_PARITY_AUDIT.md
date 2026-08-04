# Language Adapter Parity Audit

This audit uses Python and TypeScript as the mature baselines. Parity means an adapter can parse modern syntax and provide Valknut's common agent-facing primitives: semantic entities with source ranges and metadata, calls, identifiers, dependencies, AST normalization, block counts, and complexity inputs. It does not require languages to expose concepts they do not have.

The minimum contract is enforced by `tests/language_parity.rs` for every language in the registry.

The flat agent-report preserves a compact `context` object per analyzed entity. It carries ownership, children, parameters, return types, modifiers, inheritance, generics, calls, and language-specific type context when available, without copying noisy token inventories.

| Language | Contract | Semantic metadata | Notable remaining violations |
| --- | --- | --- | --- |
| Python | Pass | Rich | Dynamic assignments cannot always be resolved to a nominal owner. |
| TypeScript | Pass | Rich | Overload groups and signature summaries are linked; declaration merging across modules remains compiler-level. |
| JavaScript | Pass | Good | Class/member metadata is necessarily shallower than TypeScript's type metadata. |
| Rust | Pass | Rich | External implemented types retain a stable owner path but cannot be resolved without dependency metadata. |
| Go | Pass | Rich | Package ownership and generic parameter text are retained; constraint semantics remain shallow. |
| C++ | Pass | Rich | Macro-generated declarations remain opaque until preprocessing. |
| C# | Pass | Rich | Partial declarations share stable namespace-qualified owner paths; member sets are not physically merged. |

## C# parity coverage

C# supports namespaces (including file-scoped namespaces), classes, records, structs, interfaces, enums, delegates, fields/constants, properties/indexers/events, methods, constructors, destructors, operators, accessors, local functions, lambdas, and anonymous methods. Entities include hierarchy, source ranges, identifiers, calls, parameters, return types, visibility, modifiers, generic parameters, base types, and enum members. Dependency extraction covers ordinary, global, static, and aliased `using` directives.

Complexity recognizes C# `foreach`, `do`, switch statements/expressions and sections, conditionals, logical operators, try/catch, loops, and nesting. The same additions improve equivalent constructs in the other grammars.

## Priorities

The high-priority parity violations identified in the initial audit are fixed and covered by executable tests. Remaining items above are language-specific enhancements rather than common-contract failures.

The continuing audit also closed these cross-cutting gaps:

- `.tsx` now selects the TSX grammar instead of the TypeScript-only grammar.
- Anonymous functions, generators, closures, and lambdas are retained consistently.
- Python multi-imports, JavaScript/TypeScript side-effect and multiline imports, and Rust multiline grouped imports preserve their dependency semantics.
- Parameter signatures and per-callable call targets reach semantic context consistently.
- Registry extensions, default configuration, structure analysis, and cache language defaults describe the same supported set.
- Agent artifacts expose sorted, normalized dependency rows with a compact dependency-kind catalog.
- TypeScript overloads carry group/signature context; C# partial owners, Go packages, and external Rust impl owners have stable query keys.
