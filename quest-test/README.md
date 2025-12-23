# Quest OpenXR Test

Standalone test app to validate Quest OpenXR + Passthrough AR support before integrating into fastn-shell.

## What This Tests

1. OpenXR instance creation on Quest
2. Extension enumeration (especially `XR_FB_passthrough`)
3. System/HMD detection
4. View configuration

## Prerequisites

### 1. Install Android NDK

```bash
# macOS with Homebrew
brew install openjdk@17
brew install android-ndk
brew install android-commandlinetools

# Or download from https://developer.android.com/ndk/downloads
```

### 2. Install cargo-apk

```bash
cargo install cargo-apk
```

### 3. Add Android target

```bash
rustup target add aarch64-linux-android
```

### 4. Download Oculus OpenXR Loader

1. Download from: https://developer.oculus.com/downloads/package/oculus-openxr-mobile-sdk/
2. Extract `libopenxr_loader.so` from `OpenXR/Libs/Android/arm64-v8a/Release/`
3. Place in `quest-test/libs/arm64-v8a/libopenxr_loader.so`

```bash
mkdir -p libs/arm64-v8a
# Copy libopenxr_loader.so there
```

### 5. Enable Developer Mode on Quest

1. Create developer account at https://developer.oculus.com/
2. Enable Developer Mode in Oculus mobile app
3. Connect Quest via USB and allow debugging

## Build & Install

```bash
# Build APK
cargo apk build --release

# Install to connected Quest
adb install -r target/release/apk/quest-test.apk
```

## Run & Check Logs

```bash
# Watch logs
adb logcat -s quest-test:* RustStdoutStderr:*
```

You should see output like:
```
=== Quest OpenXR Test Started ===
Initializing OpenXR...
Available OpenXR extensions:
  KHR_vulkan_enable2: true
  FB_passthrough: true
OpenXR runtime: Oculus v1.x.x
System: Oculus Quest 3
Passthrough support: true
SUCCESS: This device supports AR passthrough!
```

## Desktop Testing

You can verify the code compiles on desktop:

```bash
cargo check
cargo run  # Will fail at runtime without VR runtime, but checks compilation
```

## Next Steps

Once this test confirms passthrough support:
1. Add Vulkan rendering (render a cube in VR)
2. Enable passthrough layer (AR mode)
3. Integrate into fastn-shell as Android target


export JAVA_HOME="/opt/homebrew/opt/openjdk@17"
export PATH="/opt/homebrew/opt/openjdk@17/bin:$PATH"
export ANDROID_HOME="/opt/homebrew/share/android-commandlinetools"
export ANDROID_NDK_HOME="/opt/homebrew/share/android-ndk"
