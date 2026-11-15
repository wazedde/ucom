// This file is part of the ucom command line tool (https://github.com/wazedde/ucom).
// Copyright 2022-2026 Jakko van Hunen
// SPDX-License-Identifier: Apache-2.0

#if UNITY_2021_3_OR_NEWER
using System;
using System.Collections.Generic;
using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using JetBrains.Annotations;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEngine;
using Debug = UnityEngine.Debug;

// ReSharper disable once CheckNamespace
namespace Ucom
{
    /// <summary>
    /// Build companion script for ucom.<br/>
    /// - Lines after "[Builder] Build Report" until an empty line are printed to the console after the build finishes.<br/>
    /// - Lines starting with "[Builder] Error:" are printed to the console if the build fails.<br/>
    /// </summary>
    public static class UnityBuilder
    {
        // Command line arguments that ucom passes to Build().
        private const string BuildOutputArg = "--ucom-build-output"; // The output directory.
        private const string BuildTargetArg = "--ucom-build-target"; // A BuildTarget name.
        private const string BuildOptionsArg = "--ucom-build-options"; // Combined BuildOptions as an int.
        private const string PreBuildArgs = "--ucom-pre-build-args"; // Passed to the UcomPreProcessBuild method.
        private const string AddDefinesArg = "--ucom-add-defines"; // Extra scripting define symbols, ';'-separated.

        // The supported build targets: their directory in Builds/ and the extension of the build output.
        // A null extension means the output is a directory.
        private static readonly Dictionary<BuildTarget, (string Dir, string Extension)> Targets = new()
        {
            [BuildTarget.StandaloneWindows] = ("Win", ".exe"),
            [BuildTarget.StandaloneWindows64] = ("Win64", ".exe"),
            [BuildTarget.StandaloneOSX] = ("OSXUniversal", ".app"),
            [BuildTarget.StandaloneLinux64] = ("Linux64", ".x86_64"),
            [BuildTarget.Android] = ("Android", ".apk"),
            [BuildTarget.iOS] = ("iOS", null),
            [BuildTarget.WebGL] = ("WebGL", null),
        };

        /// <summary>
        /// This method is called by ucom to build the project.
        /// </summary>
        [UsedImplicitly]
        public static void Build()
        {
            var args = Environment.GetCommandLineArgs();
            var active = EditorUserBuildSettings.activeBuildTarget;
            var invalidArgs = false;

            void Fail(string message)
            {
                Log($"[Builder] Error: {message}", LogType.Error);
                invalidArgs = true;
            }

            if (!args.TryGetArgValue(BuildOutputArg, out var outputDirectory))
                Fail($"Output path '{BuildOutputArg} <path>' not specified.");

            if (!args.TryGetArgValue(BuildTargetArg, out var targetValue))
                Fail($"Build target '{BuildTargetArg} <target>' not specified.");
            else if (!Enum.TryParse(targetValue, out BuildTarget target))
                Fail($"Invalid build target: {BuildTargetArg} {targetValue}");
            else if (target != active)
                // ucom starts Unity with the requested build target, but that fails if it is not installed.
                Fail(BuildPipeline.IsBuildTargetSupported(BuildPipeline.GetBuildTargetGroup(target), target)
                    ? $"Build target '{target}' does not match active build target '{active}'"
                    : $"Build target '{target}' is not supported or installed.");

            var options = BuildOptions.None;
            if (args.TryGetArgValue(BuildOptionsArg, out var optionsValue))
            {
                if (int.TryParse(optionsValue, out var bo))
                    options = (BuildOptions)bo;
                else
                    Fail($"Invalid build options: {BuildOptionsArg} {optionsValue}");
            }

            var extraScriptingDefines = args.TryGetArgValue(AddDefinesArg, out var defines) ? defines.Split(';') : null;
            if (!args.TryGetArgValue(PreBuildArgs, out var preBuildArgs))
                preBuildArgs = "";

            var succeeded = !invalidArgs
                            && TryGetBuildLocationPath(outputDirectory, Application.productName, active, out var path)
                            && Build(path, GetActiveScenes(), options, extraScriptingDefines, preBuildArgs);

            if (args.Contains("-quit"))
                EditorApplication.Exit(succeeded ? 0 : 1);
        }

