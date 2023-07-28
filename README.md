# Unity Commander

Unity Commander (`ucom`) is a command line interface for Unity projects, written in Rust. It's an alternative to Unity
Hub with additional features.

For building projects, `ucom` provides quick commands like `ucom build ios` for building an iOS version,
or `ucom build android` for Android. These commands initiate a batch-mode build of the project in the current directory.

Alongside building, `ucom` also helps manage installed Unity versions. By using `ucom list`, you can view all installed
Unity versions. To see specific versions, `ucom list -u <version>` will show you all installed Unity versions within a
given range. This tool simplifies the task of tracking and managing multiple Unity installations.

## Command Examples

- `ucom open` or `ucom o` opens the Unity project in the current directory.
- `ucom build ios` or `ucom b ios` initiates an iOS batch-mode build for the project in the current directory.
- `ucom build android ~/Develop/MyProject --mode editor-quit` starts an Android build for the specified project and then
  closes the editor.
- `ucom list` or `ucom l` lists all Unity versions installed on your system.
- `ucom list updates` checks for any updates available for your installed Unity versions.
- `ucom list latest` Lists the latest versions of all officially released Unity versions.
- `ucom list -u 2021.3` displays all Unity versions within the 2021.3 range installed on your system.
- `ucom info` or `ucom i` provides information about the Unity project in the current directory.
- `ucom check` or `ucom c` checks for Unity updates for the project in the current directory.
- `ucom new ~/Develop/MyProject` or `ucom n ~/Develop/MyProject` creates a new project using the latest system Unity
  version and initializes a git repository with a Unity-specific `.gitignore`.
- `ucom new ~/Develop/MyProject -Q -u 2021.3` creates a new project using the latest 2021.3 version on the system and
  closes the editor afterward.
- `ucom open ~/Develop/MyProject` opens the project in the specified directory.
- `ucom open ~/Develop/MyProject -U 2021.3` opens the project and upgrades it to the latest installed 2021.3 version.

## Installation

