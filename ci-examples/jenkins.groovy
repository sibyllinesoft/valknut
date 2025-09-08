// Jenkins Pipeline - Code Quality Gate with Valknut
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
        
        stage('Setup Valknut') {
            steps {
                script {
                    // Download Valknut binary
                    sh '''
                        curl -L -o valknut "https://github.com/your-repo/valknut/releases/latest/download/valknut-linux-x86_64"
                        chmod +x valknut
                    '''
                }
            }
        }
        
        stage('Code Quality Analysis') {
            steps {
                script {
                    // Run analysis with quality gates
                    def analysisResult = sh(
                        script: '''
                            ./valknut analyze . \
                              --format ci-summary \
                              --quality-gate \
                              --max-issues 5 \
                              --min-health 70 \
                              --max-complexity 80 \
                              --min-maintainability 60 \
                              --quiet
                        ''',
                        returnStatus: true
                    )
                    
                    // Parse results
                    if (fileExists('out/ci_summary.json')) {
                        def results = readJSON file: 'out/ci_summary.json'
                        
                        echo "Quality Analysis Results:"
                        echo "========================"
                        echo "Status: ${results.status}"
                        echo "Health Score: ${results.metrics.overall_health_score}"
                        echo "Total Issues: ${results.summary.total_issues}"
                        echo "Critical Issues: ${results.summary.critical_issues}"
                        
                        // Set build properties
                        currentBuild.displayName = "#${env.BUILD_NUMBER} - Health: ${results.metrics.overall_health_score}%"
                        
                        if (results.summary.total_issues > 0) {
                            echo "Recommendations:"
                            results.quality_gates.recommendations.each { rec ->
                                echo "- ${rec}"
                            }
                        }
                        
                        // Create quality gate summary
                        def summary = [
                            healthScore: results.metrics.overall_health_score,
                            totalIssues: results.summary.total_issues,
                            criticalIssues: results.summary.critical_issues,
                            status: results.status
                        ]
                        
                        writeJSON file: 'quality_summary.json', json: summary
                        
                        // Fail the build if quality gates failed
                        if (analysisResult != 0) {
                            currentBuild.result = 'UNSTABLE'
                            error("Quality gates failed! Health Score: ${results.metrics.overall_health_score}%, Issues: ${results.summary.total_issues}")
                        }
                    } else {
                        error("Analysis results not found")
                    }
                }
            }
            post {
                always {
                    // Archive analysis results
                    archiveArtifacts artifacts: 'out/**/*', allowEmptyArchive: true
                    archiveArtifacts artifacts: 'quality_summary.json', allowEmptyArchive: true
                }
            }
        }
        
        stage('Generate Reports') {
            when {
                anyOf {
                    branch 'main'
                    branch 'develop'
                }
            }
            steps {
                script {
                    // Generate detailed HTML report for main branches
                    sh '''
                        ./valknut analyze . \
                          --format html \
                          --out detailed-report \
                          --quiet
                    '''
                    
                    // Publish HTML report
                    publishHTML([
                        allowMissing: false,
                        alwaysLinkToLastBuild: true,
                        keepAll: true,
                        reportDir: 'detailed-report',
                        reportFiles: 'team_report.html',
                        reportName: 'Valknut Code Quality Report'
                    ])
                }
            }
        }
    }
    
    post {
        always {
            // Clean workspace
            cleanWs()
        }
        
        success {
            echo "✅ Pipeline completed successfully!"
            
            // Send notification for successful quality gates
            script {
                if (fileExists('quality_summary.json')) {
                    def summary = readJSON file: 'quality_summary.json'
                    if (summary.status == 'success') {
                        // You can add Slack/Teams notification here
                        echo "Quality gates passed! Health Score: ${summary.healthScore}%"
                    }
                }
            }
        }
        
        unstable {
            echo "⚠️ Pipeline completed with quality issues"
            
            // Send notification for failed quality gates
            script {
                if (fileExists('quality_summary.json')) {
                    def summary = readJSON file: 'quality_summary.json'
                    // You can add Slack/Teams notification here
                    echo "Quality gates failed! Health Score: ${summary.healthScore}%, Issues: ${summary.totalIssues}"
                }
            }
        }
        
        failure {
            echo "❌ Pipeline failed"
        }
    }
}