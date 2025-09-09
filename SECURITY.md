# Security Policy

## Supported Versions

We actively support the following versions with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in Valknut, please report it responsibly:

### Preferred Method: Private Security Advisory
1. Go to the [Security tab](https://github.com/nathanricedev/valknut/security) on GitHub
2. Click "Report a vulnerability" 
3. Fill out the security advisory form with details

### Alternative: Direct Email
Send an email to **nathan@sibylline.dev** with:
- Subject line: "Valknut Security Vulnerability Report"
- Detailed description of the vulnerability
- Steps to reproduce (if applicable)
- Potential impact assessment
- Any suggested mitigations

### What to Include
Please include as much of the following information as possible:
- Type of vulnerability (e.g., buffer overflow, injection, etc.)
- Product version(s) affected
- Special configuration required to reproduce
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if available)
- Impact of the vulnerability and how an attacker might exploit it

## Response Timeline

- **Acknowledgment**: We will acknowledge receipt of your report within 2 business days
- **Initial Assessment**: We will provide an initial assessment within 5 business days
- **Status Updates**: We will provide regular updates on our progress every 5-7 days
- **Resolution Timeline**: We aim to resolve critical vulnerabilities within 30 days

## Security Update Process

1. **Vulnerability Assessment**: We evaluate the severity and impact
2. **Fix Development**: We develop and test a security patch
3. **Coordinated Disclosure**: We coordinate with you on disclosure timing
4. **Security Advisory**: We publish a security advisory with details
5. **Patch Release**: We release a patched version
6. **Community Notification**: We notify the community through appropriate channels

## Disclosure Policy

- We practice **responsible disclosure**
- We request that you do not publicly disclose the vulnerability until we have had a chance to address it
- We will publicly acknowledge your contribution (with your permission)
- We may offer recognition in our security hall of fame

## Security Best Practices for Users

### General Usage
- Always use the latest stable version
- Regularly update dependencies with `cargo update`
- Run security audits with `cargo audit`
- Review configuration files for sensitive information

### CI/CD Integration
- Use secure environment variables for sensitive configuration
- Limit analysis scope to necessary directories only
- Review generated reports before sharing publicly
- Implement access controls for analysis results

### Configuration Security
- Avoid hardcoding sensitive paths or credentials in configuration files
- Use appropriate file permissions for configuration files (600 or 644)
- Regularly rotate any API keys or tokens used with external services

## Known Security Considerations

### Static Analysis Limitations
- Valknut performs static code analysis and does not execute analyzed code
- However, it does parse and process file contents, so ensure input sources are trusted
- Be cautious when analyzing code from untrusted sources

### Dependency Security
- We actively monitor our dependencies for security vulnerabilities
- We use automated tools to scan for known vulnerabilities
- Critical security updates are prioritized for immediate release

### Data Privacy
- Valknut processes source code locally by default
- No code is sent to external services without explicit configuration
- Generated reports may contain code snippets - review before sharing

## Security Features

- **Input Validation**: Comprehensive validation of all user inputs and configuration
- **Sandboxed Analysis**: Code analysis runs in a controlled environment
- **Secure Defaults**: Conservative default settings prioritize security
- **Audit Logging**: Comprehensive logging for security monitoring
- **Dependency Scanning**: Automated scanning of dependencies for vulnerabilities

## Hall of Fame

We recognize security researchers who help make Valknut more secure:

<!-- Future security researchers will be acknowledged here -->

---

For general questions about Valknut security, please see our [FAQ](README.md#faq) or open a general [issue](https://github.com/nathanricedev/valknut/issues).

**Last Updated**: September 9, 2025