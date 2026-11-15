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
cargo install --git https://github.com/wazedde/ucom
```

Or build from source:

```bash
git clone https://github.com/wazedde/ucom
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
ucom build android --force-editor-build            # Allow platform switch in open editor
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
ucom list all -u '2022.3.*'                        # Show all 2022.3 releases
ucom install 2022.3.5f1                            # Install specific version
ucom install 2022.3                                # Install latest 2022.3 release
```

### Version Patterns

Commands that take a Unity version (`new -u`, `run -u`, `list -u`, `install`, `open --upgrade=`) accept a full version
or a pattern. A pattern selects the latest matching version, or filters the list for `list`.

| Pattern      | Matches                                                  |
|--------------|----------------------------------------------------------|
| `2022.3.5f1` | Exactly 2022.3.5f1                                       |
| `2022.3.5`   | 2022.3.5 with any build (`f1`, `f2`, ...), not 2022.3.50 |
| `2022.3`     | Any 2022.3 release                                       |
| `2022.3.62f` | 2022.3.62 final builds (`f1`, `f2`, ...)                 |
| `2022.3.*`   | Any 2022.3 release (same as `2022.3`)                    |
| `2022.3.6*`  | 2022.3.6 and 2022.3.60 to 2022.3.69                      |
| `202*`       | Any 2020 to 2029 release                                 |

Each number must match as a whole: `2022.3.1` does not match `2022.3.10f1`, and `202` does not match `2022`. Use `*` to
match any digits, or `*`/`x` as a whole component to match any value. Quote patterns that contain `*` so the shell does
not expand them.

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
- After updating ucom, run `ucom add builder --force` to update the script in your project

Flags:

- `--force-editor-build` - Allow platform switching and play mode exit, and wait for script compilation
- `--build-args` - Passed to the `[UcomPreProcessBuild]` method. As in batch builds, that method runs before every
  build, with an empty argument when `--build-args` is not set
- `--dry-run` - Show the command that would be sent to the editor, without building
- `--clean` - Clean the output directory after a successful build
- `--quiet` - Show a status line instead of progress messages

Not applicable when building via the open editor:

- `--mode` and `--inject` - The build runs in the open editor, which needs the persistent build script
- `--build-function` and extra Unity arguments after `--` - Not supported over IPC; ucom prints a warning when
  they are set

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

Release data and release notes are cached and refreshed after one hour. Use `ucom list -f` to force a full refresh of
the release data. A cache file that cannot be parsed is discarded and downloaded again when needed. If the cache
directory is not writable, ucom continues without caching and prints a warning.

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
- The build script (`UnityBuilder.cs`, `EditorMenu.cs`) requires Unity 2021.3 or newer; `build`, `add builder`, `add builder-menu` and `new --add-builder-menu` refuse older versions, except batch builds with `--inject off` and your own `--build-function`

## Help

Run `ucom help` or `ucom help <command>` for detailed usage information.
