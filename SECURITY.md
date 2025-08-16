# Security Policy

## Supported Versions

We take security seriously. The following versions of Redfire Switch are currently supported with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We appreciate your efforts to responsibly disclose security vulnerabilities. Please follow these guidelines:

### How to Report

**Do NOT** file a public GitHub issue for security vulnerabilities.

Instead, please report security vulnerabilities via email to:
- **Email**: security@carrierone.com
- **Subject**: [SECURITY] Redfire Switch Vulnerability Report

### What to Include

Please include the following information in your report:

1. **Description** - A clear description of the vulnerability
2. **Impact** - What systems or data could be affected
3. **Reproduction Steps** - Detailed steps to reproduce the issue
4. **Proof of Concept** - Code or demonstration (if applicable)
5. **Suggested Fix** - If you have ideas for remediation
6. **Contact Information** - How we can reach you for follow-up

### Response Timeline

We commit to the following response times:

- **Initial Response**: Within 48 hours
- **Impact Assessment**: Within 1 week  
- **Fix Development**: Based on severity (see below)
- **Public Disclosure**: Coordinated with reporter

### Severity Levels

| Severity | Description | Fix Timeline |
|----------|-------------|--------------|
| **Critical** | Remote code execution, privilege escalation | 1-7 days |
| **High** | Data exposure, authentication bypass | 1-2 weeks |
| **Medium** | Information disclosure, DoS | 2-4 weeks |
| **Low** | Minor security issues | Next release |

## Security Features

Redfire Switch includes several security features:

### Network Security
- **TLS Encryption** - End-to-end encryption for SIP signaling
- **SRTP** - Secure Real-time Transport Protocol for media
- **Authentication** - Digest authentication and certificate-based auth
- **Rate Limiting** - Protection against DoS attacks
- **Firewall Integration** - fail2ban integration for brute force protection

### Application Security
- **Input Validation** - Comprehensive input sanitization
- **Memory Safety** - Rust's memory safety guarantees
- **Secure Defaults** - Security-first configuration defaults
- **Privilege Separation** - Run with minimal required privileges
- **Audit Logging** - Comprehensive security event logging

### Codec Security
- **Buffer Protection** - GPU memory bounds checking
- **State Isolation** - Per-session codec state isolation
- **Fallback Safety** - Secure CPU fallback mechanisms
- **Input Validation** - Audio frame validation and sanitization

## Security Best Practices

### Deployment
- Run Redfire Switch with dedicated user account (non-root)
- Use TLS for all SIP communications
- Enable SRTP for media streams
- Configure firewall rules appropriately
- Regularly update dependencies
- Monitor security logs

### Configuration
- Change default passwords and secrets
- Use strong authentication mechanisms
- Enable security logging
- Configure rate limiting
- Disable unnecessary features
- Use least-privilege access controls

### Monitoring
- Monitor for suspicious SIP activity
- Track authentication failures
- Monitor codec transcoding anomalies
- Set up alerts for security events
- Regular security audits

## Vulnerability Management

### Dependency Security
- We regularly audit and update dependencies
- Use `cargo audit` for Rust security advisories
- Monitor security advisories for CUDA/ROCm
- Apply security patches promptly

### Code Security
- Static analysis with Clippy and additional security linters
- Memory safety verification
- Fuzzing of codec implementations
- Security-focused code reviews
- Regular penetration testing

## Security Contact

For security-related questions or concerns:

- **General Security**: security@carrierone.com
- **Commercial Support**: enterprise@carrierone.com
- **Urgent Issues**: Call +1-XXX-XXX-XXXX (24/7 for enterprise customers)

## Acknowledgments

We appreciate the security research community and will acknowledge responsible disclosure contributors (with their permission) in our security advisories.

### Hall of Fame
*Reserved for future security researchers who help improve Redfire Switch security*

## Legal

- We will not pursue legal action against good-faith security researchers
- We request that you do not access user data or disrupt services
- Please delete any data obtained during testing
- Coordinate public disclosure timing with our team

Thank you for helping keep Redfire Switch secure! 🔒