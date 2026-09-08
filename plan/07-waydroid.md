# Waydroid development validation

Waydroid can reuse the Android APKs from CI for local app, Rust/UniFFI, lifecycle and
regtest checks. It does not qualify physical camera/USB hardware or replace Xcode/iOS
tests. Use a disposable development instance: the instrumentation test clears Tundra's
development database before importing public fixtures.

## Run matching CI artifacts

Use the application and instrumentation APKs from the **same CI run** so their debug
signatures and API versions match. ADB/platform-tools and Bitcoin Core can be installed
in user-owned directories; Android SDK/NDK/Gradle are unnecessary for this path.

1. Start Waydroid in an active Wayland session. A headless account also needs a Wayland
   compositor and PulseAudio-compatible socket. Starting the container service alone does
   not establish an Android user session.
2. Follow [Waydroid's ADB instructions](https://docs.waydro.id/faq/using-adb-with-waydroid),
   then authorize the debugging connection in Android. `adb devices` must report `device`,
   not `offline` or `unauthorized`.
3. Download `tundra-development-apk` and `tundra-instrumentation-apk` from one CI run to
   ignored directories under `build/`.
4. Run `python3 tests/native_chain.py` in a separate terminal with `bitcoind` and
   `bitcoin-cli` on PATH. Wait for `build/native-chain-port`. This creates a fresh,
   keyless 103-block regtest chain on loopback port 3002; it refuses an occupied endpoint
   and removes its own temporary chain on termination.
5. Select the disposable target explicitly and run:

```sh
ANDROID_SERIAL='<Waydroid-IP>:5555' TUNDRA_DISPOSABLE_ANDROID=1 \
  ./scripts/check-android-apks.sh build/app/app-debug.apk \
  build/tests/app-debug-androidTest.apk
```

The script installs both APKs, forwards port 3002, runs instrumentation through the real
Android ABI, then force-stops and relaunches the app to check the saved balance/draft.
It rejects instrumentation failures even if ADB exits successfully. Existing app data is
only appropriate if it is disposable; the script does not resolve signing conflicts by
uninstalling an existing application.

The matching CI path uses Gradle's
`-Pandroid.injected.androidTest.leaveApksInstalledAfterRun=true` so the actual saved state
survives Gradle's test cleanup. Reinstalling an empty app is not restart validation.

## Host observations on September 8, 2026

- Waydroid's existing system service and images were usable. A user-local Weston 14.0.2
  headless compositor and PulseAudio 17 socket allowed Android to finish booting.
- Official ADB 37.0.1 was extracted under ignored `build/tools/platform-tools`.
- Tundra Dev from commit `82b14db` installed and appeared in Waydroid's application list.
  Installation/launch requests are not proof of a successful app runtime test.
- Android had no IPv4 DHCP lease; ADB was unreachable. UFW was enabled. The operator was
  asked to allow inbound DHCP **only on `waydroid0`, UDP port 67**. No firewall rule was
  changed by this account, which has no sudo access. ADB/runtime tests on Waydroid remain
  unexecuted until connectivity and debugging authorization work.

Keep the [validation report](VALIDATION.md) as the authority for tests actually executed.
