# SyncMD - MVP Implementation

A minimal viable implementation of an efficient markdown file synchronization tool for macOS.

## Features

- ✅ File indexing with BLAKE3 hashing
- ✅ Basic TCP networking for device discovery
- ✅ Conflict resolution (last-write-wins)
- ✅ CLI interface with server/client modes
- ✅ Configuration management
- ✅ Markdown and image file support
- ✅ Real-time file watching with notify
- ✅ Chunked file transfer with progress tracking
- ✅ Checksum verification for file integrity
- ✅ Error handling and retry mechanisms

## Building

```bash
cargo build --release
```

## Usage

### Initialize a sync folder

```bash
./target/release/syncmd init --path /path/to/your/folder --name "My Device"
```

### Start in server mode

```bash
./target/release/syncmd sync --path /path/to/your/folder --server --port 8080
```

### Connect to a server

```bash
./target/release/syncmd sync --path /path/to/your/folder --connect server-ip:8080
```

### Check status

```bash
./target/release/syncmd status
```

### List devices

```bash
./target/release/syncmd list-devices
```

## Architecture

- **File Indexer**: Scans directories and creates file metadata with hashes
- **Sync Engine**: Calculates sync operations and handles conflicts
- **Network Manager**: Handles TCP connections and device discovery
- **Device Manager**: Manages connected devices and their state

## Testing

The MVP has been tested with basic functionality:
- Server starts successfully
- Client connects to server
- File indexing works
- Configuration management works
- Basic handshake protocol works

## Limitations (MVP)

- Basic conflict resolution only
- No encryption
- No Android support yet
- No automatic reconnection
- No background service on macOS

## Next Steps

1. ✅ Add real-time file watching with `notify`
2. ✅ Implement proper file transfer
3. Add encryption support
4. Improve conflict resolution
5. Build Android client
6. Add macOS background service