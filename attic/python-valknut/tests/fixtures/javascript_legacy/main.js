/**
 * Legacy JavaScript code with various refactoring opportunities
 */

// Global variables - code smell
var globalConfig = {
    debug: false,
    apiUrl: 'https://api.example.com',
    timeout: 5000,
    retries: 3
};

var globalCache = {};
var globalMetrics = {};

// Large function with multiple responsibilities
function processUserRegistration(userData) {
    // Input validation
    if (!userData) {
        console.error('User data is required');
        return null;
    }
    
    if (!userData.email) {
        console.error('Email is required');
        return null;
    }
    
    if (!userData.password) {
        console.error('Password is required');
        return null;
    }
    
    if (!userData.name) {
        console.error('Name is required');
        return null;
    }
    
    // Email validation
    var emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(userData.email)) {
        console.error('Invalid email format');
        return null;
    }
    
    // Password validation
    if (userData.password.length < 8) {
        console.error('Password too short');
        return null;
    }
    
    var hasUpperCase = /[A-Z]/.test(userData.password);
    var hasLowerCase = /[a-z]/.test(userData.password);
    var hasNumbers = /\d/.test(userData.password);
    var hasSpecialChar = /[!@#$%^&*(),.?":{}|<>]/.test(userData.password);
    
    if (!hasUpperCase || !hasLowerCase || !hasNumbers || !hasSpecialChar) {
        console.error('Password must contain uppercase, lowercase, numbers, and special characters');
        return null;
    }
    
    // Name validation
    if (userData.name.length < 2) {
        console.error('Name too short');
        return null;
    }
    
    if (userData.name.length > 50) {
        console.error('Name too long');
        return null;
    }
    
    // Data sanitization
    var sanitizedData = {
        email: userData.email.toLowerCase().trim(),
        name: userData.name.trim(),
        password: userData.password,
        registrationDate: new Date().toISOString()
    };
    
    // Age handling
    if (userData.age) {
        if (typeof userData.age !== 'number' || userData.age < 0 || userData.age > 150) {
            console.warn('Invalid age provided');
        } else {
            sanitizedData.age = userData.age;
        }
    }
    
    // Phone number handling
    if (userData.phone) {
        var cleanPhone = userData.phone.replace(/[^\d+]/g, '');
        if (cleanPhone.length >= 10) {
            sanitizedData.phone = cleanPhone;
        } else {
            console.warn('Invalid phone number format');
        }
    }
    
    // Generate user ID
    sanitizedData.id = 'user_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
    
    // Cache the user data
    globalCache[sanitizedData.id] = sanitizedData;
    
    // Update metrics
    if (!globalMetrics.registrations) {
        globalMetrics.registrations = 0;
    }
    globalMetrics.registrations++;
    
    if (!globalMetrics.registrationsByDate) {
        globalMetrics.registrationsByDate = {};
    }
    
    var today = new Date().toDateString();
    if (!globalMetrics.registrationsByDate[today]) {
        globalMetrics.registrationsByDate[today] = 0;
    }
    globalMetrics.registrationsByDate[today]++;
    
    return sanitizedData;
}

// Complex function with deep nesting
function generateUserReport(userId, options) {
    options = options || {};
    
    if (!userId) {
        return { error: 'User ID is required' };
    }
    
    var user = globalCache[userId];
    if (!user) {
        return { error: 'User not found' };
    }
    
    var report = {
        userId: userId,
        generatedAt: new Date().toISOString(),
        userData: {}
    };
    
    // Basic user data
    report.userData.email = user.email;
    report.userData.name = user.name;
    report.userData.registrationDate = user.registrationDate;
    
    if (user.age) {
        report.userData.age = user.age;
        
        // Age group classification
        if (user.age < 18) {
            report.userData.ageGroup = 'minor';
            report.userData.restrictions = ['limited_features', 'parental_consent_required'];
        } else if (user.age < 25) {
            report.userData.ageGroup = 'young_adult';
            report.userData.features = ['student_discount', 'premium_trial'];
        } else if (user.age < 45) {
            report.userData.ageGroup = 'adult';
            report.userData.features = ['full_features', 'priority_support'];
        } else if (user.age < 65) {
            report.userData.ageGroup = 'middle_aged';
            report.userData.features = ['full_features', 'premium_support', 'loyalty_rewards'];
        } else {
            report.userData.ageGroup = 'senior';
            report.userData.features = ['full_features', 'senior_discount', 'priority_support'];
        }
    }
    
    if (user.phone) {
        report.userData.phone = user.phone;
        report.userData.contactMethods = ['email', 'phone'];
    } else {
        report.userData.contactMethods = ['email'];
    }
    
    // Activity analysis (simulated)
    if (options.includeActivity) {
        report.activity = {
            loginCount: Math.floor(Math.random() * 100),
            lastLogin: new Date(Date.now() - Math.random() * 30 * 24 * 60 * 60 * 1000).toISOString(),
            averageSessionDuration: Math.floor(Math.random() * 3600),
            favoriteFeatures: ['dashboard', 'reports', 'settings']
        };
        
        // Engagement level calculation
        if (report.activity.loginCount > 50) {
            if (report.activity.averageSessionDuration > 1800) {
                report.activity.engagementLevel = 'high';
                report.activity.recommendations = ['advanced_features', 'beta_program'];
            } else {
                report.activity.engagementLevel = 'medium';
                report.activity.recommendations = ['tutorial_completion', 'feature_exploration'];
            }
        } else if (report.activity.loginCount > 10) {
            report.activity.engagementLevel = 'low';
            report.activity.recommendations = ['onboarding_follow_up', 'feature_highlights'];
        } else {
            report.activity.engagementLevel = 'very_low';
            report.activity.recommendations = ['re_engagement_campaign', 'support_outreach'];
        }
    }
    
    // Preferences analysis
    if (options.includePreferences) {
        report.preferences = {
            theme: Math.random() > 0.5 ? 'dark' : 'light',
            language: 'en',
            notifications: {
                email: Math.random() > 0.3,
                sms: user.phone && Math.random() > 0.7,
                push: Math.random() > 0.5
            },
            privacy: {
                shareData: Math.random() > 0.6,
                analytics: Math.random() > 0.4,
                marketing: Math.random() > 0.8
            }
        };
    }
    
    // Risk assessment
    if (options.includeRiskAssessment) {
        report.riskAssessment = {
            level: 'low',
            factors: [],
            score: 0
        };
        
        var riskScore = 0;
        var riskFactors = [];
        
        // Age-based risk
        if (user.age && user.age < 18) {
            riskScore += 20;
            riskFactors.push('minor_user');
        }
        
        // Activity-based risk
        if (report.activity) {
            if (report.activity.loginCount === 0) {
                riskScore += 30;
                riskFactors.push('inactive_user');
            } else if (report.activity.loginCount > 200) {
                riskScore += 10;
                riskFactors.push('high_activity_user');
            }
        }
        
        // Email-based risk (basic heuristics)
        if (user.email.includes('temp') || user.email.includes('disposable')) {
            riskScore += 40;
            riskFactors.push('disposable_email');
        }
        
        report.riskAssessment.score = riskScore;
        report.riskAssessment.factors = riskFactors;
        
        if (riskScore < 20) {
            report.riskAssessment.level = 'low';
        } else if (riskScore < 50) {
            report.riskAssessment.level = 'medium';
        } else {
            report.riskAssessment.level = 'high';
        }
    }
    
    return report;
}

