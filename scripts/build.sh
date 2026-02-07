#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$ROOT/.env"

# Load .env
if [[ ! -f "$ENV_FILE" ]]; then
  echo "Error: .env file not found. Copy .env.example to .env and fill in your values."
  exit 1
fi

# Source .env but only export what we need
source "$ENV_FILE"

# Validate signing identity
if [[ -z "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  echo "Error: APPLE_SIGNING_IDENTITY is not set in .env"
  exit 1
fi
export APPLE_SIGNING_IDENTITY

# Only enable notarization if all three credentials are present
if [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
  export APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
  echo "Building StepCast (signed + notarized release)..."
else
  # Unset so Tauri doesn't attempt notarization
  unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID 2>/dev/null || true
  echo "Building StepCast (signed release, no notarization)..."
  echo "  (Set APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID in .env to enable notarization)"
fi

# Export updater signing keys if present
if [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  export TAURI_SIGNING_PRIVATE_KEY
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
fi

echo "  Signing identity: $APPLE_SIGNING_IDENTITY"
echo ""

cd "$ROOT"

# NOTE: For macOS Screen Recording permissions, LaunchServices must be able to
# resolve our bundle identifier to the installed app bundle.
#
# Building DMGs tends to create temporary /Volumes/dmg.* mounts during the build
# process, which can leave behind LaunchServices records for StepCast.app in
# those locations. On macOS 26 this can break the Screen Recording toggle entry
# (System Settings can't resolve the bundle and StepCast won't show up).
#
# For local testing we only build the .app bundle.
bun tauri build --bundles app 2>&1

APP_PATH="$ROOT/src-tauri/target/release/bundle/macos/StepCast.app"
INSTALL_PATH="/Applications/StepCast.app"

if [[ -d "$APP_PATH" ]]; then
  echo ""
  echo "Build complete: $APP_PATH"
  echo ""
  # Verify signature
  codesign -dvv "$APP_PATH" 2>&1 | grep -E "^(Authority|TeamIdentifier|Identifier)" || true

  echo ""
  echo "Installing to: $INSTALL_PATH"
  rm -rf "$INSTALL_PATH"
  # Use ditto for macOS bundles to preserve structure/metadata.
  ditto "$APP_PATH" "$INSTALL_PATH"

  echo ""
  echo "Verifying installed app signature..."
  codesign --verify --deep --strict --verbose=4 "$INSTALL_PATH" 2>&1
  codesign -dvv "$INSTALL_PATH" 2>&1 | grep -E "^(Authority|TeamIdentifier|Identifier|Timestamp)" || true

  echo ""
  echo "Detaching leftover StepCast build disk images (if any)..."
  # Tauri's DMG bundling step can mount temporary HFS images under /Volumes/dmg.*
  # and sometimes leaves them attached. These mounts can confuse LaunchServices and
  # break TCC's bundle resolution for Screen Recording.
  if command -v hdiutil >/dev/null 2>&1; then
    while IFS= read -r mp; do
      if [[ -n "$mp" ]]; then
        echo "  Detach: $mp"
        hdiutil detach "$mp" || hdiutil detach -force "$mp" || true
      fi
    done < <(
      hdiutil info | awk -v root="$ROOT" '
        /^================================================$/ { image=""; next }
        /^image-path[[:space:]]*:/ { image=$3; next }
        /^\/dev\/disk/ {
          mp=$NF;
          if (index(image, root) == 1 && index(image, "StepCast_") > 0 && index(mp, "/Volumes/dmg.") == 1) print mp;
        }
      ' | sort -u
    )

    # Remove leftover temporary RW disk images created during DMG packaging.
    rm -f "$ROOT/src-tauri/target/release/bundle/macos/rw."*.StepCast_*.dmg 2>/dev/null || true
  fi

  echo ""
  echo "Cleaning LaunchServices registrations (keep only /Applications copy)..."
  LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
  BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$INSTALL_PATH/Contents/Info.plist" 2>/dev/null || true)"
  cleanup_ls_for_bundle_id() {
    local want="$1"
    if [[ -z "$want" ]]; then
      echo "  Warning: Could not read CFBundleIdentifier from installed app Info.plist; skipping LS cleanup."
      return 0
    fi

    # Unregister all other paths for this bundle ID, then re-register the installed app.
    "$LSREGISTER" -dump 2>/dev/null | awk -v want="$want" '
        /^--------------------------------------------------------------------------------$/ {
          if (id == want && path != "" && path != "/Applications/StepCast.app") print path;
          id=""; path="";
          next
        }
        /^identifier:[[:space:]]+/ { id=$2 }
        /^path:[[:space:]]+/ { path=$2 }
        END {
          if (id == want && path != "" && path != "/Applications/StepCast.app") print path;
        }
      ' | while IFS= read -r p; do
        echo "  Unregister: $p"
        "$LSREGISTER" -u "$p" || true
      done

    "$LSREGISTER" -gc || true
    "$LSREGISTER" -f "$INSTALL_PATH" || true
  }

  # Clean current and old bundle IDs from LaunchServices
  cleanup_ls_for_bundle_id "$BUNDLE_ID"
  cleanup_ls_for_bundle_id "com.stepcast.desktop"
  cleanup_ls_for_bundle_id "com.stepcast.app"
  sleep 1
  cleanup_ls_for_bundle_id "$BUNDLE_ID"

  echo ""
  echo "Removing build output .app bundles to avoid duplicates..."
  rm -rf "$ROOT/src-tauri/target/release/bundle/macos/StepCast.app" || true
  rm -rf "$ROOT/src-tauri/target/debug/bundle/macos/StepCast.app" || true
else
  echo ""
  echo "Build finished but .app not found at expected path."
  exit 1
fi
