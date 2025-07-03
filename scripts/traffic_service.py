#!/usr/bin/env python3

"""
Traffic Generator Service Manager

This script provides easy management of the obsctl traffic generator service.
It handles plist installation, service start/stop, and status checking.
"""

import os
import sys
import subprocess
import argparse
from pathlib import Path

# Service configuration
SERVICE_NAME = "com.obsctl.traffic-generator"
PLIST_FILE = "com.obsctl.traffic-generator.plist"
LOCK_FILE = "/tmp/obsctl-traffic-generator.lock"

def get_script_dir():
    """Get the directory where this script is located"""
    return Path(__file__).parent.absolute()

def get_plist_path():
    """Get the path to the plist file"""
    return get_script_dir() / PLIST_FILE

def get_user_agents_dir():
    """Get the user's LaunchAgents directory"""
    home = Path.home()
    agents_dir = home / "Library" / "LaunchAgents"
    agents_dir.mkdir(exist_ok=True)
    return agents_dir

def install_plist():
    """Install the plist file to LaunchAgents"""
    plist_source = get_plist_path()
    if not plist_source.exists():
        print(f"ERROR: Plist file not found: {plist_source}")
        return False

    agents_dir = get_user_agents_dir()
    plist_dest = agents_dir / PLIST_FILE

    # Copy plist file
    import shutil
    shutil.copy2(plist_source, plist_dest)
    print(f"Installed plist to: {plist_dest}")
    return True

def uninstall_plist():
    """Remove the plist file from LaunchAgents"""
    agents_dir = get_user_agents_dir()
    plist_dest = agents_dir / PLIST_FILE

    if plist_dest.exists():
        plist_dest.unlink()
        print(f"Removed plist from: {plist_dest}")
        return True
    else:
        print("Plist not installed")
        return False

def run_launchctl(command, service_name=SERVICE_NAME):
    """Run launchctl command"""
    cmd = ["launchctl", command, service_name]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True)
        return result.returncode == 0, result.stdout, result.stderr
    except Exception as e:
        return False, "", str(e)

def start_service():
    """Start the traffic generator service"""
    print("Starting traffic generator service...")

    # First install plist if not already installed
    if not install_plist():
        return False

    # Load the service
    success, stdout, stderr = run_launchctl("load")
    if not success:
        print(f"Failed to load service: {stderr}")
        return False

    # Start the service
    success, stdout, stderr = run_launchctl("start")
    if success:
        print("✅ Traffic generator service started successfully")
        print(f"📋 Check status with: python3 {__file__} status")
        print(f"📝 View logs with: tail -f traffic_generator.log")
        return True
    else:
        print(f"Failed to start service: {stderr}")
        return False

def stop_service():
    """Stop the traffic generator service"""
    print("Stopping traffic generator service...")

    # Stop the service
    success, stdout, stderr = run_launchctl("stop")
    if success or "Could not find specified service" in stderr:
        print("✅ Traffic generator service stopped")
    else:
        print(f"Warning: {stderr}")

    # Unload the service
    success, stdout, stderr = run_launchctl("unload")
    if success or "Could not find specified service" in stderr:
        print("✅ Service unloaded")
    else:
        print(f"Warning: {stderr}")

    return True

def get_service_status():
    """Get the current status of the service"""
    # Check if plist is installed
    agents_dir = get_user_agents_dir()
    plist_dest = agents_dir / PLIST_FILE

    if not plist_dest.exists():
        return "not_installed", "Plist not installed"

    # Check launchctl list
    cmd = ["launchctl", "list", SERVICE_NAME]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode == 0:
            # Service is loaded, check if it's running
            lines = result.stdout.strip().split('\n')
            for line in lines:
                if SERVICE_NAME in line:
                    parts = line.split()
                    if len(parts) >= 3:
                        pid = parts[0]
                        status = parts[1]
                        if pid != "-":
                            return "running", f"Running (PID: {pid})"
                        else:
                            return "loaded", "Loaded but not running"
            return "loaded", "Loaded but status unclear"
        else:
            return "not_loaded", "Not loaded"
    except Exception as e:
        return "error", f"Error checking status: {e}"

def check_lock_file():
    """Check if the lock file exists and get PID"""
    if os.path.exists(LOCK_FILE):
        try:
            with open(LOCK_FILE, 'r') as f:
                pid = int(f.read().strip())

            # Check if process is still running
            try:
                os.kill(pid, 0)  # Signal 0 just checks if process exists
                return True, pid
            except OSError:
                # Process not running, stale lock file
                return False, None
        except:
            return False, None
    return False, None

def status():
    """Show detailed status of the traffic generator"""
    print("🔍 Traffic Generator Service Status")
    print("=" * 40)

    # Check service status
    service_status, message = get_service_status()
    print(f"Service Status: {message}")

    # Check lock file
    lock_exists, pid = check_lock_file()
    if lock_exists:
        print(f"Lock File: Active (PID: {pid})")
    else:
        print("Lock File: Not found")

    # Check log files
    log_files = [
        "traffic_generator.log",
        "traffic_generator_service.log",
        "traffic_generator_service.error.log"
    ]

    print("\n📝 Log Files:")
    for log_file in log_files:
        if os.path.exists(log_file):
            stat = os.stat(log_file)
            size_mb = stat.st_size / (1024 * 1024)
            print(f"  {log_file}: {size_mb:.1f} MB")
        else:
            print(f"  {log_file}: Not found")

    # Show recent log entries if service is running
    if service_status == "running" and os.path.exists("traffic_generator.log"):
        print("\n📋 Recent Activity (last 5 lines):")
        try:
            result = subprocess.run(["tail", "-5", "traffic_generator.log"],
                                  capture_output=True, text=True)
            if result.returncode == 0:
                for line in result.stdout.strip().split('\n'):
                    print(f"  {line}")
        except:
            print("  Could not read log file")

def main():
    """Main function"""
    parser = argparse.ArgumentParser(description="Traffic Generator Service Manager")
    parser.add_argument("command", choices=["start", "stop", "restart", "status", "install", "uninstall"],
                       help="Command to execute")

    args = parser.parse_args()

    if args.command == "start":
        success = start_service()
        sys.exit(0 if success else 1)

    elif args.command == "stop":
        success = stop_service()
        sys.exit(0 if success else 1)

    elif args.command == "restart":
        print("Restarting traffic generator service...")
        stop_service()
        import time
        time.sleep(2)
        success = start_service()
        sys.exit(0 if success else 1)

    elif args.command == "status":
        status()

    elif args.command == "install":
        success = install_plist()
        print("✅ Plist installed. Use 'start' command to begin service.")
        sys.exit(0 if success else 1)

    elif args.command == "uninstall":
        stop_service()
        success = uninstall_plist()
        sys.exit(0 if success else 1)

if __name__ == "__main__":
    main()
