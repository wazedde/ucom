# Unity Commander

`ucom` is a command line interface for Unity projects, written in Rust. It serves as an alternative to Unity Hub with
additional features like building projects and running tests. Inspired by cargo and dotnet CLI tools.

## Core Features

- **Building Projects**: Quick commands for building projects
    - `ucom build ios`     - Build an iOS version
    - `ucom build android` - Build an Android version
    - `ucom build webgl`   - Build a WebGL version

- **Testing**: Integrated testing support
    - `ucom test editmode` - Run EditMode tests
    - `ucom test playmode` - Run PlayMode tests
    - `ucom test android`  - Run tests on Android platform

- **Project Management**:
    - `ucom new -u 2022.3 ~/Projects/MyGame --add-builder-menu` - Create a new project with builder scripts
    - `ucom open -t ios` - Open project with iOS build target

- **Managing Unity Versions**:
    - `ucom list`         - View all installed Unity versions
    - `ucom list updates` - Check for available updates
    - `ucom install`      - Install a specific Unity version

## Command Examples

| Command              | Alias             | Description                                 |
|----------------------|-------------------|---------------------------------------------|
| `ucom open`          | `ucom o`          | Open the Unity project in current directory |
| `ucom build ios`     | `ucom b ios`      | Build iOS version in batch mode             |
| `ucom test editmode` | `ucom t editmode` | Run EditMode tests                          |
| `ucom list`          | `ucom ls`         | List all installed Unity versions           |
| `ucom info`          | `ucom i`          | Show project information                    |
| `ucom updates`       | `ucom u`          | Check project for Unity updates             |
| `ucom add builder`   |                   | Add build helper script to project          |

### Advanced Examples

- `ucom build android ~/Develop/MyProject --mode editor-quit` - Build Android for specific project and close editor
- `ucom test playmode --categories "!Slow;UI"` - Run playmode tests, excluding slow tests but including UI tests
- `ucom new ~/Develop/MyProject -u 2021.3.1 --lfs` - Create new project with Git LFS support
- `ucom open ~/Develop/MyProject -U=2021.3` - Open and upgrade project to latest Unity version in that range
- `ucom info ~/Develop/MyProject --install-required` - Install Unity version if not present
- `ucom updates ~/Develop/MyProject --install-latest` - Install latest Unity version if not present
- `ucom run -u 2022.3 -- -createProject ~/path/to/project -quit` - Run Unity with custom arguments

## Installation

