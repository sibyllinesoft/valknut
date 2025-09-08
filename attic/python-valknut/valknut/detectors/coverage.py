"""
Coverage analysis detector for identifying code that lacks test coverage.

This detector parses various coverage report formats and extracts code context
for uncovered lines, providing actionable coverage improvement recommendations.
"""

import json
import logging
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple, Union
import re

from valknut.lang.common_ast import Entity, ParseIndex
from valknut.core.featureset import FeatureExtractor, FeatureVector

logger = logging.getLogger(__name__)


@dataclass
class UncoveredLine:
    """Represents an uncovered line with context."""
    line_number: int
    line_content: str
    file_path: str
    function_name: Optional[str] = None
    class_name: Optional[str] = None
    complexity_hint: Optional[str] = None  # e.g., "conditional", "loop", "exception_handler"


@dataclass 
class UncoveredBlock:
    """Represents a block of uncovered lines."""
    start_line: int
    end_line: int
    file_path: str
    lines: List[UncoveredLine] = field(default_factory=list)
    entity_id: Optional[str] = None
    block_type: Optional[str] = None  # e.g., "function", "class", "conditional"
    

@dataclass
class FileCoverage:
    """Coverage information for a single file."""
    file_path: str
    total_lines: int
    covered_lines: int
    uncovered_lines: List[int] = field(default_factory=list)
    line_coverage: Dict[int, bool] = field(default_factory=dict)  # line -> is_covered
    branch_coverage: Dict[int, Tuple[int, int]] = field(default_factory=dict)  # line -> (covered, total)
    
    @property
    def coverage_percentage(self) -> float:
        """Calculate line coverage percentage."""
        if self.total_lines == 0:
            return 0.0
        return (self.covered_lines / self.total_lines) * 100.0
    
    @property 
    def uncovered_percentage(self) -> float:
        """Calculate uncovered percentage."""
        return 100.0 - self.coverage_percentage


@dataclass
class CoverageReport:
    """Complete coverage report."""
    files: Dict[str, FileCoverage] = field(default_factory=dict)
    total_coverage_percentage: float = 0.0
    report_format: str = "unknown"
    
    def get_worst_files(self, limit: int = 10) -> List[FileCoverage]:
        """Get files with worst coverage."""
        return sorted(
            self.files.values(), 
            key=lambda f: f.coverage_percentage
        )[:limit]
    
    def get_uncovered_blocks(self, min_block_size: int = 3) -> List[UncoveredBlock]:
        """Get significant uncovered blocks across all files."""
        blocks = []
        
        for file_coverage in self.files.values():
            file_blocks = self._extract_blocks_from_file(file_coverage, min_block_size)
            blocks.extend(file_blocks)
        
        return sorted(blocks, key=lambda b: len(b.lines), reverse=True)
    
    def _extract_blocks_from_file(self, file_coverage: FileCoverage, min_size: int) -> List[UncoveredBlock]:
        """Extract uncovered blocks from a file."""
        if not file_coverage.uncovered_lines:
            return []
        
        blocks = []
        current_block = []
        
        # Sort uncovered lines
        sorted_lines = sorted(file_coverage.uncovered_lines)
        
        for i, line_num in enumerate(sorted_lines):
            # Check if this line is consecutive to the previous
            if current_block and line_num != sorted_lines[i-1] + 1:
                # End current block if it's large enough
                if len(current_block) >= min_size:
                    blocks.append(UncoveredBlock(
                        start_line=current_block[0],
                        end_line=current_block[-1], 
                        file_path=file_coverage.file_path
                    ))
                current_block = []
            
            current_block.append(line_num)
        
        # Handle final block
        if len(current_block) >= min_size:
            blocks.append(UncoveredBlock(
                start_line=current_block[0],
                end_line=current_block[-1],
                file_path=file_coverage.file_path
            ))
        
        return blocks


