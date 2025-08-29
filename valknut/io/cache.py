"""
Disk cache for AST/graphs/features using diskcache.
"""

import hashlib
import logging
import pickle
from pathlib import Path
from typing import Any, Optional

import diskcache

from valknut.core.errors import CacheError
from valknut.lang.common_ast import ParseIndex

logger = logging.getLogger(__name__)


class CacheManager:
    """Manages disk cache for analysis results."""
    
    def __init__(self, cache_dir: str, ttl: int = 86400) -> None:
        """
        Initialize cache manager.
        
        Args:
            cache_dir: Directory for cache storage
            ttl: Time to live in seconds
        """
        self.cache_dir = Path(cache_dir)
        self.ttl = ttl
        
        # Create cache directory
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        
        # Initialize diskcache
        try:
            self.cache = diskcache.Cache(str(self.cache_dir))
        except Exception as e:
            logger.error(f"Failed to initialize cache: {e}")
            raise CacheError(f"Failed to initialize cache: {e}") from e
    
    def close(self) -> None:
        """Close cache connection."""
        if hasattr(self, 'cache'):
            self.cache.close()
    
    def __enter__(self) -> "CacheManager":
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()
    
    async def get_parse_index(self, cache_key: str) -> Optional[ParseIndex]:
        """
        Get cached parse index.
        
        Args:
            cache_key: Cache key
            
        Returns:
            Cached parse index or None if not found
        """
        try:
            key = f"parse_index:{cache_key}"
            data = self.cache.get(key)
            
            if data is not None:
                logger.debug(f"Cache hit for parse index: {cache_key}")
                return pickle.loads(data)
            
            logger.debug(f"Cache miss for parse index: {cache_key}")
            return None
            
        except Exception as e:
            logger.warning(f"Failed to get cached parse index: {e}")
            return None
    
    async def set_parse_index(self, cache_key: str, index: ParseIndex) -> None:
        """
        Cache parse index.
        
        Args:
            cache_key: Cache key
            index: Parse index to cache
        """
        try:
            key = f"parse_index:{cache_key}"
            data = pickle.dumps(index)
            
            self.cache.set(key, data, expire=self.ttl)
            logger.debug(f"Cached parse index: {cache_key}")
            
        except Exception as e:
            logger.warning(f"Failed to cache parse index: {e}")
    
    async def get_features(self, entity_id: str) -> Optional[dict]:
        """
        Get cached features for entity.
        
        Args:
            entity_id: Entity ID
            
        Returns:
            Cached features or None if not found
        """
        try:
            key = f"features:{self._hash_key(entity_id)}"
            return self.cache.get(key)
            
        except Exception as e:
            logger.warning(f"Failed to get cached features for {entity_id}: {e}")
            return None
    
    async def set_features(self, entity_id: str, features: dict) -> None:
        """
        Cache features for entity.
        
        Args:
            entity_id: Entity ID
            features: Features to cache
        """
        try:
            key = f"features:{self._hash_key(entity_id)}"
            self.cache.set(key, features, expire=self.ttl)
            
        except Exception as e:
            logger.warning(f"Failed to cache features for {entity_id}: {e}")
    
    async def get_analysis_result(self, result_id: str) -> Optional[Any]:
        """
        Get cached analysis result.
        
        Args:
            result_id: Result ID
            
        Returns:
            Cached result or None if not found
        """
        try:
            key = f"result:{result_id}"
            data = self.cache.get(key)
            
            if data is not None:
                return pickle.loads(data)
            
            return None
            
        except Exception as e:
            logger.warning(f"Failed to get cached result: {e}")
            return None
    
    async def set_analysis_result(self, result_id: str, result: Any) -> None:
        """
        Cache analysis result.
        
        Args:
            result_id: Result ID
            result: Result to cache
        """
        try:
            key = f"result:{result_id}"
            data = pickle.dumps(result)
            
            self.cache.set(key, data, expire=self.ttl * 7)  # Keep results longer
            
        except Exception as e:
            logger.warning(f"Failed to cache result: {e}")
    
    def clear_all(self) -> None:
        """Clear all cached data."""
        try:
            self.cache.clear()
            logger.info("Cleared all cache data")
        except Exception as e:
            logger.error(f"Failed to clear cache: {e}")
    
    def clear_expired(self) -> None:
        """Clear expired cache entries."""
        try:
            self.cache.expire()
            logger.info("Cleared expired cache entries")
        except Exception as e:
            logger.warning(f"Failed to clear expired entries: {e}")
    
    def get_stats(self) -> dict:
        """Get cache statistics."""
        try:
            return {
                "size": len(self.cache),
                "volume": self.cache.volume(),
                "directory": str(self.cache_dir),
            }
        except Exception:
            return {}
    
    def _hash_key(self, key: str) -> str:
        """Generate hash for cache key."""
        return hashlib.sha256(key.encode()).hexdigest()[:16]


class NullCache:
    """Null cache implementation for when caching is disabled."""
    
    async def get_parse_index(self, cache_key: str) -> Optional[ParseIndex]:
        return None
    
    async def set_parse_index(self, cache_key: str, index: ParseIndex) -> None:
        pass
    
    async def get_features(self, entity_id: str) -> Optional[dict]:
        return None
    
    async def set_features(self, entity_id: str, features: dict) -> None:
        pass
    
    async def get_analysis_result(self, result_id: str) -> Optional[Any]:
        return None
    
    async def set_analysis_result(self, result_id: str, result: Any) -> None:
        pass
    
    def clear_all(self) -> None:
        pass
    
    def clear_expired(self) -> None:
        pass
    
    def get_stats(self) -> dict:
        return {"size": 0, "volume": 0, "directory": "none"}
    
    def close(self) -> None:
        pass