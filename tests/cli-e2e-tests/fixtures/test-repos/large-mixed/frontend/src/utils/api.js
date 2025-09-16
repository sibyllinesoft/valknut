/**
 * API utility functions for interacting with the backend
 */

const API_BASE_URL = process.env.REACT_APP_API_URL || 'http://localhost:8000';

class ApiError extends Error {
    constructor(message, status, data = null) {
        super(message);
        this.name = 'ApiError';
        this.status = status;
        this.data = data;
    }
}

/**
 * Generic fetch wrapper with error handling
 */
async function apiRequest(endpoint, options = {}) {
    const url = `${API_BASE_URL}${endpoint}`;
    
    const defaultOptions = {
        headers: {
            'Content-Type': 'application/json',
            ...options.headers,
        },
        ...options,
    };

    try {
        const response = await fetch(url, defaultOptions);
        
        let data = null;
        const contentType = response.headers.get('content-type');
        
        if (contentType && contentType.includes('application/json')) {
            data = await response.json();
        } else {
            data = await response.text();
        }

        if (!response.ok) {
            throw new ApiError(
                data?.detail || data?.message || `HTTP ${response.status}`,
                response.status,
                data
            );
        }

        return data;
    } catch (error) {
        if (error instanceof ApiError) {
            throw error;
        }
        
        // Network or other errors
        throw new ApiError(
            'Network error. Please check your connection.',
            0,
            { originalError: error }
        );
    }
}

/**
 * Fetch all items with optional filtering
 */
export async function fetchItems(params = {}) {
    const queryParams = new URLSearchParams();
    
    Object.entries(params).forEach(([key, value]) => {
        if (value !== null && value !== undefined) {
            queryParams.append(key, value.toString());
        }
    });
    
    const queryString = queryParams.toString();
    const endpoint = `/items${queryString ? `?${queryString}` : ''}`;
    
    return apiRequest(endpoint);
}

/**
 * Fetch a single item by ID
 */
export async function fetchItem(itemId) {
    return apiRequest(`/items/${itemId}`);
}

/**
 * Create a new item
 */
export async function createItem(itemData) {
    return apiRequest('/items', {
        method: 'POST',
        body: JSON.stringify(itemData),
    });
}

/**
 * Update an existing item
 */
export async function updateItem(itemId, updateData) {
    return apiRequest(`/items/${itemId}`, {
        method: 'PUT',
        body: JSON.stringify(updateData),
    });
}

/**
 * Delete an item
 */
export async function deleteItem(itemId) {
    return apiRequest(`/items/${itemId}`, {
        method: 'DELETE',
    });
}

/**
 * Health check
 */
export async function healthCheck() {
    return apiRequest('/health');
}

/**
 * Utility function to validate item data
 */
export function validateItemData(itemData) {
    const errors = {};

    if (!itemData.name || itemData.name.trim().length === 0) {
        errors.name = 'Name is required';
    }

    if (itemData.price === undefined || itemData.price === null || itemData.price <= 0) {
        errors.price = 'Price must be a positive number';
    }

    if (!itemData.category || itemData.category.trim().length === 0) {
        errors.category = 'Category is required';
    }

    return {
        isValid: Object.keys(errors).length === 0,
        errors,
    };
}

/**
 * Utility function to format price
 */
export function formatPrice(price) {
    return new Intl.NumberFormat('en-US', {
        style: 'currency',
        currency: 'USD',
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
    }).format(price);
}

/**
 * Utility function to format date
 */
export function formatDate(dateString) {
    const date = new Date(dateString);
    return new Intl.DateTimeFormat('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    }).format(date);
}

/**
 * Debounce utility for search inputs
 */
export function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}
