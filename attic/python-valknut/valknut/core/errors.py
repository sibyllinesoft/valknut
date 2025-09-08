"""
Core exception classes for valknut.
"""

from typing import Any, Optional


class RefactorRankError(Exception):
    """Base exception for all valknut errors."""
    
    def __init__(self, message: str, details: Optional[dict[str, Any]] = None) -> None:
        super().__init__(message)
        self.message = message
        self.details = details or {}


class ConfigurationError(RefactorRankError):
    """Raised when configuration is invalid or missing."""
    pass


class ParseError(RefactorRankError):
    """Raised when file parsing fails."""
    
    def __init__(self, file_path: str, message: str, details: Optional[dict[str, Any]] = None) -> None:
        super().__init__(f"Parse error in {file_path}: {message}", details)
        self.file_path = file_path


class LanguageNotSupportedError(RefactorRankError):
    """Raised when a language is not supported."""
    
    def __init__(self, language: str, supported_languages: list[str]) -> None:
        message = f"Language '{language}' not supported. Supported: {', '.join(supported_languages)}"
        super().__init__(message)
        self.language = language
        self.supported_languages = supported_languages


class FeatureExtractionError(RefactorRankError):
    """Raised when feature extraction fails."""
    
    def __init__(self, feature_name: str, entity_id: str, message: str) -> None:
        super().__init__(f"Feature extraction failed for {feature_name} on {entity_id}: {message}")
        self.feature_name = feature_name
        self.entity_id = entity_id


class CacheError(RefactorRankError):
    """Raised when cache operations fail."""
    pass


class ServerError(RefactorRankError):
    """Raised when server operations fail."""
    pass


class MCPError(RefactorRankError):
    """Raised when MCP operations fail."""
    pass