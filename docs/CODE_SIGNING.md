# Windows code signing (SmartScreen)

The updater signature and the Windows SmartScreen warning are **different things**:

- **Tauri updater (minisign)**: proves that an update came from the holder of the
  updater private key. It is already configured and working.
- **Authenticode**: proves to Windows who published the installer. Without it,
  Windows shows **"Unknown publisher"** and SmartScreen may block the installer.

That is why the 0.1.1 installer showed "Windows protected your PC". The file is
valid; Windows simply does not know the publisher.

## Right now

- Click **More info → Run anyway** (or **Run anyway** on the warning screen).
- This is expected for unsigned builds.
- The updater itself still verifies every downloaded update with the embedded
  minisign public key.

## To remove the warning

You need an Authenticode code-signing certificate. It is a paid/identity-verified
certificate and is separate from the updater key.

Common options:

1. **OV/EV code-signing certificate** from a CA (DigiCert, Sectigo, SSL.com, etc.).
2. **Azure Trusted Signing** (good fit for open source and smaller teams; requires
   identity validation).

### Option A — certificate in the Windows certificate store

Install the certificate on the build machine and configure Tauri:

```json
{
  "bundle": {
    "windows": {
      "certificateThumbprint": "THUMBPRINT",
      "digestAlgorithm": "sha256",
      "timestampUrl": "http://timestamp.digicert.com"
    }
  }
}
```

The CI runner must have the certificate available (import it from a secret/PFX in
a workflow step).

### Option B — custom signing command

Tauri can call a signing tool through `bundle.windows.signCommand`. This is the
usual path for Azure Trusted Signing / `signtool` wrappers:

```json
{
  "bundle": {
    "windows": {
      "signCommand": "trusted-signing-cli -e https://<region>.codesigning.azure.net -a <account> -c <profile> -d BetaStart %1"
    }
  }
}
```

Keep the signing credentials in GitHub Actions Secrets, never in the repository.

## CI

Add the signing credentials as repository secrets and pass them to the release
step. The rest of the workflow already builds, signs the updater artifact, and
publishes `latest.json`.

## Important

- Code signing does **not** replace the updater signature. Keep both.
- Losing the updater private key or password means existing users cannot receive
  new updates. Store them in a vault.
- SmartScreen reputation is also built over time; an EV certificate or Azure
  Trusted Signing gives a much better first-run experience than an OV certificate.

Reference: <https://v2.tauri.app/distribute/sign/windows/>