        /// <summary>
        /// Builds the application for the <see cref="EditorUserBuildSettings.activeBuildTarget"/>.
        /// </summary>
        /// <param name="locationPathName">The path where the application will be built.</param>
        /// <param name="scenes">The scenes to include in the build</param>
        /// <param name="options">Building options. Multiple options can be combined together.</param>
        /// <param name="extraScriptingDefines">User-specified preprocessor defines used while compiling assemblies for the player.</param>
        /// <param name="preBuildArgs">The pre-build arguments that are passed to methods with the <see cref="UcomPreProcessBuildAttribute"/>.</param>
        /// <returns><c>true</c> if the build succeeded; <c>false</c> otherwise.</returns>
        public static bool Build(string locationPathName,
            string[] scenes,
            BuildOptions options = BuildOptions.None,
            string[] extraScriptingDefines = null,
            string preBuildArgs = "")
        {
            if (scenes.Length == 0)
            {
                Log("[Builder] Error: no scenes to build specified.", LogType.Error);
                return false;
            }

            var buildPlayerOptions = new BuildPlayerOptions
            {
                scenes = scenes,
                extraScriptingDefines = extraScriptingDefines,
                options = options,
                locationPathName = locationPathName,
                target = EditorUserBuildSettings.activeBuildTarget,
            };

            if (!RunPreProcessBuildMethod(preBuildArgs))
                return false;

            BuildReport report;
            try
            {
                report = BuildPipeline.BuildPlayer(buildPlayerOptions);
            }
            catch (Exception e)
            {
                Log($"[Builder] Error: {e}", LogType.Exception);
                return false;
            }

            Environment.GetCommandLineArgs().TryGetArgValue("-logFile", out var logFile);
            var succeeded = report.summary.result == BuildResult.Succeeded;
            Log(FormatBuildResult(report, logFile), succeeded ? LogType.Log : LogType.Error);
            return succeeded;
        }

        /// <summary>
        /// Formats the build's error messages, the build result line and the "[Builder] Build Report"
        /// block that ucom reads from the log, ending with an empty line.
        /// </summary>
        /// <param name="report">The report of the finished build.</param>
        /// <param name="logFile">The log file to mention in the report, or <c>null</c> to leave it out.</param>
        public static string FormatBuildResult(BuildReport report, string logFile)
        {
            var summary = report.summary;
            var sb = new StringBuilder();

            // Unity logs these in many different forms; ucom finds them by the "[Builder] Error:" prefix.
            foreach (var error in GetBuildErrors(report))
                sb.AppendLine($"[Builder] Error: {error}");

            var buildStartTime = summary.buildStartedAt.ToLocalTime().ToString(CultureInfo.InvariantCulture);
            sb.AppendLine(summary.result == BuildResult.Succeeded ? "[Builder] Build succeeded." : "[Builder] Build failed.")
                .AppendLine("[Builder] Build Report")
                .AppendLine($"    Build result: {summary.result}")
                .AppendLine($"    Platform:     {summary.platform}")
                .AppendLine($"    Output path:  {summary.outputPath}")
                .AppendLine($"    Size:         {summary.totalSize / 1024 / 1024} MB")
                .AppendLine($"    Start time:   {buildStartTime}")
                .AppendLine($"    Build time:   {summary.totalTime.TotalSeconds:0.00}s")
                .AppendLine($"    Errors:       {summary.totalErrors}")
                .AppendLine($"    Warnings:     {summary.totalWarnings}");

            if (!string.IsNullOrEmpty(logFile))
                sb.AppendLine($"    Log file:     {logFile}");

            // End of build report.
            return sb.AppendLine().ToString();
        }

