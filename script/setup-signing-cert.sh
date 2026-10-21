#!/bin/bash
# Create a STABLE self-signed code-signing identity in the login keychain.
# Why: ad-hoc signatures (`codesign -s -`) change every build, so macOS treats each rebuild as a new
# app and forgets privacy grants (Desktop/Documents access, Apple events). Signing with a fixed cert
# keeps the designated requirement stable, so permissions survive rebuilds.
#
# Idempotent: does nothing if the identity already exists.
set -euo pipefail

CERT_NAME="Repodeck Local Signing"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-certificate -c "$CERT_NAME" "$KEYCHAIN" >/dev/null 2>&1; then
    echo "==> identity '$CERT_NAME' already exists — nothing to do"
    exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
PASS="$(openssl rand -hex 16)"

cat > "$TMP/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = v3
prompt = no
[dn]
CN = $CERT_NAME
[v3]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
CNF

echo "==> generating self-signed code-signing certificate (10y)"
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
    -keyout "$TMP/key.pem" -out "$TMP/cert.pem" -config "$TMP/cert.cnf" >/dev/null 2>&1

# -legacy + sha1 MAC: the macOS Security framework can't read OpenSSL 3's default PKCS12 MAC.
openssl pkcs12 -export -legacy -macalg sha1 -inkey "$TMP/key.pem" -in "$TMP/cert.pem" \
    -name "$CERT_NAME" -out "$TMP/cert.p12" -passout "pass:$PASS" >/dev/null 2>&1

echo "==> importing into login keychain (usable by codesign without a prompt)"
# -A: any app may use this local-only key without a keychain dialog, so builds can sign
# non-interactively (same setup as the other "* Local Signing" identities).
security import "$TMP/cert.p12" -k "$KEYCHAIN" -P "$PASS" -A >/dev/null

echo "==> done. Identity installed:"
security find-certificate -c "$CERT_NAME" "$KEYCHAIN" | grep -E '"labl"' || true
