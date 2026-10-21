# scrcpy server

Bundled version: **4.1** (Apache-2.0).

Official binary: https://github.com/Genymobile/scrcpy/releases/download/v4.1/scrcpy-server-v4.1

SHA-256 (verified against the release SHA256SUMS.txt):
`deacb991ed2509715160ffdc7907e47b4160eb30d1566217e9047fd5b8850cae`

When upgrading, update `src/scrcpy.rs` and `build.rs` together with the binary.
Protocol reference: https://github.com/Genymobile/scrcpy/blob/v4.1/server/src/main/java/com/genymobile/scrcpy/device/Streamer.java

Version 4 uses a four-byte codec id, followed by a 12-byte session header
(flags, width, height). Session headers also occur after capture restarts.
Packet flag bits are session=63, config=62, keyframe=61.
Touch (32 bytes), key (14 bytes), and text control messages retain their layouts.

Verify preview, tap, drag, navigation keys, and rotation on the affected Android
device. Successful socket writes alone do not prove Android accepted input.