        /// <summary>
        /// Returns the distinct error messages of a build, one line each.
        /// </summary>
        private static IEnumerable<string> GetBuildErrors(BuildReport report) => report.steps
            .SelectMany(step => step.messages)
            .Where(message => message.type is LogType.Error or LogType.Exception or LogType.Assert)
            .Select(message => string.IsNullOrEmpty(message.content) ? "" : message.content.Split('\n')[0].Trim())
            .Where(line => line.Length > 0)
            .Distinct();

        /// <summary>
        /// Tries to get the full path of the build location: <paramref name="buildRootPath"/> plus the app file name.
        /// </summary>
        /// <returns>True if the build target is supported; False otherwise.</returns>
        public static bool TryGetBuildLocationPath(string buildRootPath,
            string appName,
            BuildTarget buildTarget,
            out string fullOutputPath)
        {
            var supported = TryGetAppFileName(appName, buildTarget, out var appFileName);
            fullOutputPath = supported ? Path.Combine(buildRootPath, appFileName) : null;
            return supported;
        }

        /// <summary>
        /// Tries to get the file name of the application, without characters that are invalid in file names.
        /// </summary>
        /// <returns>True if the build target is supported; False otherwise.</returns>
        public static bool TryGetAppFileName(string appName, BuildTarget buildTarget, out string fileName)
        {
            if (!Targets.TryGetValue(buildTarget, out var target))
            {
                Log($"[Builder] Error: '{buildTarget}' build target not supported.", LogType.Error);
                fileName = null;
                return false;
            }

            fileName = string.Join("_", appName.Split(Path.GetInvalidFileNameChars()));
            fileName = target.Extension != null
                ? fileName + target.Extension
                : string.Join("_", fileName.Split(Path.GetInvalidPathChars())).Replace(" ", "_");
            return true;
        }

        /// <summary>
        /// Returns the paths of the active scenes in the build settings.
        /// </summary>
        [JetBrains.Annotations.NotNull]
        public static string[] GetActiveScenes() =>
            EditorBuildSettings.scenes.Where(scene => scene.enabled).Select(scene => scene.path).ToArray();

        /// <summary>
        /// Tries to get the default build output path for the current build target:
        /// <c>Builds/&lt;output type&gt;/&lt;target&gt;</c> in the project root.
        /// </summary>
        /// <returns>True if current build target is supported; False otherwise.</returns>
        public static bool TryGetDefaultBuildOutputPath(out string outputPath,
            OutputType outputTypeType = OutputType.Release)
        {
            outputPath = Targets.TryGetValue(EditorUserBuildSettings.activeBuildTarget, out var target)
                ? Path.Combine(GetBuildsDirectoryPath(), outputTypeType.ToString(), target.Dir)
                : null;
            return outputPath != null;
        }

        /// <summary>
        /// Returns the project root path.
        /// </summary>
        public static string GetProjectRootPath() => new DirectoryInfo(Application.dataPath).Parent?.FullName;

        /// <summary>
        /// Returns the builds directory path.
        /// </summary>
        public static string GetBuildsDirectoryPath() => Path.Combine(GetProjectRootPath(), "Builds");

        /// <summary>
        /// Logs a message to the Unity console without the stack trace.
        /// </summary>
        public static void Log(string message, LogType logType = LogType.Log) =>
            // The message is an argument, not the format string: it can contain braces (paths, exceptions).
            Debug.LogFormat(logType, LogOption.NoStacktrace, null, "{0}", message);

        /// <summary>
        /// Tries to get the value of the specified argument (the next argument in the array).
        /// </summary>
        /// <returns>True if the argument was found; False otherwise.</returns>
        public static bool TryGetArgValue(this string[] source, string arg, out string value)
        {
            var index = Array.IndexOf(source, arg);
            value = index >= 0 && index < source.Length - 1 ? source[index + 1] : null;
            return value != null;
        }

