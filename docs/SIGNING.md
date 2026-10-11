# Optional Windows signing

Signing is **not configured**, and current builds are unsigned. No signing certificate, token, API key, or test credential is included.

Tauri supports a Windows `bundle.windows.signCommand` configuration for a securely provisioned external signer. After configuring the certificate issuer's tooling, create a private `signing.local.json` overlay, for example:

```json
{
  "bundle": {
    "windows": {
      "signCommand": "your-organization-signer sign %1"
    }
  }
}
```

The executable above is a **placeholder** for your own installed signer, not a shipped tool. `%1` is the file supplied by Tauri. The signer must fail with a nonzero exit code on any signing failure and obtain credentials through the issuer's secure mechanism (hardware key, OIDC or protected environment/secret store), not inline configuration or command arguments. Then build with:

```powershell
npm run desktop:build -- --config signing.local.json
```

For CI, provision the signer and short-lived credentials in a protected release environment, then supply the same overlay to the build command. Do not expose signing secrets to pull requests. Verify the final installer and executable with `Get-AuthenticodeSignature`, the correct publisher identity and timestamp, and only then generate checksums and publish. The provided release workflow currently creates **unsigned draft prereleases** and must be deliberately updated to use the signer once provisioned.

See the [official Tauri Windows signing guide](https://v2.tauri.app/distribute/sign/windows/). Updater signing keys do not replace Windows Authenticode signing. No updater is included in this version.
