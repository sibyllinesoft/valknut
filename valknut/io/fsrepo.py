"""
File system repository discovery and operations.
"""

import logging
from pathlib import Path
from typing import List, Set

import pathspec
from git import InvalidGitRepositoryError, Repo

from valknut.core.registry import get_supported_extensions

logger = logging.getLogger(__name__)


class FileDiscovery:
    """Handles file discovery with include/exclude patterns."""
    
    def __init__(self) -> None:
        self.supported_extensions = get_supported_extensions()
    
    def discover(
        self,
        roots: List[str],
        include_patterns: List[str],
        exclude_patterns: List[str],
        languages: List[str],
    ) -> List[Path]:
        """
        Discover files matching the criteria.
        
        Args:
            roots: Root directories to search
            include_patterns: Include glob patterns
            exclude_patterns: Exclude glob patterns
            languages: Enabled languages
            
        Returns:
            List of discovered file paths
        """
        all_files = []
        
        # Get extensions for enabled languages
        enabled_extensions = self._get_extensions_for_languages(languages)
        
        for root_str in roots:
            root_path = Path(root_str).resolve()
            
            if not root_path.exists():
                logger.warning(f"Root path does not exist: {root_path}")
                continue
            
            if not root_path.is_dir():
                # Single file
                if self._should_include_file(root_path, include_patterns, exclude_patterns, enabled_extensions):
                    all_files.append(root_path)
                continue
            
            # Directory traversal
            files = self._discover_in_directory(
                root_path,
                include_patterns,
                exclude_patterns,
                enabled_extensions
            )
            all_files.extend(files)
        
        return all_files
    
    def _get_extensions_for_languages(self, languages: List[str]) -> Set[str]:
        """Get file extensions for enabled languages."""
        from valknut.core.registry import language_registry
        
        extensions = set()
        
        for language in languages:
            try:
                adapter = language_registry.get_adapter(language)
                extensions.update(adapter.file_extensions)
            except Exception:
                logger.warning(f"Could not get extensions for language: {language}")
        
        return extensions
    
    def _discover_in_directory(
        self,
        root_path: Path,
        include_patterns: List[str],
        exclude_patterns: List[str],
        enabled_extensions: Set[str],
    ) -> List[Path]:
        """Discover files in a directory."""
        files = []
        
        # Build pathspec for include/exclude
        include_spec = pathspec.PathSpec.from_lines('gitwildmatch', include_patterns)
        exclude_spec = pathspec.PathSpec.from_lines('gitwildmatch', exclude_patterns)
        
        # Add gitignore if available
        gitignore_spec = self._get_gitignore_spec(root_path)
        
        try:
            for file_path in root_path.rglob("*"):
                if not file_path.is_file():
                    continue
                
                # Get relative path for pattern matching
                try:
                    rel_path = file_path.relative_to(root_path)
                    rel_path_str = str(rel_path).replace("\\", "/")  # Normalize for pathspec
                except ValueError:
                    continue
                
                if self._should_include_file_with_specs(
                    file_path,
                    rel_path_str,
                    include_spec,
                    exclude_spec,
                    gitignore_spec,
                    enabled_extensions,
                ):
                    files.append(file_path)
                    
        except Exception as e:
            logger.error(f"Error discovering files in {root_path}: {e}")
        
        return files
    
    def _get_gitignore_spec(self, root_path: Path) -> pathspec.PathSpec:
        """Get gitignore pathspec if available."""
        try:
            repo = Repo(root_path, search_parent_directories=True)
            gitignore_path = Path(repo.working_dir) / ".gitignore"
            
            if gitignore_path.exists():
                with gitignore_path.open("r", encoding="utf-8", errors="ignore") as f:
                    gitignore_lines = f.readlines()
                
                return pathspec.PathSpec.from_lines('gitwildmatch', gitignore_lines)
        
        except (InvalidGitRepositoryError, Exception):
            pass
        
        return pathspec.PathSpec.from_lines('gitwildmatch', [])
    
    def _should_include_file(
        self,
        file_path: Path,
        include_patterns: List[str],
        exclude_patterns: List[str],
        enabled_extensions: Set[str],
    ) -> bool:
        """Check if a single file should be included."""
        # Check extension first
        if file_path.suffix not in enabled_extensions:
            return False
        
        # For single files, use simple pattern matching
        file_str = str(file_path)
        
        # Check exclude patterns
        exclude_spec = pathspec.PathSpec.from_lines('gitwildmatch', exclude_patterns)
        if exclude_spec.match_file(file_str):
            return False
        
        # Check include patterns
        include_spec = pathspec.PathSpec.from_lines('gitwildmatch', include_patterns)
        return include_spec.match_file(file_str)
    
    def _should_include_file_with_specs(
        self,
        file_path: Path,
        rel_path_str: str,
        include_spec: pathspec.PathSpec,
        exclude_spec: pathspec.PathSpec,
        gitignore_spec: pathspec.PathSpec,
        enabled_extensions: Set[str],
    ) -> bool:
        """Check if file should be included using pathspecs."""
        # Check extension
        if file_path.suffix not in enabled_extensions:
            return False
        
        # Check gitignore first
        if gitignore_spec.match_file(rel_path_str):
            return False
        
        # Check explicit excludes
        if exclude_spec.match_file(rel_path_str):
            return False
        
        # Check includes
        return include_spec.match_file(rel_path_str)


def is_text_file(file_path: Path) -> bool:
    """Check if a file appears to be a text file."""
    try:
        with file_path.open("rb") as f:
            chunk = f.read(1024)
            
        # Check for null bytes (common in binary files)
        if b'\x00' in chunk:
            return False
        
        # Try to decode as UTF-8
        try:
            chunk.decode('utf-8')
            return True
        except UnicodeDecodeError:
            pass
        
        # Try other common encodings
        for encoding in ['latin1', 'cp1252']:
            try:
                chunk.decode(encoding)
                return True
            except UnicodeDecodeError:
                continue
        
        return False
        
    except Exception:
        return False


def read_file_safe(file_path: Path) -> str:
    """Safely read a file with encoding detection."""
    for encoding in ['utf-8', 'latin1', 'cp1252']:
        try:
            with file_path.open('r', encoding=encoding) as f:
                return f.read()
        except UnicodeDecodeError:
            continue
        except Exception as e:
            logger.warning(f"Failed to read {file_path}: {e}")
            break
    
    return ""