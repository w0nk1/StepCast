#!/usr/bin/env swift
// Diagnostic script: checks screen recording permission detection methods
// Run: swift scripts/diagnose_screen_permission.swift

import Cocoa
import CoreGraphics

print("=== Screen Recording Permission Diagnostics ===")
print("macOS version: \(ProcessInfo.processInfo.operatingSystemVersionString)")
print("Our PID: \(ProcessInfo.processInfo.processIdentifier)")
print()

// Method 1: CGPreflightScreenCaptureAccess
let preflight = CGPreflightScreenCaptureAccess()
print("1) CGPreflightScreenCaptureAccess() = \(preflight)")
print()

// Method 2: Window-name heuristic
print("2) Window-name heuristic (CGWindowListCopyWindowInfo):")
let ourPid = ProcessInfo.processInfo.processIdentifier

guard let windowList = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else {
    print("   ERROR: CGWindowListCopyWindowInfo returned nil")
    exit(1)
}

print("   Total on-screen windows: \(windowList.count)")
print()

var foundNamedForeignWindow = false

for (idx, window) in windowList.enumerated() {
    let ownerPid = window[kCGWindowOwnerPID as String] as? Int32 ?? 0
    let ownerName = window[kCGWindowOwnerName as String] as? String ?? "?"
    let windowName = window[kCGWindowName as String] as? String
    let windowId = window[kCGWindowNumber as String] as? Int ?? 0
    let layer = window[kCGWindowLayer as String] as? Int ?? 0
    let sharingState = window[kCGWindowSharingState as String] as? Int ?? -1

    let isOurs = ownerPid == ourPid
    let isDock = ownerName == "Dock"
    let isWindowServer = ownerName == "Window Server"

    let hasName = windowName != nil
    let flag: String
    if isOurs {
        flag = "[OUR]"
    } else if isDock {
        flag = "[DOCK]"
    } else if isWindowServer {
        flag = "[WS]"
    } else if hasName {
        flag = "[HAS NAME -> PERMISSION OK]"
        foundNamedForeignWindow = true
    } else {
        flag = "[NO NAME]"
    }

    let nameDisplay = windowName.map { "\"\($0)\"" } ?? "nil"
    print("   [\(idx)] pid=\(ownerPid) owner=\"\(ownerName)\" name=\(nameDisplay) id=\(windowId) layer=\(layer) sharing=\(sharingState) \(flag)")
}

print()
print("   Result: \(foundNamedForeignWindow ? "PERMISSION DETECTED (found named foreign window)" : "NO PERMISSION DETECTED (no named foreign windows)")")
print()

// Method 3: kCGWindowSharingState check
print("3) kCGWindowSharingState check:")
var foundNonZeroSharing = false
for window in windowList {
    let ownerPid = window[kCGWindowOwnerPID as String] as? Int32 ?? 0
    if ownerPid == ourPid { continue }
    let ownerName = window[kCGWindowOwnerName as String] as? String ?? "?"
    if ownerName == "Dock" || ownerName == "Window Server" { continue }
    let sharingState = window[kCGWindowSharingState as String] as? Int ?? 0
    if sharingState != 0 {
        foundNonZeroSharing = true
        print("   Found non-zero sharing state: pid=\(ownerPid) owner=\"\(ownerName)\" sharing=\(sharingState)")
        break
    }
}
if !foundNonZeroSharing {
    print("   All foreign windows have sharingState=0 (or none found)")
}
print()

// Summary
print("=== SUMMARY ===")
print("CGPreflightScreenCaptureAccess: \(preflight ? "TRUE" : "FALSE")")
print("Window-name heuristic:          \(foundNamedForeignWindow ? "TRUE" : "FALSE")")
print("SharingState heuristic:         \(foundNonZeroSharing ? "TRUE" : "FALSE")")
print()
if !preflight && !foundNamedForeignWindow {
    print("IMPORTANT: Make sure other app windows are visible on screen (e.g. Finder, Safari)")
    print("when running this script. The heuristic needs at least one foreign window to check.")
}
