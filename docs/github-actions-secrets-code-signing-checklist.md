# GitHub Actions Code Signing Secrets Checklist

**TruthBeacon Release Engineering — Orange Heart Industries**  
*Comprehensive Guide: From Obtaining Credentials to GitHub Actions Secrets Setup*

---

## 1. Overview & Architecture

To eliminate security warnings (**Apple macOS Gatekeeper** and **Microsoft Windows SmartScreen**) across releases, binaries must be digitally signed and notarized during the automated build pipeline.

Private keys and certificates are **never checked into git** or embedded into application code. Instead, they are stored as encrypted **GitHub Actions Repository Secrets** and injected into the runner's ephemeral keychain / certificate store only during release builds.

---

## 2. Secrets Reference Matrix

Add the following secrets to GitHub under:  
**Repository Settings → Secrets and variables → Actions → New repository secret**

| Secret Name | Platform | Purpose | Value Format / Example |
| :--- | :--- | :--- | :--- |
| `APPLE_CERTIFICATE_BASE64` | macOS | Base64-encoded Developer ID Application `.p12` certificate | Single line Base64 string (`MIIKqQIBAzCC...`) |
| `APPLE_CERTIFICATE_PASSWORD` | macOS | Password used when exporting the `.p12` certificate | Plain text string |
| `APPLE_SIGNING_IDENTITY` | macOS | Name of the certificate identity matching `codesign` | `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | macOS | Apple Developer Account email address | `developer@example.com` |
| `APPLE_APP_SPECIFIC_PASSWORD` | macOS | 16-character Apple Notary service password | `xxxx-xxxx-xxxx-xxxx` |
| `APPLE_TEAM_ID` | macOS | 10-character alphanumeric Apple Developer Team ID | `ABC123XYZ4` |
| `WINDOWS_CERT_BASE64` | Windows | Base64-encoded Authenticode `.pfx` certificate | Single line Base64 string (`MIIKqQIBAzCC...`) |
| `WINDOWS_CERT_PASSWORD` | Windows | Password for the `.pfx` certificate file | Plain text string |

---

## 3. Apple Developer Credentials Setup (macOS)

### Step 1: Retrieve Your Apple Team ID & Apple ID
1. Log in to [developer.apple.com/account](https://developer.apple.com/account).
2. Click on **Membership details** in the sidebar.
3. Note your **Team ID** (10-character alphanumeric code, e.g., `A1B2C3D4E5`).
4. Note your **Apple ID** (the email associated with your developer account).

### Step 2: Generate an App-Specific Password for Notarization
*Apple Notary Service (`xcrun notarytool`) requires an app-specific password instead of your primary Apple ID password.*
1. Go to [appleid.apple.com](https://appleid.apple.com) and sign in.
2. Navigate to **Sign-In and Security → App-Specific Passwords**.
3. Click **+** (Generate an app-specific password).
4. Label it: `truthbeacon-notarization`.
5. Copy the generated 16-character password (formatted as `xxxx-xxxx-xxxx-xxxx`).
   - **GitHub Secret**: `APPLE_APP_SPECIFIC_PASSWORD`

### Step 3: Create & Export Developer ID Application Certificate (`.p12`)
1. On your Mac, open **Keychain Access** (`/Applications/Utilities/Keychain Access.app`).
2. In the menu bar, select **Keychain Access → Certificate Assistant → Request a Certificate from a Certificate Authority...**
   - User Email: Your developer email.
   - Common Name: Your name or organization name.
   - Request is: **Saved to disk**.
   - Click **Continue** and save `CertificateSigningRequest.certSigningRequest`.
3. In [developer.apple.com/account/resources/certificates/list](https://developer.apple.com/account/resources/certificates/list):
   - Click the blue **+** button next to Certificates.
   - Under *Software*, select **Developer ID Application** (do **not** select Apple Development or Mac App Store).
   - Click **Continue**.
   - Upload the `CertificateSigningRequest.certSigningRequest` file you saved.
   - Click **Generate**, then click **Download** to obtain `developerID_application.cer`.
4. Double-click `developerID_application.cer` on your Mac to import it into your **login** keychain.
5. In **Keychain Access**, select **My Certificates** in the left sidebar.
6. Find your new certificate: `Developer ID Application: <Your Name/Org> (<TEAMID>)`.
7. Expand the arrow next to it to verify the private key is nested underneath.
8. Right-click the certificate and choose **Export "Developer ID Application: ..."**.
9. Select format: **Personal Information Exchange (.p12)**.
10. Save as `DeveloperID_Application.p12`.
11. Enter an export password when prompted (remember this password).
    - **GitHub Secret**: `APPLE_CERTIFICATE_PASSWORD`

### Step 4: Extract the Signing Identity String
Run the following in Terminal on your Mac:
```bash
security find-identity -v -p codesigning
```
Locate the line reading:
```text
1) 1A2B3C4D5E6F... "Developer ID Application: Orange Heart Industries LLC (ABC123XYZ4)"
```
Copy the full string inside the quotes (e.g., `Developer ID Application: Orange Heart Industries LLC (ABC123XYZ4)`).
- **GitHub Secret**: `APPLE_SIGNING_IDENTITY`

### Step 5: Convert the `.p12` to Base64
In Terminal on your Mac, convert your `.p12` file to a single-line Base64 string and copy it to the clipboard:
```bash
base64 -i DeveloperID_Application.p12 | pbcopy
```
- **GitHub Secret**: `APPLE_CERTIFICATE_BASE64`

---

## 4. Microsoft Code Signing Credentials Setup (Windows)

### Step 1: Obtain Authenticode Certificate (`.pfx`)
You will have received a code signing certificate file (`.pfx` or `.p12`) from an authorized Certificate Authority (DigiCert, Sectigo, GlobalSign, SSL.com) or an export from your Windows Certificate Store.

### Step 2: Convert the `.pfx` to Base64
#### If using macOS:
```bash
base64 -i your_windows_cert.pfx | pbcopy
```

#### If using Windows (PowerShell):
```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("your_windows_cert.pfx")) | Set-Clipboard
```
- **GitHub Secret**: `WINDOWS_CERT_BASE64`

### Step 3: Certificate Password
Note the password assigned to your `.pfx` certificate file.
- **GitHub Secret**: `WINDOWS_CERT_PASSWORD`

### Step 4: Instant SmartScreen Reputation vs OV Certificate
- **EV (Extended Validation) Certificate**: Instant zero-warning reputation across all Windows 10 & 11 PCs upon signing.
- **Standard (OV) Certificate**: To bypass download volume reputation build-up:
  1. Once the signed `.exe` is generated, submit it to [Microsoft Defender Security Intelligence](https://www.microsoft.com/en-us/wdsi/filesubmission).
  2. Select **Software Developer** → **Incorrectly detected as malware / SmartScreen warning**.
  3. Microsoft will verify the Authenticode signature and whitelist the binary within hours.

---

## 5. Inputting Secrets into GitHub Repository

1. Open your browser and navigate to:  
   `https://github.com/boredpolymath/truth-beacon/settings/secrets/actions`
