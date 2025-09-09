# SyncMD Technical Implementation Plan

## Overview
A high-performance, cross-platform markdown and image synchronization system built in Rust, designed to handle tens of thousands of files efficiently across multiple devices (macOS, Android) with near real-time sync capabilities.

## Architecture

### Core Components

1. **File Watcher Service**
   - Cross-platform file system monitoring (inotify on Linux, FSEvents on macOS, FileObserver on Android)
   - Efficient debouncing to handle rapid file changes
   - Focus on .md files and common image formats (.jpg, .png, .gif, .svg, .webp)

2. **Indexing System**
   - Content-based file hashing using BLAKE3 for fast change detection
   - SQLite database for file metadata and sync state
   - Incremental indexing to handle large file trees efficiently

3. **Sync Engine**
   - Hybrid P2P and client-server architecture
   - WebRTC for direct device-to-device communication
   - HTTP/RESTful API for server-mediated sync
   - Delta synchronization using rsync-like algorithms

4. **Conflict Resolution**
   - Single-user optimized: automatic 3-way merge for text files
   - Last-write-wins for binary files (images)
   - YAML frontmatter awareness for metadata preservation

5. **Network Layer**
   - QUIC protocol for low-latency, reliable transport
   - Connection multiplexing for efficiency
   - Automatic reconnection and offline support
   - Bandwidth optimization for mobile

## Technical Specifications

### Data Structures

```rust
struct FileMetadata {
    path: PathBuf,
    hash: String,        // BLAKE3 hash
    size: u64,
    modified: SystemTime,
    created: SystemTime,
    version: u64,        // Logical timestamp
    device_id: String,
}

struct SyncState {
    local_files: HashMap<PathBuf, FileMetadata>,
    remote_files: HashMap<String, HashMap<PathBuf, FileMetadata>>,
    conflicts: Vec<Conflict>,
    sync_queue: VecDeque<SyncOperation>,
}
```

### Sync Protocol

1. **Handshake Phase**
   - Device authentication and capability exchange
   - Sync root verification
   - Initial state comparison

2. **Delta Calculation**
   - Compare file hashes and versions
   - Identify changed, added, and deleted files
   - Generate minimal sync operations

3. **File Transfer**
   - Chunked transfer for large files
   - Compression for text files
   - Resumable downloads
   - Progress tracking

4. **Conflict Handling**
   - Automatic merge for markdown files using diff3
   - Preserve YAML frontmatter
   - Fallback to last-write-wins for unmergeable conflicts

## Platform-Specific Implementations

### Desktop (macOS/Linux)
- CLI application with background service
- System tray integration for status monitoring (set folder, start, restart, stop)
- Automatic startup on login
- Native file system integration

### Android
- Foreground service with persistent notification
- WorkManager for periodic sync when background service is restricted
- Optional Battery optimization (sync only when charging or on WiFi)
- Storage Access Framework compliance

## Performance Optimizations

1. **Efficient File Watching**
   - Recursive directory monitoring with single watcher
   - Event coalescing for rapid changes
   - Ignore patterns for temporary files

2. **Memory Management**
   - Streaming file processing for large files
   - LRU cache for frequently accessed files
   - Database connection pooling

3. **Network Efficiency**
   - Delta compression for similar files
   - Batch operations for small files
   - Adaptive chunk sizing based on network conditions

## Security Considerations

1. **Authentication**
   - Device pairing using PIN
   - JWT tokens for session management

2. **Data Integrity**
   - Cryptographic hashing for file verification
   - Checksum validation during transfer
   - Atomic file operations

## Implementation Roadmap

### Phase 1: Core Infrastructure
1. Set up Rust project structure
2. Implement file watching and indexing
3. Create basic SQLite database schema
4. Build file hashing and metadata extraction

### Phase 2: Sync Engine
1. Implement delta calculation algorithm
2. Create file transfer mechanisms
3. Add basic conflict resolution
4. Build network protocol layer

### Phase 3: Platform Integration
1. Desktop CLI application
2. Android service implementation
3. Background service management
4. System integration features

### Phase 4: Advanced Features
1. Advanced conflict resolution
2. Performance optimizations
3. Monitoring and debugging tools
4. Battery optimization for mobile

## Dependencies and Libraries

- **Rust Core**: tokio, async-std, serde
- **File System**: notify, walkdir
- **Database**: rusqlite
- **Networking**: quinn, reqwest, webRTC
- **Cryptography**: blake3, sha2
- **Android**: android-activity, ndk-glue
- **Compression**: zstd, flate2

## Testing Strategy

1. **Unit Tests**: Core algorithms and data structures
2. **Integration Tests**: End-to-end sync workflows
3. **Performance Tests**: Large file tree handling
4. **Network Tests**: Various network conditions
5. **Platform Tests**: Device-specific behavior

## Deployment

- **Desktop**: Homebrew, apt packages, standalone binaries
- **Android**: F-Droid, apk
- **Server**: apt package, binary

This architecture addresses the key requirements: efficient handling of large file trees, near real-time sync, battery optimization on mobile, and robust conflict resolution for single-user workflows.