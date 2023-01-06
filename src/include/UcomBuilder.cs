using System;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using JetBrains.Annotations;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEngine;

namespace ucom
{
    /// <summary>
    /// Build companion script for ucom.<br/>
    /// - Lines after "[Builder] Build Report" until an empty line are printed to the console after the build finishes.<br/>
    /// - Lines starting with "[Builder] Error:" are printed to the console if the build fails.<br/>
    /// </summary>
    public static class UcomBuilder
    {
        private const string BuildOutputArg = "--ucom-build-output";
        private const string BuildTargetArg = "--ucom-build-target";

        /// <summary>
        /// This method is called by ucom to build the project.
        /// </summary>
        [UsedImplicitly]
        public static void Build()
        {
            string[] args = Environment.GetCommandLineArgs();

            bool invalidArgs = false;

            // Get the output directory.
            if (!args.TryGetArgValue(BuildOutputArg, out string outputDirectory))
            {
                // No output path specified.
                Log("[Builder] Error: Output path '--ucom-build-output <path>' not specified.", LogType.Error);
                invalidArgs = true;
            }

            // Get the build target.
            if (!args.TryGetArgValue(BuildTargetArg, out string argValue))
            {
                // No build target specified.
                Log("[Builder] Error: Build target '--ucom-build-target <target>' not specified.", LogType.Error);
                invalidArgs = true;
            }
            else if (!Enum.TryParse(argValue, out BuildTarget target))
            {
                // Nonexistent build target value specified.
                Log($"[Builder] Error: Invalid build target: --ucom-build-target {argValue}", LogType.Error);
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

            bool buildFailed = invalidArgs || !Build(outputDirectory);

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
        /// <returns><c>true</c> if the build succeeded; <c>false</c> otherwise.</returns>
        private static bool Build(string outputDirectory)
        {
            var scenes = GetScenePaths();

            if (scenes.Length == 0)
            {
                Log("[Builder] Error: no active scenes in Build Settings.", LogType.Error);
                return false;
            }

            if (!TryCreateApplicationPath(outputDirectory, Application.productName, EditorUserBuildSettings.activeBuildTarget, out string applicationPath))
            {
                return false;
            }

            var buildPlayerOptions = new BuildPlayerOptions
            {
                scenes = scenes,
                locationPathName = applicationPath,
                target = EditorUserBuildSettings.activeBuildTarget,
                options = BuildOptions.None
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
        /// Tries to create the full path of the application to build.
        /// </summary>
        private static bool TryCreateApplicationPath(string outputDirectory, string appName, BuildTarget buildTarget, out string fullOutputPath)
        {
            switch (buildTarget)
            {
                case BuildTarget.iOS:
                case BuildTarget.WebGL:
                    fullOutputPath = Path.Combine(outputDirectory, appName);
                    return true;
                case BuildTarget.StandaloneWindows:
                case BuildTarget.StandaloneWindows64:
                    fullOutputPath = Path.Combine(outputDirectory, $"{appName}.exe");
                    return true;
                case BuildTarget.StandaloneOSX:
                    fullOutputPath = Path.Combine(outputDirectory, $"{appName}.app");
                    return true;
                case BuildTarget.StandaloneLinux64:
                    fullOutputPath = Path.Combine(outputDirectory, $"{appName}.x86_64");
                    return true;
                case BuildTarget.Android:
                    fullOutputPath = Path.Combine(outputDirectory, $"{appName}.apk");
                    return true;
                default:
                    Log($"[Builder] Error: '{buildTarget}' build target not supported.", LogType.Error);
                    fullOutputPath = null;
                    return false;
            }
        }

        /// <summary>
        /// Tries to get the value of the specified argument.
        /// </summary>
        private static bool TryGetArgValue(this string[] source, string arg, out string value)
        {
            int index = Array.IndexOf(source, arg);
            if (index == -1 || index + 1 >= source.Length)
            {
                value = null;
                return false;
            }

            value = source[index + 1];
            return true;
        }

        /// <summary>
        /// Returns the paths of the active scenes in the build settings.
        /// </summary>
        [NotNull]
        private static string[] GetScenePaths()
        {
            return EditorBuildSettings
                   .scenes
                   .Where(scene => scene.enabled)
                   .Select(scene => scene.path)
                   .ToArray();
        }

        /// <summary>
        /// Logs a message to the Unity console without the stack trace.
        /// </summary>
        private static void Log(string message, LogType logType)
        {
            Debug.LogFormat(logType, LogOption.NoStacktrace, null, message);
        }
    }
}