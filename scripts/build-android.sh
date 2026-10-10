#!/bin/sh
# Build Project Depot for Android (arm64) as dist/android/depot-<version>.apk: the game and
# its launcher in one app (NativeActivity + libopenomsi_game.so, see
# crates/omsi-app/src/android.rs). Needs Rust with the aarch64-linux-android target
# (rustup target add aarch64-linux-android), the Android SDK (platform 34+, build-tools,
# the NDK) and a JDK 17; see android/env.sh for where they are looked for.
#
#   scripts/build-android.sh            build the APK
#   scripts/build-android.sh install    ... and install it on the phone attached by USB (adb)
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
. android/env.sh
[ -x "$CC_aarch64_linux_android" ] || { echo "The Android NDK was not found (set ANDROID_NDK_HOME)." >&2; exit 1; }
[ -f "$ANDROID_JAR" ] || { echo "No Android platform in $ANDROID_HOME/platforms (sdkmanager \"platforms;android-35\")." >&2; exit 1; }
[ -x "$BUILD_TOOLS/aapt" ] || { echo "No build-tools in $ANDROID_HOME (sdkmanager \"build-tools;35.0.0\")." >&2; exit 1; }
version="${OPENOMSI_VERSION:-$(sh scripts/version.sh 2>/dev/null || echo 0.0.0)}"
export OPENOMSI_VERSION="$version"
# the version code: the commit count (each release one higher)
code=$(git rev-list --count HEAD 2>/dev/null || echo 1)
# a pull request's test build (OPENOMSI_PR=<number>) is an app of its own, "Project Depot PR #N":
# it installs beside the release without replacing it or being refused for its signing key,
# and shares the Project Depot folder (settings, content) with it
package=org.openomsi.game
label="Project Depot"
if [ -n "${OPENOMSI_PR:-}" ]; then package=org.openomsi.game.pr; label="Project Depot PR #$OPENOMSI_PR"; fi
target=aarch64-linux-android
build=android/build
rm -rf "$build"
mkdir -p "$build/classes" "$build/apk/lib/arm64-v8a" dist/android

# --- the native code
cargo rustc --locked --profile android --target "$target" -p omsi-app --lib --crate-type cdylib
so="target/$target/android/libopenomsi_game.so"
cp "$so" "$build/apk/lib/arm64-v8a/libdepot_game.so"
# every symbol the library needs must be in the system's libraries (Android loads it with
# BIND_NOW: one missing symbol and the app closes before its window opens)
sysroot="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG/sysroot/usr/lib/aarch64-linux-android/$ANDROID_API"
"$NDK_BIN/llvm-nm" -D "$so" | awk '$1 == "U" {print $2}' | sed 's/@.*//' | sort -u > "$build/needed.txt"
for l in libc libm libdl liblog libandroid libOpenSLES libaaudio libvulkan; do
    [ -f "$sysroot/$l.so" ] && "$NDK_BIN/llvm-nm" -D --defined-only "$sysroot/$l.so" | awk '{print $3}'
done | sed 's/@.*//' | sort -u > "$build/provided.txt"
missing=$(comm -23 "$build/needed.txt" "$build/provided.txt")
if [ -n "$missing" ]; then
    echo "libdepot_game.so needs symbols no system library of API $ANDROID_API has:" >&2
    echo "$missing" >&2
    exit 1
fi
# (a C++ dependency linked against the shared C++ library needs it beside it)
if "$NDK_BIN/llvm-readelf" -d "$so" | grep -q 'libc++_shared.so'; then
    cp "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" "$build/apk/lib/arm64-v8a/"
fi

# --- the activity (Java)
"$JAVA_HOME/bin/javac" -source 8 -target 8 -nowarn -Xlint:-options -encoding UTF-8 \
    -classpath "$ANDROID_JAR" -d "$build/classes" android/java/org/openomsi/game/OmsiActivity.java
"$BUILD_TOOLS/d8" --release --min-api 26 --lib "$ANDROID_JAR" --output "$build/apk" \
    $(find "$build/classes" -name '*.class')

# --- the package
sed -e "s/@VERSION_CODE@/$code/" -e "s/@VERSION_NAME@/$version/" -e "s/android:label=\"Project Depot\"/android:label=\"$label\"/"     android/AndroidManifest.xml > "$build/AndroidManifest.xml"
"$BUILD_TOOLS/aapt" package -f --rename-manifest-package "$package" -M "$build/AndroidManifest.xml" -S android/res -I "$ANDROID_JAR" -F "$build/unsigned.apk"
( cd "$build/apk" && "$BUILD_TOOLS/aapt" add ../unsigned.apk classes.dex lib/arm64-v8a/* >/dev/null )
"$BUILD_TOOLS/zipalign" -f -p 4 "$build/unsigned.apk" "$build/aligned.apk"
key=android/debug.keystore
# The keystore password is never in the repository. CI gives it as the secret
# ANDROID_KEYSTORE_PASSWORD (next to ANDROID_KEYSTORE_B64); on a developer machine it is
# ANDROID_KEYSTORE_PASSWORD too, or - when this script makes a new debug key - a random one
# written (mode 600) to android/debug.keystore.pass, which is git-ignored like the key.
pass_file=android/debug.keystore.pass
if [ -z "${ANDROID_KEYSTORE_PASSWORD:-}" ] && [ -f "$pass_file" ]; then
    ANDROID_KEYSTORE_PASSWORD=$(cat "$pass_file")
fi
# (the CI signs with the same key every time when it is given as a secret: an update then
# installs over the last release)
if [ ! -f "$key" ] && [ -n "${ANDROID_KEYSTORE_B64:-}" ]; then
    printf '%s' "$ANDROID_KEYSTORE_B64" | base64 -d > "$key"
fi
if [ ! -f "$key" ]; then
    if [ -z "${ANDROID_KEYSTORE_PASSWORD:-}" ]; then
        ANDROID_KEYSTORE_PASSWORD=$(LC_ALL=C tr -dc 'A-Za-z0-9' < /dev/urandom | head -c 32)
        ( umask 077; printf '%s' "$ANDROID_KEYSTORE_PASSWORD" > "$pass_file" )
        echo "A new debug key was made; its random password is in $pass_file (not in git)." >&2
    fi
    export ANDROID_KEYSTORE_PASSWORD
    "$JAVA_HOME/bin/keytool" -genkeypair -keystore "$key" -storepass:env ANDROID_KEYSTORE_PASSWORD -keypass:env ANDROID_KEYSTORE_PASSWORD \
        -alias openomsi -keyalg RSA -keysize 2048 -validity 10000 \
        -dname "CN=Project Depot, O=Project Depot, C=DE" >/dev/null 2>&1
fi
if [ -z "${ANDROID_KEYSTORE_PASSWORD:-}" ]; then
    echo "The keystore password is missing: set ANDROID_KEYSTORE_PASSWORD (the secret next to ANDROID_KEYSTORE_B64)." >&2
    exit 1
fi
export ANDROID_KEYSTORE_PASSWORD
apk="dist/android/depot-$version.apk"
"$BUILD_TOOLS/apksigner" sign --ks "$key" --ks-pass env:ANDROID_KEYSTORE_PASSWORD --key-pass env:ANDROID_KEYSTORE_PASSWORD \
    --ks-key-alias openomsi --out "$apk" "$build/aligned.apk"
rm -f "$apk.idsig"
printf '\nProject Depot %s for Android: %s (%s)\n' "$version" "$PWD/$apk" "$(du -h "$apk" | cut -f1)"
if [ "${1:-}" = install ]; then
    "$ANDROID_HOME/platform-tools/adb" install -r "$apk"
fi
