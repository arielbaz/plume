#!/usr/bin/env bash
# Construit un APK Android arm64 signé (si le keystore est fourni).
#
# Prérequis : Rust + cibles Android, JDK 17, Android SDK, NDK 27,
#             CMake et Ninja dans le PATH, Node.js.
#
# Variables facultatives :
#   PLUME_ANDROID_KEYSTORE   chemin du .jks
#   PLUME_ANDROID_ALIAS      alias (défaut : plume)
#   PLUME_ANDROID_STOREPASS  mot de passe du keystore
#   PLUME_ANDROID_KEYPASS    mot de passe de la clé (défaut : storepass)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export ANDROID_HOME="${ANDROID_HOME:-$HOME/android-sdk}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export NDK_HOME="${NDK_HOME:-$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1)}"
export JAVA_HOME="${JAVA_HOME:-$(/usr/libexec/java_home -v 17 2>/dev/null || true)}"
export PATH="$JAVA_HOME/bin:$PATH"
export CMAKE_GENERATOR="${CMAKE_GENERATOR:-Ninja}"

if [ -z "${NDK_HOME:-}" ] || [ ! -d "$NDK_HOME" ]; then
  echo "NDK Android introuvable. Installez-le avec sdkmanager 'ndk;27.2.12479018'." >&2
  exit 1
fi

if [ ! -d "src-tauri/gen/android" ]; then
  npx tauri android init --ci
fi

# Le template Tauri utilise compileSdk 37 ; le SDK 36 suffit et est
# disponible dans les dépôts stables.
GRADLE="src-tauri/gen/android/app/build.gradle.kts"
sed -i.bak 's/compileSdk = 37/compileSdk = 36/; s/targetSdk = 37/targetSdk = 36/' "$GRADLE"
rm -f "$GRADLE.bak"

npx tauri android build --target aarch64

APK="src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
mkdir -p dist
OUT="dist/Plume-android-arm64.apk"

if [ -n "${PLUME_ANDROID_KEYSTORE:-}" ]; then
  BUILD_TOOLS="$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)"
  "$BUILD_TOOLS/zipalign" -p -f 4 "$APK" /tmp/plume-aligned.apk
  "$BUILD_TOOLS/apksigner" sign \
    --ks "$PLUME_ANDROID_KEYSTORE" \
    --ks-key-alias "${PLUME_ANDROID_ALIAS:-plume}" \
    --ks-pass "pass:${PLUME_ANDROID_STOREPASS:?}" \
    --key-pass "pass:${PLUME_ANDROID_KEYPASS:-$PLUME_ANDROID_STOREPASS}" \
    --out "$OUT" /tmp/plume-aligned.apk
  "$BUILD_TOOLS/apksigner" verify "$OUT"
  echo "APK signé : $OUT"
else
  cp "$APK" "$OUT"
  echo "APK non signé : $OUT (définir PLUME_ANDROID_KEYSTORE pour signer)" >&2
fi
