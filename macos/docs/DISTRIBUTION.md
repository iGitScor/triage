# Signing and distribution

## Local builds

Remora keeps all secrets in **one** Keychain item (service `fr.igitscor.remora`, account `secrets`),
read once per launch.

macOS lets an app read its Keychain item without asking only if it recognizes the app. For apps signed
with an Apple-issued certificate it remembers the **team ID**, which stays the same across rebuilds.
For self-signed or ad hoc builds it remembers the binary's fingerprint, which changes on every rebuild:
expect **one** prompt per new build (choose *Always Allow*).

`scripts/build-app.sh` picks, in order: `REMORA_SIGN_IDENTITY`, an *Apple Development* certificate,
*Remora Local Signing* (or *Perch Local Signing*, its earlier name), then ad hoc.

**No prompts at all (recommended):** get a free Apple Development certificate. Xcode → Settings →
Accounts → add your Apple ID → Manage Certificates → + → Apple Development. Then `make run`.

`scripts/make-signing-cert.sh` creates the self-signed *Remora Local Signing* certificate. It keeps a
stable code identity but, without a team ID, does not stop the Keychain prompt.

## Sharing with colleagues

Users of a distributed build never create certificates. What you need on your side:

1. **Apple Developer Program** membership, to get a *Developer ID Application* certificate.
2. Sign with it and the hardened runtime:
   `REMORA_SIGN_IDENTITY="Developer ID Application: <Name> (<TEAMID>)"`, adding `--options runtime` to `codesign`.
3. **Notarize**: `xcrun notarytool submit Remora.zip --wait`, then `xcrun stapler staple build/Remora.app`.
4. Ship a `.dmg` or `.zip`.

A self-signed build can be shared too, but Gatekeeper blocks it ("Apple cannot verify the developer"):
each person must allow it in System Settings → Privacy & Security.
