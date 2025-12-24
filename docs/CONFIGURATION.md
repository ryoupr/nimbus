# EC2 Connect Configuration Guide

## Configuration Files

EC2 Connect supports both JSON and TOML configuration formats:

- **JSON**: `config.json` (default)
- **TOML**: `config.toml`

### Default Configuration Locations

- **Linux/macOS**: `~/.config/ec2-connect/config.json`
- **Windows**: `%APPDATA%\ec2-connect\config.json`

### Example Files

Copy and customize the example configuration files:

```bash
# JSON format
cp config.json.example ~/.config/ec2-connect/config.json

# TOML format  
cp config.toml.example ~/.config/ec2-connect/config.toml
```

## Targets File (Per-Server Settings)

EC2 Connect can optionally load a separate **targets file** to manage per-server connection settings (instance ID, ports, profile/region, SSH user/key) by name.

The targets file supports:

- **JSON** (default): `targets.json`
- **TOML**: `targets.toml`

### Default Targets File Location

- **Linux/macOS**: `~/.config/ec2-connect/targets.json`
- **Windows**: `%APPDATA%\ec2-connect\targets.json`

### Example

Start from the repository example:

```bash
cp targets.json.example ~/.config/ec2-connect/targets.json
```

Minimal JSON structure:

```json
{
 "targets": {
  "dev": {
   "instance_id": "i-1234567890abcdef0",
   "local_port": 5555,
   "remote_port": 22,
   "profile": "default",
   "region": "ap-northeast-1",
   "ssh_user": "ubuntu",
   "ssh_identity_file": "~/.ssh/dev.pem",
   "ssh_identities_only": true
  }
 }
}
```

Supported fields per target:

- `instance_id` (required)
- `local_port`, `remote_port` (optional)
- `profile`, `region` (optional)
- `ssh_user`, `ssh_identity_file`, `ssh_identities_only` (optional)

CLI values take precedence over targets file values.

## Environment Variable Overrides

All configuration values can be overridden using environment variables. This is useful for:

- CI/CD environments
- Docker containers
- Different deployment environments
- Temporary configuration changes

### AWS Configuration

| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `EC2_CONNECT_AWS_PROFILE` | AWS profile to use | `production` |
| `EC2_CONNECT_AWS_REGION` | AWS region | `us-west-2` |
| `EC2_CONNECT_CONNECTION_TIMEOUT` | Connection timeout (seconds) | `45` |
| `EC2_CONNECT_REQUEST_TIMEOUT` | Request timeout (seconds) | `90` |

### Session Management

| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `EC2_CONNECT_MAX_SESSIONS` | Max sessions per instance | `5` |
| `EC2_CONNECT_HEALTH_CHECK_INTERVAL` | Health check interval (seconds) | `3` |
| `EC2_CONNECT_INACTIVE_TIMEOUT` | Inactive timeout (seconds) | `60` |

### Reconnection Policy

| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `EC2_CONNECT_RECONNECTION_ENABLED` | Enable auto-reconnection | `true` |
| `EC2_CONNECT_MAX_RECONNECTION_ATTEMPTS` | Max reconnection attempts | `10` |
| `EC2_CONNECT_RECONNECTION_BASE_DELAY_MS` | Base delay (milliseconds) | `2000` |
| `EC2_CONNECT_RECONNECTION_MAX_DELAY_MS` | Max delay (milliseconds) | `30000` |
| `EC2_CONNECT_AGGRESSIVE_RECONNECTION` | Enable aggressive mode | `true` |
| `EC2_CONNECT_AGGRESSIVE_ATTEMPTS` | Aggressive attempts count | `15` |
| `EC2_CONNECT_AGGRESSIVE_INTERVAL_MS` | Aggressive interval (ms) | `250` |

### Performance Monitoring

| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `EC2_CONNECT_PERFORMANCE_MONITORING` | Enable monitoring | `true` |
| `EC2_CONNECT_LATENCY_THRESHOLD_MS` | Latency threshold (ms) | `150` |
| `EC2_CONNECT_OPTIMIZATION_ENABLED` | Enable optimization | `true` |

### Resource Limits

| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `EC2_CONNECT_MAX_MEMORY_MB` | Max memory usage (MB) | `8` |
| `EC2_CONNECT_MAX_CPU_PERCENT` | Max CPU usage (%) | `0.3` |
| `EC2_CONNECT_LOW_POWER_MODE` | Enable low power mode | `true` |

### User Interface

| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `EC2_CONNECT_RICH_UI` | Enable rich terminal UI | `false` |
| `EC2_CONNECT_UI_UPDATE_INTERVAL_MS` | UI update interval (ms) | `500` |
| `EC2_CONNECT_NOTIFICATIONS` | Enable notifications | `false` |

