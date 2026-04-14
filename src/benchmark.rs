//! `sift benchmark` — measures filter reduction using realistic fixture output.
//!
//! Runs each built-in filter against a representative sample of real-world
//! command output and reports bytes saved and reduction percentage.

use crate::filters::{
    self, agvtool, codesign, crashlog, curl, fastlane, git_diff, git_log, git_status, grep,
    periphery, read, swift_build, swift_package, swift_test, swiftformat, swiftlint,
    xcodebuild_build, xcodebuild_test, xcrun_simctl, Verbosity,
};

// ---------------------------------------------------------------------------
// Fixtures — representative real-world output for each supported family
// ---------------------------------------------------------------------------

const GIT_STATUS: &str = "\
On branch feature/my-feature
Your branch is ahead of 'origin/feature/my-feature' by 2 commits.

Changes to be committed:
  (use \"git restore --staged <file>...\" to unstage)
\tmodified:   src/executor.rs
\tnew file:   src/benchmark.rs

Changes not staged for commit:
  (use \"git restore <file>...\" to discard changes in working directory)
\tmodified:   src/main.rs
\tmodified:   src/cli.rs
\tmodified:   Cargo.toml

Untracked files:
  (use \"git add <file>...\" to include in what will be committed)
\tscratch.txt
\ttests/fixtures/
";

const GIT_DIFF: &str = "\
diff --git a/src/executor.rs b/src/executor.rs
index a1b2c3d..e4f5a6b 100644
--- a/src/executor.rs
+++ b/src/executor.rs
@@ -1,6 +1,8 @@
 use std::io::{BufRead, BufReader};
 use std::process::{Command, Stdio};
 use std::time::Instant;
+use std::io::Write;
+use std::collections::HashMap;
 
 use crate::error::SiftError;
 
@@ -32,12 +34,18 @@ pub fn execute(program: &str, args: &[String]) -> Result<ExecutorOutput, SiftEr
     let output = Command::new(program).args(args).output().map_err(|e| {
         if e.kind() == std::io::ErrorKind::NotFound {
             SiftError::CommandNotFound(program.to_string())
+        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
+            SiftError::Io(e)
         } else {
             SiftError::Io(e)
         }
     })?;
+
+    let duration = start.elapsed();
     Ok(ExecutorOutput {
         stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
         stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
         exit_code: output.status.code().unwrap_or(1),
+        duration_ms: duration.as_millis() as u64,
     })
 }
diff --git a/src/main.rs b/src/main.rs
index b1c2d3e..f4a5b6c 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -29,7 +29,12 @@ fn run() -> Result<i32> {
     let cli = cli::Cli::parse();
     let cfg = config::load();
 
+    let cli_verbosity_override: Option<Verbosity> = if cli.raw {
+        Some(Verbosity::Raw)
+    } else if cli.verbose > 0 {
+        Some(cli.verbosity())
+    } else {
+        None
+    };
+
     match cli.command {
";

const GIT_LOG: &str = "\
commit a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0
Author: Alice Developer <alice@example.com>
Date:   Mon Apr 13 11:22:33 2026 +0200

    feat: add per-command config overrides

    Adds support for [commands.git] verbosity = \"verbose\" in config.toml.
    Priority chain: CLI > per-command > global > compact.

commit b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0a1
Author: Bob Engineer <bob@example.com>
Date:   Sun Apr 12 15:44:21 2026 +0200

    fix: cargo fmt fixes in config.rs tests

commit c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0a1b2
Author: Alice Developer <alice@example.com>
Date:   Sat Apr 11 09:30:00 2026 +0200

    chore: bump version to 0.6.0

commit d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0a1b2c3
Author: Alice Developer <alice@example.com>
Date:   Fri Apr 10 17:15:42 2026 +0200

    feat: shell completions for zsh, bash, fish

commit e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0a1b2c3d4
Author: Bob Engineer <bob@example.com>
Date:   Thu Apr 09 14:00:00 2026 +0200

    refactor: extract streaming executor module
";

const GREP: &str = "\
src/executor.rs:32:pub fn execute(program: &str, args: &[String]) -> Result<ExecutorOutput, SiftError> {
src/executor.rs:45:    let start = Instant::now();
src/executor.rs:68:pub fn execute_streaming(program: &str, args: &[String], handler: Box<dyn StreamHandler>) -> Result<ExecutorOutput, SiftError> {
src/executor.rs:72:    let child = Command::new(program).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
src/executor.rs:88:    let mut line = String::new();
src/executor.rs:102:    let duration = start.elapsed();
src/main.rs:147:            let filter_output = apply_filter(&args, &output.stdout, verbosity);
src/main.rs:160:                    eprintln!(\"[sift] filter produced empty output — raw saved to {}\", path.display());
src/main.rs:221:fn apply_filter(args: &[String], stdout: &str, verbosity: Verbosity) -> filters::FilterOutput {
src/main.rs:225:    if verbosity == Verbosity::Raw {
src/main.rs:232:        CommandFamily::Git(sub) => match sub {
src/main.rs:241:        CommandFamily::Grep => filters::grep::filter(stdout, verbosity),
src/main.rs:242:        CommandFamily::Read => filters::read::filter(stdout, verbosity),
src/main.rs:243:        CommandFamily::Ls => filters::ls_xcode::filter_ls(stdout, verbosity),
src/main.rs:244:        CommandFamily::Find => filters::ls_xcode::filter_find(stdout, verbosity),
src/main.rs:245:        CommandFamily::Curl => filters::curl::filter(stdout, verbosity),
src/filters/mod.rs:47:pub struct FilterOutput {
src/filters/mod.rs:60:impl FilterOutput {
src/filters/mod.rs:72:pub fn passthrough(raw: &str) -> Self {
src/filters/xcodebuild_build.rs:14:pub fn filter(stdout: &str, verbosity: Verbosity) -> FilterOutput {
src/filters/xcodebuild_build.rs:22:    let lines: Vec<&str> = stdout.lines().collect();
src/filters/xcodebuild_build.rs:45:    let errors: Vec<String> = lines.iter().filter(|l| l.contains(\"error:\")).map(|l| l.to_string()).collect();
src/filters/xcodebuild_test.rs:12:pub fn filter(stdout: &str, verbosity: Verbosity) -> FilterOutput {
src/filters/xcodebuild_test.rs:18:    let lines: Vec<&str> = stdout.lines().collect();
src/filters/git_diff.rs:10:pub fn filter(stdout: &str, verbosity: Verbosity) -> FilterOutput {
src/filters/git_status.rs:8:pub fn filter(stdout: &str, verbosity: Verbosity) -> FilterOutput {
src/filters/grep.rs:9:pub fn filter(stdout: &str, verbosity: Verbosity) -> FilterOutput {
src/filters/grep.rs:14:    let mut files: IndexMap<String, Vec<String>> = IndexMap::new();
src/filters/grep.rs:28:    for (file, matches) in &files {
src/lib.rs:3:pub mod cli;
src/lib.rs:4:pub mod commands;
src/lib.rs:5:pub mod completions;
src/lib.rs:6:pub mod config;
src/lib.rs:7:pub mod error;
src/lib.rs:8:pub mod executor;
src/lib.rs:9:pub mod filters;
";

const READ: &str = "\
import Foundation
import SwiftUI

// MARK: - PaymentService

/// Handles all payment-related API calls.
/// This service is responsible for initiating, confirming, and cancelling payments.
@MainActor
final class PaymentService: ObservableObject {
    @Published private(set) var isLoading = false
    @Published private(set) var lastError: Error?

    private let session: URLSession
    private let baseURL: URL

    init(session: URLSession = .shared, baseURL: URL) {
        self.session = session
        self.baseURL = baseURL
    }

    func initiatePayment(amount: Decimal, currency: String) async throws -> PaymentResult {
        isLoading = true
        defer { isLoading = false }

        var request = URLRequest(url: baseURL.appendingPathComponent(\"/payments\"))
        request.httpMethod = \"POST\"
        request.setValue(\"application/json\", forHTTPHeaderField: \"Content-Type\")

        let body = [\"amount\": amount, \"currency\": currency] as [String: Any]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse,
              httpResponse.statusCode == 200 else {
            throw PaymentError.invalidResponse
        }
        return try JSONDecoder().decode(PaymentResult.self, from: data)
    }
}
";

const XCODEBUILD_BUILD: &str = "\
Build settings from command line:
    SDKROOT = iphoneos

=== BUILD TARGET MyApp OF PROJECT MyApp WITH CONFIGURATION Debug ===

Check dependencies

CompileSwift normal arm64
    /Users/dev/MyApp/Sources/PaymentService.swift
/Users/dev/MyApp/Sources/PaymentService.swift:22:18: error: use of unresolved identifier 'PaymentResult'
/Users/dev/MyApp/Sources/PaymentService.swift:31:40: error: cannot convert value of type 'String' to expected argument type 'Amount'

CompileSwift normal arm64
    /Users/dev/MyApp/Sources/NetworkClient.swift
/Users/dev/MyApp/Sources/NetworkClient.swift:15:42: error: value of type 'URLSession' has no member 'dataTaskAsync'
/Users/dev/MyApp/Sources/NetworkClient.swift:44:8: warning: result of 'Task.init(priority:operation:)' is unused

CompileSwift normal arm64
    /Users/dev/MyApp/Sources/CartView.swift
/Users/dev/MyApp/Sources/CartView.swift:88:19: warning: 'init(_:)' is deprecated

** BUILD FAILED **

Build log written to /Users/dev/Library/Logs/Build/MyApp.log
";

const XCODEBUILD_TEST: &str = "\
Build settings from command line:
    SDKROOT = iphonesimulator

=== BUILD TARGET MyAppTests OF PROJECT MyApp WITH CONFIGURATION Debug ===

Test Suite 'All tests' started at 2026-04-13 11:00:00.000
Test Suite 'MyAppTests.xctest' started at 2026-04-13 11:00:00.001

Test Suite 'PaymentTests' started at 2026-04-13 11:00:00.002
Test Case '-[PaymentTests testCheckout]' started.
Test Case '-[PaymentTests testCheckout]' failed (0.234 seconds).
/Users/dev/MyApp/Tests/PaymentTests.swift:44: error: -[PaymentTests testCheckout] : XCTAssertEqual failed: (\"200\") is not equal to (\"404\")
Test Case '-[PaymentTests testCancellation]' started.
Test Case '-[PaymentTests testCancellation]' passed (0.012 seconds).
Test Case '-[PaymentTests testRefund]' started.
Test Case '-[PaymentTests testRefund]' passed (0.018 seconds).
Test Suite 'PaymentTests' failed at 2026-04-13 11:00:00.265.

Test Suite 'NetworkTests' started at 2026-04-13 11:00:00.266
Test Case '-[NetworkTests testTimeoutHandling]' started.
Test Case '-[NetworkTests testTimeoutHandling]' failed (1.001 seconds).
/Users/dev/MyApp/Tests/NetworkTests.swift:88: error: -[NetworkTests testTimeoutHandling] : XCTAssertEqual failed: (\"408\") is not equal to (\"200\")
Test Case '-[NetworkTests testRetryLogic]' started.
Test Case '-[NetworkTests testRetryLogic]' passed (0.050 seconds).
Test Suite 'NetworkTests' failed at 2026-04-13 11:00:01.317.

Test Suite 'All tests' failed at 2026-04-13 11:00:01.320.
     Executed 5 tests, with 2 failures (0 unexpected) in 1.315 seconds

** TEST FAILED **
";

const SWIFTLINT: &str = "\
Linting Swift files at paths Sources
Loading configuration from '.swiftlint.yml'
Sources/PaymentService.swift:12:5: warning: Line Length Violation: Line should be 120 characters or less; currently it is 143 characters (line_length)
Sources/PaymentService.swift:44:1: warning: Trailing Whitespace Violation: Lines should not have trailing whitespace (trailing_whitespace)
Sources/NetworkClient.swift:8:1: error: Force Cast Violation: Force casts should be avoided (force_cast)
Sources/NetworkClient.swift:67:22: warning: Identifier Name Violation: Variable name 'r' should be between 3 and 40 characters long (identifier_name)
Sources/CartView.swift:15:5: warning: Todo Violation: TODOs should be resolved in issue tracker (todo)
Sources/CartView.swift:102:9: warning: Function Body Length Violation: Function body should span 40 lines or less; currently spans 67 lines (function_body_length)
Sources/CheckoutView.swift:33:13: warning: Optional Binding Violation: Use 'guard let' to early exit from scope (optional_binding)
Done linting! Found 7 violations, 1 serious in 4 files.
";

const SWIFT_BUILD: &str = "\
Build complete!
warning: 'MyPackage': dependency 'swift-algorithms' is not used by any target
Compiling MyPackage Sources/MyPackage/Parser.swift
Sources/MyPackage/Parser.swift:22:18: error: use of unresolved identifier 'TokenKind'
Sources/MyPackage/Parser.swift:45:30: error: cannot convert value of type '[String]' to expected argument type 'TokenStream'
Compiling MyPackage Sources/MyPackage/Lexer.swift
Sources/MyPackage/Lexer.swift:88:8: warning: initialization of variable 'result' was never used
build error: could not build 'MyPackage' for 'release'
error: fatalError
";

const SWIFT_TEST: &str = "\
Test Suite 'All tests' started at 2026-04-13 12:00:00.000
Test Suite 'MyPackageTests.xctest' started at 2026-04-13 12:00:00.001
Test Suite 'ParserTests' started at 2026-04-13 12:00:00.002
Test Case 'testBasicParsing' passed (0.005 seconds).
Test Case 'testNestedBlocks' passed (0.003 seconds).
Test Case 'testErrorRecovery' failed (0.002 seconds).
/Sources/Tests/ParserTests.swift:55: XCTAssertEqual failed: (\"block\") is not equal to (\"expression\")
Test Case 'testEmptyInput' passed (0.001 seconds).
Test Suite 'ParserTests' failed at 2026-04-13 12:00:00.013.
Test Suite 'All tests' failed at 2026-04-13 12:00:00.014.
     Executed 4 tests, with 1 failure in 0.011 seconds
";

const SWIFT_PACKAGE: &str = "\
Fetching https://github.com/apple/swift-algorithms.git from cache
Fetching https://github.com/apple/swift-collections.git from cache
Fetching https://github.com/apple/swift-numerics.git from cache
Cloning https://github.com/apple/swift-algorithms.git
Version 1.2.0 of package swift-algorithms
Cloning https://github.com/apple/swift-collections.git
Version 1.1.2 of package swift-collections
Cloning https://github.com/apple/swift-numerics.git
Version 1.0.2 of package swift-numerics
Resolving https://github.com/apple/swift-algorithms.git at 1.2.0
Resolving https://github.com/apple/swift-collections.git at 1.1.2
Resolving https://github.com/apple/swift-numerics.git at 1.0.2
Build complete!
";

const FASTLANE: &str = "\
[13:00:01]: fastlane detected a Gemfile in the current directory
[13:00:01]: However, it seems like you don't use `bundle exec`
[13:00:02]: Get started using a Gemfile for fastlane https://docs.fastlane.tools/getting-started/ios/setup/
[13:00:02]: ------------------------------
[13:00:02]: --- Step: default_platform ---
[13:00:02]: ------------------------------
[13:00:03]: Driving the lane 'beta' 🚀
[13:00:03]: -------------------------
[13:00:03]: --- Step: increment_build_number ---
[13:00:03]: -------------------------
[13:00:03]: Successfully incremented CFBundleVersion to 42
[13:00:05]: -------------------------
[13:00:05]: --- Step: gym ---
[13:00:05]: -------------------------
[13:00:05]: Generated .app file: /Users/dev/build/MyApp.app
[13:00:05]: Generated .ipa file: /Users/dev/build/MyApp.ipa
[13:00:06]: -------------------------
[13:00:06]: --- Step: pilot ---
[13:00:06]: -------------------------
[13:00:06]: Successfully uploaded package to App Store Connect.
[13:00:07]: Build successfully uploaded to TestFlight! ✅
[13:00:07]: Successfully distributed build 42 to External Testers
[13:00:07]: -------------------------------------------------------
[13:00:07]: fastlane.tools finished successfully 🎉
[13:00:07]: Total time: 0:01:04.312
";

const XCRUN_SIMCTL: &str = "\
== Devices ==
-- iOS 18.0 --
    iPhone 16 (11111111-1111-1111-1111-111111111111) (Shutdown)
    iPhone 16 Pro (22222222-2222-2222-2222-222222222222) (Booted)
    iPhone 16 Pro Max (33333333-3333-3333-3333-333333333333) (Shutdown)
    iPhone SE (3rd generation) (44444444-4444-4444-4444-444444444444) (Shutdown)
    iPad Pro 13-inch (M4) (55555555-5555-5555-5555-555555555555) (Shutdown)
-- iOS 17.5 --
    iPhone 15 (66666666-6666-6666-6666-666666666666) (Shutdown)
    iPhone 15 Pro (77777777-7777-7777-7777-777777777777) (Shutdown)
-- watchOS 11.0 --
    Apple Watch Series 10 (44mm) (88888888-8888-8888-8888-888888888888) (Shutdown)
-- tvOS 18.0 --
    Apple TV 4K (3rd generation) (99999999-9999-9999-9999-999999999999) (Shutdown)
-- visionOS 2.0 --
    Apple Vision Pro (aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa) (Shutdown)
";

const CURL: &str = "\
*   Trying 93.184.216.34:443...
* Connected to example.com (93.184.216.34) port 443 (#0)
* ALPN, offering h2
* ALPN, offering http/1.1
* successfully set certificate verify locations
* SSL connection using TLSv1.3 / TLS_AES_256_GCM_SHA384
* Server certificate:
*  subject: C=US; O=Example Corp; CN=example.com
*  expire date: Dec 31 2026 00:00:00 GMT
> GET /api/v1/users HTTP/2
> Host: example.com
> User-Agent: curl/7.79.1
> Accept: */*
>
< HTTP/2 200
< content-type: application/json
< content-length: 142
<
{\"users\":[{\"id\":1,\"name\":\"Alice\"},{\"id\":2,\"name\":\"Bob\"}],\"total\":2,\"page\":1,\"per_page\":20}
* Connection #0 to host example.com left intact
";

const AGVTOOL: &str = "\
Getting the build version...
Current version of project MyApp is:
42
";

const CODESIGN: &str = "\
MyApp.app: code object is not signed at all
In architecture: x86_64
In architecture: arm64
MyApp.app/Frameworks/MyFramework.framework: valid on disk
MyApp.app/Frameworks/MyFramework.framework: satisfies its Designated Requirement
MyApp.app: valid on disk
MyApp.app: satisfies its Designated Requirement
";

const SWIFTFORMAT: &str = "\
Running SwiftFormat...
/Users/dev/Sources/PaymentService.swift
/Users/dev/Sources/NetworkClient.swift
/Users/dev/Sources/CartView.swift
SwiftFormat completed. 3 files formatted, 1 file skipped.
";

const CRASHLOG: &str = "\
Incident Identifier: 12AB34CD-FAKE-0000-0000-000000000000
Hardware Model:      iPhone15,2
Process:             MyApp [1234]
Path:                /private/var/containers/Bundle/Application/ABCD/MyApp.app/MyApp
Identifier:          com.example.myapp
Version:             2.1.0 (47)
AppStoreTools:       14A309
Code Type:           ARM-64 (Native)
Role:                Foreground
Parent Process:      launchd [1]
Coalition:           com.example.myapp [1234]
Date/Time:           2024-01-15 12:34:56.789 +0000
Launch Time:         2024-01-15 12:30:00.000 +0000
OS Version:          iPhone OS 17.2 (21C62)
Release Type:        User
Baseband Version:    2.50.00
Report Version:      104

Exception Type:  EXC_BAD_ACCESS (SIGSEGV)
Exception Subtype: KERN_INVALID_ADDRESS at 0x0000000000000010
VM Region Info: 0x10 is not in any region.  Bytes before following region: 4367638528
      REGION TYPE                    START - END         [ VSIZE] PRT/MAX SHRMOD  REGION DETAIL
      UNUSED SPACE AT START
--->
      __TEXT                      104000000-104800000    [ 8192K] r-x/r-x SM=COW  ...MyApp

Termination Reason: SIGNAL 11 Segmentation fault: 11
Terminating Process: exc handler [1234]

Triggered by Thread:  0

Thread 0 name:  Dispatch queue: com.apple.main-thread
Thread 0 Crashed:
0   MyApp                    0x00000001045abc12 CheckoutViewModel.checkout(with:) + 48
1   MyApp                    0x00000001045abc45 closure #1 in CheckoutViewModel.loadCart() + 124
2   libswiftCore.dylib        0x0000000180000000 swift_task_run + 92
3   libdispatch.dylib         0x0000000181000000 _dispatch_main_queue_callback_4CF + 44
4   CoreFoundation             0x0000000182000000 __CFRunLoopRun + 1996
5   CoreFoundation             0x0000000182000100 CFRunLoopRunSpecific + 608
6   UIKit                      0x0000000183000000 UIApplicationMain + 800
7   MyApp                      0x00000001045abc99 main + 56

Thread 1:
0   libsystem_kernel.dylib   0x0000000182000000 mach_msg_trap + 8
1   libsystem_kernel.dylib   0x0000000182000010 mach_msg + 72
2   CoreFoundation             0x0000000182100000 __CFRunLoopServiceMachPort + 372

Thread 2:
0   libsystem_pthread.dylib  0x0000000182200000 start_wqthread + 8

Binary Images:
0x100000000 - 0x1047fffff MyApp arm64 <AAAAAAAABBBBCCCCDDDDEEEEFFFFGGGG> /private/var/containers/Bundle/Application/ABCD/MyApp.app/MyApp
0x180000000 - 0x18001ffff libswiftCore.dylib arm64 <1111222233334444> /usr/lib/swift/libswiftCore.dylib
0x181000000 - 0x1811fffff libdispatch.dylib arm64 <5555666677778888> /usr/lib/libdispatch.dylib
0x182000000 - 0x1829fffff CoreFoundation arm64 <9999AAAABBBBCCCC> /System/Library/Frameworks/CoreFoundation.framework/CoreFoundation
0x183000000 - 0x1839fffff UIKit arm64 <DDDDEEEEFFFFGGGG> /System/Library/Frameworks/UIKit.framework/UIKit
0x184000000 - 0x1849fffff Foundation arm64 <HHHHIIIIJJJJKKKK> /System/Library/Frameworks/Foundation.framework/Foundation
";

const PERIPHERY: &str = "\
Unused code detected — 12 occurrences:

src/PaymentService.swift:42: warning: Function 'validateCard(_:)' is unused
src/PaymentService.swift:67: warning: Function 'legacyFormatAmount(_:)' is unused
src/PaymentService.swift:103: warning: Variable 'debugLog' is unused
src/NetworkClient.swift:15: warning: Class 'MockURLSession' is unused
src/NetworkClient.swift:88: warning: Function 'retryRequest(_:attempts:)' is unused
src/CartView.swift:22: warning: Protocol 'CartDelegate' is unused
src/CartView.swift:55: warning: Function 'configureAccessibility()' is unused
src/CartView.swift:78: warning: Variable 'animationDuration' is unused
src/CheckoutViewModel.swift:31: warning: Function 'resetInternalState()' is unused
src/CheckoutViewModel.swift:95: warning: Variable 'analyticsBuffer' is unused
src/Models/Order.swift:14: warning: Function 'toDictionary()' is unused
src/Models/Order.swift:47: warning: Typealias 'OrderID' is unused
";

/// One row of benchmark output.
pub struct BenchmarkResult {
    pub label: String,
    pub original_bytes: usize,
    pub filtered_bytes: usize,
}

impl BenchmarkResult {
    pub fn savings_bytes(&self) -> usize {
        self.original_bytes.saturating_sub(self.filtered_bytes)
    }

    pub fn savings_percent(&self) -> f64 {
        if self.original_bytes == 0 {
            return 0.0;
        }
        self.savings_bytes() as f64 / self.original_bytes as f64 * 100.0
    }
}

/// Run all built-in fixtures through their filters and return results.
pub fn run_all() -> Vec<BenchmarkResult> {
    type FilterFn = Box<dyn Fn(&str) -> filters::FilterOutput>;
    let fixtures: Vec<(&str, FilterFn)> = vec![
        (
            "git status",
            Box::new(|s| git_status::filter(s, Verbosity::Compact)),
        ),
        (
            "git diff",
            Box::new(|s| git_diff::filter(s, Verbosity::Compact)),
        ),
        (
            "git log",
            Box::new(|s| git_log::filter(s, Verbosity::Compact)),
        ),
        (
            "grep / rg",
            Box::new(|s| grep::filter(s, Verbosity::Compact)),
        ),
        (
            "cat / read",
            Box::new(|s| read::filter(s, Verbosity::Compact)),
        ),
        (
            "xcodebuild build",
            Box::new(|s| xcodebuild_build::filter(s, Verbosity::Compact)),
        ),
        (
            "xcodebuild test",
            Box::new(|s| xcodebuild_test::filter(s, Verbosity::Compact)),
        ),
        (
            "swiftlint",
            Box::new(|s| swiftlint::filter(s, Verbosity::Compact)),
        ),
        (
            "swift build",
            Box::new(|s| swift_build::filter(s, Verbosity::Compact)),
        ),
        (
            "swift test",
            Box::new(|s| swift_test::filter(s, Verbosity::Compact)),
        ),
        (
            "swift package",
            Box::new(|s| swift_package::filter(s, Verbosity::Compact)),
        ),
        (
            "fastlane",
            Box::new(|s| fastlane::filter(s, Verbosity::Compact)),
        ),
        (
            "xcrun simctl",
            Box::new(|s| xcrun_simctl::filter(s, Verbosity::Compact)),
        ),
        ("curl", Box::new(|s| curl::filter(s, Verbosity::Compact))),
        (
            "agvtool",
            Box::new(|s| agvtool::filter(s, Verbosity::Compact)),
        ),
        (
            "codesign",
            Box::new(|s| codesign::filter(s, Verbosity::Compact)),
        ),
        (
            "swiftformat",
            Box::new(|s| swiftformat::filter(s, Verbosity::Compact)),
        ),
        (
            "crashlog",
            Box::new(|s| crashlog::filter(s, Verbosity::Compact)),
        ),
        (
            "periphery",
            Box::new(|s| periphery::filter(s, Verbosity::Compact)),
        ),
    ];

    let inputs: &[&str] = &[
        GIT_STATUS,
        GIT_LOG,
        GIT_DIFF,
        GREP,
        READ,
        XCODEBUILD_BUILD,
        XCODEBUILD_TEST,
        SWIFTLINT,
        SWIFT_BUILD,
        SWIFT_TEST,
        SWIFT_PACKAGE,
        FASTLANE,
        XCRUN_SIMCTL,
        CURL,
        AGVTOOL,
        CODESIGN,
        SWIFTFORMAT,
        CRASHLOG,
        PERIPHERY,
    ];

    fixtures
        .into_iter()
        .zip(inputs.iter())
        .map(|((label, filter_fn), input)| {
            let out = filter_fn(input);
            BenchmarkResult {
                label: label.to_string(),
                original_bytes: input.len(),
                filtered_bytes: out.filtered_bytes,
            }
        })
        .collect()
}

/// Format a byte count as a human-readable string (B / KB).
fn fmt_bytes(n: usize) -> String {
    if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

/// Print the benchmark table to stdout.
pub fn print_results(results: &[BenchmarkResult]) {
    let col = 22usize;
    println!("Sift Benchmark — filter reduction at Compact verbosity");
    println!("{}", "─".repeat(62));
    println!(
        "  {:<col$} {:>8}  {:>8}  {:>7}",
        "Command", "Input", "Output", "Saved"
    );
    println!("{}", "─".repeat(62));

    let mut total_in = 0usize;
    let mut total_out = 0usize;

    for r in results {
        total_in += r.original_bytes;
        total_out += r.filtered_bytes;
        println!(
            "  {:<col$} {:>8}  {:>8}  {:>6.1}%",
            r.label,
            fmt_bytes(r.original_bytes),
            fmt_bytes(r.filtered_bytes),
            r.savings_percent(),
        );
    }

    let total_saved = total_in.saturating_sub(total_out);
    let total_pct = if total_in > 0 {
        total_saved as f64 / total_in as f64 * 100.0
    } else {
        0.0
    };

    println!("{}", "─".repeat(62));
    println!(
        "  {:<col$} {:>8}  {:>8}  {:>6.1}%",
        "Total / Average",
        fmt_bytes(total_in),
        fmt_bytes(total_out),
        total_pct,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fixtures_run_without_panic() {
        let results = run_all();
        assert!(!results.is_empty());
    }

    #[test]
    fn all_filters_produce_non_empty_output_for_non_empty_input() {
        for r in run_all() {
            assert!(
                r.original_bytes > 0,
                "{} fixture has zero-length input",
                r.label
            );
        }
    }

    #[test]
    fn overall_reduction_is_meaningful() {
        let results = run_all();
        let total_in: usize = results.iter().map(|r| r.original_bytes).sum();
        let total_out: usize = results.iter().map(|r| r.filtered_bytes).sum();
        let pct = (total_in.saturating_sub(total_out)) as f64 / total_in as f64 * 100.0;
        assert!(
            pct > 20.0,
            "expected >20% overall reduction across fixtures, got {pct:.1}%"
        );
    }

    #[test]
    fn fmt_bytes_formats_correctly() {
        assert_eq!(fmt_bytes(500), "500 B");
        assert_eq!(fmt_bytes(1024), "1.0 KB");
        assert_eq!(fmt_bytes(2048), "2.0 KB");
    }
}