        private static bool RunPreProcessBuildMethod(string arg)
        {
            const string attribute = nameof(UcomPreProcessBuildAttribute);
            var methods = TypeCache.GetMethodsWithAttribute<UcomPreProcessBuildAttribute>()
                .Where(m => m.IsStatic)
                .ToList();
            if (methods.Count == 0)
                return true;

            if (methods.Count > 1)
            {
                var names = string.Join(", ", methods.Select(MethodName));
                Log($"[Builder] Error: Multiple {attribute} methods found, there should only be one: {names}", LogType.Error);
                return false;
            }

            var method = methods[0];
            var parameters = method.GetParameters();
            if (parameters.Length != 1 || parameters[0].ParameterType != typeof(string))
            {
                Log($"[Builder] Error: Invalid method signature for {attribute}: {MethodName(method)}", LogType.Error);
                return false;
            }

            try
            {
                method.Invoke(null, new object[] { arg });
                return true;
            }
            catch (Exception e)
            {
                // Invoke wraps what the method throws in a TargetInvocationException.
                var error = (e as TargetInvocationException)?.InnerException ?? e;
                Log($"[Builder] Error: {MethodName(method)} failed: {error}", LogType.Error);
                return false;
            }
        }

        private static string MethodName(MethodBase method) => $"{method.ReflectedType?.FullName}.{method.Name}";
    }

    /// <summary>
    /// This is mainly a flag used in the output directory, it doesn't dictate the physical type of build.
    /// The default value for builds is <see cref="OutputType.Release"/>.
    /// </summary>
    public enum OutputType
    {
        /// <summary>Build will be outputted to the <c>Builds/Release</c> directory.</summary>
        Release,

        /// <summary>Build will be outputted to the <c>Builds/Debug</c> directory.</summary>
        Debug,
    }

    /// <summary>
    /// Add this attribute to a method to get a notification just before building the player.
    /// <remarks>
    /// There can only be one method with this attribute in the project and it must be static with a single string parameter.
    /// The string parameter will be the argument passed in from the command line.
    /// </remarks>
    /// </summary>
    /// <example>
    /// <code>
    /// <![CDATA[
    /// [UcomPreProcessBuild]
    /// public static void PreProcessBuild(string args)
    /// {
    ///     Debug.Log(args);
    /// }
    /// ]]>
    /// </code>
    /// </example>
    [AttributeUsage(AttributeTargets.Method)]
    public class UcomPreProcessBuildAttribute : Attribute { }

    /// <summary>
    /// Editor script that watches for ucom command files and executes them.
    /// This enables ucom to communicate with an already-running Unity editor.
    /// </summary>
    [InitializeOnLoad]
    public class EditorCommandWatcher
    {
        // Command and result files are named <prefix><uuid>.json; ucom uses the same names (build_cmd/editor.rs).
        private const string CommandFilePrefix = "build-";
        private const double PollIntervalSeconds = 1.0;
        private static readonly string CommandDir = Path.Combine(Application.dataPath, "..", "Temp", "ucom-commands");
        private static readonly string ResultDir = Path.Combine(Application.dataPath, "..", "Temp", "ucom-results");
        private static FileSystemWatcher _watcher;
        private static double _lastPollTime;

        // Command files whose read failure has been logged, so a file that stays unreadable
        // is logged once instead of on every poll.
        private static readonly HashSet<string> UnreadableCommandFiles = new();

        // Command files that are answered but could not be deleted; skipped from then on.
        private static readonly HashSet<string> RejectedCommandFiles = new();

        static EditorCommandWatcher()
        {
            Directory.CreateDirectory(CommandDir);
            Directory.CreateDirectory(ResultDir);

            try
            {
                _watcher = new FileSystemWatcher(CommandDir, "*.json")
                {
                    NotifyFilter = NotifyFilters.FileName | NotifyFilters.LastWrite,
                    EnableRaisingEvents = true
                };

                _watcher.Created += OnCommandFileChanged;
                _watcher.Changed += OnCommandFileChanged;
                // ucom writes command files to a temporary name and renames them into place.
                _watcher.Renamed += OnCommandFileChanged;
            }
            catch (Exception e)
            {
                Debug.LogError($"[Ucom] Failed to initialize command watcher: {e.Message}");
            }

            // Commands written before the editor loaded, then leftovers from earlier runs.
            CheckForCommands();
            CleanupOldFiles();

            // Polling also picks up commands while Unity is in the background.
            EditorApplication.update += PollForCommands;
            _lastPollTime = EditorApplication.timeSinceStartup;
        }

