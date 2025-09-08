/**
 * Utility functions with various code quality issues
 */

export interface ConfigOptions {
    timeout?: number;
    retries?: number;
    validateInput?: boolean;
    debug?: boolean;
}

export interface ValidationResult {
    isValid: boolean;
    errors: string[];
    warnings: string[];
}

// Large utility class with multiple responsibilities (God Object)
export class MegaUtility {
    private static instance: MegaUtility;
    private config: ConfigOptions = {};
    private cache = new Map<string, any>();
    
    constructor(config: ConfigOptions = {}) {
        this.config = { timeout: 5000, retries: 3, validateInput: true, debug: false, ...config };
    }
    
    static getInstance(config?: ConfigOptions): MegaUtility {
        if (!MegaUtility.instance) {
            MegaUtility.instance = new MegaUtility(config);
        }
        return MegaUtility.instance;
    }
    
    // Complex string validation with repeated patterns
    validateEmail(email: string): ValidationResult {
        const errors: string[] = [];
        const warnings: string[] = [];
        
        if (!email) {
            errors.push('Email is required');
            return { isValid: false, errors, warnings };
        }
        
        if (typeof email !== 'string') {
            errors.push('Email must be a string');
            return { isValid: false, errors, warnings };
        }
        
        email = email.trim().toLowerCase();
        
        if (email.length === 0) {
            errors.push('Email cannot be empty');
            return { isValid: false, errors, warnings };
        }
        
        if (email.length > 254) {
            errors.push('Email too long');
            return { isValid: false, errors, warnings };
        }
        
        const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
        if (!emailRegex.test(email)) {
            errors.push('Invalid email format');
            return { isValid: false, errors, warnings };
        }
        
        const [localPart, domain] = email.split('@');
        
        if (localPart.length > 64) {
            warnings.push('Local part is very long');
        }
        
        if (domain.startsWith('.') || domain.endsWith('.')) {
            errors.push('Domain cannot start or end with a dot');
            return { isValid: false, errors, warnings };
        }
        
        if (domain.includes('..')) {
            errors.push('Domain cannot contain consecutive dots');
            return { isValid: false, errors, warnings };
        }
        
        return { isValid: true, errors, warnings };
    }
    
    // Similar validation pattern - code duplication
    validatePhone(phone: string): ValidationResult {
        const errors: string[] = [];
        const warnings: string[] = [];
        
        if (!phone) {
            errors.push('Phone is required');
            return { isValid: false, errors, warnings };
        }
        
        if (typeof phone !== 'string') {
            errors.push('Phone must be a string');
            return { isValid: false, errors, warnings };
        }
        
        phone = phone.trim();
        
        if (phone.length === 0) {
            errors.push('Phone cannot be empty');
            return { isValid: false, errors, warnings };
        }
        
        // Remove formatting characters
        const cleanPhone = phone.replace(/[^\d+]/g, '');
        
        if (cleanPhone.length < 10) {
            errors.push('Phone number too short');
            return { isValid: false, errors, warnings };
        }
        
        if (cleanPhone.length > 15) {
            errors.push('Phone number too long');
            return { isValid: false, errors, warnings };
        }
        
        const phoneRegex = /^\+?[\d\s\-\(\)\.]+$/;
        if (!phoneRegex.test(phone)) {
            errors.push('Invalid phone format');
            return { isValid: false, errors, warnings };
        }
        
        if (cleanPhone.startsWith('0')) {
            warnings.push('Phone number starts with 0');
        }
        
        return { isValid: true, errors, warnings };
    }
    
    // Another similar validation - more duplication
    validateUsername(username: string): ValidationResult {
        const errors: string[] = [];
        const warnings: string[] = [];
        
        if (!username) {
            errors.push('Username is required');
            return { isValid: false, errors, warnings };
        }
        
        if (typeof username !== 'string') {
            errors.push('Username must be a string');
            return { isValid: false, errors, warnings };
        }
        
        username = username.trim();
        
        if (username.length === 0) {
            errors.push('Username cannot be empty');
            return { isValid: false, errors, warnings };
        }
        
        if (username.length < 3) {
            errors.push('Username too short');
            return { isValid: false, errors, warnings };
        }
        
        if (username.length > 30) {
            errors.push('Username too long');
            return { isValid: false, errors, warnings };
        }
        
        const usernameRegex = /^[a-zA-Z0-9_-]+$/;
        if (!usernameRegex.test(username)) {
            errors.push('Username contains invalid characters');
            return { isValid: false, errors, warnings };
        }
        
        if (username.startsWith('_') || username.startsWith('-')) {
            warnings.push('Username starts with special character');
        }
        
        if (username.endsWith('_') || username.endsWith('-')) {
            warnings.push('Username ends with special character');
        }
        
        return { isValid: true, errors, warnings };
    }
    
