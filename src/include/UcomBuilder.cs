using System;
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
    /// ucom build companion script.
    /// </summary>
    public static class UcomBuilder
    {
        private const string BuildOutputArg = "--ucom-build-output";
        private const string BuildTargetArg = "--ucom-build-target";

        [UsedImplicitly]
        public static void Build()
        {
            string[] args = Environment.GetCommandLineArgs();

            bool invalidArgs = false;

            if (!args.TryGetArgValue(BuildOutputArg, out string buildOutput))
            {
                Debug.LogError("[UcomBuilder] Error: Output path '--ucom-build-output <path>' not specified.");
                invalidArgs = true;
            }

            if (!args.TryGetArgValue(BuildTargetArg, out string buildTarget))
            {
                Debug.LogError("[UcomBuilder] Error: Build target '--ucom-build-target <target>' not specified.");
                invalidArgs = true;
            }
            else if (!Enum.TryParse(buildTarget, out BuildTarget target))
            {
                Debug.LogError($"[UcomBuilder] Error: Invalid build target: --ucom-build-target {buildTarget}");
                invalidArgs = true;
            }
            else if (target != EditorUserBuildSettings.activeBuildTarget)
            {
                Debug.LogError(BuildPipeline.IsBuildTargetSupported(BuildPipeline.GetBuildTargetGroup(target), target)
                    ? $"[UcomBuilder] Error: Build target '{buildTarget}' does not match active build target '{EditorUserBuildSettings.activeBuildTarget}'"
                    : $"[UcomBuilder] Error: Build target '{buildTarget}' is not supported."
                );

                invalidArgs = true;
            }

            if (invalidArgs || !Build(buildOutput))
            {
                EditorApplication.Exit(1);
            }
            else
            {
                EditorApplication.Exit(0);
            }
        }

        private static bool Build(string buildOutput)
        {
            var buildTarget = EditorUserBuildSettings.activeBuildTarget;

            switch (buildTarget)
            {
                case BuildTarget.iOS:
                case BuildTarget.WebGL:
                    buildOutput = Path.Combine(buildOutput, Application.productName);
                    break;
                case BuildTarget.StandaloneWindows:
                case BuildTarget.StandaloneWindows64:
                    buildOutput = Path.Combine(buildOutput, $"{Application.productName}.exe");
                    break;
                case BuildTarget.StandaloneOSX:
                    buildOutput = Path.Combine(buildOutput, $"{Application.productName}.app");
                    break;
                case BuildTarget.StandaloneLinux64:
                    buildOutput = Path.Combine(buildOutput, $"{Application.productName}.x86_64");
                    break;
                case BuildTarget.Android:
                    buildOutput = Path.Combine(buildOutput, $"{Application.productName}.apk");
                    break;
                default:
                    Debug.LogError($"[UcomBuilder] Error: '{buildTarget}' build target not supported.");
                    EditorApplication.Exit(1);
                    return false;
            }

            var scenes = GetScenePaths();

            if (scenes == null || scenes.Length == 0)
            {
                Debug.LogError("[UcomBuilder] Error: no active scenes in Build Settings.");
                EditorApplication.Exit(1);
                return false;
            }

            var buildPlayerOptions = new BuildPlayerOptions
            {
                scenes = scenes,
                locationPathName = buildOutput,
                target = EditorUserBuildSettings.activeBuildTarget,
                options = BuildOptions.None
            };

            var report = BuildPipeline.BuildPlayer(buildPlayerOptions);
            var summary = report.summary;

            var sb = new StringBuilder();
            sb.AppendLine("[UcomBuilder] Build Report Begin")
              .AppendLine($"Build result: {summary.result}")
              .AppendLine($"Output path:  {summary.outputPath}")
              .AppendLine($"Size:         {summary.totalSize / 1024 / 1024} MB")
              .AppendLine($"Total time:   {summary.totalTime}")
              .AppendLine($"Errors:       {summary.totalErrors}")
              .AppendLine($"Warnings:     {summary.totalWarnings}")
              .AppendLine("[UcomBuilder] Build Report End");

            if (summary.result != BuildResult.Succeeded)
            {
                Debug.LogError(sb.ToString());
                return false;
            }

            Debug.Log(sb.ToString());
            return true;
        }

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

        private static string[] GetScenePaths()
        {
            return EditorBuildSettings
                   .scenes
                   .Where(scene => scene.enabled)
                   .Select(scene => scene.path)
                   .ToArray();
        }
    }
}