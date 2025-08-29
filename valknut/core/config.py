"""
Configuration models for valknut using Pydantic v2.
"""

from pathlib import Path
from typing import Any, Literal, Optional, Union

import yaml
from pydantic import BaseModel, Field, field_validator, model_validator

from valknut.core.errors import ConfigurationError


class RootConfig(BaseModel):
    """Configuration for a source code root directory."""
    
    path: str = Field(default="./", description="Root path to scan")
    include: list[str] = Field(default_factory=lambda: ["**/*"], description="Include patterns")
    exclude: list[str] = Field(
        default_factory=lambda: [
            "**/node_modules/**",
            "**/dist/**", 
            "**/.venv/**",
            "**/venv/**",
            "**/target/**",
            "**/__pycache__/**",
            "**/.git/**",
            "**/build/**",
        ],
        description="Exclude patterns"
    )


class WeightsConfig(BaseModel):
    """Feature weights configuration."""
    
    complexity: float = Field(default=0.25, ge=0.0, le=1.0)
    clone_mass: float = Field(default=0.20, ge=0.0, le=1.0)
    centrality: float = Field(default=0.15, ge=0.0, le=1.0)
    cycles: float = Field(default=0.15, ge=0.0, le=1.0)
    type_friction: float = Field(default=0.15, ge=0.0, le=1.0)
    smell_prior: float = Field(default=0.10, ge=0.0, le=1.0)
    
    @model_validator(mode="after")
    def validate_weights(self) -> "WeightsConfig":
        """Ensure at least one weight is positive."""
        total = sum([
            self.complexity,
            self.clone_mass,
            self.centrality,
            self.cycles,
            self.type_friction,
            self.smell_prior,
        ])
        if total <= 0.0:
            raise ValueError("At least one weight must be positive")
        return self


class EchoConfig(BaseModel):
    """Configuration for echo clone detection."""
    
    enabled: bool = Field(default=True)
    min_similarity: float = Field(default=0.85, ge=0.0, le=1.0)
    min_tokens: int = Field(default=30, ge=1)


class SemgrepConfig(BaseModel):
    """Configuration for semgrep rule scanning."""
    
    enabled: bool = Field(default=False)
    rules_path: Optional[str] = Field(default=None, description="Path to custom rules")
    rule_packs: list[str] = Field(
        default_factory=lambda: ["python.lang.best-practice"],
        description="Semgrep rule packs to use"
    )


class DetectorsConfig(BaseModel):
    """Configuration for all detectors."""
    
    echo: EchoConfig = Field(default_factory=EchoConfig)
    semgrep: SemgrepConfig = Field(default_factory=SemgrepConfig)


class RankingConfig(BaseModel):
    """Configuration for ranking system."""
    
    top_k: int = Field(default=100, ge=1, description="Number of top entities to return")
    granularity: Literal["auto", "file", "function", "class"] = Field(
        default="auto",
        description="Granularity of analysis"
    )


class NormalizationConfig(BaseModel):
    """Configuration for feature normalization."""
    
    scheme: Literal["robust", "minmax", "zscore", "robust_bayesian", "minmax_bayesian", "zscore_bayesian"] = Field(
        default="robust_bayesian",
        description="Normalization scheme to use. Bayesian schemes provide intelligent fallbacks for zero-variance features."
    )
    clip_bounds: tuple[float, float] = Field(
        default=(0.0, 1.0),
        description="Bounds to clip normalized values"
    )
    use_bayesian_fallbacks: bool = Field(
        default=True,
        description="Enable Bayesian priors for zero-variance cases"
    )
    confidence_reporting: bool = Field(
        default=True,
        description="Report variance confidence metrics in logs"
    )


class CloneConfig(BaseModel):
    """Configuration for clone consolidation."""
    
    min_total_loc: int = Field(default=60, ge=10, description="Minimum total LOC for clone groups")


class ImpactPacksConfig(BaseModel):
    """Configuration for impact packs generation."""
    
    enable_cycle_packs: bool = Field(default=True, description="Enable cycle-cutting packs")
    enable_chokepoint_packs: bool = Field(default=True, description="Enable chokepoint elimination packs") 
    max_packs: int = Field(default=20, ge=1, description="Maximum number of packs to generate")
    centrality_samples: int = Field(default=64, ge=8, description="Samples for betweenness centrality")
    non_overlap: bool = Field(default=True, description="Ensure packs don't overlap entities")


