#!/usr/bin/env python3
"""
Valknut Agent Wrapper - Ensures proper installation and usage for AI agents.

This script automatically uses the pipx-installed version of valknut with full
tree-sitter parser support, avoiding common installation issues.
"""

import subprocess
import sys
from pathlib import Path


def check_pipx_installation():
    """Check if valknut is properly installed via pipx."""
    try:
        result = subprocess.run(['which', 'valknut'], 
                              capture_output=True, text=True, check=True)
        valknut_path = result.stdout.strip()
        
        if '/home/nathan/.local/bin/valknut' in valknut_path:
            return True, valknut_path
        else:
            return False, f"Found valknut at {valknut_path} but expected pipx installation"
    except subprocess.CalledProcessError:
        return False, "valknut command not found"


def ensure_valknut_installation():
    """Ensure valknut is properly installed with pipx."""
    is_installed, message = check_pipx_installation()
    
    if not is_installed:
        print(f"⚠️  Issue detected: {message}")
        print("🔧 Installing valknut with pipx...")
        
        try:
            # Install valknut with pipx
            valknut_project_path = "/media/nathan/Seagate Hub/Projects/valknut"
            subprocess.run(['pipx', 'install', valknut_project_path, '--force'], 
                          check=True)
            print("✅ Valknut installed successfully with pipx")
        except subprocess.CalledProcessError as e:
            print(f"❌ Failed to install valknut: {e}")
            sys.exit(1)
    else:
        print(f"✅ Valknut properly installed at: {message}")


def run_valknut(args):
    """Run valknut with the provided arguments."""
    try:
        # Use the pipx-installed valknut directly
        cmd = ['valknut'] + args
        result = subprocess.run(cmd, check=False)
        return result.returncode
    except FileNotFoundError:
        print("❌ Valknut not found. Installation may have failed.")
        return 1


def main():
    """Main entry point for agent wrapper."""
    print("🤖 Valknut Agent Wrapper")
    print("========================")
    
    # Check installation
    ensure_valknut_installation()
    
    # Verify language support
    print("🔍 Checking language support...")
    try:
        result = subprocess.run(['valknut', 'list-languages'], 
                              capture_output=True, text=True, check=True)
        if "✅ Full Support" in result.stdout:
            print("✅ Language parsers working correctly")
        else:
            print("⚠️  Some language parsers may not be available")
            print("💡 Consider running: pipx install /media/nathan/Seagate\\ Hub/Projects/valknut --force")
    except subprocess.CalledProcessError:
        print("⚠️  Could not verify language support")
    
    # Run valknut with provided arguments
    if len(sys.argv) > 1:
        print("🚀 Running valknut analysis...")
        exit_code = run_valknut(sys.argv[1:])
        sys.exit(exit_code)
    else:
        print("📚 Usage: python3 valknut_agent.py [valknut-arguments]")
        print("📚 Example: python3 valknut_agent.py analyze /path/to/code --format json")
        run_valknut(['--help'])


if __name__ == "__main__":
    main()