        private static void OnCommandFileChanged(object sender, FileSystemEventArgs e)
        {
            // Processed on the main thread.
            if (e.FullPath.EndsWith(".json", StringComparison.OrdinalIgnoreCase))
                EditorApplication.delayCall += () => ProcessCommandFile(e.FullPath);
        }

        private static void PollForCommands()
        {
            var now = EditorApplication.timeSinceStartup;
            if (now - _lastPollTime < PollIntervalSeconds)
                return;

            _lastPollTime = now;
            CheckForCommands();
        }

        private static void CheckForCommands()
        {
            if (!Directory.Exists(CommandDir))
                return;

            foreach (var file in Directory.GetFiles(CommandDir, "*.json"))
                ProcessCommandFile(file);
        }

        private static void CleanupOldFiles()
        {
            // Removes leftovers older than one hour: commands that were never processed,
            // results and build logs that ucom stopped waiting for (e.g. after Ctrl+C), and
            // temporary files from interrupted writes.
            var cutoff = DateTime.UtcNow.AddHours(-1);
            var extensions = new[] { ".json", ".log", ".tmp" };

            foreach (var dir in new[] { CommandDir, ResultDir })
            {
                try
                {
                    if (!Directory.Exists(dir))
                        continue;

                    foreach (var file in Directory.GetFiles(dir))
                    {
                        if (extensions.Contains(Path.GetExtension(file), StringComparer.OrdinalIgnoreCase)
                            && File.GetLastWriteTimeUtc(file) < cutoff)
                            File.Delete(file);
                    }
                }
                catch (Exception e)
                {
                    Debug.LogError($"[Ucom] Failed to clean up old files in {dir}: {e.Message}");
                }
            }
        }

        private static void ProcessCommandFile(string commandFilePath)
        {
            if (RejectedCommandFiles.Contains(commandFilePath) || !File.Exists(commandFilePath))
                return;

            string json;
            try
            {
                json = File.ReadAllText(commandFilePath);
                UnreadableCommandFiles.Remove(commandFilePath);
            }
            catch (Exception e)
            {
                // For example locked by another process; tried again on the next poll.
                if (UnreadableCommandFiles.Add(commandFilePath))
                    Debug.LogWarning($"[Ucom] Cannot read command file {commandFilePath}, will retry: {e.Message}");
                return;
            }

            CommandFile command = null;
            string parseError;
            try
            {
                command = JsonUtility.FromJson<CommandFile>(json);
                parseError = command == null ? "The file is empty." : null;
            }
            catch (Exception e) // JsonUtility throws ArgumentException for invalid JSON.
            {
                parseError = e.Message;
            }

            if (parseError != null)
            {
                // Removed rather than tried again: it would fail on every poll,
                // and ucom would wait for a result that never comes.
                Debug.LogError($"[Ucom] Invalid command file {commandFilePath}: {parseError}");
                if (!TryDeleteFile(commandFilePath))
                    RejectedCommandFiles.Add(commandFilePath);

                var uuid = UuidFromCommandFileName(commandFilePath);
                if (uuid != null)
                    WriteErrorResult(uuid, "INTERNAL_ERROR", $"The editor could not read the build command: {parseError}");
                return;
            }

            // A command that waits stays on disk and is tried again on the next poll. Keeping it in
            // memory instead would lose it in the domain reload that follows compilation.
            if (command.command == "build" && MustWait(command))
                return;

            // Removed before it runs, so it runs only once: a file that stays would run on every poll.
            if (!TryDeleteFile(commandFilePath))
            {
                RejectedCommandFiles.Add(commandFilePath);
                WriteErrorResult(command.uuid, "INTERNAL_ERROR", $"The editor could not remove the command file {commandFilePath}.");
                return;
            }

            if (command.command == "build")
                ProcessBuildCommand(command);
            else
                WriteErrorResult(command.uuid, "UNKNOWN_COMMAND", $"Unknown command: {command.command}");
        }