class CoverageReportParser:
    """Parses various coverage report formats."""
    
    def __init__(self):
        self.parsers = {
            'coverage.py': self._parse_coverage_py,
            'lcov': self._parse_lcov,
            'jacoco': self._parse_jacoco,
            'cobertura': self._parse_cobertura,
            'istanbul': self._parse_istanbul,
        }
    
    def parse(self, report_path: Path, format_hint: Optional[str] = None) -> Optional[CoverageReport]:
        """
        Parse coverage report from file.
        
        Args:
            report_path: Path to coverage report
            format_hint: Optional hint about format ('coverage.py', 'lcov', etc.)
            
        Returns:
            Parsed coverage report or None if parsing failed
        """
        if not report_path.exists():
            logger.warning(f"Coverage report not found: {report_path}")
            return None
        
        if format_hint and format_hint in self.parsers:
            logger.info(f"Parsing {format_hint} coverage report: {report_path}")
            return self.parsers[format_hint](report_path)
        
        # Auto-detect format based on file content/extension
        detected_format = self._detect_format(report_path)
        if detected_format and detected_format in self.parsers:
            logger.info(f"Detected {detected_format} coverage report: {report_path}")
            return self.parsers[detected_format](report_path)
        
        logger.error(f"Could not detect coverage report format: {report_path}")
        return None
    
    def _detect_format(self, report_path: Path) -> Optional[str]:
        """Auto-detect coverage report format."""
        try:
            # Check by filename patterns
            name = report_path.name.lower()
            if name.endswith('.lcov') or name == 'lcov.info':
                return 'lcov'
            elif name.endswith('.xml'):
                # Peek at XML content to distinguish between formats
                with open(report_path, 'r', encoding='utf-8') as f:
                    content = f.read(1000)  # First 1000 chars
                    if 'jacoco' in content or 'org.jacoco' in content:
                        return 'jacoco'
                    elif 'cobertura' in content or 'http://cobertura.sourceforge.net' in content:
                        return 'cobertura'
            elif name.endswith('.json'):
                # Check JSON structure
                with open(report_path, 'r', encoding='utf-8') as f:
                    try:
                        data = json.load(f)
                        if isinstance(data, dict):
                            # Istanbul/nyc format
                            if any('statementMap' in v for v in data.values() if isinstance(v, dict)):
                                return 'istanbul'
                            # coverage.py JSON format
                            elif 'files' in data and 'totals' in data:
                                return 'coverage.py'
                    except json.JSONDecodeError:
                        pass
        except Exception as e:
            logger.debug(f"Error detecting format for {report_path}: {e}")
        
        return None
    
    def _parse_coverage_py(self, report_path: Path) -> Optional[CoverageReport]:
        """Parse coverage.py JSON report."""
        try:
            with open(report_path, 'r', encoding='utf-8') as f:
                data = json.load(f)
            
            report = CoverageReport(report_format='coverage.py')
            
            # Parse totals
            if 'totals' in data:
                totals = data['totals']
                if 'percent_covered' in totals:
                    report.total_coverage_percentage = totals['percent_covered']
            
            # Parse files
            if 'files' in data:
                for file_path, file_data in data['files'].items():
                    file_coverage = FileCoverage(
                        file_path=file_path,
                        total_lines=file_data.get('summary', {}).get('num_statements', 0),
                        covered_lines=file_data.get('summary', {}).get('covered_lines', 0)
                    )
                    
                    # Parse line-by-line coverage
                    if 'executed_lines' in file_data:
                        for line in file_data['executed_lines']:
                            file_coverage.line_coverage[line] = True
                    
                    if 'missing_lines' in file_data:
                        for line in file_data['missing_lines']:
                            file_coverage.line_coverage[line] = False
                            file_coverage.uncovered_lines.append(line)
                    
                    # Parse branch coverage if available
                    if 'executed_branches' in file_data:
                        for branch_data in file_data['executed_branches']:
                            # Branch format: [line, branch_id] 
                            if len(branch_data) >= 1:
                                line = branch_data[0]
                                if line not in file_coverage.branch_coverage:
                                    file_coverage.branch_coverage[line] = (0, 0)
                                covered, total = file_coverage.branch_coverage[line]
                                file_coverage.branch_coverage[line] = (covered + 1, total + 1)
                    
                    if 'missing_branches' in file_data:
                        for branch_data in file_data['missing_branches']:
                            if len(branch_data) >= 1:
                                line = branch_data[0]
                                if line not in file_coverage.branch_coverage:
                                    file_coverage.branch_coverage[line] = (0, 0)
                                covered, total = file_coverage.branch_coverage[line]
                                file_coverage.branch_coverage[line] = (covered, total + 1)
                    
                    report.files[file_path] = file_coverage
            
            return report
        except Exception as e:
            logger.error(f"Failed to parse coverage.py report {report_path}: {e}")
            return None
    
    def _parse_lcov(self, report_path: Path) -> Optional[CoverageReport]:
        """Parse LCOV format report."""
        try:
            with open(report_path, 'r', encoding='utf-8') as f:
                content = f.read()
            
            report = CoverageReport(report_format='lcov')
            current_file = None
            
            for line in content.strip().split('\n'):
                line = line.strip()
                
                if line.startswith('SF:'):
                    # Source file
                    file_path = line[3:]
                    current_file = FileCoverage(file_path=file_path, total_lines=0, covered_lines=0)
                    
                elif line.startswith('DA:') and current_file:
                    # Line data: DA:line_number,hit_count
                    parts = line[3:].split(',')
                    if len(parts) >= 2:
                        line_num = int(parts[0])
                        hit_count = int(parts[1])
                        current_file.total_lines += 1
                        
                        if hit_count > 0:
                            current_file.covered_lines += 1
                            current_file.line_coverage[line_num] = True
                        else:
                            current_file.line_coverage[line_num] = False
                            current_file.uncovered_lines.append(line_num)
                
                elif line.startswith('BRDA:') and current_file:
                    # Branch data: BRDA:line,block,branch,taken
                    parts = line[5:].split(',')
                    if len(parts) >= 4:
                        line_num = int(parts[0])
                        taken = parts[3]
                        
                        if line_num not in current_file.branch_coverage:
                            current_file.branch_coverage[line_num] = (0, 0)
                        
                        covered, total = current_file.branch_coverage[line_num]
                        total += 1
                        if taken != '-' and int(taken) > 0:
                            covered += 1
                        current_file.branch_coverage[line_num] = (covered, total)
                
                elif line == 'end_of_record' and current_file:
                    report.files[current_file.file_path] = current_file
                    current_file = None
            
            # Calculate overall coverage
            total_lines = sum(f.total_lines for f in report.files.values())
            total_covered = sum(f.covered_lines for f in report.files.values())
            if total_lines > 0:
                report.total_coverage_percentage = (total_covered / total_lines) * 100.0
            
            return report
        except Exception as e:
            logger.error(f"Failed to parse LCOV report {report_path}: {e}")
            return None
    
    def _parse_jacoco(self, report_path: Path) -> Optional[CoverageReport]:
        """Parse JaCoCo XML report."""
        try:
            tree = ET.parse(report_path)
            root = tree.getroot()
            
            report = CoverageReport(report_format='jacoco')
            
            # Parse packages and source files
            for package in root.findall('.//package'):
                package_name = package.get('name', '')
                
                for sourcefile in package.findall('.//sourcefile'):
                    filename = sourcefile.get('name', '')
                    file_path = f"{package_name.replace('/', '/')}/{filename}" if package_name else filename
                    
                    file_coverage = FileCoverage(file_path=file_path, total_lines=0, covered_lines=0)
                    
                    # Parse line coverage
                    for line in sourcefile.findall('.//line'):
                        line_num = int(line.get('nr', 0))
                        instruction_covered = int(line.get('ci', 0))
                        instruction_missed = int(line.get('mi', 0))
                        
                        if instruction_covered + instruction_missed > 0:
                            file_coverage.total_lines += 1
                            
                            if instruction_covered > 0:
                                file_coverage.covered_lines += 1
                                file_coverage.line_coverage[line_num] = True
                            else:
                                file_coverage.line_coverage[line_num] = False
                                file_coverage.uncovered_lines.append(line_num)
                        
                        # Parse branch coverage
                        branch_covered = int(line.get('cb', 0))
                        branch_missed = int(line.get('mb', 0))
                        if branch_covered + branch_missed > 0:
                            file_coverage.branch_coverage[line_num] = (branch_covered, branch_covered + branch_missed)
                    
                    if file_coverage.total_lines > 0:
                        report.files[file_path] = file_coverage
            
            # Calculate overall coverage from report counter
            for counter in root.findall('.//counter[@type="LINE"]'):
                covered = int(counter.get('covered', 0))
                missed = int(counter.get('missed', 0))
                if covered + missed > 0:
                    report.total_coverage_percentage = (covered / (covered + missed)) * 100.0
                break  # Use first LINE counter found
            
            return report
        except Exception as e:
            logger.error(f"Failed to parse JaCoCo report {report_path}: {e}")
            return None
    
    def _parse_cobertura(self, report_path: Path) -> Optional[CoverageReport]:
        """Parse Cobertura XML report."""
        try:
            tree = ET.parse(report_path)
            root = tree.getroot()
            
            report = CoverageReport(report_format='cobertura')
            
            # Get overall line rate
            line_rate = float(root.get('line-rate', 0))
            report.total_coverage_percentage = line_rate * 100.0
            
            # Parse packages and classes
            for package in root.findall('.//package'):
                for class_elem in package.findall('.//class'):
                    filename = class_elem.get('filename', '')
                    if not filename:
                        continue
                    
                    file_coverage = report.files.get(filename)
                    if not file_coverage:
                        file_coverage = FileCoverage(file_path=filename, total_lines=0, covered_lines=0)
                        report.files[filename] = file_coverage
                    
                    # Parse line coverage
                    for line in class_elem.findall('.//line'):
                        line_num = int(line.get('number', 0))
                        hits = int(line.get('hits', 0))
                        
                        file_coverage.total_lines += 1
                        
                        if hits > 0:
                            file_coverage.covered_lines += 1
                            file_coverage.line_coverage[line_num] = True
                        else:
                            file_coverage.line_coverage[line_num] = False
                            file_coverage.uncovered_lines.append(line_num)
                        
                        # Parse branch coverage
                        branch = line.get('branch', 'false')
                        if branch == 'true':
                            condition_coverage = line.get('condition-coverage', '')
                            if condition_coverage:
                                # Parse "50% (1/2)" format
                                match = re.search(r'\((\d+)/(\d+)\)', condition_coverage)
                                if match:
                                    covered = int(match.group(1))
                                    total = int(match.group(2))
                                    file_coverage.branch_coverage[line_num] = (covered, total)
            
            return report
        except Exception as e:
            logger.error(f"Failed to parse Cobertura report {report_path}: {e}")
            return None
    
    def _parse_istanbul(self, report_path: Path) -> Optional[CoverageReport]:
        """Parse Istanbul/nyc JSON report."""
        try:
            with open(report_path, 'r', encoding='utf-8') as f:
                data = json.load(f)
            
            report = CoverageReport(report_format='istanbul')
            
            for file_path, file_data in data.items():
                if not isinstance(file_data, dict):
                    continue
                
                file_coverage = FileCoverage(file_path=file_path, total_lines=0, covered_lines=0)
                
                # Parse statement coverage
                if 's' in file_data:  # statement coverage counts
                    statement_map = file_data.get('statementMap', {})
                    for stmt_id, count in file_data['s'].items():
                        if stmt_id in statement_map:
                            location = statement_map[stmt_id]
                            if 'start' in location:
                                line_num = location['start']['line']
                                file_coverage.total_lines = max(file_coverage.total_lines, line_num)
                                
                                if count > 0:
                                    file_coverage.covered_lines += 1
                                    file_coverage.line_coverage[line_num] = True
                                else:
                                    file_coverage.line_coverage[line_num] = False
                                    file_coverage.uncovered_lines.append(line_num)
                
                # Parse branch coverage
                if 'b' in file_data:
                    branch_map = file_data.get('branchMap', {})
                    for branch_id, branch_counts in file_data['b'].items():
                        if branch_id in branch_map and isinstance(branch_counts, list):
                            location = branch_map[branch_id]['loc']
                            if 'start' in location:
                                line_num = location['start']['line']
                                total_branches = len(branch_counts)
                                covered_branches = sum(1 for c in branch_counts if c > 0)
                                file_coverage.branch_coverage[line_num] = (covered_branches, total_branches)
                
                # Recalculate covered lines (avoid duplicates)
                file_coverage.covered_lines = sum(1 for covered in file_coverage.line_coverage.values() if covered)
                
                if file_coverage.total_lines > 0:
                    report.files[file_path] = file_coverage
            
            # Calculate overall coverage
            total_lines = sum(f.total_lines for f in report.files.values())
            total_covered = sum(f.covered_lines for f in report.files.values())
            if total_lines > 0:
                report.total_coverage_percentage = (total_covered / total_lines) * 100.0
            
            return report
        except Exception as e:
            logger.error(f"Failed to parse Istanbul report {report_path}: {e}")
            return None


