/*
 * This file is part of the ucom command line tool (https://github.com/jakkovanhunen/ucom).
 *
 * Copyright 2022-2026 Jakko van Hunen
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#if UNITY_2018_3_OR_NEWER
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

// ReSharper disable SwitchStatementHandlesSomeKnownEnumValuesWithDefault
// ReSharper disable once ConvertSwitchStatementToSwitchExpression
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
        /// <summary>
        /// The output path for the build.
        /// </summary>
        private const string BuildOutputArg = "--ucom-build-output";

        /// <summary>
        /// The <see cref="BuildTarget"/>.
        /// </summary>
        private const string BuildTargetArg = "--ucom-build-target";

        /// <summary>
        /// Combined <see cref="BuildOptions"/> are passed as an int.
        /// </summary>
        private const string BuildOptionsArg = "--ucom-build-options";

        /// <summary>
        /// Custom arguments passed to the build scripts.
        /// </summary>
        private const string PreBuildArgs = "--ucom-pre-build-args";

        /// <summary>
        /// Scripting Define Symbols to add to the target Player Settings.
        /// </summary>
        private const string AddDefinesArg = "--ucom-add-defines";

        /// <summary>
        /// This method is called by ucom to build the project.
        /// </summary>
        [UsedImplicitly]
        public static void Build()
        {
            var args = Environment.GetCommandLineArgs();

            var invalidArgs = false;

            // Get the output directory.
            if (!args.TryGetArgValue(BuildOutputArg, out var outputDirectory))
            {
                // No output path specified.
                Log($"[Builder] Error: Output path '{BuildOutputArg} <path>' not specified.", LogType.Error);
                invalidArgs = true;
            }

            // Get the build target.
            if (!args.TryGetArgValue(BuildTargetArg, out var argValue))
            {
                // No build target specified.
                Log($"[Builder] Error: Build target '{BuildTargetArg} <target>' not specified.", LogType.Error);
                invalidArgs = true;
            }
            else if (!Enum.TryParse(argValue, out BuildTarget target))
            {
                // Nonexistent build target value specified.
                Log($"[Builder] Error: Invalid build target: {BuildTargetArg} {argValue}", LogType.Error);
                invalidArgs = true;
            }
            else if (target != EditorUserBuildSettings.activeBuildTarget)
            {
                // The desired build target does not match the active build target. Bail out.
                // ucom attempts to start Unity with the desired build target, however, this is not always possible
                // because it might not be installed on the machine.
                Log(BuildPipeline.IsBuildTargetSupported(BuildPipeline.GetBuildTargetGroup(target), target)
                        ? $"[Builder] Error: Build target '{target}' does not match active build target '{EditorUserBuildSettings.activeBuildTarget}'"
                        : $"[Builder] Error: Build target '{target}' is not supported or installed.",
                    LogType.Error
                );

                invalidArgs = true;
            }

            var options = BuildOptions.None;

            if (args.TryGetArgValue(BuildOptionsArg, out var boValue))
            {
                if (int.TryParse(boValue, out var bo))
                {
                    options = (BuildOptions)bo;
                }
                else
                {
                    // Nonexistent build target value specified.
                    Log($"[Builder] Error: Invalid build options: {BuildOptionsArg} {argValue}", LogType.Error);
                    invalidArgs = true;
                }
            }

            string[] extraScriptingDefines = null;
            if (args.TryGetArgValue(AddDefinesArg, out var defines))
                extraScriptingDefines = defines.Split(';');

            if (!args.TryGetArgValue(PreBuildArgs, out var preBuildArgs))
                preBuildArgs = "";

            var buildFailed = !TryGetBuildLocationPath(outputDirectory,
                Application.productName,
                EditorUserBuildSettings.activeBuildTarget,
                out var locationPathName
            );

            buildFailed |= invalidArgs
                           || !Build(locationPathName, GetActiveScenes(), options, extraScriptingDefines, preBuildArgs);

            if (Array.IndexOf(args, "-quit") != -1)
            {
                // Force quit the editor.
                EditorApplication.Exit(buildFailed ? 1 : 0);
            }
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
            Log(FormatBuildResult(report.summary, logFile), succeeded ? LogType.Log : LogType.Error);
            return succeeded;
        }

        /// <summary>
        /// Formats the build result line followed by the "[Builder] Build Report" block that ucom
        /// reads from the log, ending with an empty line.
        /// </summary>
        /// <param name="summary">The summary of the finished build.</param>
        /// <param name="logFile">The log file to mention in the report, or <c>null</c> to leave it out.</param>
        public static string FormatBuildResult(BuildSummary summary, string logFile)
        {
            var sb = new StringBuilder();

            var buildStartTime = summary.buildStartedAt.ToLocalTime().ToString(CultureInfo.InvariantCulture);
            sb.AppendLine(summary.result == BuildResult.Succeeded
                    ? "[Builder] Build succeeded."
                    : "[Builder] Build failed."
                )
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
            sb.AppendLine();
            return sb.ToString();
        }

        /// <summary>
        /// Tries to get the full path of build location.
        /// </summary>
        /// <param name="buildRootPath">The parent directory where the application will be built.</param>
        /// <param name="appName">The name of the application.</param>
        /// <param name="buildTarget">The <see cref="BuildTarget"/>.</param>
        /// <param name="fullOutputPath">The full path of the build location.</param>
        /// <returns>True if the build target is supported; False otherwise.</returns>
        public static bool TryGetBuildLocationPath(string buildRootPath,
            string appName,
            BuildTarget buildTarget,
            out string fullOutputPath)
        {
            if (TryGetAppFileName(appName, buildTarget, out var appFileName))
            {
                fullOutputPath = Path.Combine(buildRootPath, appFileName);
                return true;
            }

            fullOutputPath = null;
            return false;
        }

        /// <summary>
        /// Tries to get the file name of the application.
        /// </summary>
        /// <param name="appName">The name of the application.</param>
        /// <param name="buildTarget">The <see cref="BuildTarget"/>.</param>
        /// <param name="fileName">The file name.</param>
        /// <returns>True if the build target is supported; False otherwise.</returns>
        public static bool TryGetAppFileName(string appName, BuildTarget buildTarget, out string fileName)
        {
            fileName = string.Join("_", appName.Split(Path.GetInvalidFileNameChars()));

            switch (buildTarget)
            {
                case BuildTarget.iOS:
                case BuildTarget.WebGL:
                    // Build output is a directory.
                    fileName = string.Join("_", fileName.Split(Path.GetInvalidPathChars()))
                        .Replace(" ", "_");
                    break;
                case BuildTarget.StandaloneWindows:
                case BuildTarget.StandaloneWindows64:
                    fileName = $"{fileName}.exe";
                    break;
                case BuildTarget.StandaloneOSX:
                    fileName = $"{fileName}.app";
                    break;
                case BuildTarget.StandaloneLinux64:
                    fileName = $"{fileName}.x86_64";
                    break;
                case BuildTarget.Android:
                    fileName = $"{fileName}.apk";
                    break;
                default:
                    Log($"[Builder] Error: '{buildTarget}' build target not supported.", LogType.Error);
                    fileName = null;
                    return false;
            }

            return true;
        }

        /// <summary>
        /// Returns the paths of the active scenes in the build settings.
        /// </summary>
        /// <returns>The paths of the active scenes in the build settings.</returns>
        [JetBrains.Annotations.NotNull]
        public static string[] GetActiveScenes()
        {
            return EditorBuildSettings
                .scenes
                .Where(scene => scene.enabled)
                .Select(scene => scene.path)
                .ToArray();
        }

        /// <summary>
        /// Tries to get the default build output path for the current build target.
        /// The path is in the Builds directory relative to the project root.
        /// </summary>
        /// <param name="outputPath">The output path for the current build target.</param>
        /// <param name="outputTypeType">The <see cref="OutputType"/> (default: <see cref="OutputType.Release"/>).</param>
        /// <returns>True if current build target is supported; False otherwise.</returns>
        public static bool TryGetDefaultBuildOutputPath(out string outputPath,
            OutputType outputTypeType = OutputType.Release)
        {
            outputPath = null;

            if (!TryGetBuildTargetDirName(EditorUserBuildSettings.activeBuildTarget, out var target))
                return false;

            outputPath = Path.Combine(GetBuildsDirectoryPath(), outputTypeType.ToString(), target);
            return true;
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
        /// <param name="message">The message.</param>
        /// <param name="logType">The <see cref="LogType"/>.</param>
        public static void Log(string message, LogType logType = LogType.Log)
        {
#if UNITY_2019_1_OR_NEWER
            Debug.LogFormat(logType, LogOption.NoStacktrace, null, message);
#else // UNITY_2019_1_OR_NEWER
            switch (logType)
            {
                case LogType.Error:
                    Debug.LogError(message);
                    break;
                case LogType.Assert:
                    Debug.LogAssertion(message);
                    break;
                case LogType.Warning:
                    Debug.LogWarning(message);
                    break;
                case LogType.Log:
                    Debug.Log(message);
                    break;
                case LogType.Exception:
                    Debug.LogException(new Exception(message));
                    break;
                default:
                    Debug.Log(message);
                    break;
            }
#endif // UNITY_2019_1_OR_NEWER
        }

        /// <summary>
        /// Tries to get the build target directory name.
        /// </summary>
        /// <param name="buildTarget">The <see cref="BuildTarget"/>.</param>
        /// <param name="dirName">The directory name for the specified build target.</param>
        /// <returns>True if the specified build target is supported; False otherwise.</returns>
        private static bool TryGetBuildTargetDirName(BuildTarget buildTarget, out string dirName)
        {
            switch (buildTarget)
            {
                case BuildTarget.StandaloneWindows:
                    dirName = "Win";
                    break;
                case BuildTarget.StandaloneWindows64:
                    dirName = "Win64";
                    break;
                case BuildTarget.StandaloneOSX:
                    dirName = "OSXUniversal";
                    break;
                case BuildTarget.StandaloneLinux64:
                    dirName = "Linux64";
                    break;
                case BuildTarget.Android:
                    dirName = "Android";
                    break;
                case BuildTarget.iOS:
                    dirName = "iOS";
                    break;
                case BuildTarget.WebGL:
                    dirName = "WebGL";
                    break;
                default:
                    dirName = null; // Unsupported
                    break;
            }

            return dirName != null;
        }

        /// <summary>
        /// Tries to get the value of the specified argument (the next argument in the array).
        /// </summary>
        /// <param name="source">The command line arguments.</param>
        /// <param name="arg">The argument.</param>
        /// <param name="value">The value of the argument.</param>
        /// <returns>True if the argument was found; False otherwise.</returns>
        public static bool TryGetArgValue(this string[] source, string arg, out string value)
        {
            var index = Array.IndexOf(source, arg);
            if (index < 0 || index == source.Length - 1)
            {
                value = null;
                return false;
            }

            value = source[index + 1];
            return true;
        }

        private static bool RunPreProcessBuildMethod(string arg)
        {
            var methods = AppDomain.CurrentDomain
                .GetAssemblies()
                .SelectMany(assembly => assembly.GetTypes())
                .SelectMany(GetPreProcessBuildMethods)
                .ToList();
            if (!methods.Any())
                return true;

            try
            {
                var method = methods.Single();
                return InvokeMethod(method, arg);
            }
            catch (Exception)
            {
                var m = string.Join(", ", methods.Select(m => $"{m.ReflectedType?.FullName}.{m.Name}"));
                Log(
                    $"[Builder] Multiple {nameof(UcomPreProcessBuildAttribute)} methods found, there should only be one: {m}",
                    LogType.Error
                );
                return false;
            }
        }

        private static IEnumerable<MethodInfo> GetPreProcessBuildMethods(Type type)
        {
            return type.GetMethods(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static)
                .Where(m => m.GetCustomAttributes(typeof(UcomPreProcessBuildAttribute), false).Any());
        }

        private static bool InvokeMethod(MethodBase method, string args)
        {
            var parameters = method.GetParameters();
            if (parameters.Length == 1 && parameters[0].ParameterType == typeof(string))
            {
                method.Invoke(null, new object[] { args });
                return true;
            }

            Log(
                $"[Builder] Invalid method signature for UcomPreProcessBuildAttribute: {method.ReflectedType?.FullName}.{method.Name}",
                LogType.Error
            );
            return false;
        }
    }

    /// <summary>
    /// This is mainly a flag used in the output directory, it doesn't dictate the physical type of build.
    /// The default value for builds is <see cref="OutputType.Release"/>.
    /// </summary>
    public enum OutputType
    {
        /// <summary>
        /// Build will be outputted to the <c>Builds/Release</c> directory.
        /// </summary>
        Release,

        /// <summary>
        /// Build will be outputted to the <c>Builds/Debug</c> directory.
        /// </summary>
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

#if UNITY_EDITOR
    /// <summary>
    /// Editor script that watches for ucom command files and executes them.
    /// This enables ucom to communicate with an already-running Unity editor.
    /// </summary>
    [InitializeOnLoad]
    public class EditorCommandWatcher
    {
        private static FileSystemWatcher _watcher;
        private static readonly string CommandDir;
        private static readonly string ResultDir;
        private static double _lastPollTime;
        private const double PollIntervalSeconds = 1.0;

        // Deferred command handling for play mode exit and compilation waiting
        private static CommandFile _deferredCommand;
        private static bool _waitingForPlayModeExit;
        private static bool _waitingForCompilation;

        static EditorCommandWatcher()
        {
            CommandDir = Path.Combine(Application.dataPath, "..", "Temp", "ucom-commands");
            ResultDir = Path.Combine(Application.dataPath, "..", "Temp", "ucom-results");

            Directory.CreateDirectory(CommandDir);
            Directory.CreateDirectory(ResultDir);

            InitializeWatcher();

            // Also check for existing commands on load
            CheckForCommands();

            // Cleanup old command files (older than 1 hour)
            CleanupOldFiles();

            // Register polling callback that works even when Unity is in background
            EditorApplication.update += PollForCommands;
            _lastPollTime = EditorApplication.timeSinceStartup;
        }

        private static void InitializeWatcher()
        {
            try
            {
                _watcher = new FileSystemWatcher(CommandDir, "*.json")
                {
                    NotifyFilter = NotifyFilters.FileName | NotifyFilters.LastWrite,
                    EnableRaisingEvents = true
                };

                _watcher.Created += OnCommandFileCreated;
                _watcher.Changed += OnCommandFileCreated;
                // ucom writes command files to a temporary name and renames them into place.
                _watcher.Renamed += OnCommandFileCreated;
            }
            catch (Exception e)
            {
                Debug.LogError($"[Ucom] Failed to initialize command watcher: {e.Message}");
            }
        }

        private static void OnCommandFileCreated(object sender, FileSystemEventArgs e)
        {
            if (!e.FullPath.EndsWith(".json", StringComparison.OrdinalIgnoreCase))
                return;

            // Schedule processing on main thread
            EditorApplication.delayCall += () => ProcessCommandFile(e.FullPath);
        }

        private static void PollForCommands()
        {
            // Check for new commands every N seconds, even when Unity is in background
            var currentTime = EditorApplication.timeSinceStartup;
            if (currentTime - _lastPollTime < PollIntervalSeconds)
                return;

            _lastPollTime = currentTime;

            // Process deferred command if conditions are met
            if (_deferredCommand != null)
            {
                if (_waitingForPlayModeExit)
                {
                    if (!EditorApplication.isPlaying)
                    {
                        _waitingForPlayModeExit = false;
                        ProcessBuildCommand(_deferredCommand);
                        _deferredCommand = null;
                    }

                    return; // Keep waiting
                }

                if (_waitingForCompilation)
                {
                    if (!EditorApplication.isCompiling)
                    {
                        _waitingForCompilation = false;
                        ProcessBuildCommand(_deferredCommand);
                        _deferredCommand = null;
                    }

                    return; // Keep waiting
                }
            }

            // Check for new commands
            CheckForCommands();
        }

        private static void CheckForCommands()
        {
            if (!Directory.Exists(CommandDir))
                return;

            foreach (var file in Directory.GetFiles(CommandDir, "*.json"))
            {
                ProcessCommandFile(file);
            }
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
                        if (!extensions.Any(ext => file.EndsWith(ext, StringComparison.OrdinalIgnoreCase)))
                            continue;

                        if (File.GetLastWriteTimeUtc(file) < cutoff)
                        {
                            File.Delete(file);
                        }
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
            if (!File.Exists(commandFilePath))
                return;

            try
            {
                var json = File.ReadAllText(commandFilePath);
                var command = JsonUtility.FromJson<CommandFile>(json);

                switch (command.command)
                {
                    case "build":
                        ProcessBuildCommand(command);
                        break;
                    default:
                        WriteErrorResult(command.uuid,
                            "UNKNOWN_COMMAND",
                            $"Unknown command: {command.command}"
                        );
                        break;
                }

                // Cleanup command file
                File.Delete(commandFilePath);
            }
            catch (Exception e)
            {
                Debug.LogError($"[Ucom] Failed to process command: {e.Message}");
            }
        }

        private static void ProcessBuildCommand(CommandFile cmd)
        {
            // Always answer with a result: ucom waits until one arrives while the editor is running,
            // and an exception here would also leave the command file to be processed again.
            try
            {
                ValidateAndBuild(cmd);
            }
            catch (Exception e)
            {
                Debug.LogException(e);
                WriteErrorResult(cmd.uuid,
                    "INTERNAL_ERROR",
                    $"Unexpected error while processing the build command: {e.Message}"
                );
            }
        }

        private static void ValidateAndBuild(CommandFile cmd)
        {
            // Validate: Check if in play mode
            if (EditorApplication.isPlaying)
            {
                if (!cmd.force_play_mode_exit)
                {
                    WriteErrorResult(cmd.uuid,
                        "IN_PLAY_MODE",
                        "Unity editor is in Play Mode. Use --force-editor-build to exit play mode."
                    );
                    return;
                }

                // Exit play mode and defer the command
                EditorApplication.isPlaying = false;
                _deferredCommand = cmd;
                _waitingForPlayModeExit = true;
                return; // Will be processed in next poll cycle after play mode exits
            }

            // Validate: Check if compiling
            if (EditorApplication.isCompiling)
            {
                if (!cmd.force_platform_switch)
                {
                    WriteErrorResult(cmd.uuid,
                        "COMPILING",
                        "Unity editor is compiling. Use --force-editor-build to wait for compilation."
                    );
                    return;
                }

                // Defer the command until compilation finishes
                _deferredCommand = cmd;
                _waitingForCompilation = true;
                return; // Will be processed in next poll cycle after compilation finishes
            }

            // Validate: Check platform match
            if (!Enum.TryParse(cmd.platform, out BuildTarget target))
            {
                WriteErrorResult(cmd.uuid,
                    "INVALID_PLATFORM",
                    $"Invalid build platform: {cmd.platform}"
                );
                return;
            }

            var needsPlatformSwitch = target != EditorUserBuildSettings.activeBuildTarget;

            if (needsPlatformSwitch && !cmd.force_platform_switch)
            {
                // Platform mismatch without permission to switch
                WriteErrorResult(cmd.uuid,
                    "PLATFORM_MISMATCH",
                    $"Editor is in {EditorUserBuildSettings.activeBuildTarget} mode, " +
                    $"but build target is {target}. Use --force-editor-build to switch platforms."
                );
                return;
            }

            // Execute build (with platform switch if needed)
            ExecuteBuild(cmd, target, needsPlatformSwitch);
        }

        private static bool TrySwitchPlatform(BuildTarget target, out string errorMessage, out float switchTime)
        {
            switchTime = 0f;

            if (target == EditorUserBuildSettings.activeBuildTarget)
            {
                errorMessage = null;
                return true;
            }

            var targetGroup = BuildPipeline.GetBuildTargetGroup(target);

            // Check if target is supported/installed
            if (!BuildPipeline.IsBuildTargetSupported(targetGroup, target))
            {
                errorMessage = $"Build target '{target}' is not supported or installed.";
                return false;
            }

            var startTime = EditorApplication.timeSinceStartup;

            var switched = EditorUserBuildSettings.SwitchActiveBuildTarget(targetGroup, target);

            if (!switched)
            {
                errorMessage = $"Failed to switch from {EditorUserBuildSettings.activeBuildTarget} to {target}. " +
                               "This may happen if the user cancelled the operation.";
                return false;
            }

            switchTime = (float)(EditorApplication.timeSinceStartup - startTime);
            errorMessage = null;
            return true;
        }

        private static void ExecuteBuild(CommandFile cmd, BuildTarget target, bool needsSwitch)
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
                SwitchPlatformAndBuild(cmd, target, needsSwitch, result, builderLines);
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
            bool needsSwitch,
            ResultFile result,
            StringBuilder builderLines)
        {
            // Handle platform switch if needed
            if (needsSwitch)
            {
                if (!TrySwitchPlatform(target, out var switchError, out var switchTime))
                {
                    SetFailed(result, builderLines, "PLATFORM_SWITCH_FAILED", switchError);
                    return;
                }

                result.platform_switched = true;
                result.switched_to = target.ToString();
                result.platform_switch_time_seconds = switchTime;
            }

            // Get the full build location path (directory + filename) using UnityBuilder
            if (!UnityBuilder.TryGetBuildLocationPath(cmd.output_path,
                    Application.productName,
                    target,
                    out var locationPathName
                ))
            {
                SetFailed(result, builderLines, "BUILD_FAILED", $"Build target '{target}' is not supported for building.");
                return;
            }

            var buildStartTime = EditorApplication.timeSinceStartup;

            // Build the player using BuildPipeline to get BuildReport.
            // Exceptions are handled by the caller, which still copies the log.
            var report = BuildPipeline.BuildPlayer(new BuildPlayerOptions
            {
                scenes = UnityBuilder.GetActiveScenes(),
                locationPathName = locationPathName,
                target = target,
                options = (BuildOptions)cmd.build_options
            });

            result.build_time_seconds =
                (float)(EditorApplication.timeSinceStartup - buildStartTime);
            result.output_path = locationPathName;

            // Extract build report data
            var summary = report.summary;
            result.build_result = summary.result.ToString();
            result.platform = summary.platform.ToString();
            result.total_size = summary.totalSize;
            result.total_errors = summary.totalErrors;
            result.total_warnings = summary.totalWarnings;

            // The log file is left out: ucom prints its path once it has moved the log into place.
            builderLines.Append(UnityBuilder.FormatBuildResult(summary, null));

            if (summary.result == BuildResult.Succeeded)
            {
                result.status = "success";
                result.message = "Build completed successfully";
            }
            else
            {
                result.status = "failed";
                result.error_code = "BUILD_FAILED";
                result.message = "Build failed. Check Unity console for errors.";
            }
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
#if UNITY_2019_1_OR_NEWER
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
#else
            return false;
#endif
        }

        /// <summary>
        /// Copies the part of <paramref name="sourcePath"/> after <paramref name="startOffset"/>
        /// to <paramref name="destinationPath"/>, followed by <paramref name="trailer"/>.
        /// </summary>
        /// <returns><c>true</c> if the log was copied; <c>false</c> otherwise.</returns>
        private static bool TryCopyLogSince(string sourcePath, long startOffset, string trailer, string destinationPath)
        {
            if (string.IsNullOrEmpty(destinationPath))
                return false;

            try
            {
                // The editor keeps the log open for writing, so it has to be shared.
                using (var source = new FileStream(sourcePath,
                           FileMode.Open,
                           FileAccess.Read,
                           FileShare.ReadWrite | FileShare.Delete
                       ))
                using (var destination = new FileStream(destinationPath, FileMode.Create, FileAccess.Write))
                {
                    // Start from the beginning if the log was truncated or replaced during the build.
                    source.Seek(startOffset <= source.Length ? startOffset : 0, SeekOrigin.Begin);
                    source.CopyTo(destination);

                    var trailerBytes = Encoding.UTF8.GetBytes(trailer);
                    destination.Write(trailerBytes, 0, trailerBytes.Length);
                }

                return true;
            }
            catch (Exception e)
            {
                Debug.LogWarning($"[Ucom] Failed to write build log to {destinationPath}: {e.Message}");
                return false;
            }
        }

        private static void WriteErrorResult(string uuid, string errorCode, string message)
        {
            var result = new ResultFile
            {
                uuid = uuid,
                status = "error",
                error_code = errorCode,
                message = message
            };

            WriteResult(result);
        }

        private static void WriteResult(ResultFile result)
        {
            try
            {
                var json = JsonUtility.ToJson(result, true);
                var resultPath = Path.Combine(ResultDir, $"build-{result.uuid}.json");
                var tempPath = Path.Combine(ResultDir, $"build-{result.uuid}.tmp");

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

    [Serializable]
    [SuppressMessage("ReSharper", "InconsistentNaming")]
    public class CommandFile
    {
        public string command;
        public string uuid;
        public string timestamp;
        public string platform;
        public string output_path;
        public string log_path;
        public int build_options;
        public bool development_build;
        public bool force_platform_switch;
        public bool force_play_mode_exit;
    }

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
#endif // UNITY_EDITOR
}
#else
#error "Ucom command line building is not supported for this version of Unity; version 2018.3 or newer is required."
#endif // UNITY_2018_3_OR_NEWER