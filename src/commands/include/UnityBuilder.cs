using System;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using JetBrains.Annotations;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEngine;

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
        /// This method is called by ucom to build the project.
        /// </summary>
        [UsedImplicitly]
        public static void Build()
        {
            var args = Environment.GetCommandLineArgs();

            var invalidArgs = false;

            // Get the output directory.
            if (!args.TryGetArgValue(BuildOutputArg, out string outputDirectory))
            {
                // No output path specified.
                Log($"[Builder] Error: Output path '{BuildOutputArg} <path>' not specified.", LogType.Error);
                invalidArgs = true;
            }

            // Get the build target.
            if (!args.TryGetArgValue(BuildTargetArg, out string argValue))
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

            if (args.TryGetArgValue(BuildOptionsArg, out string boValue))
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

            var buildFailed = invalidArgs || !Build(outputDirectory, GetActiveScenes(), options);

            if (Array.IndexOf(args, "-quit") != -1)
            {
                // Force quit the editor.
                EditorApplication.Exit(buildFailed ? 1 : 0);
            }
        }

        /// <summary>
        /// Builds the application for the <see cref="EditorUserBuildSettings.activeBuildTarget"/>.
        /// </summary>
        /// <param name="outputDirectory">The parent directory where the application will be built.</param>
        /// <param name="scenes">The scenes to include in the build</param>
        /// <param name="options">Building options. Multiple options can be combined together.</param>
        /// <returns><c>true</c> if the build succeeded; <c>false</c> otherwise.</returns>
        public static bool Build(string outputDirectory, string[] scenes, BuildOptions options)
        {
            if (scenes.Length == 0)
            {
                Log("[Builder] Error: no active scenes in Build Settings.", LogType.Error);
                return false;
            }

            if (!TryGetBuildLocationPath(outputDirectory,
                    Application.productName,
                    EditorUserBuildSettings.activeBuildTarget,
                    out var applicationPath))
            {
                return false;
            }

            var buildPlayerOptions = new BuildPlayerOptions
            {
                scenes = scenes,
                locationPathName = applicationPath,
                target = EditorUserBuildSettings.activeBuildTarget,
                options = options,
            };

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

            var summary = report.summary;

            var sb = new StringBuilder();
            sb.AppendLine("[Builder] Build Report")
              .AppendLine($"    Build result: {summary.result}")
              .AppendLine($"    Platform:     {summary.platform}")
              .AppendLine($"    Output path:  {summary.outputPath}")
              .AppendLine($"    Size:         {summary.totalSize / 1024 / 1024} MB")
              .AppendLine($"    Start time:   {summary.buildStartedAt.ToLocalTime().ToString(CultureInfo.InvariantCulture)}")
              .AppendLine($"    Total time:   {summary.totalTime}")
              .AppendLine($"    Errors:       {summary.totalErrors}")
              .AppendLine($"    Warnings:     {summary.totalWarnings}");

            if (Environment.GetCommandLineArgs().TryGetArgValue("-logFile", out string logFile))
            {
                sb.AppendLine($"    Log file:     {logFile}");
            }

            // End of build report.
            sb.AppendLine();

            switch (summary.result)
            {
                case BuildResult.Succeeded:
                    Log("[Builder] Build succeeded.", LogType.Log);
                    Log(sb.ToString(), LogType.Log);
                    return true;
                default:
                    Log("[Builder] Build failed.", LogType.Error);
                    Log(sb.ToString(), LogType.Error);
                    return false;
            }
        }

        /// <summary>
        /// Tries to get the full path of build location.
        /// </summary>
        /// <param name="outputDirectory">The parent directory where the application will be built.</param>
        /// <param name="appName">The name of the application.</param>
        /// <param name="buildTarget">The <see cref="BuildTarget"/>.</param>
        /// <param name="fullOutputPath">The full path of the build location.</param>
        /// <returns>True if the build target is supported; False otherwise.</returns>
        [PublicAPI]
        public static bool TryGetBuildLocationPath(string outputDirectory,
            string appName,
            BuildTarget buildTarget,
            out string fullOutputPath)
        {
            if (TryGetAppFileName(appName, buildTarget, out var appFileName))
            {
                fullOutputPath = Path.Combine(outputDirectory, appFileName);
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
        [PublicAPI]
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
        [PublicAPI, NotNull]
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
        /// <returns>True if current build target is supported; False otherwise.</returns>
        [PublicAPI]
        public static bool TryGetDefaultBuildOutputPath(out string outputPath)
        {
            outputPath = null;

            if (!TryGetBuildTargetDirName(EditorUserBuildSettings.activeBuildTarget, out var target))
                return false;

            var parent = new DirectoryInfo(Application.dataPath).Parent;
            if (parent == null)
                return false;

            outputPath = Path.Combine(parent.FullName, "Builds", target);
            return true;
        }


        /// <summary>
        /// Tries to get the build target directory name.
        /// </summary>
        /// <param name="buildTarget">The <see cref="BuildTarget"/>.</param>
        /// <param name="dirName">The directory name for the specified build target.</param>
        /// <returns>True if the specified build target is supported; False otherwise.</returns>
        private static bool TryGetBuildTargetDirName(BuildTarget buildTarget, out string dirName)
        {
            dirName = buildTarget switch
            {
                BuildTarget.StandaloneWindows => "Win",
                BuildTarget.StandaloneWindows64 => "Win64",
                BuildTarget.StandaloneOSX => "OSXUniversal",
                BuildTarget.StandaloneLinux64 => "Linux64",
                BuildTarget.Android => "Android",
                BuildTarget.iOS => "iOS",
                BuildTarget.WebGL => "WebGL",
                _ => null, // Unsupported
            };
            return dirName != null;
        }

        /// <summary>
        /// Tries to get the value of the specified argument.
        /// </summary>
        /// <param name="source">The command line arguments.</param>
        /// <param name="arg">The argument.</param>
        /// <param name="value">The value of the argument.</param>
        /// <returns>True if the argument was found; False otherwise.</returns>
        private static bool TryGetArgValue(this string[] source, string arg, out string value)
        {
            var index = Array.IndexOf(source, arg);
            if (index == -1 || index + 1 >= source.Length)
            {
                value = null;
                return false;
            }

            value = source[index + 1];
            return true;
        }

        /// <summary>
        /// Logs a message to the Unity console without the stack trace.
        /// </summary>
        /// <param name="message">The message.</param>
        /// <param name="logType">The <see cref="LogType"/>.</param>
        public static void Log(string message, LogType logType = LogType.Log)
        {
            Debug.LogFormat(logType, LogOption.NoStacktrace, null, message);
        }
    }
}