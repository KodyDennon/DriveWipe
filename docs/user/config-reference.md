# Configuration Reference

Default location: `~/.config/drivewipe/config.toml`

## General Settings

| Key | Type | Default | Description |
|---|---|---|---|
| `default_method` | string | `"dod-short"` | Default wipe method ID |
| `parallel_drives` | integer | `2` | Max concurrent wipe operations |
| `auto_verify` | bool | `true` | Automatically verify after wipe |
| `auto_report_json` | bool | `true` | Generate JSON report after wipe |
| `log_level` | string | `"info"` | Logging level (trace, debug, info, warn, error) |
| `operator_name` | string | `""` | Operator name for reports and audit |
| `state_save_interval_secs` | integer | `10` | Resume state save frequency |

## New Settings

| Key | Type | Default | Description |
|---|---|---|---|
| `notifications_enabled` | bool | `true` | Desktop notifications on completion |
| `sleep_prevention_enabled` | bool | `true` | Prevent system sleep during operations |
| `auto_health_pre_wipe` | bool | `true` | Health check before wipe |
| `keyboard_lock_sequence` | string | `"unlock"` | Character sequence to unlock keyboard |
| `profiles_dir` | string | `"~/.config/drivewipe/profiles"` | Custom drive profiles directory |
| `audit_dir` | string | `"~/.local/share/drivewipe/audit"` | Audit log output directory |
| `performance_history_dir` | string | `"~/.local/share/drivewipe/performance"` | Historical performance data |

## Custom Methods

```toml
[[custom_methods]]
id = "my-method"
name = "My Custom Method"
description = "Description"
verify_after = true

[[custom_methods.passes]]
pattern_type = "zero"          # zero, one, random, constant, repeating

[[custom_methods.passes]]
pattern_type = "constant"
constant_value = 170           # 0xAA

[[custom_methods.passes]]
pattern_type = "repeating"
repeating_pattern = [0xDE, 0xAD, 0xBE, 0xEF]
```
