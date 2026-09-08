#!/usr/bin/env bash
#
# Build and install the Android APK.
#
# This used to hand-assemble an APK with `aapt2 link` + `zip` out of whatever
# cargo-apk had left in target/release/apk, because `cargo apk build --release`
# was failing — and it hid that failure with `set +e` and a message about
# "continuing only if required intermediate artifacts exist". The failure was
# `Configure a release keystore via [package.metadata.android.signing.release]`:
# cargo-apk compiled everything and then aborted before packaging. The APK that
# got installed therefore contained a manifest and a .so and nothing else.
#
# Both halves are fixed in Cargo.toml now (a signing config and the landscape
# orientation), so this script builds the normal way and does NOT hide errors.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT_DIR"

INSTALL_AFTER_BUILD=1
TARGET_ARGS=(--target aarch64-linux-android)
for arg in "$@"; do
  case "$arg" in
    --no-install) INSTALL_AFTER_BUILD=0 ;;
    --all-abis)   TARGET_ARGS=() ;;
    *) echo "Unknown option: $arg" >&2; exit 1 ;;
  esac
done

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

need_cmd cargo
need_cmd rustup

if ! command -v cargo-apk >/dev/null 2>&1; then
  echo "Installing cargo-apk..."
  cargo install cargo-apk
fi

if [[ -z "${ANDROID_SDK_ROOT:-}" ]]; then
  if [[ -d "/opt/homebrew/share/android-commandlinetools" ]]; then
    export ANDROID_SDK_ROOT="/opt/homebrew/share/android-commandlinetools"
  else
    echo "ANDROID_SDK_ROOT is not set and the default SDK path was not found." >&2
    exit 1
  fi
fi
export ANDROID_HOME="$ANDROID_SDK_ROOT"

if [[ ! -d "$ANDROID_SDK_ROOT/platforms/android-33" ]]; then
  need_cmd sdkmanager
  echo "Installing Android platform 33..."
  yes | sdkmanager "platforms;android-33" "build-tools;33.0.2"
fi

if [[ -z "${ANDROID_NDK_ROOT:-}" ]]; then
  if [[ -d "$ANDROID_SDK_ROOT/ndk" ]]; then
    NDK_VERSION="$(ls -1 "$ANDROID_SDK_ROOT/ndk" | sort -V | tail -n 1)"
    export ANDROID_NDK_ROOT="$ANDROID_SDK_ROOT/ndk/$NDK_VERSION"
  else
    echo "ANDROID_NDK_ROOT is not set and no NDK was found under the SDK root." >&2
    exit 1
  fi
fi
if [[ ! -d "$ANDROID_NDK_ROOT" ]]; then
  echo "ANDROID_NDK_ROOT does not exist: $ANDROID_NDK_ROOT" >&2
  exit 1
fi

for target in aarch64-linux-android armv7-linux-androideabi; do
  if ! rustup target list --installed | grep -q "^${target}$"; then
    echo "Installing Rust target $target..."
    rustup target add "$target"
  fi
done

# cargo-apk signs the release APK with the keystore named in Cargo.toml, whose
# path is relative to this directory. The standard debug keystore is not a
# secret and is not tracked (see .gitignore) — it is copied in so a test build
# can be signed at all.
if [[ ! -f debug.keystore ]]; then
  if [[ -f "$HOME/.android/debug.keystore" ]]; then
    echo "Copying the Android debug keystore in for signing..."
    cp "$HOME/.android/debug.keystore" debug.keystore
  else
    echo "No debug keystore at ~/.android/debug.keystore." >&2
    echo "Create one with:" >&2
    echo "  keytool -genkey -v -keystore ~/.android/debug.keystore -storepass android \\" >&2
    echo "    -alias androiddebugkey -keypass android -keyalg RSA -validity 10000 \\" >&2
    echo "    -dname 'CN=Android Debug,O=Android,C=US'" >&2
    exit 1
  fi
fi

# `--lib` is not optional. The crate has bin targets as well as the cdylib, and
# without it cargo-apk picks a bin and dies with "Bin is not compatible with
# Cdylib" — the .so Android loads is the LIBRARY, which is what carries
# `android_main`.
echo "Building the APK..."
cargo apk build --release --lib "${TARGET_ARGS[@]}"

APK="target/release/apk/main.apk"
if [[ ! -f "$APK" ]]; then
  echo "cargo-apk reported success but produced no APK at $APK" >&2
  exit 1
fi
echo "Built APK: $APK"

if [[ $INSTALL_AFTER_BUILD -eq 1 ]]; then
  need_cmd adb
  DEVICE_LIST="$(adb devices | awk 'NR>1 && $2=="device" {print $1}')"
  if [[ -z "$DEVICE_LIST" ]]; then
    echo "No connected adb device. The APK is built; install skipped." >&2
    exit 0
  fi
  DEVICE_SERIAL="$(echo "$DEVICE_LIST" | head -n 1)"
  echo "Installing on device $DEVICE_SERIAL..."
  adb -s "$DEVICE_SERIAL" install -r "$APK"
  echo "Install complete."
  echo
  echo "If it still fails to start, the reason will be in logcat:"
  echo "  adb -s $DEVICE_SERIAL logcat -c && adb -s $DEVICE_SERIAL logcat | grep -iE 'orange|rust|panic|libmain|SIGABRT|DEBUG|AndroidRuntime'"
fi
