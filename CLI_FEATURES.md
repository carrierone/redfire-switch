# RedFire Switch Interactive CLI

## Overview

The RedFire Switch Interactive CLI (`redfire-cli`) provides a comprehensive command-line interface for managing and monitoring the RedFire Switch telecommunications platform, similar to FreeSWITCH's `fs_cli`.

## Features

### 🎯 **Core Capabilities**
- **Interactive Shell** - Full-featured readline interface with history
- **Tab Completion** - Intelligent command and argument completion
- **Comprehensive Help** - Built-in documentation for all commands
- **Colored Output** - Syntax highlighting and colored status indicators
- **Session Management** - Connection state and configuration persistence

### 🚀 **Command Categories**

#### **Status & Monitoring**
```bash
status [calls|channels|gateways|trunks|codecs|memory|cpu|network|database|security|all]
show <object> [filter]
calls [list|count|active|history|search]
channels
```

#### **Call Control**
```bash
hangup <call-id|all> [reason]
bridge <call-id-1> <call-id-2>
transfer <call-id> <destination>
```

#### **Gateway & Routing**
```bash
gateway [list|status|enable|disable|test|stats] [gateway-name]
trunk [list|status|stats|test|reset]
route [list|add|remove|test|stats|refresh]
lcr [lookup|test|stats|refresh|export]
```

#### **Configuration**
```bash
set <parameter> <value>
get <parameter>
reload [component]
save
```

#### **Codec & Transcoding**
```bash
codec [list|test|benchmark|stats|priority] [codec-name]
transcode <from-codec> <to-codec>
gpu
```

#### **Debug & Diagnostics**
```bash
debug [sip|rtp|codec|routing|security|all|off] [level]
trace [start|stop|status|export]
log [level|tail|export|rotate|clear]
test [component]
```

#### **Security**
```bash
security [action]
auth [action]
firewall [action]
```

### 🔧 **Advanced Features**

#### **Tab Completion Engine**
- **Command Completion** - Complete command names as you type
- **Argument Completion** - Context-aware argument suggestions
- **Value Completion** - Complete valid values for parameters
- **File Completion** - File and directory path completion
- **Smart Filtering** - Filters suggestions based on current input

#### **Help System**
```bash
help                    # General help
help <command>          # Detailed command help
?                       # Alias for help
```

#### **Session Features**
```bash
connect [host] [port]   # Connect to RedFire Switch
disconnect              # Disconnect
version                 # Show version info
uptime                  # Show system uptime
clear                   # Clear screen
quit/exit/bye           # Exit CLI
```

### 📊 **Example Usage**

#### **Basic Status Monitoring**
```bash
redfire@localhost > status calls
┌─────────────────────┬─────────┐
│ Metric              │ Value   │
├─────────────────────┼─────────┤
│ Active Calls        │ 42      │
│ Total Calls Today   │ 1,234   │
│ Failed Calls        │ 5       │
│ Average Call Duration │ 00:03:45 │
└─────────────────────┴─────────┘

redfire@localhost > show calls
┌──────────────┬─────────────┬─────────────┬──────────┬────────┬────────┐
│ Call ID      │ From        │ To          │ Duration │ Codec  │ Status │
├──────────────┼─────────────┼─────────────┼──────────┼────────┼────────┤
│ 12345678-abcd│ +1234567890 │ +0987654321 │ 00:02:35 │ G.729  │ Active │
│ 87654321-dcba│ +5555551234 │ +4444440987 │ 00:01:12 │ G.711u │ Active │
└──────────────┴─────────────┴─────────────┴──────────┴────────┴────────┘
```

#### **Gateway Management**
```bash
redfire@localhost > gateway list
┌──────────┬────────┬───────────────────────┬──────────────┐
│ Name     │ Status │ Address               │ Active Calls │
├──────────┼────────┼───────────────────────┼──────────────┤
│ carrier1 │ Online │ sip.carrier1.com:5060 │ 25           │
│ carrier2 │ Online │ gw.carrier2.net:5061  │ 17           │
└──────────┴────────┴───────────────────────┴──────────────┘

redfire@localhost > gateway test carrier1
Testing gateway carrier1... OK (response time: 45ms)
```