    // Complex async function with multiple responsibilities
    async fetchAndProcessData(
        url: string, 
        processCallback?: (data: any) => any,
        options: { timeout?: number; retries?: number } = {}
    ): Promise<any> {
        const finalOptions = { ...this.config, ...options };
        let lastError: Error | null = null;
        
        for (let attempt = 1; attempt <= finalOptions.retries!; attempt++) {
            try {
                if (finalOptions.debug) {
                    console.log(`Attempt ${attempt} to fetch ${url}`);
                }
                
                // Simulate fetch with timeout
                const fetchPromise = this.simulateFetch(url);
                const timeoutPromise = new Promise((_, reject) => {
                    setTimeout(() => reject(new Error('Request timeout')), finalOptions.timeout);
                });
                
                const response = await Promise.race([fetchPromise, timeoutPromise]);
                
                if (!response || typeof response !== 'object') {
                    throw new Error('Invalid response format');
                }
                
                // Process data if callback provided
                let processedData = response;
                if (processCallback && typeof processCallback === 'function') {
                    try {
                        processedData = processCallback(response);
                    } catch (callbackError) {
                        if (finalOptions.debug) {
                            console.error('Processing callback failed:', callbackError);
                        }
                        processedData = response; // Fall back to original data
                    }
                }
                
                // Cache the result
                const cacheKey = `fetch_${url}_${JSON.stringify(options)}`;
                this.cache.set(cacheKey, {
                    data: processedData,
                    timestamp: Date.now(),
                    url
                });
                
                // Cleanup old cache entries
                if (this.cache.size > 100) {
                    const entries = Array.from(this.cache.entries());
                    entries.sort(([, a], [, b]) => a.timestamp - b.timestamp);
                    for (let i = 0; i < 20; i++) {
                        this.cache.delete(entries[i][0]);
                    }
                }
                
                return processedData;
                
            } catch (error) {
                lastError = error instanceof Error ? error : new Error('Unknown error');
                
                if (finalOptions.debug) {
                    console.error(`Attempt ${attempt} failed:`, lastError.message);
                }
                
                if (attempt < finalOptions.retries!) {
                    // Exponential backoff
                    const delay = Math.min(1000 * Math.pow(2, attempt - 1), 5000);
                    await new Promise(resolve => setTimeout(resolve, delay));
                }
            }
        }
        
        throw lastError || new Error('All retry attempts failed');
    }
    
    private async simulateFetch(url: string): Promise<any> {
        // Simulate network request
        await new Promise(resolve => setTimeout(resolve, Math.random() * 1000));
        
        if (Math.random() < 0.1) { // 10% failure rate
            throw new Error('Network error');
        }
        
        return {
            url,
            data: { message: 'Success', timestamp: Date.now() },
            status: 200
        };
    }
    
    // Complex data transformation with nested conditions
    transformObjectStructure(
        obj: Record<string, any>, 
        rules: Record<string, string | ((value: any) => any)>
    ): Record<string, any> {
        if (!obj || typeof obj !== 'object') {
            return {};
        }
        
        const result: Record<string, any> = {};
        
        for (const [sourceKey, rule] of Object.entries(rules)) {
            if (sourceKey in obj) {
                if (typeof rule === 'string') {
                    // Simple key mapping
                    result[rule] = obj[sourceKey];
                } else if (typeof rule === 'function') {
                    // Transformation function
                    try {
                        result[sourceKey] = rule(obj[sourceKey]);
                    } catch (error) {
                        if (this.config.debug) {
                            console.error(`Transformation failed for key ${sourceKey}:`, error);
                        }
                        result[sourceKey] = obj[sourceKey]; // Fallback to original
                    }
                }
            }
        }
        
        // Handle nested objects
        for (const [key, value] of Object.entries(obj)) {
            if (!(key in rules) && typeof value === 'object' && value !== null) {
                if (Array.isArray(value)) {
                    result[key] = value.map((item, index) => {
                        if (typeof item === 'object' && item !== null) {
                            return this.transformObjectStructure(item, rules);
                        }
                        return item;
                    });
                } else {
                    result[key] = this.transformObjectStructure(value, rules);
                }
            } else if (!(key in rules)) {
                result[key] = value;
            }
        }
        
        return result;
    }
    
    getCacheStats(): { size: number; keys: string[] } {
        return {
            size: this.cache.size,
            keys: Array.from(this.cache.keys())
        };
    }
    
    clearCache(): void {
        this.cache.clear();
    }
}

// Standalone functions with complexity issues
export function deepEqual(obj1: any, obj2: any): boolean {
    if (obj1 === obj2) return true;
    if (obj1 == null || obj2 == null) return obj1 === obj2;
    if (typeof obj1 !== typeof obj2) return false;
    
    if (typeof obj1 === 'object') {
        if (Array.isArray(obj1)) {
            if (!Array.isArray(obj2) || obj1.length !== obj2.length) return false;
            for (let i = 0; i < obj1.length; i++) {
                if (!deepEqual(obj1[i], obj2[i])) return false;
            }
            return true;
        } else {
            const keys1 = Object.keys(obj1);
            const keys2 = Object.keys(obj2);
            if (keys1.length !== keys2.length) return false;
            for (const key of keys1) {
                if (!keys2.includes(key)) return false;
                if (!deepEqual(obj1[key], obj2[key])) return false;
            }
            return true;
        }
    }
    
    return false;
}

export function formatCurrency(
    amount: number,
    currency: string = 'USD',
    locale: string = 'en-US',
    options: Intl.NumberFormatOptions = {}
): string {
    if (typeof amount !== 'number' || isNaN(amount)) {
        return '0.00';
    }
    
    const defaultOptions: Intl.NumberFormatOptions = {
        style: 'currency',
        currency: currency,
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
        ...options
    };
    
    try {
        const formatter = new Intl.NumberFormat(locale, defaultOptions);
        return formatter.format(amount);
    } catch (error) {
        // Fallback formatting
        const fixed = amount.toFixed(2);
        return `${currency} ${fixed}`;
    }
}