        /// <summary>
        /// Returns <c>true</c> if the command has to wait for play mode to exit or compilation to finish.
        /// </summary>
        private static bool MustWait(CommandFile cmd)
        {
            if (EditorApplication.isPlaying && cmd.force_play_mode_exit)
            {
                // Play mode exits at the end of the frame.
                EditorApplication.isPlaying = false;
                return true;
            }

            return EditorApplication.isCompiling && cmd.wait_for_compilation;
        }

        private static void ProcessBuildCommand(CommandFile cmd)
        {
            // Always answer with a result: ucom waits until one arrives while the editor is running.
            try
            {
                ValidateAndBuild(cmd);
            }
            catch (Exception e)
            {
                Debug.LogException(e);
                WriteErrorResult(cmd.uuid, "INTERNAL_ERROR", $"Unexpected error while processing the build command: {e.Message}");
            }
        }

        private static void ValidateAndBuild(CommandFile cmd)
        {
            // Commands that may wait for these do not get here (MustWait).
            if (EditorApplication.isPlaying)
            {
                WriteErrorResult(cmd.uuid,
                    "IN_PLAY_MODE",
                    "Unity editor is in Play Mode. Use --force-editor-build to exit play mode.");
                return;
            }

            if (EditorApplication.isCompiling)
            {
                WriteErrorResult(cmd.uuid,
                    "COMPILING",
                    "Unity editor is compiling. Use --force-editor-build to wait for compilation.");
                return;
            }

            if (!Enum.TryParse(cmd.platform, out BuildTarget target))
            {
                WriteErrorResult(cmd.uuid, "INVALID_PLATFORM", $"Invalid build platform: {cmd.platform}");
                return;
            }

            var active = EditorUserBuildSettings.activeBuildTarget;
            if (target != active && !cmd.force_platform_switch)
            {
                WriteErrorResult(cmd.uuid,
                    "PLATFORM_MISMATCH",
                    $"Editor is in {active} mode, but build target is {target}. Use --force-editor-build to switch platforms.");
                return;
            }

            ExecuteBuild(cmd, target);
        }

        private static void ExecuteBuild(CommandFile cmd, BuildTarget target)
        {
            // Everything the editor logs from here on (platform switch, compilation, build)
            // is copied to the log file that ucom asked for.
            var hasConsoleLog = TryGetConsoleLogEnd(out var consoleLogPath, out var logStart);

            var result = new ResultFile
            {
                uuid = cmd.uuid,
                original_platform = EditorUserBuildSettings.activeBuildTarget.ToString(),
                editor_log_path = consoleLogPath
            };

            // The "[Builder]" lines are appended to the log file directly instead of being read back
            // from the editor log: the editor may write error-level messages to its log only after
            // the build has returned, which is after the log has been copied.
            var builderLines = new StringBuilder();
            try
            {
                SwitchPlatformAndBuild(cmd, target, result, builderLines);
            }
            catch (Exception e)
            {
                SetFailed(result, builderLines, "BUILD_FAILED", $"Build exception: {e.Message}", e.ToString());
            }

            // ucom reads the log only after the result has been written, so it is written in place.
            var lines = builderLines.ToString();
            result.log_written = hasConsoleLog && TryCopyLogSince(consoleLogPath, logStart, lines, cmd.log_path);

            // Logged after the copy, so the lines appear in the log file only once.
            if (lines.Length > 0)
                UnityBuilder.Log(lines.TrimEnd(), result.status == "success" ? LogType.Log : LogType.Error);

            WriteResult(result);
        }

