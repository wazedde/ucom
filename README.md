# Unity Commander

`ucom` is a command line interface for Unity projects, written in Rust. It serves as an alternative to Unity Hub with
additional features.

## Core Features

- **Building Projects**: Quick commands for building projects
    - `ucom build ios` - Build an iOS version
    - `ucom build android` - Build an Android version

- **Managing Unity Versions**:
    - `ucom list` - View all installed Unity versions
    - `ucom list updates` - Check for available updates
    - `ucom install` - Install a specific Unity version

## Command Examples

| Command          | Alias        | Description                                 |
|------------------|--------------|---------------------------------------------|
| `ucom open`      | `ucom o`     | Open the Unity project in current directory |
| `ucom build ios` | `ucom b ios` | Build iOS version in batch mode             |
| `ucom list`      | `ucom ls`    | List all installed Unity versions           |
| `ucom info`      | `ucom i`     | Show project information                    |
| `ucom check`     | `ucom c`     | Check project for Unity updates             |

### Advanced Examples

- `ucom build android ~/Develop/MyProject --mode editor-quit` - Build Android for specific project and close editor
- `ucom new ~/Develop/MyProject -u 2021.3` - Create new project with specific Unity version
- `ucom open ~/Develop/MyProject -U 2021.3` - Open and upgrade project to newer Unity version

## Installation

