# SyncMD VPS Setup Guide

## Overview

This guide shows how to set up SyncMD with your Linux VPS as the central server and your Macs as clients. This architecture provides:

- **Central Storage**: All files stored on your VPS
- **Multi-Client Support**: Connect multiple Macs to the same server
- **Automatic Sync**: Files sync between clients via the VPS
- **Conflict Resolution**: Last-write-wins strategy

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Mac 1     │     │   Mac 2     │     │   Mac N     │
│   (Client)  │     │   (Client)  │     │   (Client)  │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       └───────────────────┼───────────────────┘
                           │
                  ┌────────▼─────────┐
                  │   Your VPS       │
                  │   (Server)       │
                  │  • File Storage  │
                  │  • Sync Logic    │
                  │  • Client Mgmt   │
                  └───────────────────┘
```

## Quick Start

### 1. Build the Binaries

```bash
# On your development machine
cargo build --release
```

This creates three binaries:
- `target/release/syncmd` - Client application
- `target/release/syncmd-vps` - VPS server application
- `target/release/syncmd-server` - Basic server (legacy)

### 2. Deploy to VPS

```bash
# Copy the VPS binary to your server
scp target/release/syncmd-vps user@your-vps-ip:~/

# SSH into your VPS
ssh user@your-vps-ip

# Make it executable
chmod +x syncmd-vps
```

### 3. Start VPS Server

```bash
# On your VPS
./syncmd-vps sync --path /home/user/syncmd_storage --port 8080
```

### 4. Set Up Clients

```bash
# On each Mac
./target/release/syncmd init --path /path/to/your/docs --name "MacBook Pro"

# Connect to VPS
./target/release/syncmd sync --path /path/to/your/docs --connect your-vps-ip:8080
```

## Detailed Setup

### VPS Server Configuration

#### 1. Create Storage Directory
```bash
# On VPS
mkdir -p /home/user/syncmd_storage
chmod 755 /home/user/syncmd_storage
```

#### 2. Start Server with Options
```bash
# Basic start
./syncmd-vps sync --path /home/user/syncmd_storage --port 8080

# Start with systemd (persistent)
sudo systemctl edit syncmd-vps.service
```

#### 3. Systemd Service (Optional)

Create `/etc/systemd/system/syncmd-vps.service`:

```ini
[Unit]
Description=SyncMD VPS Server
After=network.target

[Service]
Type=simple
User=user
WorkingDirectory=/home/user
ExecStart=/home/user/syncmd-vps sync --path /home/user/syncmd_storage --port 8080
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```bash
# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable syncmd-vps
sudo systemctl start syncmd-vps

# Check status
sudo systemctl status syncmd-vps
```

### Client Configuration

#### 1. Initialize Sync Directory
```bash
# One-time setup
./syncmd init --path ~/Documents/markdown --name "Work MacBook"

# Check configuration
./syncmd status
```

#### 2. Start Syncing
```bash
# Connect to VPS
./syncmd sync --path ~/Documents/markdown --connect your-vps-ip:8080

# Run in background
nohup ./syncmd sync --path ~/Documents/markdown --connect your-vps-ip:8080 &
```

## How It Works

### Server Features
- **File Storage**: Stores all markdown files and images
- **Multi-Client**: Handles multiple simultaneous connections
- **Version Management**: Tracks file versions and timestamps
- **Conflict Resolution**: Uses last-write-wins strategy
- **Persistence**: Files stored on disk permanently

### Client Features
- **Local Indexing**: Tracks local file state with BLAKE3 hashes
- **Sync Operations**: Calculates what needs to be synced
- **File Transfer**: Downloads missing files from server
- **Periodic Sync**: Syncs every 30 seconds automatically

### Sync Process
1. Client connects to VPS server
2. Client sends local file list to server
3. Server calculates sync operations (add/update/delete)
4. Client requests missing files from server
5. Server sends file content
6. Client applies changes locally
7. Process repeats every 30 seconds

## Network Requirements

### VPS Server
- **Port 8080** (default) - TCP for client connections
- **Firewall**: Allow incoming TCP on port 8080
- **Storage**: Enough disk space for all files

### Client Machines
- **Outbound TCP**: Access to VPS on port 8080
- **No special ports**: Clients make outbound connections only

### Firewall Configuration

#### VPS (ufw example)
```bash
# Allow port 8080
sudo ufw allow 8080/tcp

# Check rules
sudo ufw status
```

#### VPS (iptables example)
```bash
# Allow port 8080
sudo iptables -A INPUT -p tcp --dport 8080 -j ACCEPT

# Save rules
sudo iptables-save > /etc/iptables/rules.v4
```

## Security Considerations

### Current Limitations
- No encryption (data sent in clear text)
- No authentication (anyone can connect)
- No access control

### Basic Security
1. **Firewall**: Restrict access to specific IP addresses
2. **SSH Tunnel**: Use SSH port forwarding for encrypted transport
3. **VPN**: Use WireGuard or OpenVPN for secure connection

### SSH Tunnel Method

```bash
# On client, create SSH tunnel
ssh -L 8080:localhost:8080 user@your-vps-ip -N

# Then connect to local tunnel
./syncmd sync --path ~/docs --connect localhost:8080
```

## Monitoring and Logs

### Server Logs
```bash
# View real-time logs
journalctl -u syncmd-vps -f

# View recent logs
journalctl -u syncmd-vps --since "1 hour ago"
```

### Client Logs
```bash
# Client logs to stdout
./syncmd sync --path ~/docs --connect vps-ip:8080 2>&1 | tee syncmd.log
```

## Troubleshooting

### Common Issues

#### Connection Refused
```bash
# Check if server is running
ps aux | grep syncmd-vps

# Check port
netstat -tlnp | grep 8080

# Check firewall
sudo ufw status
```

#### Permission Denied
```bash
# Check storage directory permissions
ls -la /home/user/syncmd_storage

# Fix permissions
chmod 755 /home/user/syncmd_storage
```

#### Sync Not Working
```bash
# Check client connection
telnet your-vps-ip 8080

# Check server logs
journalctl -u syncmd-vps -f

# Test with verbose output
./syncmd --verbose sync --path ~/docs --connect vps-ip:8080
```

## Performance Optimization

### Server Side
- Use SSD storage for better I/O
- Monitor disk usage
- Consider log rotation

### Client Side
- Exclude large binary files if not needed
- Monitor network bandwidth
- Use efficient file structures

## Backup Strategy

### VPS Backup
```bash
# Backup storage directory
tar -czf syncmd_backup_$(date +%Y%m%d).tar.gz /home/user/syncmd_storage/

# Copy to backup location
scp syncmd_backup_*.tar.gz backup-server:/backups/
```

### Client Backup
Clients maintain local copies and can restore from VPS if needed.

## Scaling Considerations

### Multiple Users
- Create separate storage directories per user
- Use different ports for different users
- Implement authentication (future enhancement)

### High Availability
- Use multiple VPS instances
- Load balancing (future enhancement)
- Database clustering (future enhancement)

## Next Steps

The current implementation provides a solid foundation. Future enhancements:

1. **Encryption**: Add TLS/SSL support
2. **Authentication**: User authentication and access control
3. **Real-time Sync**: File system watching for instant sync
4. **Web Interface**: Browser-based management
5. **Mobile Apps**: iOS and Android clients
6. **Docker Support**: Containerized deployment