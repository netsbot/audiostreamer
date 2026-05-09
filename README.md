# Audiostreamer

Lossless Apple Music client for Linux.

### Build Dependencies
- `base-devel`
- `curl`
- `unzip`
- `cmake`
- `pkgconf`
- `openssl`
- `librsvg`
- `ffmpeg`
- `gpac`
- `cef`
- `rust`
- `node`

## Build
```bash
git clone --recursive https://github.com/netsbot/audiostreamer.git
cd audiostreamer
npm install
cd ui && npm install
cd ..
npm run tauri:dev
```

## Login
See [WorldObservationLog/wrapper](https://github.com/WorldObservationLog/wrapper) for login instructions.