1. Requires [Rust](https://www.rust-lang.org) v1.85.0+
2. Run: `cargo install --git https://github.com/jakkovanhunen/ucom`

### Manual Build

```bash
# Clone repository
# Then run:
cargo build --release
# Executable will be in target/release directory
```

## Build Script

The `build` command injects a [script](https://gist.github.com/jakkovanhunen/b56a70509616b6ff3492a17ae670a5e7) to handle
platform builds. Use `--inject persistent` to retain this script after building.

View the script with: `ucom template build-script`

## Environment Variables

| Variable             | Purpose                               |
|----------------------|---------------------------------------|
| `UCOM_EDITOR_DIR`    | Path to editor installation directory |
| `UCOM_BUILD_TARGET`  | Default build target                  |
| `UCOM_PACKAGE_LEVEL` | Default package info detail level     |

## Limitations

- macOS and Windows only
- Requires default editor location (or `UCOM_EDITOR_DIR`)
- Git required for `new` command
- Cannot build projects that are already open
- iOS builds don't build the exported Xcode project

## `ucom help`

```
Unity Commander: A command-line interface for Unity projects

Usage: ucom [OPTIONS] [COMMAND]

Commands:
  list     List installed Unity versions [aliases: ls]
  install  Install Unity version
  info     Display project information [aliases: i]
  check    Check for Unity updates [aliases: c]
  new      Create new Unity project and Git repository
  open     Open Unity project in the editor [aliases: o]
  build    Build Unity project [aliases: b]
  test     Run project tests [aliases: t]
  run      Run Unity with specified arguments [aliases: r]
  add      Add helper script or configuration file
  cache    Manage download cache
  help     Print this message or the help of the given subcommand(s)

Options:
  -D, --disable-color  Disable colored output
  -h, --help           Print help
  -V, --version        Print version
```

## `ucom help list`

```
List installed Unity versions

Usage: ucom list [OPTIONS] [LIST_TYPE]

Arguments:
  [LIST_TYPE]
          Specify what to list

          [default: installed]

          Possible values:
          - installed: List installed Unity versions
          - updates:   Show installed versions and check for updates
          - latest:    Show latest available Unity versions
          - all:       Show all available Unity versions

Options:
  -u, --unity <VERSION>
          Filter by Unity version prefix (e.g. '2021')

  -f, --force
          Force downloading release data from Unity API

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help info`

```
Display project information

Usage: ucom info [OPTIONS] [DIRECTORY]

Arguments:
  [DIRECTORY]
          Project directory path

          [default: .]

Options:
      --install-required
          Install required Unity version if not present

  -R, --recursive
          Recursively search for Unity projects

  -p, --packages <PACKAGES>
          Package information detail level

          [env: UCOM_PACKAGE_LEVEL=]
          [default: no-unity]

          Possible values:
          - none:      No package information
          - no-unity:  Non-Unity packages only
          - inc-unity: Include Unity registry packages
          - all:       All packages including built-in and dependencies

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help check`

```
Check for Unity updates

Usage: ucom check [OPTIONS] [DIRECTORY]

Arguments:
  [DIRECTORY]  Project directory path [default: .]

Options:
      --install-latest  Install latest Unity version if not present
  -r, --report          Generate Markdown report of release notes
  -h, --help            Print help
```

## `ucom help new`

```
Create new Unity project and Git repository

Usage: ucom new [OPTIONS] --unity <VERSION> <DIRECTORY> [-- <UNITY_ARGS>...]

Arguments:
  <DIRECTORY>
          Target directory (must not exist)

  [UNITY_ARGS]...
          Arguments to pass directly to Unity

Options:
  -u, --unity <VERSION>
          Unity version for new project (e.g. '2021' uses latest 2021.x.y)

  -t, --target <NAME>
          Set active build target

          [possible values: standalone, win32, win64, macos, linux64, ios, android, webgl, winstore, tvos]

      --add-builder-menu
          Add build menu script to project

          Adds both EditorMenu.cs and UnityBuilder.cs scripts to Assets/Plugins/Ucom/Editor directory

      --lfs
          Initialize Git LFS with Unity-specific attributes

      --no-git
          Skip Git repository initialization

  -w, --wait
          Wait for Unity to exit before returning

  -Q, --quit
          Close editor after project creation

  -q, --quiet
          Suppress messages

  -n, --dry-run
          Show command without executing

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help open`

```
Open Unity project in the editor

Usage: ucom open [OPTIONS] [DIRECTORY] [-- <UNITY_ARGS>...]

Arguments:
  [DIRECTORY]      Project directory path [default: .]
  [UNITY_ARGS]...  Arguments to pass directly to Unity

Options:
  -U, --upgrade [<VERSION>]  Upgrade project's Unity version. If no version specified, uses latest in project's
                             major.minor range. Version prefix like '2021' selects latest installed in that range
  -t, --target <NAME>        Set active build target [possible values: standalone, win32, win64, macos, linux64, ios,
                             android, webgl, winstore, tvos]
  -w, --wait                 Wait for Unity to exit before returning
  -Q, --quit                 Close editor after opening project
  -q, --quiet                Suppress messages
  -n, --dry-run              Show command without executing
  -h, --help                 Print help
```

## `ucom help build`

```
Build Unity project

Usage: ucom build [OPTIONS] <TARGET> [DIRECTORY] [-- <UNITY_ARGS>...]

Arguments:
  <TARGET>
          Target platform for build

          [env: UCOM_BUILD_TARGET=]
          [possible values: win32, win64, macos, linux64, ios, android, webgl]

  [DIRECTORY]
          Project directory path

          [default: .]

  [UNITY_ARGS]...
          Arguments to pass directly to Unity

Options:
  -o, --output <DIRECTORY>
          Output directory for build [default: <PROJECT_DIR>/Builds/<TYPE>/<TARGET>]

  -t, --type <TYPE>
          Output type for build directory naming

          Used in output directory path structure Ignored if --output is set

          [default: release]

          Possible values:
          - release: Output to Builds/Release directory
          - debug:   Output to Builds/Debug directory

  -r, --run
          Run built player

          Same as --build-options auto-run-player

  -d, --development
          Build development version

          Same as --build-options development

  -S, --show
          Show built player

          Same as --build-options show-built-player

  -D, --debugging
          Allow remote script debugging

          Same as --build-options allow-debugging

  -p, --profiling
          Connect to editor profiler

          Same as --build-options connect-with-profiler

  -P, --deep-profiling
          Enable deep profiling support

          Same as --build-options enable-deep-profiling-support

  -H, --connect-host
          Connect player to editor

          Same as --build-options connect-to-host

  -O, --build-options [<OPTION>...]
          Set Unity build options (space-separated)

          [default: none]

          Possible values:
          - none:                                    Default build with no special settings
          - development:                             Build development version
          - auto-run-player:                         Run built player
          - show-built-player:                       Show built player
          - build-additional-streamed-scenes:        Build compressed asset bundle with streamed scenes
          - accept-external-modifications-to-player: Used for Xcode (iOS) or Eclipse (Android) projects
          - clean-build-cache:                       Force full rebuild of all scripts and player data
          - connect-with-profiler:                   Connect to profiler in editor
          - allow-debugging:                         Allow remote script debugging
          - symlink-sources:                         Symlink sources for project generation
          - uncompressed-asset-bundle:               Skip asset bundle compression
          - connect-to-host:                         Connect player to editor
          - custom-connection-id:                    Use custom connection ID
          - build-scripts-only:                      Build only scripts
          - patch-package:                           Patch Android development package
          - compress-with-lz4:                       Use LZ4 compression
          - compress-with-lz4-hc:                    Use LZ4 high-compression
          - strict-mode:                             Fail build on any errors
          - include-test-assemblies:                 Include test assemblies
          - no-unique-identifier:                    Use zero GUID
          - wait-for-player-connection:              Wait for player connection on start
          - enable-code-coverage:                    Enable code coverage
          - enable-deep-profiling-support:           Enable deep profiling support
          - detailed-build-report:                   Generate detailed build report
          - shader-livelink-support:                 Enable shader livelink

  -a, --build-args <STRING>
          Custom argument string for UcomPreProcessBuild

          Passed to functions with UcomPreProcessBuild attribute Useful for version numbers or build configuration flags
          Requires ucom's injected build script

  -C, --clean
          Remove unused files from output directory

  -i, --inject <METHOD>
          Build script injection method

          [default: auto]

          Possible values:
          - auto:       Inject build script temporarily
          - persistent: Inject build script permanently
          - off:        Use existing build script only

  -m, --mode <MODE>
          Build mode

          [default: batch]

          Possible values:
          - batch:       Build in batch mode
          - batch-nogfx: Build in batch mode without graphics
          - editor-quit: Build in editor and quit
          - editor:      Build in editor and stay open

  -f, --build-function <FUNCTION>
          Static build method in project

          [default: Ucom.UnityBuilder.Build]

  -l, --log-file <FILE>
          Log file for Unity build output [default: <PROJECT_DIR>/Logs directory]

  -q, --quiet
          Suppress build log output

  -n, --dry-run
          Show command without executing

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help test`

```
Run project tests

Usage: ucom test [OPTIONS] <PLATFORM> [DIRECTORY] [-- <UNITY_ARGS>...]

Arguments:
  <PLATFORM>
          Platform to run tests on

          Build target is automatically determined by platform. 'editmode' and 'playmode' use 'standalone' build target
          'macos' uses 'macos' build target, etc. Use --target to override this behavior.

          [possible values: editmode, playmode, macos, win32, win64, linux64, ios, android, webgl]

  [DIRECTORY]
          Project directory path

          [default: .]

  [UNITY_ARGS]...
          Arguments to pass directly to Unity

Options:
  -t, --target <NAME>
          Set active build target

          Default build target matches test platform. Override to run tests with different build target (e.g., editmode
          tests with ios target)

          [possible values: standalone, win32, win64, macos, linux64, ios, android, webgl, winstore, tvos]

  -r, --show-results <RESULTS>
          Test results display level

          [default: all]

          Possible values:
          - all:    Display all results
          - errors: Display only errors
          - none:   Display no results

      --no-batch-mode
          Disable batch mode

          Batch mode prevents manual inputs but disables graphics device which may cause some tests to fail

      --forget-project-path
          Skip adding project to Unity launcher/hub history

      --categories <LIST>
          Filter by test categories

          Use semicolon-separated list in quotes: "category1;category2" Works with --tests to run only tests matching
          both filters Use negation with ! prefix: "!excludedCategory"

      --tests <LIST>
          Filter by test names or regex pattern

          Use semicolon-separated list in quotes: "Test1;Test2" Supports negation with ! prefix: "!TestToExclude" For
          parameterized tests: "ClassName\.MethodName\(Param1,Param2\)"

      --assemblies <LIST>
          Filter by test assemblies

          Use semicolon-separated list in quotes: "Assembly1;Assembly2"

  -q, --quiet
          Suppress messages

  -n, --dry-run
          Show command without executing

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help run`

```
Run Unity with specified arguments

Usage: ucom run [OPTIONS] --unity <VERSION> -- <UNITY_ARGS>...

Arguments:
  <UNITY_ARGS>...  Arguments to pass directly to Unity

Options:
  -u, --unity <VERSION>  Unity version to run (e.g. '2021' for latest 2021.x.y)
  -w, --wait             Wait for Unity to exit before returning
  -q, --quiet            Suppress messages
  -n, --dry-run          Show command without executing
  -h, --help             Print help
```

## `ucom help add`

```
Add helper script or configuration file

Usage: ucom add [OPTIONS] <TEMPLATE> [DIRECTORY]

Arguments:
  <TEMPLATE>
          Template file to add to project

          Possible values:
          - builder:        C# helper script for project building
          - builder-menu:   C# helper script adding build commands to Unity menu (includes 'builder')
          - git-ignore:     Unity-specific .gitignore file
          - git-attributes: Unity-specific .gitattributes file

  [DIRECTORY]
          Project directory path

          [default: .]

Options:
  -f, --force
          Overwrite existing template files

  -c, --display-content
          Print template content to stdout

  -u, --display-url
          Print template source URL

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help cache`

```
Manage download cache

By default, cached files expire after one hour. The system will re-download required files after this timeout.

Control caching with UCOM_ENABLE_CACHE environment variable. Set to 'false' to disable caching and always download fresh
data.

Usage: ucom cache <ACTION>

Arguments:
  <ACTION>
          Possible values:
          - clear: Remove all cached files
          - list:  Show list of cached files

Options:
  -h, --help
          Print help (see a summary with '-h')
```
