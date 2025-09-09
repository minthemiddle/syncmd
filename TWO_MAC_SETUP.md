# SyncMD Two-Mac Setup Guide

## Overview

SyncMD now supports **true peer-to-peer discovery**! Both Macs can discover each other automatically on the same network without needing to specify which is server/client.

## Quick Start (Recommended)

### On Both Macs:

1. **Initialize your sync folder:**
```bash
./target/debug/syncmd init --path /path/to/your/markdown/folder --name "MacBook Pro"
```

2. **Start peer discovery mode:**
```bash
./target/debug/syncmd sync --path /path/to/your/markdown/folder --discover
```

That's it! Both Macs will automatically discover each other and start syncing.

## How It Works

- **UDP Broadcast**: Both Macs broadcast discovery messages on port 8081
- **Auto-Discovery**: When a Mac receives a discovery message, it responds with its IP address
- **Peer-to-Peer**: Both Macs act as both server and client simultaneously
- **Automatic Sync**: Every 30 seconds, each Mac scans for peers and initiates sync

## Commands

### 1. Initialize (One-time setup)
```bash
./target/debug/syncmd init --path ./docs --name "My Mac"
```

### 2. Manual Peer Discovery
```bash
# See who's on the network
./target/debug/syncmd discover --port 8081
```

### 3. Start Syncing (Peer Discovery Mode)
```bash
# Automatic peer discovery and sync
./target/debug/syncmd sync --path ./docs --discover
```

### 4. Traditional Server/Client (Optional)

**On Mac 1 (Server):**
```bash
./target/debug/syncmd sync --path ./docs --server --port 8080
```

**On Mac 2 (Client):**
```bash
./target/debug/syncmd sync --path ./docs --connect 192.168.1.100:8080
```

## Network Requirements

- Both Macs must be on the **same local network**
- **Port 8081** must be open for UDP discovery
- **Port 8080** must be open for TCP sync (default)
- No special router configuration needed

## Finding Your IP Address

To find your Mac's IP address:
```bash
# For wireless
ipconfig getifaddr en0

# For wired
ipconfig getifaddr en1
```

## Testing Your Setup

1. **Test discovery:**
```bash
# On Mac 1
./target/debug/syncmd discover --port 8081

# On Mac 2  
./target/debug/syncmd discover --port 8081
```

2. **Test connectivity:**
```bash
# Create a test file
echo "# Test" > ./docs/test.md

# Check if it appears on the other Mac
ls ./docs/
```

## Troubleshooting

### No peers found?
- Check both Macs are on the same network
- Verify port 8081 is not blocked by firewall
- Try manual IP connection method

### Firewall Issues
Allow these ports in System Preferences > Security & Privacy > Firewall:
- UDP port 8081 (discovery)
- TCP port 8080 (sync)

### Different Networks?
Use the traditional server/client method with the actual IP addresses.

## Features Working

✅ **Peer Discovery** - Automatic Mac finding via UDP broadcast  
✅ **File Indexing** - BLAKE3 hashing for change detection  
✅ **Basic Sync** - File transfer between Macs  
✅ **Conflict Resolution** - Last-write-wins strategy  
✅ **CLI Interface** - Easy command-line controls  

## Next Steps

The current MVP handles basic sync well. Future improvements:
- Real-time file watching (instant sync)
- Better conflict resolution
- Encryption
- Android app
- Background service on macOS