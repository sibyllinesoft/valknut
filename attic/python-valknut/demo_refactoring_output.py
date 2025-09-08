#!/usr/bin/env python3
"""
Demonstrate the refactoring analyzer output with realistic examples.
"""

import sys
import os
import json
from pathlib import Path

# Add the valknut package to the path
sys.path.insert(0, os.path.join(os.path.dirname(__file__)))

from valknut.detectors.refactoring import RefactoringAnalyzer, RefactoringType
from valknut.lang.common_ast import Entity, EntityKind, SourceLocation, ParseIndex
from valknut.core.featureset import FeatureVector


def create_demo_entity() -> Entity:
    """Create a demo entity with complex code for refactoring analysis."""
    complex_code = '''def process_user_registration(first_name, last_name, email, phone, 
                     address_line1, address_line2, city, state, zip_code,
                     emergency_contact_name, emergency_contact_phone,
                     marketing_opt_in, newsletter_frequency):
    """Complex user registration function demonstrating multiple code smells."""
    
    # Input validation with complex conditionals and magic numbers
    if not first_name or len(first_name.strip()) < 2 or len(first_name) > 50:
        return {"error": "First name must be between 2 and 50 characters", "code": 400}
    
    if not last_name or len(last_name.strip()) < 2 or len(last_name) > 50:
        return {"error": "Last name must be between 2 and 50 characters", "code": 400}
    
    if not email or "@" not in email or len(email) < 5 or len(email) > 255:
        return {"error": "Invalid email format", "code": 400}
    
    # Phone validation with magic numbers and complex logic
    if phone and len(phone.replace("-", "").replace("(", "").replace(")", "").replace(" ", "")) != 10:
        return {"error": "Phone must be exactly 10 digits", "code": 400}
    
    # Data normalization - should be extracted to separate methods
    normalized_first = first_name.strip().title()
    normalized_last = last_name.strip().title()
    normalized_email = email.lower().strip()
    full_name = normalized_first + " " + normalized_last
    
    # Phone formatting
    if phone:
        clean_phone = phone.replace("-", "").replace("(", "").replace(")", "").replace(" ", "")
        formatted_phone = f"({clean_phone[:3]}) {clean_phone[3:6]}-{clean_phone[6:]}"
    else:
        formatted_phone = None
    
    # Address validation - complex nested conditionals
    if address_line1:
        if len(address_line1.strip()) < 5 or len(address_line1) > 100:
            return {"error": "Address line 1 must be between 5 and 100 characters", "code": 400}
        if not city or len(city.strip()) < 2 or len(city) > 50:
            return {"error": "City must be between 2 and 50 characters", "code": 400}
        if not state or len(state) != 2:
            return {"error": "State must be 2 characters", "code": 400}
        if not zip_code or len(zip_code) not in [5, 9, 10]:
            return {"error": "ZIP code must be 5 or 9 digits", "code": 400}
    
    # Database operations with magic numbers for retry logic
    for attempt in range(3):  # Magic number
        try:
            # Simulate database save
            user_id = f"user_{int(time.time() * 1000000)}"  # Magic number calculation
            
            user_data = {
                "id": user_id,
                "first_name": normalized_first,
                "last_name": normalized_last,
                "full_name": full_name,
                "email": normalized_email,
                "phone": formatted_phone,
                "address": {
                    "line1": address_line1.strip().title() if address_line1 else None,
                    "line2": address_line2.strip().title() if address_line2 else None,
                    "city": city.strip().title() if city else None,
                    "state": state.upper() if state else None,
                    "zip_code": zip_code.strip() if zip_code else None
                },
                "emergency_contact": {
                    "name": emergency_contact_name.strip().title() if emergency_contact_name else None,
                    "phone": emergency_contact_phone.strip() if emergency_contact_phone else None
                },
                "preferences": {
                    "marketing_opt_in": marketing_opt_in,
                    "newsletter_frequency": newsletter_frequency
                },
                "created_at": datetime.now().isoformat(),
                "status": "active"
            }
            
            # Simulate save to database
            database.users.insert(user_data)
            
            # Analytics tracking with magic numbers
            if len(database.users.find()) % 100 == 0:  # Magic number
                print(f"Milestone: {len(database.users.find())} users registered!")
            
            break  # Success, exit retry loop
            
        except DatabaseConnectionError as e:
            if attempt == 2:  # Magic number
                return {"error": f"Database error after 3 attempts: {str(e)}", "code": 500}
            time.sleep(2 ** attempt)  # Exponential backoff with magic number base
            
        except ValidationError as e:
            return {"error": f"Data validation failed: {str(e)}", "code": 400}
    
    # Email notification logic - should be extracted
    if marketing_opt_in:
        try:
            welcome_subject = "Welcome to " + "Our Platform" + " - " + "Let's Get Started!"
            welcome_message = "Dear " + full_name + ", welcome to our platform! "
            welcome_message += "We're excited to have you join our community of " + str(len(database.users.find())) + " members."
            
            if newsletter_frequency in ["weekly", "monthly"]:
                welcome_message += " You'll receive our " + newsletter_frequency + " newsletter with updates and tips."
            
            send_email(normalized_email, welcome_subject, welcome_message)
            
            # Schedule follow-up emails with magic numbers
            schedule_email(normalized_email, "Getting Started Tips", get_tips_email_content(), 
                         delay_hours=24)  # Magic number
            
            if newsletter_frequency == "weekly":
                schedule_recurring_email(normalized_email, "Weekly Newsletter", 
                                       frequency_days=7)  # Magic number
            elif newsletter_frequency == "monthly":
                schedule_recurring_email(normalized_email, "Monthly Newsletter", 
                                       frequency_days=30)  # Magic number
                
        except EmailServiceError as e:
            # Don't fail registration for email errors, just log
            logger.warning(f"Failed to send welcome email to {normalized_email}: {e}")
    
    # Success response
    return {
        "success": True,
        "user_id": user_id,
        "message": f"User {full_name} registered successfully",
        "next_steps": [
            "Check your email for welcome message",
            "Complete your profile setup", 
            "Verify your phone number" if formatted_phone else "Add a phone number"
        ]
    }'''
    
    entity = Entity(
        id="demo://UserRegistrationService::process_user_registration",
        name="process_user_registration",
        kind=EntityKind.FUNCTION,
        location=SourceLocation(
            file_path=Path("user_service.py"),
            start_line=15,
            end_line=120
        ),
        language="python",
        raw_text=complex_code,
        parameters=[
            "first_name", "last_name", "email", "phone", 
            "address_line1", "address_line2", "city", "state", "zip_code",
            "emergency_contact_name", "emergency_contact_phone",
            "marketing_opt_in", "newsletter_frequency"
        ]
    )
    
    # Add complexity metrics
    entity.metrics = {
        "cyclomatic": 18,
        "cognitive": 25,
        "max_nesting": 4,
        "param_count": 13
    }
    
    return entity


