# Contributing to TruthBeacon

Thank you for your interest in contributing to **TruthBeacon**! We welcome contributions that help protect community leaders and members from identity impersonation and deception.

Please take a moment to review this document before submitting your contribution.

---

## Code of Conduct

By participating in this project, you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md). Please report any unacceptable behavior to [conduct@orangeheart.industries](mailto:conduct@orangeheart.industries).

---

## Security Vulnerabilities

**Do not file public GitHub issues for security vulnerabilities.**

If you discover a potential security bug or vulnerability, please refer to our [Security Policy](.github/SECURITY.md) and report it privately through [GitHub Private Vulnerability Reporting](https://github.com/orangeheart-industries/truth-beacon/security/advisories/new).

---

## How Can I Contribute?

### 1. Reporting Bugs
- Search existing issues before creating a new one to avoid duplicates.
- Use the **Bug Report** template provided in `.github/ISSUE_TEMPLATE/bug_report.yml`.
- Include your operating system, TruthBeacon version, steps to reproduce, and any relevant sanitized log excerpts.

### 2. Suggesting Enhancements
- Use the **Feature Request** template in `.github/ISSUE_TEMPLATE/feature_request.yml`.
- Describe the problem you are solving, the proposed solution, and alternative approaches considered.

### 3. Submitting Pull Requests
1. **Fork** the repository and create your feature branch:
   ```bash
   git checkout -b feature/impersonation-detection-enhancement
   ```
2. **Make your changes** following the project structure:
   - Rust backend & Tauri core: `src-tauri/`
   - Frontend presentation layer: `ui/`
   - Documentation & specifications: `docs/`
3. **Run local verification checks** (see [Testing & Quality Checks](#testing--quality-checks) below).
4. **Commit your changes** with clear, descriptive commit messages.
5. **Push to your fork** and submit a Pull Request targeting the `main` branch. Fill in the [Pull Request Template](.github/PULL_REQUEST_TEMPLATE.md).

---

## Development Setup

### Prerequisites
- **Rust**: Latest stable toolchain (via `rustup`).
- **Node.js** (Optional/Standard runtime): Node.js 18+ (if building UI bundles, though TruthBeacon maintains a zero-dependency, vanilla UI).
- **System Dependencies** (Linux only):
  ```bash
  sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev libsecret-1-dev
  ```

### Building & Running Locally
```bash
# Navigate to Tauri core
cd src-tauri

# Run in development mode with live window
cargo tauri dev
```

---

## Testing & Quality Checks

Our CI pipeline enforces strict compiler and supply-chain quality standards. Before opening a PR, ensure all the following pass locally:

```bash
# 1. Format check
cd src-tauri
cargo fmt -- --check

# 2. Compiler linting with zero warnings allowed
cargo clippy --all-targets --all-features -- -D warnings

# 3. Automated test suite
cargo test --all-targets

# 4. Supply-chain policy and dependency ban audit
cargo deny check

# 5. Known security vulnerability audit
cargo audit --ignore RUSTSEC-2023-0080

# 6. Licensing and asset compliance (from repo root)
cd ..
python3 scripts/audit_licenses.py

# 7. Packaging pipeline verification
python3 scripts/verify_packaging_pipeline.py
```

---

## Coding Guidelines

- **Zero Cloud Telemetry**: TruthBeacon strictly operates 100% locally. Never introduce outbound telemetry, network tracking, or remote metric beacons.
- **Credential Storage**: Sensitive secrets (like Discord bot tokens) must strictly be stored in native OS keychains via the `keyring` crate. Never write secrets to plaintext disk files or databases.
- **Database Migrations**: Changes to the SQLite schema must be backward-compatible and use WAL mode.

---

## License

By contributing to TruthBeacon, you agree that your contributions will be licensed under the dual **MIT** and **Apache-2.0** licenses as described in the [LICENSE](LICENSE) file.
