# Unity Commander

Command-line interface for Unity projects, written in Rust.

## What It Is

`ucom` provides command-line access to Unity project operations that would typically require Unity Hub or manual editor
interaction. It handles project creation, builds, tests, and Unity version management from the terminal.

## Why Use It

- **Automation** - Script builds and tests for CI/CD pipelines without GUI interaction
- **Speed** - Build via open editor using IPC instead of waiting for batch mode startup
- **Developer workflow** - Cargo/dotnet-style CLI for Unity developers who prefer terminal tools
- **Less Hub dependency** - Manage Unity versions and projects without running Unity Hub

## Installation

Requires [Rust](https://www.rust-lang.org) v1.85.0+

```bash
cargo install --git https://github.com/jakkovanhunen/ucom
```

Or build from source:

```bash
git clone https://github.com/jakkovanhunen/ucom
cd ucom
cargo build --release
```

## Quick Reference

### Project Management

```bash
ucom new ~/Projects/MyGame -u 2022.3               # Create new project
ucom open                                          # Open current project
ucom open -t ios                                   # Open with iOS target
ucom open --upgrade                                # Open and upgrade to latest matching major.minor
ucom open --upgrade=6000.2                         # Open and upgrade to specific version
ucom info                                          # Show project info
ucom updates                                       # Check for Unity updates
```

### Building

```bash
ucom build webgl                                   # Build for WebGL
ucom build android -d                              # Development build
ucom build ios --mode editor-quit                  # Build in editor, then quit
ucom build android --force-editor-build            # Build via open editor
```

### Testing

```bash
ucom test editmode                                 # Run EditMode tests
ucom test playmode                                 # Run PlayMode tests
ucom test android                                  # Run on Android
ucom test playmode --categories "!Slow;UI"         # Filter by category
```

### Unity Version Management

```bash
ucom list                                          # List installed versions
ucom list updates                                  # Check for updates
ucom list latest                                   # Show latest releases
ucom install 2022.3.5f1                            # Install specific version
```

### Helper Scripts

```bash
ucom add builder                                   # Add build script (required for builds)
ucom add builder-menu                              # Add Editor menu integration
ucom add gitignore                                 # Add Unity .gitignore
ucom add gitattributes                             # Add Git LFS attributes
```

## Building with Open Editor

When a project is open in Unity, `ucom build` automatically detects this and uses file-based IPC instead of launching
batch mode. This is faster and doesn't interrupt your workflow.

Requirements:

- Run `ucom add builder` to install the required script

Flags:

- `--force-editor-build` - Allow platform switching and play mode exit

## Build Options

### Platforms

`webgl`, `android`, `ios`, `win32`, `win64`, `macos`, `linux64`

### Modes

- `batch` - Headless, quit after build (default)
- `batch-nogfx` - Headless without graphics
- `editor-quit` - GUI build, quit after
- `editor` - GUI build, stay open

### Common Flags

- `-d, --development` - Development build
- `-r, --run` - Run after build
- `-S, --show` - Open output folder
- `-D, --debugging` - Enable remote debugging
- `-p, --profiling` - Enable profiler
- `-o, --output <DIR>` - Custom output path
- `-t, --type <release|debug>` - Build type subdirectory

### Build Options

Use `-O` to set Unity BuildOptions:

```bash
ucom build webgl -O Development AllowDebugging
```

Available options: `Development`, `AllowDebugging`, `ConnectWithProfiler`, `ShowBuiltPlayer`, `AutoRunPlayer`,
`CleanBuildCache`, `StrictMode`, `DetailedBuildReport`, and more. See `ucom help build` for complete list.

## Testing Options

### Platforms

`editmode`, `playmode`, `webgl`, `android`, `ios`, `win32`, `win64`, `macos`, `linux64`

### Filtering

```bash
--categories "Integration;UI"                      # Include categories
--categories "!Slow"                               # Exclude category
--tests "MyClass.MyTest"                           # Filter by name (regex supported)
--assemblies "MyTests.dll"                         # Filter by assembly
```

### Flags

- `-r, --show-results <all|errors|none>` - Result detail level
- `--no-batch-mode` - Run with graphics (may show UI popups)
- `-t, --target <PLATFORM>` - Override build target

## Environment Variables

- `UCOM_EDITOR_DIR` - Override Unity editor installation path
- `UCOM_BUILD_TARGET` - Default build target
- `UCOM_PACKAGE_LEVEL` - Package info detail level (none, no-unity, inc-unity, all)
- `UCOM_ENABLE_CACHE` - Enable/disable release data caching (default: enabled, 1-hour TTL)

## Additional Commands

### Run

Execute Unity with custom arguments:

```bash
ucom run -u 2022.3 -- -createProject ~/path/to/project -quit
```

### Cache

```bash
ucom cache clear                                   # Clear cached release data
ucom cache list                                    # List cached files
```

## Build Script Templates

The `build` command injects `UnityBuilder.cs` into your project. Control this behavior with `--inject`:

- `auto` (default) - Inject temporarily, remove after build
- `persistent` - Inject and leave in project
- `off` - Don't inject, fail if script missing

## Command Aliases

Most commands have short aliases:

- `ucom o` = `ucom open`
- `ucom b` = `ucom build`
- `ucom t` = `ucom test`
- `ucom ls` = `ucom list`
- `ucom i` = `ucom info`
- `ucom u` = `ucom updates`
- `ucom r` = `ucom run`

## Limitations

- macOS and Windows only
- Unity must be in default location or set via `UCOM_EDITOR_DIR`
- Git required for `ucom new` with version control
- iOS builds export Xcode project but don't compile it
- Editor IPC builds require `UnityBuilder.cs` in project

## Help

Run `ucom help` or `ucom help <command>` for detailed usage information.
