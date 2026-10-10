#!/usr/bin/env bash
# Creates a self-signed code-signing certificate.
#
# Without arguments: "Remora Local Signing", in the login keychain. Builds signed with it keep the same identity,
# so the Keychain stops asking after each rebuild. Local use only (see docs/develop/building.md).
#
# --release [folder]: "Remora Release Signing", for the release workflow. It is written to a folder (default
# ~/Remora Release Signing), not imported, and is valid for 20 years: the Keychain trusts Remora by this
# certificate, so replacing it costs every user one more Keychain prompt. Store it as the repository secrets
# MACOS_SIGNING_P12 and MACOS_SIGNING_PASSWORD, then keep the folder offline. It doesn't replace notarization.
set -euo pipefail

RELEASE=0
if [ "${1:-}" = "--release" ]; then
    RELEASE=1
    NAME="Remora Release Signing"
    OUT="${2:-$HOME/Remora Release Signing}"
    DAYS=7300
    if [ -e "$OUT/cert.p12" ]; then
        echo "$OUT/cert.p12 already exists: reuse it, a new certificate means a new Keychain prompt for everyone."
        exit 1
    fi
else
    NAME="Remora Local Signing"
    DAYS=3650
    if security find-certificate -c "$NAME" >/dev/null 2>&1; then
        echo "\"$NAME\" already exists."
        exit 0
    fi
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
PASS="$(openssl rand -hex 16)"

cat > "$WORK/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF

openssl req -x509 -newkey rsa:2048 -nodes -days "$DAYS" \
    -keyout "$WORK/key.pem" -out "$WORK/cert.pem" -config "$WORK/cert.cnf" 2>/dev/null
# macOS only imports PKCS#12 files using the older SHA1/3DES encryption.
openssl pkcs12 -export -inkey "$WORK/key.pem" -in "$WORK/cert.pem" \
    -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
    -name "$NAME" -out "$WORK/cert.p12" -passout "pass:$PASS"

if [ "$RELEASE" = 1 ]; then
    mkdir -p "$OUT"
    chmod 700 "$OUT"
    (umask 077; cp "$WORK/cert.p12" "$OUT/cert.p12"; printf '%s' "$PASS" > "$OUT/password.txt")
    echo "Created \"$NAME\" in $OUT. To give it to the release workflow:"
    echo "  base64 -i \"$OUT/cert.p12\" | gh secret set MACOS_SIGNING_P12"
    echo "  gh secret set MACOS_SIGNING_PASSWORD < \"$OUT/password.txt\""
    exit 0
fi

security import "$WORK/cert.p12" -k "$HOME/Library/Keychains/login.keychain-db" \
    -P "$PASS" -T /usr/bin/codesign

echo "Created \"$NAME\". The first build will ask to let codesign use it: choose Always Allow."