1. Requires [Rust](https://www.rust-lang.org) v1.85.0+
2. Run: `cargo install --git https://github.com/jakkovanhunen/ucom`

### Manual Build

```bash
# Clone repository
git clone https://github.com/jakkovanhunen/ucom
# Then run:
cd ucom
cargo build --release
# Executable will be in target/release directory
```

## Build Script and Templates

The `build` command injects a [script](https://gist.github.com/jakkovanhunen/b56a70509616b6ff3492a17ae670a5e7) to handle
platform builds. Use to retain this script after building. `--inject persistent`

View and add helper scripts with the `add` command:

- `ucom add builder --display-content` - View the build script
- `ucom add gitignore` - Add Unity-specific .gitignore file
- `ucom add gitattributes` - Add a Unity-specific .gitattributes file for Git LFS
- `ucom add builder-menu` - Add Editor a menu script for building

## Test Support

The `test` command supports running tests in various modes:

- `ucom test editmode` - Run tests in EditMode within the Editor
- `ucom test playmode` - Run tests in PlayMode within the Editor
- `ucom test ios` - Run tests on iOS platform
- `ucom test android` - Run tests on Android platform

Filter tests with:

- Filter by test categories `--categories`
- Filter by test names `--tests`
- Filter by assembly names `--assemblies`

## Environment Variables

The following environment variables can be used to set default values for commands:

| Variable             | Purpose                               |
|----------------------|---------------------------------------|
| `UCOM_EDITOR_DIR`    | Path to editor installation directory |
| `UCOM_BUILD_TARGET`  | Default build target                  |
| `UCOM_PACKAGE_LEVEL` | Default package info detail level     |

## Limitations

- macOS and Windows only
- Requires default editor location (or set environment variable) `UCOM_EDITOR_DIR`
- Git required for `new` command
- Cannot build projects that are already open
- iOS builds don't build the exported Xcode project

## `ucom help`

```
Unity Commander: A command-line interface for managing Unity projects

Usage: ucom [OPTIONS] [COMMAND]

Commands:
  list     List installed or available Unity versions [aliases: ls]
  install  Install a specific Unity version
  info     Display information about a Unity project [aliases: i]
  updates  Check for newer available Unity versions suitable for a project [aliases: u]
  new      Create a new Unity project, optionally initializing a Git repository
  open     Open a Unity project in the editor [aliases: o]
  build    Build a Unity project for a specified target platform [aliases: b]
  test     Run tests within a Unity project [aliases: t]
  run      Run the Unity editor with custom command-line arguments [aliases: r]
  add      Add a helper script or configuration file to the project
  cache    Manage the download cache for Unity release data
  help     Print this message or the help of the given subcommand(s)

Options:
  -n, --no-color  Suppress colored output in the terminal
  -h, --help      Print help
  -V, --version   Print version
```

## `ucom help list`

```
List installed or available Unity versions

Usage: ucom list [OPTIONS] [LIST_TYPE]

Arguments:
  [LIST_TYPE]
          Specify the type of versions to list

          [default: installed]

          Possible values:
          - installed: List Unity versions currently installed on the system
          - updates:   List installed versions and check for available updates for each
          - latest:    List the latest available version for each major/minor release series
          - all:       List all known available Unity versions from the release data

Options:
  -u, --unity <VERSION>
          Filter versions by a prefix (e.g., '2021', '2022.3')

  -f, --force
          Force download of release data from the Unity API, bypassing cache

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help info`

```
Display information about a Unity project

Usage: ucom info [OPTIONS] [DIRECTORY]

Arguments:
  [DIRECTORY]
          Path to the Unity project directory. Defaults to the current directory

          [default: .]

Options:
      --install-required
          Install the project's required Unity version if it's not already installed

  -R, --recursive
          Recursively search directories for the Unity project

  -p, --packages <PACKAGES>
          Set the level of detail for displaying package information

          [env: UCOM_PACKAGE_LEVEL=]
          [default: no-unity]

          Possible values:
          - none:      Do not display any package information
          - no-unity:  Display information only for non-Unity packages (e.g., custom, third-party)
          - inc-unity: Display information for non-Unity and Unity registry packages
          - all:       Display information for all packages, including built-in and dependencies

  -r, --report
          Generate a Markdown report with available updates

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help updates`

```
Check for newer available Unity versions suitable for a project

Usage: ucom updates [OPTIONS] [DIRECTORY]

Arguments:
  [DIRECTORY]  Path to the Unity project directory. Defaults to the current directory [default: .]

Options:
      --install-latest  Install the latest suitable Unity version if it's not already installed
  -r, --report          Generate a Markdown report of applicable release notes
  -h, --help            Print help
```

## `ucom help new`

```
Create a new Unity project, optionally initializing a Git repository

Usage: ucom new [OPTIONS] --unity <VERSION> <DIRECTORY> [-- <UNITY_ARGS>...]

Arguments:
  <DIRECTORY>
          Path and name for the new project directory. This directory must not exist yet. Required

  [UNITY_ARGS]...
          Additional arguments to pass directly to the Unity editor executable during project creation

Options:
  -u, --unity <VERSION>
          Specify the Unity version to use for the new project. Accepts a full version (e.g., '2022.3.5f1') or a prefix
          (e.g., '2021', '2022.3'). A prefix will select the latest installed version matching that prefix. Required

  -t, --target <NAME>
          Set the initial active build target for the new project (e.g., win64, android)

          Possible values:
          - standalone: Generic Standalone target (platform determined by editor context)
          - win32:      Standalone Windows 32-bit
          - win64:      Standalone Windows 64-bit
          - macos:      Standalone macOS (Universal Binary)
          - linux64:    Standalone Linux 64-bit
          - ios:        Apple iOS
          - android:    Google Android
          - webgl:      WebGL
          - winstore:   Universal Windows Platform
          - tvos:       Apple tvOS

      --add-builder-menu
          Add helper scripts ('UnityBuilder.cs', 'EditorMenu.cs') for building via ucom or the Editor menu.

          Files are placed in 'Assets/Plugins/Ucom/Editor/'.

      --lfs
          Initialize a Git repository, configure Git LFS, and add a Unity-specific '.gitattributes' file

      --no-git
          Skip initializing a Git repository for the new project

  -w, --wait
          Wait for the initial Unity editor process (used for project creation) to exit before returning

  -Q, --quit
          Automatically close the Unity editor immediately after the project creation process finishes

  -q, --quiet
          Suppress informational messages from ucom during project creation

  -n, --dry-run
          Show the command that would be executed without actually running it

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help open`

```
Open a Unity project in the editor

Usage: ucom open [OPTIONS] [DIRECTORY] [-- <UNITY_ARGS>...]

Arguments:
  [DIRECTORY]
          Path to the Unity project directory. Defaults to the current directory

          [default: .]

  [UNITY_ARGS]...
          Additional arguments to pass directly to the Unity editor executable

Options:
  -U, --upgrade[=<VERSION>]
          Upgrade the project to a newer Unity version before opening. If no version is specified, uses the latest
          installed version matching the project's `major.minor`. A version prefix (e.g., '2021') selects the latest
          installed version in that release series

  -t, --target <NAME>
          Set the active build target

          Possible values:
          - standalone: Generic Standalone target (platform determined by editor context)
          - win32:      Standalone Windows 32-bit
          - win64:      Standalone Windows 64-bit
          - macos:      Standalone macOS (Universal Binary)
          - linux64:    Standalone Linux 64-bit
          - ios:        Apple iOS
          - android:    Google Android
          - webgl:      WebGL
          - winstore:   Universal Windows Platform
          - tvos:       Apple tvOS

  -w, --wait
          Wait for the Unity editor process to exit before the command returns

  -Q, --quit
          Automatically close the Unity editor after the project load completes

  -q, --quiet
          Suppress informational messages from ucom before launching Unity

  -n, --dry-run
          Show the command that would be executed without actually running it

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help build`

```
Build a Unity project for a specified target platform

Usage: ucom build [OPTIONS] <TARGET> [DIRECTORY] [-- <UNITY_ARGS>...]

Arguments:
  <TARGET>
          Target platform to build the project for (e.g., win64, android, webgl). Required

          [env: UCOM_BUILD_TARGET=]

          Possible values:
          - win32:   Build for Windows 32-bit
          - win64:   Build for Windows 64-bit
          - macos:   Build for macOS (Universal Binary)
          - linux64: Build for Linux 64-bit
          - ios:     Build for Apple iOS (generates an Xcode project)
          - android: Build for Google Android (generates APK/AAB or Gradle project)
          - webgl:   Build for WebGL

  [DIRECTORY]
          Path to the Unity project directory. Defaults to the current directory

          [default: .]

  [UNITY_ARGS]...
          Additional arguments to pass directly to the Unity editor executable during the build

Options:
  -o, --output <DIRECTORY>
          Specify the exact output directory for the build artifacts. If not set, defaults to
          '<PROJECT_DIR>/Builds/<TYPE>/<TARGET>'

  -t, --type <TYPE>
          Subdirectory name ('release' or 'debug') used within the default output path structure.

          Ignored if --output is specified.

          [default: release]

          Possible values:
          - release: Use 'Release' as the subdirectory name in the default output path
          - debug:   Use 'Debug' as the subdirectory name in the default output path

  -r, --run
          Automatically run the built player after a successful build.

          Shortcut for '--build-options AutoRunPlayer'.

  -d, --development
          Create a development build (enables debugging symbols, profiler connection).

          Shortcut for '--build-options Development'.

  -S, --show
          Show the output folder in the file explorer after a successful build (Windows/macOS).

          Shortcut for '--build-options ShowBuiltPlayer'.

  -D, --debugging
          Allow the built player to be debugged remotely.

          Shortcut for '--build-options AllowDebugging'.

  -p, --profiling
          Build the player with profiling enabled and automatically connect to the editor.

          Shortcut for '--build-options ConnectWithProfiler'.

  -P, --deep-profiling
          Enable deep profiling support in the player (requires 'profiling' to be useful).

          Shortcut for '--build-options EnableDeepProfilingSupport'.

  -H, --connect-host
          Make the player attempt to connect back to the Editor instance that built it.

          Shortcut for '--build-options ConnectToHost'.

  -O, --build-options [<OPTION>...]
          Set specific Unity BuildOptions flags (space-separated, e.g., -O Development AllowDebugging).

          Use 'None' explicitly if needed, though it's the default if no flags are set.

          [default: none]

          Possible values:
          - none:                                    Perform a default build with no special options
          - development:                             Build a development version with debug symbols and profiler
            capabilities
          - auto-run-player:                         Automatically run the built player after the build finishes
          - show-built-player:                       Reveal the built player in the OS file explorer (Windows/macOS)
          - build-additional-streamed-scenes:        Include non-main scenes marked in Build Settings as streamed scenes
            in AssetBundles. (Less common)
          - accept-external-modifications-to-player: Allow patching of the generated Xcode (iOS) or Gradle (Android)
            project
          - clean-build-cache:                       Force a clean build, discarding any cached build data
          - connect-with-profiler:                   Enable the profiler and auto-connect the player to the Editor
          - allow-debugging:                         Allow script debugging connections to the built player
          - symlink-sources:                         Create symlinks for script files instead of copying them (Platform
            dependent)
          - uncompressed-asset-bundle:               Build asset bundles without compression
          - connect-to-host:                         Make the player attempt to connect back to the Editor that
            initiated the build
          - custom-connection-id:                    Use a custom connection ID for player-editor communication.
            (Advanced)
          - build-scripts-only:                      Compile scripts only, do not build player data
          - patch-package:                           Create a patch package for Android development builds
          - compress-with-lz4:                       Use LZ4 compression for player data (default for many platforms)
          - compress-with-lz4-hc:                    Use LZ4 high-compression for player data (slower build, potentially
            smaller size)
          - strict-mode:                             Treat any build errors as fatal, failing the build immediately
          - include-test-assemblies:                 Include assemblies marked for testing in the build
          - no-unique-identifier:                    Use a fixed, zero GUID for the build (internal use)
          - wait-for-player-connection:              Make the player wait for a debugger/profiler connection on startup
          - enable-code-coverage:                    Enable code coverage data collection in the build
          - enable-deep-profiling-support:           Enable deep profiling support in the player (increases overhead)
          - detailed-build-report:                   Generate a detailed report about the build process and assets
            included
          - shader-livelink-support:                 Enable shader live-link support for faster shader iteration
            (requires editor connection)

  -a, --build-args <STRING>
          Custom argument string passed to methods marked with the [UcomPreProcessBuild] attribute.

          Useful for passing version numbers or configuration flags into the build script. Requires the ucom build
          script ('UnityBuilder.cs' or similar) to be present.

  -C, --clean
          Clean the output directory by removing files not generated by the current build

  -i, --inject <METHOD>
          Control how the required build script (UnityBuilder.cs) is handled

          [default: auto]

          Possible values:
          - auto:       Automatically inject the build script if missing, remove it afterward (default)
          - persistent: Inject the build script if missing, leave it in the project permanently
          - off:        Do not inject; fail if the specified build function doesn't exist

  -m, --mode <MODE>
          Specify the execution mode for the Unity build process

          [default: batch]

          Possible values:
          - batch:       Run Unity in batch mode (no UI, exits after build). Recommended for automation
          - batch-nogfx: Run Unity in batch mode without initializing graphics. Faster for server builds
          - editor-quit: Open the Unity Editor normally, perform the build, then quit
          - editor:      Open the Unity Editor normally, perform the build, and keep the editor open

  -f, --build-function <FUNCTION>
          The static C# function to execute for building (e.g., 'MyNamespace.MyBuilder.Build').

          Defaults to the function provided by the injected ucom build script.

          [default: Ucom.UnityBuilder.Build]

  -l, --log-file <FILE>
          Redirect Unity's build log output to a specific file path.

          Defaults to a file inside the '<PROJECT_DIR>/Logs' directory.

  -q, --quiet
          Suppress Unity build log output from appearing in the terminal (stdout/stderr)

  -n, --dry-run
          Show the command that would be executed without actually running it

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help test`

```
Run tests within a Unity project

Usage: ucom test [OPTIONS] <PLATFORM> [DIRECTORY] [-- <UNITY_ARGS>...]

Arguments:
  <PLATFORM>
          The primary mode or platform on which to run tests. Required. This automatically determines the default build
          target for the test run (e.g., 'editmode' uses 'Standalone', 'macos' uses 'OSXUniversal'). Use '--target' to
          override the default build target

          Possible values:
          - editmode: Run tests directly within the Unity Editor environment
          - playmode: Run tests in Play Mode within the Unity Editor environment
          - macos:    Run tests in a standalone player build for macOS
          - win32:    Run tests in a standalone player build for Windows 32-bit
          - win64:    Run tests in a standalone player build for Windows 64-bit
          - linux64:  Run tests in a standalone player build for Linux 64-bit
          - ios:      Run tests on an iOS device or simulator (requires additional setup)
          - android:  Run tests on an Android device or emulator (requires additional setup)
          - webgl:    Run tests in a WebGL player build

  [DIRECTORY]
          Path to the Unity project directory containing the tests. Defaults to the current directory

          [default: .]

  [UNITY_ARGS]...
          Additional arguments to pass directly to the Unity editor executable running the tests

Options:
  -t, --target <NAME>
          Override the active build target for the Unity process running the tests.

          Default is inferred from the selected test platform (e.g., Standalone for editmode/playmode). Useful for
          scenarios like running EditMode tests while the project's active target is set to iOS.

          Possible values:
          - standalone: Generic Standalone target (platform determined by editor context)
          - win32:      Standalone Windows 32-bit
          - win64:      Standalone Windows 64-bit
          - macos:      Standalone macOS (Universal Binary)
          - linux64:    Standalone Linux 64-bit
          - ios:        Apple iOS
          - android:    Google Android
          - webgl:      WebGL
          - winstore:   Universal Windows Platform
          - tvos:       Apple tvOS

  -r, --show-results <RESULTS>
          Control the level of detail for displaying test results in the console

          [default: all]

          Possible values:
          - all:    Display a full summary of test results, including counts and logs for failures (default)
          - errors: Display only information about failed tests
          - none:   Suppress the display of test result summaries in the console output

      --no-batch-mode
          Run tests with the Unity Editor's graphics device enabled (not in batch mode).

          Disabling batch mode may be necessary for tests requiring graphics but allows UI popups which can interrupt
          automated runs.

      --forget-project-path
          Prevent the project path from being added to the Unity Hub/Launcher's history

      --categories <LIST>
          Filter tests to run based on categories assigned using the [Category] attribute.

          Provide a semicolon-separated list within quotes (e.g., "Integration;UI"). Use '!' prefix to exclude a
          category (e.g., "!Slow"). Combines with other filters.

      --tests <LIST>
          Filter tests to run by their full names or name patterns (regex supported).

          Provide a semicolon-separated list within quotes (e.g., "MyNamespace.MyTestClass.MyTestMethod"). Use '!'
          prefix to exclude a test (e.g., "!FailingTest"). Format for parameterized tests:
          'ClassName\.MethodName\(Param1,Param2\)'. Combines with other filters.

      --assemblies <LIST>
          Filter tests to run based on the assembly they belong to.

          Provide a semicolon-separated list of assembly names within quotes (e.g., "MyTests.dll;AnotherAssembly").
          Combines with other filters.

  -q, --quiet
          Suppress informational messages from ucom during the test execution setup

  -n, --dry-run
          Show the command that would be used to run Unity tests without actually executing it

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help add`

```
Add a helper script or configuration file to the project

Usage: ucom add [OPTIONS] <TEMPLATE> [DIRECTORY]

Arguments:
  <TEMPLATE>
          Select the helper script or configuration file template to add

          Possible values:
          - builder:        Adds 'UnityBuilder.cs', a C# script for automating builds via the command line
          - builder-menu:   Adds 'EditorMenu.cs', which includes the 'Builder' functionality and adds build commands to
            the Unity Editor menu
          - git-ignore:     Adds a standard '.gitignore' file tailored for Unity projects
          - git-attributes: Adds a standard '.gitattributes' file tailored for Unity projects, often used with Git LFS

  [DIRECTORY]
          Path to the Unity project directory where the file should be added. Defaults to the current directory

          [default: .]

Options:
  -f, --force
          Overwrite the target file if it already exists in the project directory

  -c, --display-content
          Print the content of the selected template file to standard output instead of writing it to the project

  -u, --display-url
          Print the source URL of the selected template file to standard output instead of writing it to the project

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help cache`

```
Manage the download cache for Unity release data.

By default, cached files expire after one hour. The system will automatically re-download required files after this
timeout.

Control caching behavior with the `UCOM_ENABLE_CACHE` environment variable. Set it to 'false' to disable caching and
always download fresh data.

Usage: ucom cache <ACTION>

Arguments:
  <ACTION>
          Action to perform on the cache

          Possible values:
          - clear: Remove all cached download files
          - list:  Display a list of currently cached files

Options:
  -h, --help
          Print help (see a summary with '-h')
```