class CoverageContextExtractor:
    """Extracts code context for uncovered lines."""
    
    def __init__(self):
        self.context_window = 3  # Lines of context around uncovered code
    
    def extract_context(
        self, 
        uncovered_blocks: List[UncoveredBlock], 
        parse_index: ParseIndex
    ) -> List[UncoveredBlock]:
        """
        Enhance uncovered blocks with code context and entity information.
        
        Args:
            uncovered_blocks: List of uncovered code blocks
            parse_index: Parsed code index for context extraction
            
        Returns:
            Enhanced blocks with code context
        """
        enhanced_blocks = []
        
        for block in uncovered_blocks:
            try:
                enhanced_block = self._enhance_block_with_context(block, parse_index)
                enhanced_blocks.append(enhanced_block)
            except Exception as e:
                logger.warning(f"Failed to enhance block {block.file_path}:{block.start_line}: {e}")
                enhanced_blocks.append(block)  # Keep original block
        
        return enhanced_blocks
    
    def _enhance_block_with_context(self, block: UncoveredBlock, parse_index: ParseIndex) -> UncoveredBlock:
        """Enhance a single block with context."""
        # Find the source file
        source_file = None
        for entity in parse_index.entities.values():
            if entity.file_path and Path(entity.file_path).name == Path(block.file_path).name:
                source_file = entity.file_path
                break
        
        if not source_file or not Path(source_file).exists():
            logger.debug(f"Source file not found for {block.file_path}")
            return block
        
        # Read file content
        try:
            with open(source_file, 'r', encoding='utf-8', errors='ignore') as f:
                file_lines = f.readlines()
        except Exception as e:
            logger.warning(f"Could not read {source_file}: {e}")
            return block
        
        # Extract uncovered lines with context
        enhanced_lines = []
        for line_num in range(block.start_line, block.end_line + 1):
            if 1 <= line_num <= len(file_lines):
                line_content = file_lines[line_num - 1].rstrip('\n\r')
                
                # Find containing entity
                entity = self._find_containing_entity(line_num, source_file, parse_index)
                
                # Analyze line complexity
                complexity_hint = self._analyze_line_complexity(line_content)
                
                uncovered_line = UncoveredLine(
                    line_number=line_num,
                    line_content=line_content,
                    file_path=block.file_path,
                    function_name=entity.name if entity else None,
                    class_name=self._get_class_name(entity) if entity else None,
                    complexity_hint=complexity_hint
                )
                enhanced_lines.append(uncovered_line)
        
        # Create enhanced block
        containing_entity = self._find_containing_entity(block.start_line, source_file, parse_index)
        enhanced_block = UncoveredBlock(
            start_line=block.start_line,
            end_line=block.end_line,
            file_path=block.file_path,
            lines=enhanced_lines,
            entity_id=containing_entity.id if containing_entity else None,
            block_type=self._classify_block_type(enhanced_lines)
        )
        
        return enhanced_block
    
    def _find_containing_entity(self, line_num: int, file_path: str, parse_index: ParseIndex) -> Optional[Entity]:
        """Find the entity (function/class) that contains the given line."""
        for entity in parse_index.entities.values():
            if (entity.file_path == file_path and 
                entity.start_line and entity.end_line and
                entity.start_line <= line_num <= entity.end_line):
                return entity
        return None
    
    def _get_class_name(self, entity: Entity) -> Optional[str]:
        """Extract class name from entity if it's a method."""
        if entity and entity.qualified_name and '.' in entity.qualified_name:
            parts = entity.qualified_name.split('.')
            if len(parts) >= 2:
                return parts[-2]  # Class name is second to last
        return None
    
    def _analyze_line_complexity(self, line_content: str) -> Optional[str]:
        """Analyze line for complexity hints."""
        line = line_content.strip().lower()
        
        if any(keyword in line for keyword in ['if ', 'elif ', 'else:', 'while ', 'for ']):
            return "conditional"
        elif any(keyword in line for keyword in ['try:', 'except:', 'except ', 'finally:', 'raise']):
            return "exception_handler"
        elif any(keyword in line for keyword in ['def ', 'class ', 'async def']):
            return "definition"
        elif any(keyword in line for keyword in ['return', 'yield', 'break', 'continue']):
            return "control_flow"
        elif any(op in line for op in ['and ', 'or ', 'not ']):
            return "logical_operation"
        elif line.endswith(':'):
            return "block_start"
        
        return None
    
    def _classify_block_type(self, lines: List[UncoveredLine]) -> Optional[str]:
        """Classify the type of uncovered block."""
        if not lines:
            return None
        
        # Check for function definitions
        for line in lines:
            if line.complexity_hint == "definition":
                return "function"
        
        # Check for conditional blocks
        conditional_count = sum(1 for line in lines if line.complexity_hint == "conditional")
        if conditional_count > 0:
            return "conditional"
        
        # Check for exception handling
        exception_count = sum(1 for line in lines if line.complexity_hint == "exception_handler")
        if exception_count > 0:
            return "exception_handler"
        
        # Default classification
        if len(lines) >= 10:
            return "large_block"
        elif len(lines) >= 5:
            return "medium_block"
        else:
            return "small_block"


