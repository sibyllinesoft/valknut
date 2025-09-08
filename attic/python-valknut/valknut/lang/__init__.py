"""Language adapters for valknut."""

# Re-export common types
from valknut.lang.common_ast import (
    Entity,
    EntityKind,
    ParseIndex,
    SourceLocation,
    LanguageAdapter,
    TreeSitterBaseAdapter,
    AdapterStatus,
    AdapterDiagnostic,
    LanguageSupportRegistry,
    ParsedImport,
    language_support_registry,
)

def get_language_support_status() -> str:
    """Get a human-readable language support status report."""
    return language_support_registry.generate_support_report()

def get_available_languages() -> set[str]:
    """Get set of available language names."""
    return language_support_registry.get_available_languages()

def get_language_status(language: str) -> AdapterStatus | None:
    """Get status for a specific language."""
    return language_support_registry.get_status(language)

__all__ = [
    "Entity",
    "EntityKind", 
    "ParseIndex",
    "SourceLocation",
    "LanguageAdapter",
    "TreeSitterBaseAdapter",
    "AdapterStatus",
    "AdapterDiagnostic",
    "LanguageSupportRegistry", 
    "ParsedImport",
    "language_support_registry",
    "get_language_support_status",
    "get_available_languages",
    "get_language_status",
]