### Logging

| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `EC2_CONNECT_LOG_LEVEL` | Log level | `debug` |
| `EC2_CONNECT_FILE_LOGGING` | Enable file logging | `true` |
| `EC2_CONNECT_LOG_FILE` | Log file path | `/var/log/ec2-connect.log` |
| `EC2_CONNECT_JSON_LOGGING` | Enable JSON format | `true` |

### VS Code / SSH

| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `EC2_CONNECT_VSCODE_PATH` | Path to VS Code executable | `/opt/homebrew/bin/code` |
| `EC2_CONNECT_SSH_CONFIG_PATH` | Path to SSH config file | `~/.ssh/config` |
| `EC2_CONNECT_VSCODE_AUTO_LAUNCH` | Auto-launch VS Code (true/false) | `false` |
| `EC2_CONNECT_VSCODE_NOTIFICATIONS` | Enable notifications (true/false) | `false` |
| `EC2_CONNECT_VSCODE_LAUNCH_DELAY` | Launch delay seconds | `2` |
| `EC2_CONNECT_VSCODE_AUTO_UPDATE_SSH` | Auto update SSH config (true/false) | `true` |
| `EC2_CONNECT_SSH_USER` | SSH username for generated entry | `ubuntu` |
| `EC2_CONNECT_SSH_IDENTITY_FILE` | SSH IdentityFile path for generated entry | `~/.ssh/my-key.pem` |
| `EC2_CONNECT_SSH_IDENTITIES_ONLY` | Enable IdentitiesOnly (true/false) | `true` |

You can also set these values in the main configuration file under `vscode`:

```json
{
 "vscode": {
  "ssh_user": "ubuntu",
  "ssh_identity_file": "~/.ssh/my-key.pem",
  "ssh_identities_only": true
 }
}
```

## Configuration Examples

### Development Environment

```bash
export EC2_CONNECT_LOG_LEVEL=debug
export EC2_CONNECT_MAX_MEMORY_MB=50
export EC2_CONNECT_PERFORMANCE_MONITORING=true
```

### Production Environment

```bash
export EC2_CONNECT_LOG_LEVEL=warn
export EC2_CONNECT_MAX_MEMORY_MB=10
export EC2_CONNECT_MAX_CPU_PERCENT=0.5
export EC2_CONNECT_LOW_POWER_MODE=true
export EC2_CONNECT_JSON_LOGGING=true
```

### CI/CD Environment

```bash
export EC2_CONNECT_RICH_UI=false
export EC2_CONNECT_NOTIFICATIONS=false
export EC2_CONNECT_FILE_LOGGING=false
export EC2_CONNECT_RECONNECTION_ENABLED=false
```

### Aggressive Reconnection Mode

```bash
export EC2_CONNECT_AGGRESSIVE_RECONNECTION=true
export EC2_CONNECT_AGGRESSIVE_ATTEMPTS=20
export EC2_CONNECT_AGGRESSIVE_INTERVAL_MS=200
export EC2_CONNECT_MAX_RECONNECTION_ATTEMPTS=50
```

## Configuration Validation

EC2 Connect validates all configuration values on startup and provides detailed error messages for invalid settings:

- **Range validation**: Ensures numeric values are within acceptable ranges
- **Type validation**: Ensures boolean values are properly formatted
- **Dependency validation**: Ensures related settings are consistent
- **Performance warnings**: Warns about settings that may impact performance

### Common Validation Errors

1. **Invalid boolean values**: Use `true` or `false` (case-sensitive)
2. **Out of range values**: Check minimum/maximum allowed values
3. **Inconsistent delays**: Ensure `max_delay_ms >= base_delay_ms`
4. **Zero values**: Most timeout and interval values must be > 0

## Best Practices

### Performance Optimization

- Keep `max_memory_mb` ≤ 10 for optimal performance
- Set `max_cpu_percent` ≤ 0.5 to avoid impacting other processes
- Use `low_power_mode = true` for battery-powered devices

### Reliability

- Enable `reconnection.enabled = true` for production use
- Set reasonable `max_attempts` (5-10) to avoid excessive retries
- Use `aggressive_mode = false` in production to reduce load

### Monitoring

- Enable `performance.monitoring_enabled = true` for troubleshooting
- Set appropriate `latency_threshold_ms` based on your network
- Use `json_format = true` for structured log analysis

### Security

- Avoid storing sensitive values in configuration files
- Use environment variables for credentials and sensitive settings
- Regularly rotate AWS credentials and profiles
