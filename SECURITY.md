# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.4.x   | :white_check_mark: |
| < 0.4   | :x:                |

## Reporting a Vulnerability

We take the security of `tradingview-rs` and its users seriously. If you discover a security vulnerability, please report it responsibly by contacting our security team:

- **Email**: `security@bitbytelab.io` (or `dat.nguyen@bitbytelab.io`)
- **Response Time**: We acknowledge receipt of vulnerability reports within **48 hours** and aim to provide a security patch or remediation guidance within **7 days**.

Please do **not** report security vulnerabilities through public GitHub issues or discussions.

### What to Include

To help us investigate and triage the issue quickly, please provide:

1. A clear description of the vulnerability and its potential impact.
2. Step-by-step reproduction instructions or a minimal proof-of-concept (PoC).
3. Information about your environment (OS, architecture, Rust compiler version, Python version).
4. Any potential mitigations or suggested fixes.

## Security Principles in tradingview-rs

- **Zero Secrets Logging**: Credentials, auth tokens, session cookies, and TOTP secrets are strictly excluded from `Debug` and `Display` implementations and are never printed to logs or standard error.
- **Private Cookie Storage**: Session cookie exports default to Unix permissions `0o600` (`rw-------`) and enforce `create_new(true)` to prevent symlink attacks and unintentional file overwriting.
- **Supply Chain & Signing**: All release commits and Git release tags are cryptographically signed with SSH/GPG keys. All dependencies are locked via `Cargo.lock`.