class BriefsConfig(BaseModel):
    """Configuration for refactor briefs generation."""
    
    callee_depth: int = Field(default=2, ge=0, description="Depth of callee analysis")
    max_tokens_per_item: int = Field(
        default=8000,
        ge=100,
        description="Maximum tokens per brief item"
    )
    include_signatures: bool = Field(default=True)
    include_detected_refactors: bool = Field(default=True)


class ServerConfig(BaseModel):
    """Configuration for FastAPI server."""
    
    port: int = Field(default=8140, ge=1024, le=65535)
    host: str = Field(default="localhost")
    auth: Literal["none", "bearer"] = Field(default="none")
    bearer_token: Optional[str] = Field(default=None)
    
    @model_validator(mode="after")
    def validate_auth(self) -> "ServerConfig":
        """Validate auth configuration."""
        if self.auth == "bearer" and not self.bearer_token:
            raise ValueError("bearer_token required when auth=bearer")
        return self


class RefactorRankConfig(BaseModel):
    """Main configuration model for valknut."""
    
    version: int = Field(default=1, description="Configuration version")
    languages: list[str] = Field(
        default_factory=lambda: ["python", "typescript", "javascript", "rust"],
        description="Enabled languages"
    )
    roots: list[RootConfig] = Field(
        default_factory=lambda: [RootConfig()],
        description="Source code roots"
    )
    ranking: RankingConfig = Field(default_factory=RankingConfig)
    weights: WeightsConfig = Field(default_factory=WeightsConfig)
    detectors: DetectorsConfig = Field(default_factory=DetectorsConfig)
    normalize: NormalizationConfig = Field(default_factory=NormalizationConfig)
    briefs: BriefsConfig = Field(default_factory=BriefsConfig)
    clone: CloneConfig = Field(default_factory=CloneConfig)
    impact_packs: ImpactPacksConfig = Field(default_factory=ImpactPacksConfig)
    server: ServerConfig = Field(default_factory=ServerConfig)
    
    # Cache configuration
    cache_dir: str = Field(default=".valknut_cache", description="Cache directory")
    cache_ttl: int = Field(default=86400, description="Cache TTL in seconds")
    
    @field_validator("languages")
    @classmethod
    def validate_languages(cls, v: list[str]) -> list[str]:
        """Validate supported languages."""
        supported = {
            "python", "typescript", "javascript", "rust", "go", "java"
        }
        for lang in v:
            if lang not in supported:
                raise ValueError(f"Unsupported language: {lang}")
        return v
    
    @model_validator(mode="after") 
    def validate_config(self) -> "RefactorRankConfig":
        """Cross-field validation."""
        if not self.roots:
            raise ValueError("At least one root must be specified")
        return self


def load_config(config_path: Union[str, Path]) -> RefactorRankConfig:
    """
    Load configuration from YAML file.
    
    Args:
        config_path: Path to configuration file
        
    Returns:
        Parsed configuration
        
    Raises:
        ConfigurationError: If configuration is invalid
    """
    try:
        config_file = Path(config_path)
        if not config_file.exists():
            raise ConfigurationError(f"Configuration file not found: {config_path}")
        
        with config_file.open("r") as f:
            data = yaml.safe_load(f)
        
        if not data:
            data = {}
            
        return RefactorRankConfig.model_validate(data)
        
    except (yaml.YAMLError, ValueError) as e:
        raise ConfigurationError(f"Invalid configuration: {e}") from e


def save_config(config: RefactorRankConfig, config_path: Union[str, Path]) -> None:
    """
    Save configuration to YAML file.
    
    Args:
        config: Configuration to save
        config_path: Output path
    """
    try:
        config_file = Path(config_path)
        config_file.parent.mkdir(parents=True, exist_ok=True)
        
        with config_file.open("w") as f:
            yaml.safe_dump(
                config.model_dump(exclude_unset=True, exclude_defaults=True),
                f,
                default_flow_style=False,
                sort_keys=True,
            )
    except Exception as e:
        raise ConfigurationError(f"Failed to save configuration: {e}") from e


def get_default_config() -> RefactorRankConfig:
    """Get default configuration."""
    return RefactorRankConfig()