class CoverageExtractor(FeatureExtractor):
    """Feature extractor that adds coverage-related features to entities."""
    
    def __init__(self, coverage_report: CoverageReport):
        self.coverage_report = coverage_report
        self.feature_name_prefix = "coverage"
        
    @property
    def name(self) -> str:
        return "coverage"
    
    def extract_features(self, entity: Entity, index: ParseIndex) -> Dict[str, float]:
        """Extract coverage features for an entity."""
        features = {}
        
        if not entity.file_path:
            # No file path, return default coverage features
            return {
                'coverage_percentage': 0.5,  # Neutral value
                'uncovered_lines_count': 0.5,
                'uncovered_blocks_count': 0.5,
                'branch_coverage_percentage': 0.5,
                'coverage_priority_score': 0.5,
            }
        
        # Find coverage data for this entity's file
        file_coverage = None
        entity_file_path = str(entity.file_path)
        
        # Try exact match first
        if entity_file_path in self.coverage_report.files:
            file_coverage = self.coverage_report.files[entity_file_path]
        else:
            # Try partial matching (coverage reports often have different path formats)
            entity_filename = Path(entity_file_path).name
            for report_path, coverage in self.coverage_report.files.items():
                if Path(report_path).name == entity_filename:
                    file_coverage = coverage
                    break
        
        if not file_coverage:
            # No coverage data available
            return {
                'coverage_percentage': 0.5,
                'uncovered_lines_count': 0.5, 
                'uncovered_blocks_count': 0.5,
                'branch_coverage_percentage': 0.5,
                'coverage_priority_score': 0.5,
            }
        
        # Calculate entity-specific coverage
        entity_coverage = self._calculate_entity_coverage(entity, file_coverage)
        
        # Extract features
        coverage_pct = entity_coverage['coverage_percentage'] / 100.0  # Normalize to 0-1
        uncovered_count = min(entity_coverage['uncovered_lines_count'] / 20.0, 1.0)  # Cap at 20 lines
        uncovered_blocks = min(entity_coverage['uncovered_blocks_count'] / 5.0, 1.0)  # Cap at 5 blocks
        branch_coverage = entity_coverage['branch_coverage_percentage'] / 100.0
        
        # Priority score: inverse of coverage, weighted by entity importance
        priority_score = (1.0 - coverage_pct) * self._calculate_entity_importance(entity)
        
        return {
            'coverage_percentage': coverage_pct,
            'uncovered_lines_count': uncovered_count,
            'uncovered_blocks_count': uncovered_blocks, 
            'branch_coverage_percentage': branch_coverage,
            'coverage_priority_score': priority_score,
        }
    
    def _calculate_entity_coverage(self, entity: Entity, file_coverage: FileCoverage) -> Dict[str, float]:
        """Calculate coverage metrics specific to an entity."""
        if not entity.start_line or not entity.end_line:
            # Can't determine entity boundaries
            return {
                'coverage_percentage': file_coverage.coverage_percentage,
                'uncovered_lines_count': len(file_coverage.uncovered_lines),
                'uncovered_blocks_count': 0,
                'branch_coverage_percentage': 0.0,
            }
        
        # Count covered/uncovered lines within entity boundaries
        entity_total = 0
        entity_covered = 0
        entity_uncovered_lines = []
        entity_branch_covered = 0
        entity_branch_total = 0
        
        for line_num in range(entity.start_line, entity.end_line + 1):
            if line_num in file_coverage.line_coverage:
                entity_total += 1
                if file_coverage.line_coverage[line_num]:
                    entity_covered += 1
                else:
                    entity_uncovered_lines.append(line_num)
            
            # Check branch coverage for this line
            if line_num in file_coverage.branch_coverage:
                covered, total = file_coverage.branch_coverage[line_num]
                entity_branch_covered += covered
                entity_branch_total += total
        
        # Calculate coverage percentage
        coverage_pct = 100.0
        if entity_total > 0:
            coverage_pct = (entity_covered / entity_total) * 100.0
        
        # Calculate branch coverage percentage
        branch_coverage_pct = 100.0
        if entity_branch_total > 0:
            branch_coverage_pct = (entity_branch_covered / entity_branch_total) * 100.0
        
        # Count uncovered blocks within entity
        uncovered_blocks_count = self._count_uncovered_blocks_in_range(
            entity_uncovered_lines, 
            entity.start_line, 
            entity.end_line
        )
        
        return {
            'coverage_percentage': coverage_pct,
            'uncovered_lines_count': len(entity_uncovered_lines),
            'uncovered_blocks_count': uncovered_blocks_count,
            'branch_coverage_percentage': branch_coverage_pct,
        }
    
    def _count_uncovered_blocks_in_range(
        self, 
        uncovered_lines: List[int], 
        start_line: int, 
        end_line: int
    ) -> int:
        """Count number of uncovered blocks within a line range."""
        if not uncovered_lines:
            return 0
        
        # Filter lines within range and sort
        range_lines = sorted([line for line in uncovered_lines if start_line <= line <= end_line])
        
        if not range_lines:
            return 0
        
        # Count consecutive blocks
        blocks = 0
        in_block = False
        
        for i, line in enumerate(range_lines):
            if i == 0 or line != range_lines[i-1] + 1:
                # Start of new block
                if not in_block:
                    blocks += 1
                    in_block = True
            # Continue existing block
        
        return blocks
    
    def _calculate_entity_importance(self, entity: Entity) -> float:
        """Calculate importance score for an entity (for prioritization)."""
        importance = 0.5  # Base importance
        
        # Functions/methods are more important than files
        if entity.kind.name in ['FUNCTION', 'METHOD']:
            importance += 0.3
        elif entity.kind.name == 'CLASS':
            importance += 0.2
        
        # Longer entities might be more important
        if entity.start_line and entity.end_line:
            lines = entity.end_line - entity.start_line + 1
            if lines > 50:
                importance += 0.1
            elif lines > 100:
                importance += 0.2
        
        # Public entities are more important (simple heuristic)
        if entity.name and not entity.name.startswith('_'):
            importance += 0.1
        
        return min(importance, 1.0)


def create_coverage_extractor(coverage_report_path: Path, format_hint: Optional[str] = None) -> Optional[CoverageExtractor]:
    """
    Create a coverage feature extractor from a coverage report.
    
    Args:
        coverage_report_path: Path to coverage report file
        format_hint: Optional format hint ('coverage.py', 'lcov', etc.)
        
    Returns:
        CoverageExtractor instance or None if parsing failed
    """
    parser = CoverageReportParser()
    coverage_report = parser.parse(coverage_report_path, format_hint)
    
    if coverage_report:
        logger.info(
            f"Loaded coverage report: {len(coverage_report.files)} files, "
            f"{coverage_report.total_coverage_percentage:.1f}% overall coverage"
        )
        return CoverageExtractor(coverage_report)
    
    return None