#### **Codec Operations**
```bash
redfire@localhost > codec list
┌────────┬────────┬───────────┬─────────┐
│ Codec  │ Status │ GPU Accel │ Quality │
├────────┼────────┼───────────┼─────────┤
│ G.729  │ Active │ Yes       │ 4.2     │
│ G.711u │ Active │ Yes       │ 4.5     │
│ G.711a │ Active │ Yes       │ 4.5     │
│ G.722  │ Active │ Yes       │ 4.3     │
└────────┴────────┴───────────┴─────────┘

redfire@localhost > transcode g729 g711u
Transcoding test: g729 -> g711u completed successfully
```

#### **Real-time Debugging**
```bash
redfire@localhost > debug sip
Debug enabled for sip component

redfire@localhost > calls active
42 active calls currently in progress

redfire@localhost > hangup all SYSTEM_SHUTDOWN
Hung up all active calls
```

### 🎨 **Visual Features**

#### **Colored Output**
- **Green** - Success messages and online status
- **Red** - Error messages and offline status
- **Yellow** - Warnings and informational prompts
- **Cyan** - Command names and headers
- **Blue** - Tab completion hints
- **Magenta** - Parameter values

#### **Formatted Tables**
- **Box Drawing** - Professional table borders
- **Column Alignment** - Auto-sized columns
- **Header Highlighting** - Bold cyan headers
- **Row Separation** - Clear visual boundaries

#### **Interactive Banner**
```
 ____          _   _____ _            ____        _ _       _     
|  _ \ ___  __| | |  ___(_)_ __ ___  / ___|_      _(_) |_ ___| |__  
| |_) / _ \/ _` | | |_  | | '__/ _ \ \___ \ \ /\ / / | __/ __| '_ \ 
|  _ <  __/ (_| | |  _| | | | |  __/  ___) \ V  V /| | || (__| | | |
|_| \_\___|\__,_| |_|   |_|_|  \___| |____/ \_/\_/ |_|\__\___|_| |_|

Interactive Command Line Interface
Version 0.1.0
```

### ⚙️ **Configuration Options**

#### **Command Line Arguments**
```bash
redfire-cli --help
RedFire Switch Interactive CLI

Usage: redfire-cli [OPTIONS]

Options:
  -H, --host <HOST>            Host to connect to [default: localhost]
  -P, --port <PORT>            Port to connect to [default: 8080]
  -v, --verbose                Enable verbose output
  -x, --execute <EXECUTE>      Execute command and exit (non-interactive mode)
      --no-color               Disable colored output
      --timeout <TIMEOUT>      Connection timeout in seconds [default: 10]
      --log-level <LOG_LEVEL>  Log level [default: info]
      --log-file <LOG_FILE>    Log to file instead of stdout
  -h, --help                   Print help
  -V, --version                Print version
```

#### **Non-Interactive Mode**
```bash
# Execute single command
redfire-cli -x "status calls"

# Connect to specific host
redfire-cli -H 192.168.1.100 -P 8080

# Enable verbose logging
redfire-cli -v --log-level debug
```

### 🛠️ **Installation & Usage**

#### **Build from Source**
```bash
cargo build --bin redfire-cli --release
```

#### **Run Interactive Mode**
```bash
./target/release/redfire-cli
```

#### **Run Single Command**
```bash
./target/release/redfire-cli -x "show calls"
```

### 🎯 **FreeSWITCH fs_cli Compatibility**

The RedFire CLI provides similar functionality to FreeSWITCH's fs_cli:

| FreeSWITCH fs_cli | RedFire CLI | Description |
|------------------|-------------|-------------|
| `fs_cli -x "show calls"` | `redfire-cli -x "show calls"` | Execute single command |
| `show channels` | `show channels` | Display channel info |
| `status` | `status` | System status |
| `hangup all` | `hangup all` | Hangup all calls |
| `reload` | `reload` | Reload configuration |
| `help` | `help` | Show help |

### 🚀 **Future Enhancements**

- **Real-time Updates** - Live updating displays
- **Scripting Support** - Batch command execution
- **Remote Management** - Multi-host management
- **Plugin System** - Custom command extensions
- **Configuration Templates** - Quick setup wizards
- **Performance Monitoring** - Built-in metrics dashboard

---

**The RedFire Switch CLI provides professional-grade telecommunications management with an intuitive, feature-rich interface that rivals commercial solutions.**