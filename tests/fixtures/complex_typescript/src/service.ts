/**
 * Complex TypeScript service with various refactoring opportunities
 */

import { EventEmitter } from 'events';

interface UserData {
    id: string;
    name: string;
    email: string;
    age?: number;
    preferences?: Record<string, any>;
}

interface ProcessingOptions {
    validateEmail?: boolean;
    transformData?: boolean;
    cacheResults?: boolean;
    timeout?: number;
}

interface ProcessingResult {
    success: boolean;
    data?: any;
    errors: string[];
    warnings: string[];
    processingTime: number;
}

export class DataProcessingService extends EventEmitter {
    private cache: Map<string, any> = new Map();
    private processingQueue: Array<{ id: string; data: any; options: ProcessingOptions }> = [];
    private isProcessing: boolean = false;
    private metrics: Record<string, number> = {};

    constructor(private config: { maxCacheSize?: number; defaultTimeout?: number } = {}) {
        super();
        this.config.maxCacheSize = config.maxCacheSize || 1000;
        this.config.defaultTimeout = config.defaultTimeout || 30000;
    }

    // Complex method with multiple responsibilities
    async processUserData(userData: UserData, options: ProcessingOptions = {}): Promise<ProcessingResult> {
        const startTime = Date.now();
        const errors: string[] = [];
        const warnings: string[] = [];
        
        try {
            // Input validation logic
            if (!userData) {
                errors.push('User data is required');
                return { success: false, errors, warnings, processingTime: Date.now() - startTime };
            }
            
            if (!userData.id) {
                errors.push('User ID is required');
            }
            
            if (!userData.name || userData.name.trim().length === 0) {
                errors.push('User name is required');
            }
            
            if (!userData.email) {
                errors.push('User email is required');
            }
            
            // Email validation with complex logic
            if (userData.email && options.validateEmail !== false) {
                const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
                if (!emailRegex.test(userData.email)) {
                    errors.push('Invalid email format');
                } else {
                    // Additional email validation
                    const emailParts = userData.email.split('@');
                    if (emailParts.length !== 2) {
                        errors.push('Invalid email structure');
                    } else {
                        const [localPart, domain] = emailParts;
                        if (localPart.length === 0 || localPart.length > 64) {
                            warnings.push('Email local part length is unusual');
                        }
                        if (domain.length === 0 || domain.length > 255) {
                            errors.push('Invalid domain length');
                        }
                        if (!domain.includes('.')) {
                            errors.push('Domain must contain a dot');
                        }
                    }
                }
            }
            
            // Age validation logic
            if (userData.age !== undefined) {
                if (typeof userData.age !== 'number' || userData.age < 0) {
                    errors.push('Age must be a positive number');
                } else if (userData.age > 150) {
                    warnings.push('Age seems unusually high');
                } else if (userData.age < 13) {
                    warnings.push('User may be under minimum age');
                }
            }
            
            if (errors.length > 0) {
                return { success: false, errors, warnings, processingTime: Date.now() - startTime };
            }
            
            // Data transformation logic
            let processedData = { ...userData };
            
            if (options.transformData !== false) {
                // Name transformation
                if (processedData.name) {
                    processedData.name = processedData.name.trim()
                        .split(' ')
                        .map(part => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
                        .join(' ');
                }
                
                // Email transformation
                if (processedData.email) {
                    processedData.email = processedData.email.toLowerCase().trim();
                }
                
                // Preferences processing
                if (processedData.preferences) {
                    const processedPreferences: Record<string, any> = {};
                    for (const [key, value] of Object.entries(processedData.preferences)) {
                        if (typeof value === 'string') {
                            processedPreferences[key] = value.trim();
                        } else if (typeof value === 'number' && !isNaN(value)) {
                            processedPreferences[key] = value;
                        } else if (typeof value === 'boolean') {
                            processedPreferences[key] = value;
                        } else if (Array.isArray(value)) {
                            processedPreferences[key] = value.filter(item => 
                                item !== null && item !== undefined && item !== ''
                            );
                        } else if (value && typeof value === 'object') {
                            processedPreferences[key] = this.cleanObject(value);
                        }
                    }
                    processedData.preferences = processedPreferences;
                }
            }
            
            // Caching logic
            if (options.cacheResults !== false) {
                const cacheKey = `user_${processedData.id}`;
                if (this.cache.has(cacheKey)) {
                    const cachedData = this.cache.get(cacheKey);
                    if (this.isDataEqual(cachedData.data, processedData)) {
                        warnings.push('Data unchanged, returned cached result');
                        return {
                            success: true,
                            data: cachedData.data,
                            errors,
                            warnings,
                            processingTime: Date.now() - startTime
                        };
                    }
                }
                
                // Store in cache
                this.cache.set(cacheKey, {
                    data: processedData,
                    timestamp: Date.now()
                });
                
                // Cache size management
                if (this.cache.size > this.config.maxCacheSize!) {
                    const oldestKey = Array.from(this.cache.keys())[0];
                    this.cache.delete(oldestKey);
                    warnings.push('Cache size limit reached, removed oldest entry');
                }
            }
            
            // Metrics tracking
            this.updateMetrics('processUserData', Date.now() - startTime);
            
            // Emit events
            this.emit('userProcessed', { userId: processedData.id, success: true });
            
            return {
                success: true,
                data: processedData,
                errors,
                warnings,
                processingTime: Date.now() - startTime
            };
            
        } catch (error) {
            errors.push(`Processing failed: ${error instanceof Error ? error.message : 'Unknown error'}`);
            this.emit('processingError', { userId: userData?.id, error });
            return { success: false, errors, warnings, processingTime: Date.now() - startTime };
        }
    }
    
    // Helper method with duplicated logic patterns
    private cleanObject(obj: Record<string, any>): Record<string, any> {
        const cleaned: Record<string, any> = {};
        for (const [key, value] of Object.entries(obj)) {
            if (value !== null && value !== undefined) {
                if (typeof value === 'string') {
                    const trimmed = value.trim();
                    if (trimmed.length > 0) {
                        cleaned[key] = trimmed;
                    }
                } else if (typeof value === 'number' && !isNaN(value)) {
                    cleaned[key] = value;
                } else if (typeof value === 'boolean') {
                    cleaned[key] = value;
                } else if (Array.isArray(value)) {
                    const cleanedArray = value.filter(item => 
                        item !== null && item !== undefined && item !== ''
                    );
                    if (cleanedArray.length > 0) {
                        cleaned[key] = cleanedArray;
                    }
                } else if (typeof value === 'object') {
                    const cleanedNested = this.cleanObject(value);
                    if (Object.keys(cleanedNested).length > 0) {
                        cleaned[key] = cleanedNested;
                    }
                }
            }
        }
        return cleaned;
    }
    
    // Method with high cyclomatic complexity
    private isDataEqual(data1: any, data2: any): boolean {
        if (data1 === data2) return true;
        if (data1 == null || data2 == null) return data1 === data2;
        if (typeof data1 !== typeof data2) return false;
        
        if (typeof data1 === 'object') {
            if (Array.isArray(data1)) {
                if (!Array.isArray(data2) || data1.length !== data2.length) return false;
                for (let i = 0; i < data1.length; i++) {
                    if (!this.isDataEqual(data1[i], data2[i])) return false;
                }
                return true;
            } else {
                const keys1 = Object.keys(data1);
                const keys2 = Object.keys(data2);
                if (keys1.length !== keys2.length) return false;
                for (const key of keys1) {
                    if (!keys2.includes(key)) return false;
                    if (!this.isDataEqual(data1[key], data2[key])) return false;
                }
                return true;
            }
        }
        
        return false;
    }
    
    // Method that should be extracted into smaller functions
    async processBatchData(batchData: UserData[], options: ProcessingOptions = {}): Promise<{
        results: ProcessingResult[];
        summary: {
            total: number;
            successful: number;
            failed: number;
            totalProcessingTime: number;
        };
    }> {
        const startTime = Date.now();
        const results: ProcessingResult[] = [];
        let successful = 0;
        let failed = 0;
        
        for (const userData of batchData) {
            try {
                const result = await this.processUserData(userData, options);
                results.push(result);
                
                if (result.success) {
                    successful++;
                } else {
                    failed++;
                }
                
                // Progress reporting for large batches
                if (batchData.length > 100 && results.length % 50 === 0) {
                    this.emit('batchProgress', {
                        processed: results.length,
                        total: batchData.length,
                        successful,
                        failed
                    });
                }
                
            } catch (error) {
                failed++;
                results.push({
                    success: false,
                    errors: [`Batch processing error: ${error instanceof Error ? error.message : 'Unknown error'}`],
                    warnings: [],
                    processingTime: 0
                });
            }
        }
        
        const totalProcessingTime = Date.now() - startTime;
        this.updateMetrics('processBatchData', totalProcessingTime);
        
        this.emit('batchCompleted', {
            total: batchData.length,
            successful,
            failed,
            processingTime: totalProcessingTime
        });
        
        return {
            results,
            summary: {
                total: batchData.length,
                successful,
                failed,
                totalProcessingTime
            }
        };
    }
    
    private updateMetrics(operation: string, processingTime: number): void {
        if (!this.metrics[operation]) {
            this.metrics[operation] = 0;
        }
        this.metrics[operation] += processingTime;
    }
    
    getMetrics(): Record<string, number> {
        return { ...this.metrics };
    }
    
    clearCache(): void {
        this.cache.clear();
        this.emit('cacheCleared');
    }
    
    getCacheSize(): number {
        return this.cache.size;
    }
}