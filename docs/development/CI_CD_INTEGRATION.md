# CI/CD Integration Guide

This guide provides comprehensive instructions for integrating Valknut into your CI/CD pipelines for automated code quality enforcement.

## Table of Contents
- [Overview](#overview)
- [Quality Gates](#quality-gates)
- [GitHub Actions](#github-actions)
- [Jenkins](#jenkins)
- [GitLab CI](#gitlab-ci)
- [Azure DevOps](#azure-devops)
- [CircleCI](#circleci)
- [SonarQube Integration](#sonarqube-integration)
- [Custom Integration](#custom-integration)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Overview

Valknut provides robust CI/CD integration through:

- **Quality Gates**: Configurable failure conditions with exit codes
- **Multiple Output Formats**: JSON, SonarQube, CSV for different tools  
- **Performance Optimization**: Fast analysis suitable for CI environments
- **Comprehensive Reporting**: Detailed reports for team review
- **Flexible Configuration**: Adaptable to different project standards

### Key Benefits

- **Automated Quality Enforcement**: Prevent quality regressions automatically
- **Configurable Standards**: Set team-specific quality thresholds
- **Rich Reporting**: Generate reports for stakeholders
- **Integration Flexibility**: Works with any CI/CD system
- **Performance Optimized**: Fast enough for every commit/PR

## Quality Gates

Quality gates enable automated pass/fail decisions based on code quality metrics.

### Basic Quality Gate Setup

```bash
# Enable quality gates with default thresholds
valknut analyze --quality-gate ./src

# Fail on any issues found
valknut analyze --fail-on-issues ./src

# Custom thresholds
valknut analyze \
  --quality-gate \
  --max-complexity 75 \
  --min-health 60 \
  --max-debt 30 \
  --max-issues 50 \
  --max-critical 0 \
  ./src
```

### Quality Gate Configuration

| Threshold | CLI Option | Range | Default | Description |
|-----------|------------|-------|---------|-------------|
| Max Complexity | `--max-complexity` | 0-100 | 75 | Maximum complexity score (lower is better) |
| Min Health | `--min-health` | 0-100 | 60 | Minimum health score (higher is better) |
| Max Debt | `--max-debt` | 0-100% | 30 | Maximum technical debt ratio |
| Min Maintainability | `--min-maintainability` | 0-100 | 20 | Minimum maintainability index |
| Max Issues | `--max-issues` | 0+ | 50 | Maximum total issues |
| Max Critical | `--max-critical` | 0+ | 0 | Maximum critical issues |
| Max High Priority | `--max-high-priority` | 0+ | 5 | Maximum high-priority issues |

### Exit Codes

| Code | Status | Description |
|------|--------|-------------|
| 0 | ✅ Success | Analysis completed, quality gates passed |
| 1 | ❌ Quality Gate Failure | One or more thresholds exceeded |
| 2 | ⚠️ Configuration Error | Invalid arguments or configuration |
| 3 | ⚠️ File System Error | Path or permission issues |
| 4 | ⚠️ Analysis Error | Internal analysis engine error |

## GitHub Actions

### Basic Quality Gate Workflow

```yaml
name: Code Quality Gate
on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  quality-gate:
    runs-on: ubuntu-latest
    
    steps:
    - name: Checkout code
      uses: actions/checkout@v4
      
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        
    - name: Install Valknut
      run: |
        cargo install --git https://github.com/nathanricedev/valknut
        
    - name: Run Quality Gate
      run: |
        valknut analyze \
          --quality-gate \
          --max-complexity 75 \
          --min-health 60 \
          --max-debt 30 \
          --format ci-summary \
          --out quality-reports/ \
          ./src
          
    - name: Upload Quality Reports
      uses: actions/upload-artifact@v4
      if: always()
      with:
        name: quality-reports
        path: quality-reports/
        retention-days: 30
```

### Advanced GitHub Actions Setup

```yaml
name: Comprehensive Code Analysis
on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

env:
  VALKNUT_VERSION: "latest"

jobs:
  quality-analysis:
    runs-on: ubuntu-latest
    
    steps:
    - name: Checkout code
      uses: actions/checkout@v4
      with:
        fetch-depth: 0  # Full history for better analysis
        
    - name: Cache Valknut installation
      uses: actions/cache@v4
      with:
        path: ~/.cargo/bin/valknut
        key: valknut-${{ env.VALKNUT_VERSION }}
        
    - name: Install Valknut
      run: |
        if [ ! -f ~/.cargo/bin/valknut ]; then
          cargo install --git https://github.com/nathanricedev/valknut
        fi
        
    - name: Create Quality Gate Configuration
      run: |
        cat > .valknut.yml << EOF
        analysis:
          enable_structure_analysis: true
          enable_complexity_analysis: true
          enable_refactoring_analysis: true
          max_files: 1000
        
        quality_gates:
          enabled: true
          max_complexity: 75
          min_health: 60
          max_debt: 30
          max_issues: 50
          max_critical: 0
          max_high_priority: 5
        EOF
        
    - name: Run Comprehensive Analysis
      run: |
        valknut analyze \
          --config .valknut.yml \
          --quality-gate \
          --format json \
          --out reports/ \
          ./src
          
    - name: Generate Team Report
      run: |
        valknut analyze \
          --config .valknut.yml \
          --format html \
          --out reports/html/ \
          ./src
          
    - name: Comment PR with Results
      if: github.event_name == 'pull_request'
      uses: actions/github-script@v7
      with:
        script: |
          const fs = require('fs');
          const path = 'reports/ci-summary.json';
          
          if (fs.existsSync(path)) {
            const report = JSON.parse(fs.readFileSync(path, 'utf8'));
            const comment = `## 🔍 Code Quality Report
            
            **Health Score**: ${report.health_metrics.overall_health_score.toFixed(1)}/100
            **Complexity Score**: ${report.health_metrics.complexity_score.toFixed(1)}/100
            **Technical Debt**: ${report.health_metrics.technical_debt_ratio.toFixed(1)}%
            
            **Issues Found**: ${report.summary.total_issues}
            - Critical: ${report.summary.critical_issues}
            - High Priority: ${report.summary.high_priority_issues}
            
            [View Detailed Report](https://github.com/${{ github.repository }}/actions/runs/${{ github.run_id }})
            `;
            
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: comment
            });
          }
          
    - name: Upload Reports
      uses: actions/upload-artifact@v4
      if: always()
      with:
        name: code-quality-reports
        path: reports/
        retention-days: 90
```

### Matrix Strategy for Multiple Languages

```yaml
name: Multi-Language Quality Gate
on: [push, pull_request]

jobs:
  quality-gate:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        language:
          - { name: "python", path: "./python-service", config: "python-config.yml" }
          - { name: "typescript", path: "./web-app", config: "web-config.yml" }
          - { name: "rust", path: "./rust-service", config: "rust-config.yml" }
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Valknut
      run: cargo install --git https://github.com/nathanricedev/valknut
      
    - name: Run Quality Gate for ${{ matrix.language.name }}
      run: |
        valknut analyze \
          --config configs/${{ matrix.language.config }} \
          --quality-gate \
          --format ci-summary \
          --out reports/${{ matrix.language.name }}/ \
          ${{ matrix.language.path }}
```

## Jenkins

### Basic Jenkins Pipeline

```groovy
pipeline {
    agent any
    
    environment {
        VALKNUT_VERSION = 'latest'
    }
    
    stages {
        stage('Checkout') {
            steps {
                checkout scm
            }
        }
        
        stage('Install Valknut') {
            steps {
                sh '''
                    if [ ! -f ~/.cargo/bin/valknut ]; then
                        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
                        source ~/.cargo/env
                        cargo install --git https://github.com/nathanricedev/valknut
                    fi
                '''
            }
        }
        
        stage('Quality Gate') {
            steps {
                sh '''
                    ~/.cargo/bin/valknut analyze \
                      --quality-gate \
                      --max-complexity 75 \
                      --min-health 60 \
                      --max-debt 30 \
                      --format ci-summary \
                      --out quality-reports/ \
                      ./src
                '''
            }
        }
        
        stage('Generate Reports') {
            steps {
                sh '''
                    ~/.cargo/bin/valknut analyze \
                      --format html \
                      --out reports/html/ \
                      ./src
                '''
            }
        }
    }
    
    post {
        always {
            archiveArtifacts artifacts: 'quality-reports/**/*', fingerprint: true
            archiveArtifacts artifacts: 'reports/**/*', fingerprint: true
        }
        
        failure {
            emailext (
                subject: "Quality Gate Failed: ${env.JOB_NAME} - ${env.BUILD_NUMBER}",
                body: "The code quality gate failed. Check the build logs and quality reports.",
                to: "${env.CHANGE_AUTHOR_EMAIL}"
            )
        }
    }
}
```

### Advanced Jenkins Pipeline with Quality Gates

```groovy
pipeline {
    agent any
    
    parameters {
        choice(
            name: 'QUALITY_LEVEL',
            choices: ['strict', 'normal', 'permissive'],
            description: 'Quality gate strictness level'
        )
        booleanParam(
            name: 'GENERATE_REPORTS',
            defaultValue: true,
            description: 'Generate detailed HTML reports'
        )
    }
    
    environment {
        QUALITY_CONFIG = getQualityConfig(params.QUALITY_LEVEL)
    }
    
    stages {
        stage('Setup') {
            steps {
                script {
                    // Install Valknut if not cached
                    sh '''
                        if [ ! -f ~/.cargo/bin/valknut ]; then
                            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
                            source ~/.cargo/env
                            cargo install --git https://github.com/nathanricedev/valknut
                        fi
                    '''
                }
            }
        }
        
        stage('Quality Gate') {
            steps {
                script {
                    def qualityArgs = getQualityArgs(params.QUALITY_LEVEL)
                    sh """
                        ~/.cargo/bin/valknut analyze ${qualityArgs} \
                          --quality-gate \
                          --format ci-summary \
                          --out quality-reports/ \
                          ./src
                    """
                }
            }
        }
        
        stage('Generate Reports') {
            when {
                expression { params.GENERATE_REPORTS }
            }
            parallel {
                stage('HTML Report') {
                    steps {
                        sh '''
                            ~/.cargo/bin/valknut analyze \
                              --format html \
                              --out reports/html/ \
                              ./src
                        '''
                    }
                }
                stage('Markdown Summary') {
                    steps {
                        sh '''
                            ~/.cargo/bin/valknut analyze \
                              --format markdown \
                              --out reports/markdown/ \
                              ./src
                        '''
                    }
                }
                stage('CSV Export') {
                    steps {
                        sh '''
                            ~/.cargo/bin/valknut analyze \
                              --format csv \
                              --out reports/csv/ \
                              ./src
                        '''
                    }
                }
            }
        }
    }
    
    post {
        always {
            publishHTML([
                allowMissing: false,
                alwaysLinkToLastBuild: true,
                keepAll: true,
                reportDir: 'reports/html',
                reportFiles: 'index.html',
                reportName: 'Code Quality Report'
            ])
            
            archiveArtifacts artifacts: 'quality-reports/**/*', fingerprint: true
            archiveArtifacts artifacts: 'reports/**/*', fingerprint: true
        }
        
        failure {
            script {
                def reportPath = 'quality-reports/ci-summary.json'
                if (fileExists(reportPath)) {
                    def report = readJSON file: reportPath
                    def message = """
                    Quality Gate Failed!
                    
                    Health Score: ${report.health_metrics.overall_health_score}/100
                    Issues Found: ${report.summary.total_issues}
                    Critical Issues: ${report.summary.critical_issues}
                    
                    View detailed report: ${env.BUILD_URL}Code_Quality_Report/
                    """
                    
                    slackSend(
                        channel: '#code-quality',
                        color: 'danger',
                        message: message
                    )
                }
            }
        }
    }
}

def getQualityArgs(level) {
    switch(level) {
        case 'strict':
            return '--max-complexity 60 --min-health 70 --max-debt 20 --max-issues 25 --max-critical 0'
        case 'normal':
            return '--max-complexity 75 --min-health 60 --max-debt 30 --max-issues 50 --max-critical 0'
        case 'permissive':
            return '--max-complexity 85 --min-health 40 --max-debt 50 --max-issues 100 --max-critical 5'
        default:
            return '--max-complexity 75 --min-health 60 --max-debt 30 --max-issues 50 --max-critical 0'
    }
}
```

## GitLab CI

### Basic GitLab CI Configuration

```yaml
# .gitlab-ci.yml
stages:
  - quality-gate
  - reports

variables:
  VALKNUT_VERSION: "latest"

before_script:
  - apt-get update -qq && apt-get install -y -qq curl build-essential
  - curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  - source ~/.cargo/env
  - cargo install --git https://github.com/nathanricedev/valknut

quality-gate:
  stage: quality-gate
  script:
    - |
      valknut analyze \
        --quality-gate \
        --max-complexity 75 \
        --min-health 60 \
        --max-debt 30 \
        --format ci-summary \
        --out quality-reports/ \
        ./src
  artifacts:
    when: always
    expire_in: 1 week
    reports:
      junit: quality-reports/junit.xml
    paths:
      - quality-reports/
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH

generate-reports:
  stage: reports
  script:
    - |
      valknut analyze \
        --format html \
        --out reports/html/ \
        ./src
  artifacts:
    expire_in: 30 days
    paths:
      - reports/
  pages:
    stage: deploy
    script:
      - mkdir public
      - cp -r reports/html/* public/
    artifacts:
      paths:
        - public
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
```

### Advanced GitLab CI with Docker

```yaml
# .gitlab-ci.yml
image: rust:1.70

variables:
  DOCKER_DRIVER: overlay2
  CARGO_HOME: $CI_PROJECT_DIR/.cargo

cache:
  paths:
    - .cargo/
    - target/

stages:
  - build
  - quality-gate
  - security
  - reports
  - deploy

# Custom Docker image with Valknut pre-installed
.valknut_base:
  image: registry.gitlab.com/your-org/valknut-ci:latest
  before_script:
    - valknut --version

quality-gate:
  extends: .valknut_base
  stage: quality-gate
  script:
    - |
      valknut analyze \
        --config .valknut-ci.yml \
        --quality-gate \
        --format ci-summary \
        --out quality-reports/ \
        ./src
    - |
      # Custom script to parse results and set GitLab variables
      python3 scripts/parse_quality_results.py quality-reports/ci-summary.json
  artifacts:
    when: always
    expire_in: 1 week
    reports:
      junit: quality-reports/junit.xml
    paths:
      - quality-reports/
  variables:
    QUALITY_GATE_ENABLED: "true"

security-scan:
  extends: .valknut_base
  stage: security
  script:
    - |
      valknut analyze \
        --config .valknut-security.yml \
        --fail-on-issues \
        --max-critical 0 \
        --format sonar \
        --out security-reports/ \
        ./src
  artifacts:
    reports:
      sast: security-reports/security-report.json
  allow_failure: false

comprehensive-report:
  extends: .valknut_base
  stage: reports
  script:
    - |
      valknut analyze \
        --format html \
        --out reports/html/ \
        ./src
    - |
      valknut analyze \
        --format markdown \
        --out reports/markdown/ \
        ./src
  artifacts:
    expire_in: 30 days
    paths:
      - reports/
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
    - if: $CI_PIPELINE_SOURCE == "schedule"

pages:
  stage: deploy
  dependencies:
    - comprehensive-report
  script:
    - mkdir public
    - cp -r reports/html/* public/
    - echo "Quality reports deployed to GitLab Pages"
  artifacts:
    paths:
      - public
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
```

## Azure DevOps

### Azure DevOps Pipeline

```yaml
# azure-pipelines.yml
trigger:
  branches:
    include:
      - main
      - develop
  paths:
    exclude:
      - docs/*
      - README.md

pr:
  branches:
    include:
      - main

pool:
  vmImage: 'ubuntu-latest'

variables:
  VALKNUT_VERSION: 'latest'
  buildConfiguration: 'Release'

stages:
- stage: QualityGate
  displayName: 'Code Quality Gate'
  jobs:
  - job: Analysis
    displayName: 'Run Code Analysis'
    steps:
    - task: Cache@2
      inputs:
        key: 'valknut | "$(Agent.OS)" | "$(VALKNUT_VERSION)"'
        path: ~/.cargo/bin/valknut
      displayName: 'Cache Valknut installation'

    - script: |
        if [ ! -f ~/.cargo/bin/valknut ]; then
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          source ~/.cargo/env
          cargo install --git https://github.com/nathanricedev/valknut
        fi
      displayName: 'Install Valknut'

    - script: |
        ~/.cargo/bin/valknut analyze \
          --quality-gate \
          --max-complexity 75 \
          --min-health 60 \
          --max-debt 30 \
          --format ci-summary \
          --out $(Agent.TempDirectory)/quality-reports/ \
          ./src
      displayName: 'Run Quality Gate'

    - task: PublishTestResults@2
      condition: always()
      inputs:
        testResultsFormat: 'JUnit'
        testResultsFiles: '$(Agent.TempDirectory)/quality-reports/junit.xml'
        failTaskOnFailedTests: true
      displayName: 'Publish Quality Gate Results'

    - script: |
        ~/.cargo/bin/valknut analyze \
          --format html \
          --out $(Agent.TempDirectory)/reports/html/ \
          ./src
      displayName: 'Generate HTML Report'
      condition: always()

    - task: PublishHtmlReport@1
      condition: always()
      inputs:
        reportDir: '$(Agent.TempDirectory)/reports/html/'
        tabName: 'Code Quality Report'
      displayName: 'Publish HTML Report'

    - task: PublishPipelineArtifact@1
      condition: always()
      inputs:
        targetPath: '$(Agent.TempDirectory)/quality-reports/'
        artifactName: 'quality-reports'
      displayName: 'Publish Quality Reports'

- stage: Security
  displayName: 'Security Analysis'
  dependsOn: QualityGate
  condition: succeeded()
  jobs:
  - job: SecurityScan
    displayName: 'Security Scan'
    steps:
    - script: |
        ~/.cargo/bin/valknut analyze \
          --config .valknut-security.yml \
          --max-critical 0 \
          --format sonar \
          --out $(Agent.TempDirectory)/security-reports/ \
          ./src
      displayName: 'Run Security Analysis'

    - task: PublishPipelineArtifact@1
      condition: always()
      inputs:
        targetPath: '$(Agent.TempDirectory)/security-reports/'
        artifactName: 'security-reports'
      displayName: 'Publish Security Reports'
```

## CircleCI

### CircleCI Configuration

```yaml
# .circleci/config.yml
version: 2.1

orbs:
  rust: circleci/rust@1.6.0

executors:
  rust-executor:
    docker:
      - image: cimg/rust:1.70
    working_directory: ~/project

commands:
  install-valknut:
    description: "Install Valknut code analyzer"
    steps:
      - restore_cache:
          keys:
            - valknut-v1-{{ checksum "Cargo.lock" }}
      - run:
          name: Install Valknut
          command: |
            if [ ! -f ~/.cargo/bin/valknut ]; then
              cargo install --git https://github.com/nathanricedev/valknut
            fi
      - save_cache:
          key: valknut-v1-{{ checksum "Cargo.lock" }}
          paths:
            - ~/.cargo/bin/valknut

jobs:
  quality-gate:
    executor: rust-executor
    steps:
      - checkout
      - install-valknut
      - run:
          name: Run Quality Gate
          command: |
            ~/.cargo/bin/valknut analyze \
              --quality-gate \
              --max-complexity 75 \
              --min-health 60 \
              --max-debt 30 \
              --format ci-summary \
              --out quality-reports/ \
              ./src
      - store_test_results:
          path: quality-reports/
      - store_artifacts:
          path: quality-reports/
          destination: quality-reports

  comprehensive-analysis:
    executor: rust-executor
    steps:
      - checkout
      - install-valknut
      - run:
          name: Generate Comprehensive Report
          command: |
            ~/.cargo/bin/valknut analyze \
              --format html \
              --out reports/html/ \
              ./src
      - run:
          name: Generate Team Report
          command: |
            ~/.cargo/bin/valknut analyze \
              --format markdown \
              --out reports/markdown/ \
              ./src
      - store_artifacts:
          path: reports/
          destination: reports

workflows:
  version: 2
  quality-check:
    jobs:
      - quality-gate
      - comprehensive-analysis:
          requires:
            - quality-gate
          filters:
            branches:
              only: main
```

## SonarQube Integration

### SonarQube Import Configuration

```bash
# Generate SonarQube-compatible report
valknut analyze \
  --format sonar \
  --out sonar-reports/ \
  ./src

# Import into SonarQube
sonar-scanner \
  -Dsonar.projectKey=my-project \
  -Dsonar.sources=./src \
  -Dsonar.externalIssuesReportPaths=sonar-reports/issues.json
```

### SonarQube Properties File

```properties
# sonar-project.properties
sonar.projectKey=my-project
sonar.projectName=My Project
sonar.projectVersion=1.0

# Source configuration
sonar.sources=src/
sonar.exclusions=**/node_modules/**,**/target/**

# External analyzer integration
sonar.externalIssuesReportPaths=sonar-reports/valknut-issues.json

# Quality gate configuration
sonar.qualitygate.wait=true
```

### Docker Integration with SonarQube

```yaml
# docker-compose.yml for local SonarQube testing
version: '3.8'
services:
  sonarqube:
    image: sonarqube:community
    ports:
      - "9000:9000"
    environment:
      - SONAR_ES_BOOTSTRAP_CHECKS_DISABLE=true
    volumes:
      - sonarqube_data:/opt/sonarqube/data
      - sonarqube_logs:/opt/sonarqube/logs
      - sonarqube_extensions:/opt/sonarqube/extensions

  valknut-analysis:
    image: rust:1.70
    volumes:
      - .:/workspace
    working_dir: /workspace
    command: |
      bash -c "
        cargo install --git https://github.com/nathanricedev/valknut &&
        valknut analyze --format sonar --out sonar-reports/ ./src
      "

volumes:
  sonarqube_data:
  sonarqube_logs:
  sonarqube_extensions:
```

## Custom Integration

### REST API Integration

For custom CI/CD systems, integrate via REST API calls:

```bash
#!/bin/bash
# Custom CI script

# Run analysis and capture results
RESULT=$(valknut analyze \
  --quality-gate \
  --format json \
  --quiet \
  ./src 2>&1)

EXIT_CODE=$?

# Parse results and send to custom API
if [ $EXIT_CODE -eq 0 ]; then
  curl -X POST "https://api.example.com/quality-reports" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_TOKEN" \
    -d "$RESULT"
else
  curl -X POST "https://api.example.com/quality-failures" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_TOKEN" \
    -d "{\"error\": \"Quality gate failed\", \"details\": \"$RESULT\"}"
fi

exit $EXIT_CODE
```

### Webhook Integration

```python
#!/usr/bin/env python3
"""
Custom webhook integration for Valknut results
"""
import json
import subprocess
import requests
import sys
from typing import Dict, Any

def run_valknut_analysis(path: str, config: Dict[str, Any]) -> tuple[int, str]:
    """Run Valknut analysis and return exit code and output"""
    cmd = [
        "valknut", "analyze",
        "--quality-gate",
        "--format", "json",
        "--out", "reports/",
        path
    ]
    
    # Add quality gate options from config
    if config.get("max_complexity"):
        cmd.extend(["--max-complexity", str(config["max_complexity"])])
    if config.get("min_health"):
        cmd.extend(["--min-health", str(config["min_health"])])
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
        return result.returncode, result.stdout
    except subprocess.TimeoutExpired:
        return 4, "Analysis timeout after 5 minutes"

def send_webhook(webhook_url: str, data: Dict[str, Any]) -> None:
    """Send results to webhook endpoint"""
    try:
        response = requests.post(
            webhook_url,
            json=data,
            headers={"Content-Type": "application/json"},
            timeout=10
        )
        response.raise_for_status()
        print(f"Webhook sent successfully: {response.status_code}")
    except requests.exceptions.RequestException as e:
        print(f"Webhook failed: {e}")

def main():
    config = {
        "max_complexity": 75,
        "min_health": 60,
        "webhook_url": "https://your-webhook-url.com/quality-reports"
    }
    
    exit_code, output = run_valknut_analysis("./src", config)
    
    webhook_data = {
        "status": "passed" if exit_code == 0 else "failed",
        "exit_code": exit_code,
        "analysis_output": output,
        "project": "my-project",
        "branch": os.environ.get("CI_BRANCH", "unknown"),
        "commit": os.environ.get("CI_COMMIT", "unknown")
    }
    
    send_webhook(config["webhook_url"], webhook_data)
    sys.exit(exit_code)

if __name__ == "__main__":
    main()
```

## Best Practices

### 1. Quality Gate Strategy

#### Gradual Implementation
```bash
# Phase 1: Monitoring only (no failures)
valknut analyze --format ci-summary ./src

# Phase 2: Permissive gates
valknut analyze --quality-gate --max-complexity 90 --min-health 30 ./src

# Phase 3: Standard gates  
valknut analyze --quality-gate --max-complexity 75 --min-health 60 ./src

# Phase 4: Strict gates
valknut analyze --quality-gate --max-complexity 60 --min-health 70 ./src
```

#### Branch-Specific Configuration
```yaml
# Different standards for different branches
- name: Quality Gate - Main Branch
  if: github.ref == 'refs/heads/main'
  run: |
    valknut analyze --quality-gate \
      --max-complexity 60 --min-health 70 --max-critical 0 ./src

- name: Quality Gate - Feature Branch  
  if: github.ref != 'refs/heads/main'
  run: |
    valknut analyze --quality-gate \
      --max-complexity 75 --min-health 60 --max-critical 2 ./src
```

### 2. Performance Optimization

#### Caching Strategy
```yaml
# Cache Valknut installation and analysis results
- name: Cache Valknut
  uses: actions/cache@v4
  with:
    path: |
      ~/.cargo/bin/valknut
      ~/.valknut/cache/
    key: valknut-${{ hashFiles('**/Cargo.lock') }}
```

#### Parallel Analysis
```bash
# Analyze different components in parallel
valknut analyze --config backend-config.yml ./backend &
valknut analyze --config frontend-config.yml ./frontend &
valknut analyze --config shared-config.yml ./shared &
wait
```

#### Incremental Analysis
```bash
# Only analyze changed files (future feature)
git diff --name-only HEAD~1 HEAD | \
  grep -E '\.(py|js|ts|rs)$' | \
  xargs valknut analyze --quality-gate
```

### 3. Configuration Management

#### Environment-Specific Configs
```
configs/
├── development.yml     # Permissive for dev
├── staging.yml        # Standard for staging  
├── production.yml     # Strict for production
└── security.yml       # Security-focused analysis
```

#### Team-Specific Standards
```yaml
# Team A: Frontend team
analysis:
  focus_areas: ["ui", "components"]
quality_gates:
  max_complexity: 70
  min_health: 65

# Team B: Backend team  
analysis:
  enable_security_analysis: true
  focus_areas: ["api", "services"]
quality_gates:
  max_complexity: 75
  min_health: 60
  max_critical: 0
```

### 4. Reporting Strategy

#### Multi-Format Reports
```bash
# Generate reports for different audiences
valknut analyze --format html --out reports/executive/ ./src     # Leadership
valknut analyze --format markdown --out reports/technical/ ./src # Developers  
valknut analyze --format csv --out reports/metrics/ ./src        # Analytics
```

#### Historical Tracking
```python
# Track quality metrics over time
import json
from datetime import datetime

def store_metrics(analysis_result: dict) -> None:
    timestamp = datetime.now().isoformat()
    
    metrics = {
        "timestamp": timestamp,
        "health_score": analysis_result["health_metrics"]["overall_health_score"],
        "complexity_score": analysis_result["health_metrics"]["complexity_score"],
        "technical_debt": analysis_result["health_metrics"]["technical_debt_ratio"],
        "total_issues": analysis_result["summary"]["total_issues"]
    }
    
    # Store in time series database or append to file
    with open("quality-metrics.jsonl", "a") as f:
        f.write(json.dumps(metrics) + "\n")
```

## Troubleshooting

### Common Issues

#### 1. Installation Problems
```bash
# Problem: Cargo not found
# Solution: Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Problem: Permission denied
# Solution: Use user installation
cargo install --git https://github.com/nathanricedev/valknut --locked
```

#### 2. Quality Gate Failures
```bash
# Problem: Unexpectedly failing quality gates
# Solution: Generate detailed report for investigation
valknut analyze --verbose --format html --out debug-reports/ ./src

# Problem: False positive issues
# Solution: Adjust configuration
valknut analyze --config relaxed-config.yml --quality-gate ./src
```

#### 3. Performance Issues
```bash
# Problem: Analysis too slow for CI
# Solution: Limit scope and enable parallel processing
valknut analyze --max-files 500 --exclude-patterns "**/node_modules/**" ./src

# Problem: Memory usage too high
# Solution: Use streaming analysis
export VALKNUT_MEMORY_LIMIT=512MB
valknut analyze ./src
```

#### 4. Configuration Issues
```bash
# Problem: Configuration not found
# Solution: Specify absolute path
valknut analyze --config $(pwd)/.valknut.yml ./src

# Problem: Invalid configuration
# Solution: Validate before use
valknut validate-config --config .valknut.yml --verbose
```

### Debug Commands

```bash
# Enable verbose logging
valknut --verbose analyze ./src

# Rust-level debugging
RUST_LOG=debug valknut analyze ./src

# Performance profiling
RUST_LOG=valknut::performance=trace valknut analyze ./src

# Memory usage tracking
valgrind --tool=massif valknut analyze ./src
```

### Support Resources

- **GitHub Issues**: https://github.com/nathanricedev/valknut/issues
- **Discussions**: https://github.com/nathanricedev/valknut/discussions
- **Documentation**: https://github.com/nathanricedev/valknut/docs
- **Configuration Examples**: https://github.com/nathanricedev/valknut/examples

This guide provides comprehensive integration patterns for most CI/CD systems. Adapt the examples to your specific environment and quality standards.