def generate_demo_output():
    """Generate demo output showing refactoring suggestions."""
    print("🔨 Valknut Refactoring Analyzer - Demo Output")
    print("=" * 60)
    print()
    
    # Create analyzer and demo entity
    analyzer = RefactoringAnalyzer()
    entity = create_demo_entity()
    index = ParseIndex(entities={entity.id: entity}, files={})
    
    # Analyze refactoring opportunities
    suggestions = analyzer.analyze_refactoring_opportunities(entity, index)
    
    print(f"📊 Analysis Results for: {entity.name}")
    print(f"   Entity: {entity.id}")
    print(f"   Lines of Code: {entity.location.line_count}")
    print(f"   Parameters: {len(entity.parameters)}")
    print(f"   Complexity Metrics:")
    print(f"     - Cyclomatic Complexity: {entity.metrics.get('cyclomatic', 'N/A')}")
    print(f"     - Cognitive Complexity: {entity.metrics.get('cognitive', 'N/A')}")
    print(f"     - Max Nesting: {entity.metrics.get('max_nesting', 'N/A')}")
    print()
    
    print(f"🔍 Refactoring Opportunities Found: {len(suggestions)}")
    print("-" * 40)
    
    # Group suggestions by severity
    high_priority = [s for s in suggestions if s.severity == "high"]
    medium_priority = [s for s in suggestions if s.severity == "medium"]
    low_priority = [s for s in suggestions if s.severity == "low"]
    
    for priority_group, priority_name, icon in [
        (high_priority, "High Priority", "🔴"),
        (medium_priority, "Medium Priority", "🟡"),
        (low_priority, "Low Priority", "🟢")
    ]:
        if priority_group:
            print(f"\n{icon} {priority_name} ({len(priority_group)} suggestions):")
            print("-" * 30)
            
            for i, suggestion in enumerate(priority_group, 1):
                print(f"\n{i}. {suggestion.title}")
                print(f"   Type: {suggestion.type.value.replace('_', ' ').title()}")
                print(f"   Effort: {suggestion.effort.title()}")
                print(f"   Description: {suggestion.description}")
                print(f"   Why: {suggestion.rationale}")
                
                if suggestion.benefits:
                    print(f"   Benefits:")
                    for benefit in suggestion.benefits[:3]:  # Show top 3 benefits
                        print(f"     • {benefit}")
                    if len(suggestion.benefits) > 3:
                        print(f"     • ... and {len(suggestion.benefits) - 3} more benefits")
                
                print()
    
    # Show a detailed code example for one suggestion
    if suggestions:
        example_suggestion = suggestions[0]
        if example_suggestion.before_code and example_suggestion.after_code:
            print("📝 Code Example (Extract Method Refactoring):")
            print("=" * 50)
            print()
            print("BEFORE (Current Code):")
            print("```python")
            # Show first 15 lines of before code
            before_lines = example_suggestion.before_code.split('\n')[:15]
            for line in before_lines:
                print(line)
            if len(example_suggestion.before_code.split('\n')) > 15:
                print("# ... (truncated for demo)")
            print("```")
            print()
            
            print("AFTER (Refactored Code):")
            print("```python")
            # Show first 15 lines of after code
            after_lines = example_suggestion.after_code.split('\n')[:15]
            for line in after_lines:
                print(line)
            if len(example_suggestion.after_code.split('\n')) > 15:
                print("# ... (truncated for demo)")
            print("```")
            print()
    
    # Extract features
    features = analyzer.extract(entity, index)
    print(f"📊 Extracted Features:")
    print(f"   Refactoring Urgency: {features.get('refactoring_urgency', 0):.1f}/100")
    print(f"   Total Suggestions: {features.get('suggestion_count', 0):.0f}")
    print(f"   High Priority Issues: {features.get('high_severity_suggestions', 0):.0f}")
    print()
    
    # Generate JSON output
    json_output = {
        "entity_id": entity.id,
        "analysis_summary": {
            "total_suggestions": len(suggestions),
            "high_priority": len(high_priority),
            "medium_priority": len(medium_priority), 
            "low_priority": len(low_priority)
        },
        "refactoring_suggestions": [
            {
                "type": s.type.value,
                "severity": s.severity,
                "title": s.title,
                "description": s.description,
                "rationale": s.rationale,
                "benefits": s.benefits,
                "effort": s.effort
            }
            for s in suggestions
        ],
        "features": features
    }
    
    print("💾 Sample JSON Output:")
    print(json.dumps(json_output, indent=2)[:1000] + "..." if len(json.dumps(json_output, indent=2)) > 1000 else json.dumps(json_output, indent=2))
    print()
    
    # Implementation recommendations
    print("🗓️ Implementation Recommendations:")
    print("-" * 40)
    if high_priority:
        print("Phase 1 (Immediate): Address high-priority issues")
        for suggestion in high_priority[:3]:  # Top 3 high priority
            print(f"  • {suggestion.title} ({suggestion.effort} effort)")
    
    if medium_priority:
        print("Phase 2 (Next Sprint): Address medium-priority improvements")
        for suggestion in medium_priority[:2]:  # Top 2 medium priority  
            print(f"  • {suggestion.title} ({suggestion.effort} effort)")
    
    if low_priority:
        print("Phase 3 (Maintenance): Address enhancement opportunities")
        print(f"  • {len(low_priority)} low-priority suggestions during regular development")
    
    print()
    print("✨ This refactoring analysis provides specific, actionable guidance")
    print("   with before/after examples to improve code quality systematically.")
    

if __name__ == "__main__":
    generate_demo_output()