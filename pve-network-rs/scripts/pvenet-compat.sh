#!/bin/bash
# Proxmox VE Network CLI Compatibility Wrapper
# This script provides compatibility with existing Proxmox network tools

set -e

PVENET_BIN="${PVENET_BIN:-/usr/bin/pvenet}"
SCRIPT_NAME="$(basename "$0")"

# Function to show usage
show_usage() {
    cat << EOF
Usage: $SCRIPT_NAME [OPTIONS] COMMAND [ARGS...]

Proxmox VE Network Management CLI - Compatibility Wrapper

This wrapper provides compatibility with existing Proxmox network tools
and maintains the same command-line interface and output formats.

COMMANDS:
    validate [CONFIG]           Validate network configuration
    apply [--dry-run]          Apply network configuration  
    rollback [VERSION]         Rollback network configuration
    status [--verbose]         Show network status
    list [--node NODE]         List network interfaces
    show [--interface IFACE]   Show network configuration
    reload [--node NODE]       Reload network configuration

OPTIONS:
    -h, --help                 Show this help message
    -v, --verbose              Enable verbose output
    -d, --debug                Enable debug output
    -q, --quiet                Suppress output except errors
    --version                  Show version information

EXAMPLES:
    # Validate current configuration
    $SCRIPT_NAME validate

    # Apply configuration with dry-run
    $SCRIPT_NAME apply --dry-run

    # Show detailed status
    $SCRIPT_NAME status --verbose

    # List interfaces in JSON format
    $SCRIPT_NAME list --format json

    # Show configuration for specific interface
    $SCRIPT_NAME show --interface eth0

    # Reload network configuration
    $SCRIPT_NAME reload

COMPATIBILITY:
    This tool maintains compatibility with existing Proxmox network
    utilities and provides the same output formats and behavior.

EOF
}

# Function to handle pvesh-style commands
handle_pvesh_compat() {
    case "$1" in
        "get")
            shift
            case "$1" in
                "/nodes/"*"/network")
                    # Extract node name from path
                    NODE=$(echo "$1" | sed 's|/nodes/\([^/]*\)/network.*|\1|')
                    exec "$PVENET_BIN" list --node "$NODE" --format json
                    ;;
                "/nodes/"*"/network/"*)
                    # Extract node and interface from path
                    NODE=$(echo "$1" | sed 's|/nodes/\([^/]*\)/network/.*|\1|')
                    IFACE=$(echo "$1" | sed 's|.*/network/\([^/]*\).*|\1|')
                    exec "$PVENET_BIN" show --node "$NODE" --interface "$IFACE"
                    ;;
                *)
                    echo "Error: Unsupported pvesh path: $1" >&2
                    exit 1
                    ;;
            esac
            ;;
        "set"|"create"|"delete")
            echo "Error: Write operations not yet supported in compatibility mode" >&2
            exit 1
            ;;
        *)
            echo "Error: Unsupported pvesh command: $1" >&2
            exit 1
            ;;
    esac
}

# Parse global options
VERBOSE=false
DEBUG=false
QUIET=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_usage
            exit 0
            ;;
        --version)
            exec "$PVENET_BIN" --version
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -d|--debug)
            DEBUG=true
            shift
            ;;
        -q|--quiet)
            QUIET=true
            shift
            ;;
        -*)
            echo "Error: Unknown option: $1" >&2
            exit 1
            ;;
        *)
            break
            ;;
    esac
done

# Check if we have a command
if [[ $# -eq 0 ]]; then
    echo "Error: No command specified" >&2
    show_usage
    exit 1
fi

COMMAND="$1"
shift

# Build pvenet command with global options
PVENET_ARGS=()
if [[ "$VERBOSE" == "true" ]]; then
    PVENET_ARGS+=("--verbose")
fi
if [[ "$DEBUG" == "true" ]]; then
    PVENET_ARGS+=("--debug")
fi
if [[ "$QUIET" == "true" ]]; then
    PVENET_ARGS+=("--quiet")
fi

# Handle commands
case "$COMMAND" in
    validate)
        exec "$PVENET_BIN" "${PVENET_ARGS[@]}" validate "$@"
        ;;
    apply)
        exec "$PVENET_BIN" "${PVENET_ARGS[@]}" apply "$@"
        ;;
    rollback)
        exec "$PVENET_BIN" "${PVENET_ARGS[@]}" rollback "$@"
        ;;
    status)
        exec "$PVENET_BIN" "${PVENET_ARGS[@]}" status "$@"
        ;;
    list)
        exec "$PVENET_BIN" "${PVENET_ARGS[@]}" list "$@"
        ;;
    show)
        exec "$PVENET_BIN" "${PVENET_ARGS[@]}" show "$@"
        ;;
    reload)
        exec "$PVENET_BIN" "${PVENET_ARGS[@]}" reload "$@"
        ;;
    # Handle pvesh-style commands
    get|set|create|delete)
        handle_pvesh_compat "$COMMAND" "$@"
        ;;
    *)
        echo "Error: Unknown command: $COMMAND" >&2
        echo "Run '$SCRIPT_NAME --help' for usage information" >&2
        exit 1
        ;;
esac