        /// <summary>
        /// Switches the platform if needed and builds the player, filling in <paramref name="result"/>.
        /// </summary>
        /// <param name="builderLines">Receives the "[Builder]" lines for the log: errors and the build report.</param>
        private static void SwitchPlatformAndBuild(CommandFile cmd,
            BuildTarget target,
            ResultFile result,
            StringBuilder builderLines)
        {
            if (target != EditorUserBuildSettings.activeBuildTarget)
            {
                var switchStart = EditorApplication.timeSinceStartup;
                if (!TrySwitchPlatform(target, out var switchError))
                {
                    SetFailed(result, builderLines, "PLATFORM_SWITCH_FAILED", switchError);
                    return;
                }

                result.platform_switched = true;
                result.switched_to = target.ToString();
                result.platform_switch_time_seconds = (float)(EditorApplication.timeSinceStartup - switchStart);
            }

            if (!UnityBuilder.TryGetBuildLocationPath(cmd.output_path, Application.productName, target, out var path))
            {
                SetFailed(result, builderLines, "BUILD_FAILED", $"Build target '{target}' is not supported for building.");
                return;
            }

            // Exceptions are handled by the caller, which still copies the log.
            var buildStart = EditorApplication.timeSinceStartup;
            var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
            {
                scenes = UnityBuilder.GetActiveScenes(),
                locationPathName = path,
                target = target,
                options = (BuildOptions)cmd.build_options
            });

            var summary = report.summary;
            var succeeded = summary.result == BuildResult.Succeeded;
            result.build_time_seconds = (float)(EditorApplication.timeSinceStartup - buildStart);
            result.output_path = path;
            result.build_result = summary.result.ToString();
            result.platform = summary.platform.ToString();
            result.total_size = summary.totalSize;
            result.total_errors = summary.totalErrors;
            result.total_warnings = summary.totalWarnings;
            result.status = succeeded ? "success" : "failed";
            result.error_code = succeeded ? null : "BUILD_FAILED";
            result.message = succeeded ? "Build completed successfully" : "Build failed. Check Unity console for errors.";

            // The log file is left out: ucom prints its path once it has moved the log into place.
            builderLines.Append(UnityBuilder.FormatBuildResult(report, null));
        }

        private static bool TrySwitchPlatform(BuildTarget target, out string error)
        {
            var group = BuildPipeline.GetBuildTargetGroup(target);
            if (!BuildPipeline.IsBuildTargetSupported(group, target))
                error = $"Build target '{target}' is not supported or installed.";
            else if (!EditorUserBuildSettings.SwitchActiveBuildTarget(group, target))
                error = $"Failed to switch from {EditorUserBuildSettings.activeBuildTarget} to {target}. "
                        + "This may happen if the user cancelled the operation.";
            else
                error = null;

            return error == null;
        }

        /// <summary>
        /// Marks the result as failed and adds an error line for the build log.
        /// </summary>
        /// <param name="detail">The text for the log line, if it differs from <paramref name="message"/>.</param>
        private static void SetFailed(ResultFile result,
            StringBuilder builderLines,
            string errorCode,
            string message,
            string detail = null)
        {
            result.status = "failed";
            result.error_code = errorCode;
            result.message = message;
            builderLines.AppendLine($"[Builder] Error: {detail ?? message}");
        }

        /// <summary>
        /// Gets the path and current length of the log file the editor writes to.
        /// </summary>
        /// <returns><c>false</c> if the editor does not write a log file, or its length is unknown.</returns>
        private static bool TryGetConsoleLogEnd(out string path, out long length)
        {
            path = null;
            length = 0;
            try
            {
                var info = new FileInfo(Application.consoleLogPath);
                if (!info.Exists)
                    return false;

                path = info.FullName;
                length = info.Length;
                return true;
            }
            catch (Exception)
            {
                // No log file (e.g. -logFile -), or an invalid path.
                return false;
            }
        }

