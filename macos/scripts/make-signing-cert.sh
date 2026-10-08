#!/usr/bin/env bash
# Creates a self-signed "Remora Local Signing" certificate in the login keychain.
# Builds signed with it keep the same identity, so the Keychain stops asking after each rebuild.
# Local use only: it does not make the app distributable (see docs/DISTRIBUTION.md).
set -euo pipefail

NAME="Remora Local Signing"
if security find-certificate -c "$NAME" >/dev/null 2>&1; then
    echo "\"$NAME\" already exists."
    exit 0
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

openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
    -keyout "$WORK/key.pem" -out "$WORK/cert.pem" -config "$WORK/cert.cnf" 2>/dev/null
# macOS only imports PKCS#12 files using the older SHA1/3DES encryption.
openssl pkcs12 -export -inkey "$WORK/key.pem" -in "$WORK/cert.pem" \
    -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
    -name "$NAME" -out "$WORK/cert.p12" -passout "pass:$PASS"
security import "$WORK/cert.p12" -k "$HOME/Library/Keychains/login.keychain-db" \
    -P "$PASS" -T /usr/bin/codesign

echo "Created \"$NAME\". The first build will ask to let codesign use it: choose Always Allow."
