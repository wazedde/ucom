# Unity Commander

A command line interface for Unity projects written in Rust.

Because typing `ucom open .` to open a Unity project in the current directory is sometimes more convenient than having
to deal with the Unity Hub.

Some examples:

- `ucom list` lists all the Unity versions on the system.
- `ucom list -u 2021.3` lists the Unity versions in the 2021.3 range on the system.

- `ucom new ~/Develop/MyProject` creates a new project using the latest Unity version on the system.
- `ucom new ~/Develop/MyProject -u 2021.3 -- -quit` creates a new project using the latest 2021.3 version on the system
  and closes the editor after it has been created.

- `ucom open ~/Develop/MyProject` opens the project in the directory.
- `ucom open ~/Develop/MyProject -u 2021.3` opens the project with the latest 2021.3 version. Use it to e.g. upgrade the
  project to the latest Unity version.

## How to build

- Make sure [Rust](https://www.rust-lang.org) is installed on your system (v1.65.0 or later).
- Run `cargo build --release` in the root of this project.
- After completion the executable can be found in the `target/release` directory.

## `ucom help`

```
Usage: ucom <COMMAND>

Commands:
  list  This command will show a list of the installed Unity versions.
  run   This command will run Unity.
            Unless specified otherwise, the latest installed Unity version is used. [aliases: r]
  new   This command will create a new Unity project in the given directory.
            Unless specified otherwise, the latest installed Unity version is used.
  open  This command will open the Unity project in the given directory. [aliases: o]
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```

## `ucom help list`

```
This command will show a list of the installed Unity versions.

Usage: ucom list [OPTIONS]

Options:
  -u, --unity <VERSION>  The Unity versions to list. You can specify a partial version; e.g. 2021 will list all
                         the 2021.x.y versions you have installed on your system.
  -h, --help             Print help information
```

## `ucom help run`

```
This command will run Unity.
Unless specified otherwise, the latest installed Unity version is used.

Usage: ucom run [OPTIONS] <UNITY_ARGS>...

Arguments:
  <UNITY_ARGS>...  A list of arguments passed directly to Unity.

Options:
  -u, --unity <VERSION>  The Unity version to run. You can specify a partial version; e.g. 2021 will match the
                         latest 2021.x.y version you have installed on your system.
  -w, --wait             Waits for the command to finish before continuing.
  -q, --quiet            Do not print ucom log messages.
  -n, --dry-run          Show what would be run, but do not actually run it.
  -h, --help             Print help information
```

## `ucom help new`

```
This command will create a new Unity project in the given directory.
Unless specified otherwise, the latest installed Unity version is used.

Usage: ucom new [OPTIONS] <DIR> [UNITY_ARGS]...

Arguments:
  <DIR>            The directory where the project is created. This directory should not exist yet.
  [UNITY_ARGS]...  A list of arguments passed directly to Unity.

Options:
  -u, --unity <VERSION>  The Unity version to use for the new project. You can specify a partial version;
                         e.g. 2021 will match the latest 2021.x.y version you have installed on your system.
  -w, --wait             Waits for the command to finish before continuing.
  -q, --quiet            Do not print ucom log messages.
  -n, --dry-run          Show what would be run, but do not actually run it.
  -h, --help             Print help information
```

## `ucom help open`

```
This command will open the Unity project in the given directory.

Usage: ucom open [OPTIONS] <DIR> [UNITY_ARGS]...

Arguments:
  <DIR>            The directory of the project.
  [UNITY_ARGS]...  A list of arguments passed directly to Unity.

Options:
  -u, --unity <VERSION>  The Unity version to open the project with. Use it to open a project with a newer
                         Unity version. You can specify a partial version; e.g. 2021 will match the latest
                         2021.x.y version you have installed on your system.
  -w, --wait             Waits for the command to finish before continuing.
  -q, --quiet            Do not print ucom log messages.
  -n, --dry-run          Show what would be run, but do not actually run it.
  -h, --help             Print help information
```