        /// <summary>
        /// Copies the part of <paramref name="sourcePath"/> after <paramref name="startOffset"/>
        /// to <paramref name="destinationPath"/>, followed by <paramref name="trailer"/>.
        /// </summary>
        private static bool TryCopyLogSince(string sourcePath, long startOffset, string trailer, string destinationPath)
        {
            if (string.IsNullOrEmpty(destinationPath))
                return false;

            try
            {
                // The editor keeps the log open for writing, so it has to be shared.
                using var source = new FileStream(sourcePath,
                    FileMode.Open,
                    FileAccess.Read,
                    FileShare.ReadWrite | FileShare.Delete);
                using var destination = new FileStream(destinationPath, FileMode.Create, FileAccess.Write);

                // Start from the beginning if the log was truncated or replaced during the build.
                source.Seek(startOffset <= source.Length ? startOffset : 0, SeekOrigin.Begin);
                source.CopyTo(destination);

                var trailerBytes = Encoding.UTF8.GetBytes(trailer);
                destination.Write(trailerBytes, 0, trailerBytes.Length);
                return true;
            }
            catch (Exception e)
            {
                Debug.LogWarning($"[Ucom] Failed to write build log to {destinationPath}: {e.Message}");
                return false;
            }
        }

        /// <summary>
        /// Returns the uuid in a command file name (<c>build-&lt;uuid&gt;.json</c>), or <c>null</c>.
        /// </summary>
        private static string UuidFromCommandFileName(string commandFilePath)
        {
            var name = Path.GetFileNameWithoutExtension(commandFilePath);
            if (!name.StartsWith(CommandFilePrefix, StringComparison.Ordinal))
                return null;

            var uuid = name.Substring(CommandFilePrefix.Length);
            return Guid.TryParse(uuid, out _) ? uuid : null;
        }

        private static bool TryDeleteFile(string path)
        {
            try
            {
                File.Delete(path);
                return true;
            }
            catch (Exception e)
            {
                Debug.LogError($"[Ucom] Failed to delete {path}: {e.Message}");
                return false;
            }
        }

        private static void WriteErrorResult(string uuid, string errorCode, string message) =>
            WriteResult(new ResultFile { uuid = uuid, status = "error", error_code = errorCode, message = message });

        private static void WriteResult(ResultFile result)
        {
            try
            {
                var json = JsonUtility.ToJson(result, true);
                var resultPath = Path.Combine(ResultDir, $"{CommandFilePrefix}{result.uuid}.json");
                var tempPath = Path.Combine(ResultDir, $"{CommandFilePrefix}{result.uuid}.tmp");

                // Write to a temporary file and rename it into place, so ucom never
                // reads a partially written result.
                File.WriteAllText(tempPath, json);
                if (File.Exists(resultPath))
                    File.Delete(resultPath);
                File.Move(tempPath, resultPath);
            }
            catch (Exception e)
            {
                Debug.LogError($"[Ucom] Failed to write result: {e.Message}");
            }
        }
    }

    // The command ucom sends (EditorCommand in build_cmd/editor.rs). Its timestamp and
    // development_build fields are not used here; JsonUtility skips fields this class does not have.
    [Serializable]
    [SuppressMessage("ReSharper", "InconsistentNaming")]
    public class CommandFile
    {
        public string command;
        public string uuid;
        public string platform;
        public string output_path;
        public string log_path;
        public int build_options;
        public bool force_platform_switch;
        public bool force_play_mode_exit;
        public bool wait_for_compilation;
    }

    // The result ucom reads (EditorBuildResult in build_cmd/editor.rs).
    [Serializable]
    [SuppressMessage("ReSharper", "InconsistentNaming")]
    public class ResultFile
    {
        public string uuid;
        public string status;
        public string message;
        public string error_code;
        public bool platform_switched;
        public string original_platform;
        public string switched_to;
        public float build_time_seconds;
        public float platform_switch_time_seconds;
        public string output_path;
        public string build_result;
        public string platform;
        public ulong total_size;
        public int total_errors;
        public int total_warnings;
        public bool log_written;
        public string editor_log_path;
    }
}
#else
#error "Ucom command line building is not supported for this version of Unity; version 2021.3 or newer is required."
#endif // UNITY_2021_3_OR_NEWER