2. Click the green **New repository secret** button.
3. Enter the secret name and value for each item in the table below:

### Checklist:

- [ ] **`APPLE_CERTIFICATE_BASE64`**
  - Name: `APPLE_CERTIFICATE_BASE64`
  - Value: *(Paste clipboard output from `base64 -i DeveloperID_Application.p12 | pbcopy`)*
- [ ] **`APPLE_CERTIFICATE_PASSWORD`**
  - Name: `APPLE_CERTIFICATE_PASSWORD`
  - Value: *(The password entered when exporting the `.p12`)*
- [ ] **`APPLE_SIGNING_IDENTITY`**
  - Name: `APPLE_SIGNING_IDENTITY`
  - Value: `Developer ID Application: Your Name/Org (TEAMID)`
- [ ] **`APPLE_ID`**
  - Name: `APPLE_ID`
  - Value: `your-apple-id@example.com`
- [ ] **`APPLE_APP_SPECIFIC_PASSWORD`**
  - Name: `APPLE_APP_SPECIFIC_PASSWORD`
  - Value: `xxxx-xxxx-xxxx-xxxx`
- [ ] **`APPLE_TEAM_ID`**
  - Name: `APPLE_TEAM_ID`
  - Value: `10-character Team ID (e.g. ABC123XYZ4)`
- [ ] **`WINDOWS_CERT_BASE64`**
  - Name: `WINDOWS_CERT_BASE64`
  - Value: *(Paste clipboard output from base64 encoding your `.pfx`)*
- [ ] **`WINDOWS_CERT_PASSWORD`**
  - Name: `WINDOWS_CERT_PASSWORD`
  - Value: *(The password for the `.pfx` file)*

---

## 6. Triggering & Verifying the Signed Release

Once all secrets are saved in GitHub:

1. **Trigger the Pipeline**:
   - Push a new version tag:
     ```bash
     git tag -a v0.1.1 -m "TruthBeacon v0.1.1 - Cryptographically Signed Release"
     git push origin v0.1.1
     ```
   - *Or trigger manually*: Go to **Actions → Release Packaging → Run workflow**.

2. **Monitor the Build Jobs**:
   - **`build-macos-universal`**:
     - Automatically imports the `.p12` certificate into the runner keychain.
     - Signs the `.app` with Hardened Runtime and entitlements.
     - Submits `TruthBeacon_0.1.0_universal.dmg` to Apple Notary Service via `xcrun notarytool`.
     - Validates and staples the ticket with `xcrun stapler staple`.
   - **`build-windows`**:
     - Decodes `cert.pfx` from `WINDOWS_CERT_BASE64`.
     - Invokes `signtool.exe` with SHA-256 and DigiCert timestamping across MSI, NSIS, and Portable binaries.

3. **Verify the Output Binaries**:
   - **macOS Gatekeeper Verification**:
     ```bash
     spctl --assess --type open --context context:primary-signature --verbose TruthBeacon_0.1.0_universal.dmg
     # Output: accepted, source=Notarized Developer ID
     ```
   - **Windows Authenticode Verification**:
     ```powershell
     Get-AuthenticodeSignature .\TruthBeacon_0.1.0_x64-setup.exe
     # Output: Status: Valid
     ```