// Function with repeated code patterns
function validateAndProcessEmail(email) {
    if (!email) {
        return { valid: false, error: 'Email is required' };
    }
    
    if (typeof email !== 'string') {
        return { valid: false, error: 'Email must be a string' };
    }
    
    email = email.trim().toLowerCase();
    
    if (email.length === 0) {
        return { valid: false, error: 'Email cannot be empty' };
    }
    
    if (email.length > 254) {
        return { valid: false, error: 'Email too long' };
    }
    
    var emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(email)) {
        return { valid: false, error: 'Invalid email format' };
    }
    
    var parts = email.split('@');
    var localPart = parts[0];
    var domain = parts[1];
    
    if (localPart.length > 64) {
        return { valid: false, error: 'Email local part too long' };
    }
    
    if (domain.indexOf('.') === -1) {
        return { valid: false, error: 'Domain must contain a dot' };
    }
    
    return { valid: true, email: email };
}

// Similar validation function - code duplication
function validateAndProcessPhone(phone) {
    if (!phone) {
        return { valid: false, error: 'Phone is required' };
    }
    
    if (typeof phone !== 'string') {
        return { valid: false, error: 'Phone must be a string' };
    }
    
    phone = phone.trim();
    
    if (phone.length === 0) {
        return { valid: false, error: 'Phone cannot be empty' };
    }
    
    var cleanPhone = phone.replace(/[^\d+]/g, '');
    
    if (cleanPhone.length < 10) {
        return { valid: false, error: 'Phone number too short' };
    }
    
    if (cleanPhone.length > 15) {
        return { valid: false, error: 'Phone number too long' };
    }
    
    var phoneRegex = /^\+?[\d\s\-\(\)\.]+$/;
    if (!phoneRegex.test(phone)) {
        return { valid: false, error: 'Invalid phone format' };
    }
    
    return { valid: true, phone: cleanPhone };
}

// Utility functions with high complexity
function deepClone(obj) {
    if (obj === null || typeof obj !== 'object') {
        return obj;
    }
    
    if (obj instanceof Date) {
        return new Date(obj.getTime());
    }
    
    if (obj instanceof Array) {
        return obj.map(function(item) {
            return deepClone(item);
        });
    }
    
    if (typeof obj === 'object') {
        var cloned = {};
        for (var key in obj) {
            if (obj.hasOwnProperty(key)) {
                cloned[key] = deepClone(obj[key]);
            }
        }
        return cloned;
    }
    
    return obj;
}

function mergeObjects(target, source) {
    if (!target || typeof target !== 'object') {
        target = {};
    }
    
    if (!source || typeof source !== 'object') {
        return target;
    }
    
    for (var key in source) {
        if (source.hasOwnProperty(key)) {
            if (target.hasOwnProperty(key) && 
                typeof target[key] === 'object' && 
                target[key] !== null &&
                typeof source[key] === 'object' &&
                source[key] !== null &&
                !Array.isArray(target[key]) &&
                !Array.isArray(source[key])) {
                target[key] = mergeObjects(target[key], source[key]);
            } else {
                target[key] = deepClone(source[key]);
            }
        }
    }
    
    return target;
}

// Export functions (CommonJS style)
if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        processUserRegistration: processUserRegistration,
        generateUserReport: generateUserReport,
        validateAndProcessEmail: validateAndProcessEmail,
        validateAndProcessPhone: validateAndProcessPhone,
        deepClone: deepClone,
        mergeObjects: mergeObjects,
        globalConfig: globalConfig,
        globalCache: globalCache,
        globalMetrics: globalMetrics
    };
}