Ensure you have [Rust](https://www.rust-lang.org) (v1.65.0 or later) installed on your system. Then,
execute `cargo install --git https://github.com/jakkovanhunen/ucom` in your terminal. The `ucom` command should now be
available.

For manual building:

1. Clone the repository.
2. Execute `cargo build --release` in the project root.
3. Upon completion, the executable can be found in the `target/release` directory.

## Build Script

The `ucom build` command injects a [build script](src/commands/include/UnityBuilder.cs) into the project to initiate the
build for a specific platform. This is due to Unity's lack of CLI argument support for building all target platforms.

By default, the script is removed after the build is completed. To retain the script, add the `--inject persistent`
option to the `ucom build` command.

To view the build script, use `ucom template build-script`.

## Environment Variables

- `UCOM_EDITOR_DIR`: Path to the directory where editors are installed.
- `UCOM_BUILD_TARGET`: Default build target for project builds.
- `UCOM_DEFAULT_VERSION`: Default Unity version for new projects or Unity runs.
- `UCOM_PACKAGE_LEVEL`: Default level of package information for the `info` command.

If not set, `ucom` will use the default Unity Hub installation directory for `UCOM_EDITOR_DIR`, and the latest installed
Unity version for `UCOM_DEFAULT_VERSION`. `UCOM_BUILD_TARGET` and `UCOM_PACKAGE_LEVEL` will need to be specified as
arguments.

## Limitations

- Only macOS and Windows are supported.
- Editor installations are expected in the default location. If installed elsewhere, set `UCOM_EDITOR_DIR`.
- Git is required for repository initialization with the `new` command.
- `ucom build` does not support building a project that is already open in the editor.
- `ucom build ios` does not build the exported Xcode project.

## `ucom help`

```
Unity Commander: A command-line interface for Unity projects

Usage: ucom [OPTIONS] [COMMAND]

Commands:
  list      Lists installed Unity versions [aliases: l]
  info      Displays project information [aliases: i]
  check     Checks the Unity website for updates to the project's version [aliases: c]
  new       Creates a new Unity project and Git repository, defaulting to the latest installed Unity version
                [aliases: n]
  open      Opens a specified Unity project in the Unity Editor [aliases: o]
  build     Builds a specified Unity project [aliases: b]
  run       Runs Unity with specified arguments, defaulting to the latest installed Unity version [aliases: r]
  template  Prints the specified template to standard output
  help      Print this message or the help of the given subcommand(s)

Options:
  -D, --disable-color  Disables colored output
  -h, --help           Print help
  -V, --version        Print version
```

## `ucom help list`

```
Lists installed Unity versions

Usage: ucom list [OPTIONS] [LIST_TYPE]

Arguments:
  [LIST_TYPE]
          Defines what to list
          
          [default: installed]

          Possible values:
          - installed: Lists the installed Unity versions
          - updates:   Displays installed Unity versions and checks for online updates
          - latest:    Shows the latest available Unity versions

Options:
  -u, --unity <VERSION>
          Filters the Unity versions to list based on the pattern. For example, '2021' will list all
          2021.x.y versions

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help info`

```
Displays project information

Usage: ucom info [OPTIONS] [DIRECTORY]

Arguments:
  [DIRECTORY]
          Specifies the project's directory

          [default: .]

Options:
  -p, --packages <PACKAGES>
          Determines the level of package information to display

          [env: UCOM_PACKAGE_LEVEL=]
          [default: excluding-unity]

          Possible values:
          - none:            No package information is displayed
          - excluding-unity: Displays non-Unity packages only
          - including-unity: Additionally includes information for packages from the Unity registry
          - all:             Displays all package information including built-in packages and dependencies

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help check`

```
Checks the Unity website for updates to the project's version

Usage: ucom check [OPTIONS] [DIRECTORY]

Arguments:
  [DIRECTORY]  Specifies the project's directory [default: .]

Options:
  -r, --create-report <FILE.md>  Generates a Markdown report of available releases
  -h, --help                     Print help
```

## `ucom help new`

```
Creates a new Unity project and Git repository, defaulting to the latest installed Unity version

Usage: ucom new [OPTIONS] <DIRECTORY> [-- <UNITY_ARGS>...]

Arguments:
  <DIRECTORY>      Defines the directory for creating the project. This directory should not pre-exist
  [UNITY_ARGS]...  A list of arguments to be passed directly to Unity

Options:
  -u, --unity <VERSION>  Specifies the Unity version for the new project. For example, '2021' uses the latest installed 2021.x.y version [env: UCOM_DEFAULT_VERSION=2022.3]
      --lfs              Includes a .gitattributes file configured with Unity-specific LFS settings
      --no-git           Skips initialization of a new git repository
  -w, --wait             Waits for the command to complete before proceeding
  -Q, --quit             Closes the editor after the project creation
  -q, --quiet            Suppresses ucom messages
  -n, --dry-run          Shows the command to be run without actually executing it
  -h, --help             Print help
```

## `ucom help open`

```
Opens a specified Unity project in the Unity Editor

Usage: ucom open [OPTIONS] [DIRECTORY] [-- <UNITY_ARGS>...]

Arguments:
  [DIRECTORY]      Specifies the project's directory [default: .]
  [UNITY_ARGS]...  A list of arguments to be passed directly to Unity

Options:
  -U, --upgrade [<VERSION>]  Upgrades the project's Unity version. A partial version like '2021'
                             selects the latest installed version within the 2021.x.y range. If no
                             version is specified, it defaults to the latest available version
                             within the project's major.minor range
  -t, --target <NAME>        Determines the active build target to open the project with [possible
                             values: standalone, win32, win64, macos, linux64, ios, android, webgl,
                             winstore, tvos]
  -w, --wait                 Waits for the command to complete before proceeding
  -Q, --quit                 Closes the editor after opening the project
  -q, --quiet                Suppresses ucom messages
  -n, --dry-run              Shows the command to be run without actually executing it
  -h, --help                 Print help
```

## `ucom help build`

```
Builds a specified Unity project

Usage: ucom build [OPTIONS] <TARGET> [DIRECTORY] [-- <UNITY_ARGS>...]

Arguments:
  <TARGET>
          Specifies the target platform for the build
          
          [env: UCOM_BUILD_TARGET=]
          [possible values: win32, win64, macos, linux64, ios, android, webgl]

  [DIRECTORY]
          Defines the project's directory
          
          [default: .]

  [UNITY_ARGS]...
          A list of arguments to be passed directly to Unity

Options:
  -o, --output <DIRECTORY>
          Sets the output directory for the build. If omitted, the build is placed in
          <PROJECT_DIR>/Builds/<TARGET>

  -O, --build-options [<OPTION>...]
          Sets the build options. Multiple options can be combined by separating them with spaces
          
          [default: none]

          Possible values:
          - none:                                    Perform the specified build without any special settings or extra tasks
          - development:                             Build a development version of the player
          - auto-run-player:                         Run the built player
          - show-built-player:                       Show the built player
          - build-additional-streamed-scenes:        Build a compressed asset bundle that contains streamed Scenes loadable with the UnityWebRequest class
          - accept-external-modifications-to-player: Used when building Xcode (iOS) or Eclipse (Android) projects
          - clean-build-cache:                       Clear all cached build results, resulting in a full rebuild of all scripts and all player data
          - connect-with-profiler:                   Start the player with a connection to the profiler in the editor
          - allow-debugging:                         Allow script debuggers to attach to the player remotely
          - symlink-sources:                         Symlink sources when generating the project. This is useful if you're changing source files inside the generated project and want to bring the changes back into your Unity project or a package
          - uncompressed-asset-bundle:               Don't compress the data when creating the asset bundle
          - connect-to-host:                         Sets the Player to connect to the Editor
          - custom-connection-id:                    Determines if the player should be using the custom connection ID
          - build-scripts-only:                      Only build the scripts in a Project
          - patch-package:                           Patch a Development app package rather than completely rebuilding it. Supported platforms: Android
          - compress-with-lz4:                       Use chunk-based LZ4 compression when building the Player
          - compress-with-lz4-hc:                    Use chunk-based LZ4 high-compression when building the Player
          - strict-mode:                             Do not allow the build to succeed if any errors are reporting during it
          - include-test-assemblies:                 Build will include Assemblies for testing
          - no-unique-identifier:                    Will force the buildGUID to all zeros
          - wait-for-player-connection:              Sets the Player to wait for player connection on player start
          - enable-code-coverage:                    Enables code coverage. You can use this as a complimentary way of enabling code coverage on platforms that do not support command line arguments
          - enable-deep-profiling-support:           Enables Deep Profiling support in the player
          - detailed-build-report:                   Generates more information in the BuildReport
          - shader-livelink-support:                 Enable Shader Livelink support

  -C, --clean
          Removes directories from the output directory not needed for distribution

  -i, --inject <METHOD>
          Determines the method of build script injection
          
          [default: auto]

          Possible values:
          - auto:       Inject a build script if none exists, and remove it post-build
          - persistent: Inject a build script into the project and retain it post-build
          - off:        Use the existing build script in the project, without any injection

  -m, --mode <MODE>
          Defines the build mode
          
          [default: batch]

          Possible values:
          - batch:       Execute build in 'batch' mode and await completion
          - batch-nogfx: Execute build in 'batch' mode without utilizing the graphics device, and await completion
          - editor-quit: Execute build within the editor and terminate post-build
          - editor:      Execute build within the editor, keeping it open post-build. Useful for debugging

  -f, --build-function <FUNCTION>
          Specifies the static method in the Unity project used for building the project
          
          [default: Ucom.UnityBuilder.Build]

  -l, --log-file <FILE>
          Designates the log file for Unity's build output. By default, log is written to the
          project's `Logs` directory

  -q, --quiet
          Suppresses build log output to stdout

  -n, --dry-run
          Displays the command to be run without actually executing it

  -h, --help
          Print help (see a summary with '-h')
```

## `ucom help run`

```
Runs Unity with specified arguments, defaulting to the latest installed Unity version

Usage: ucom run [OPTIONS] -- <UNITY_ARGS>...

Arguments:
  <UNITY_ARGS>...  A list of arguments to be passed directly to Unity

Options:
  -u, --unity <VERSION>  Specifies the Unity version to run. For example, '2021' runs the latest
                         installed 2021.x.y version [env: UCOM_DEFAULT_VERSION=2021]
  -w, --wait             Waits for the command to complete before proceeding
  -q, --quiet            Suppresses ucom messages
  -n, --dry-run          Displays the command to be run without actually executing it
  -h, --help             Print help
```

## `ucom help template`

```
Prints the specified template to standard output

Usage: ucom template <TEMPLATE>

Arguments:
  <TEMPLATE>
          Possible values:
          - build-script:   The C# script injected into the project when building
          - git-ignore:     The .gitignore file for newly created projects
          - git-attributes: The .gitattributes file for newly created projects

Options:
  -h, --help
          Print help (see a summary with '-h')
  ```