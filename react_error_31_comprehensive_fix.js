// Comprehensive React Error #31 Fix - Updated TreeNode component with proper children handling
// This component ensures no objects are passed as React children

window.CodeAnalysisTree = function CodeAnalysisTree({ data }) {
    console.log('📊 Parsed analysis data:', data);

    if (!data || !data.refactoringCandidatesByFile) {
        console.warn('No analysis data available');
        return React.createElement('div', { 
            style: { 
                padding: '2rem', 
                textAlign: 'center', 
                color: '#888' 
            } 
        }, 'No analysis data available');
    }

    return React.createElement('div', { className: 'tree-container' },
        React.createElement(TreeNode, {
            key: 'root',
            data: {
                name: 'Analysis Results',
                children: data.refactoringCandidatesByFile.map((file, index) => ({
                    name: `${file.fileName} (${file.candidates.length} candidates)`,
                    filePath: file.filePath,
                    children: file.candidates.map((candidate, candidateIndex) => ({
                        name: `${candidate.suggestion} (Score: ${candidate.refactoringScore.toFixed(2)})`,
                        priority: candidate.priority || 'Low',
                        type: candidate.suggestion,
                        complexity: candidate.complexityScore || 0,
                        isLeaf: true
                    }))
                }))
            },
            level: 0,
            isRoot: true
        })
    );
};

function TreeNode({ data, level = 0, isRoot = false }) {
    const [isExpanded, setIsExpanded] = React.useState(isRoot);
    
    if (!data) {
        return null; // Return null instead of undefined
    }

    const hasChildren = data.children && Array.isArray(data.children) && data.children.length > 0;
    
    const toggleExpanded = () => {
        if (hasChildren) {
            setIsExpanded(!isExpanded);
        }
    };

    // CRITICAL FIX: Ensure all children are valid React elements, not objects
    const renderChildren = () => {
        if (!hasChildren || !isExpanded) {
            return null;
        }

        // Filter out any falsy values and ensure each child is a valid React element
        return data.children
            .filter(Boolean) // Remove falsy values
            .filter(child => child && typeof child === 'object' && child.name) // Ensure valid structure
            .map((child, index) => {
                // CRITICAL: Each child must be a React element, not an object
                return React.createElement(TreeNode, {
                    key: `${child.name}-${index}`, // Ensure unique keys
                    data: child,
                    level: level + 1,
                    isRoot: false
                });
            });
    };

    // CRITICAL FIX: Ensure data props are serializable strings, not objects
    const safeDataProps = {
        'data-name': typeof data.name === 'string' ? data.name : String(data.name || ''),
        'data-level': String(level),
        'data-has-children': String(hasChildren),
        'data-expanded': String(isExpanded)
    };
    
    // Add file-specific data props only if they exist and are serializable
    if (data.filePath && typeof data.filePath === 'string') {
        safeDataProps['data-file-path'] = data.filePath;
    }
    if (data.priority && typeof data.priority === 'string') {
        safeDataProps['data-priority'] = data.priority;
    }
    if (typeof data.complexity === 'number') {
        safeDataProps['data-complexity'] = String(data.complexity);
    }

    const nodeStyle = {
        paddingLeft: `${level * 20}px`,
        padding: '4px 8px',
        cursor: hasChildren ? 'pointer' : 'default',
        display: 'flex',
        alignItems: 'center',
        gap: '8px'
    };

    const chevronStyle = {
        width: '12px',
        height: '12px',
        transition: 'transform 0.2s ease',
        transform: isExpanded ? 'rotate(90deg)' : 'rotate(0deg)',
        opacity: hasChildren ? 1 : 0
    };

    // CRITICAL FIX: All children must be strings or valid React elements
    const nodeContent = [
        // Chevron (only if has children)
        hasChildren ? React.createElement('span', {
            key: 'chevron',
            style: chevronStyle
        }, '▶') : null,
        
        // Node name - ensure it's always a string
        React.createElement('span', {
            key: 'name',
            style: { fontWeight: hasChildren ? '600' : '400' }
        }, String(data.name || '')), // CRITICAL: Convert to string
        
        // Priority badge (only if exists and is not leaf)
        data.priority && !data.isLeaf ? React.createElement('span', {
            key: 'priority',
            className: `tree-badge-${data.priority}`,
            style: {
                fontSize: '0.75rem',
                padding: '2px 6px',
                borderRadius: '4px',
                fontWeight: '500'
            }
        }, String(data.priority)) : null
    ].filter(Boolean); // Remove null values

    return React.createElement('div', {
        className: 'tree-node',
        ...safeDataProps
    }, [
        // Node header
        React.createElement('div', {
            key: 'header',
            className: 'tree-node-header',
            style: nodeStyle,
            onClick: toggleExpanded
        }, nodeContent), // nodeContent is already an array of valid React elements
        
        // Children container
        hasChildren && isExpanded ? React.createElement('div', {
            key: 'children',
            className: 'tree-children'
        }, renderChildren()) : null
    ].filter(Boolean)); // Remove null values
}