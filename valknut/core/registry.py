"""
Registry system for language adapters and feature detectors.
"""

from typing import Dict, List, Optional, Set, Type, TypeVar

from valknut.core.errors import LanguageNotSupportedError
from valknut.lang.common_ast import LanguageAdapter

T = TypeVar("T")


class Registry:
    """Generic registry for components."""
    
    def __init__(self) -> None:
        self._registry: Dict[str, Type[T]] = {}
        self._instances: Dict[str, T] = {}
    
    def register(self, name: str, cls: Type[T]) -> None:
        """Register a component class."""
        self._registry[name] = cls
    
    def get_class(self, name: str) -> Optional[Type[T]]:
        """Get component class by name."""
        return self._registry.get(name)
    
    def get_instance(self, name: str, *args, **kwargs) -> Optional[T]:
        """Get or create component instance."""
        if name not in self._registry:
            return None
        
        if name not in self._instances:
            self._instances[name] = self._registry[name](*args, **kwargs)
        
        return self._instances[name]
    
    def list_registered(self) -> List[str]:
        """List all registered component names."""
        return list(self._registry.keys())
    
    def is_registered(self, name: str) -> bool:
        """Check if a component is registered."""
        return name in self._registry


class LanguageRegistry:
    """Registry for language adapters."""
    
    def __init__(self) -> None:
        self._adapters: Dict[str, Type[LanguageAdapter]] = {}
        self._instances: Dict[str, LanguageAdapter] = {}
        self._extensions: Dict[str, str] = {}  # extension -> language mapping
    
    def register_adapter(self, adapter_cls: Type[LanguageAdapter]) -> None:
        """
        Register a language adapter.
        
        Args:
            adapter_cls: Adapter class to register
        """
        # Create instance to get metadata
        instance = adapter_cls()
        language = instance.language
        extensions = instance.file_extensions
        
        self._adapters[language] = adapter_cls
        
        # Register file extensions
        for ext in extensions:
            self._extensions[ext] = language
    
    def get_adapter(self, language: str) -> LanguageAdapter:
        """
        Get language adapter instance.
        
        Args:
            language: Language name
            
        Returns:
            Adapter instance
            
        Raises:
            LanguageNotSupportedError: If language not supported
        """
        if language not in self._adapters:
            supported = list(self._adapters.keys())
            raise LanguageNotSupportedError(language, supported)
        
        if language not in self._instances:
            self._instances[language] = self._adapters[language]()
        
        return self._instances[language]
    
    def get_adapter_by_extension(self, extension: str) -> Optional[LanguageAdapter]:
        """
        Get adapter by file extension.
        
        Args:
            extension: File extension (with or without dot)
            
        Returns:
            Adapter instance or None
        """
        # Normalize extension
        if not extension.startswith("."):
            extension = f".{extension}"
        
        language = self._extensions.get(extension)
        if language:
            return self.get_adapter(language)
        return None
    
    def supported_languages(self) -> Set[str]:
        """Get set of supported languages."""
        return set(self._adapters.keys())
    
    def supported_extensions(self) -> Set[str]:
        """Get set of supported file extensions."""
        return set(self._extensions.keys())
    
    def is_supported(self, language: str) -> bool:
        """Check if language is supported."""
        return language in self._adapters


# Global registry instances
language_registry = LanguageRegistry()
detector_registry = Registry()
feature_registry = Registry()


def register_language_adapter(adapter_cls: Type[LanguageAdapter]) -> None:
    """Register a language adapter globally."""
    language_registry.register_adapter(adapter_cls)


def register_detector(name: str, detector_cls: Type) -> None:
    """Register a feature detector globally."""
    detector_registry.register(name, detector_cls)


def register_feature(name: str, feature_cls: Type) -> None:
    """Register a feature extractor globally."""
    feature_registry.register(name, feature_cls)


def get_language_adapter(language: str) -> LanguageAdapter:
    """Get language adapter by name."""
    return language_registry.get_adapter(language)


def get_supported_languages() -> Set[str]:
    """Get supported languages."""
    return language_registry.supported_languages()


def get_supported_extensions() -> Set[str]:
    """Get supported file extensions."""
    return language_registry.supported_extensions()