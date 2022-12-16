using System;
using System.IO;
using System.Linq;
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
                Debug.LogError("[ucom] Output path not specified.");
                invalidArgs = true;
            }

            if (!args.TryGetArgValue(BuildTargetArg, out string buildTarget))
            {
                Debug.LogError("[ucom] Build target not specified.");
                invalidArgs = true;
            }
            else if (!Enum.TryParse(buildTarget, out BuildTarget target))
            {
                Debug.LogError($"[ucom] Invalid build target: {buildTarget}");
                invalidArgs = true;
            }
            else if (target != EditorUserBuildSettings.activeBuildTarget)
            {
                Debug.LogError($"[ucom] Build target {buildTarget} does not match active build target {EditorUserBuildSettings.activeBuildTarget}");
                invalidArgs = true;
            }

            if (invalidArgs)
            {
                Debug.LogError("[ucom] Build failed: output path not specified.");
                EditorApplication.Exit(1);
                return;
            }

            Build(buildOutput);
        }

        private static void Build(string buildOutput)
        {
            var buildTarget = EditorUserBuildSettings.activeBuildTarget;

            switch (buildTarget)
            {
                case BuildTarget.iOS:
                case BuildTarget.WebGL:
                    // Use unmodified buildOutput path.
                    break;
                case BuildTarget.StandaloneWindows:
                case BuildTarget.StandaloneWindows64:
                    buildOutput = Path.Join(buildOutput, $"{Application.productName}.exe");
                    break;
                case BuildTarget.StandaloneOSX:
                    buildOutput = Path.Join(buildOutput, $"{Application.productName}.app");
                    break;
                case BuildTarget.StandaloneLinux64:
                    buildOutput = Path.Join(buildOutput, $"{Application.productName}.x86_64");
                    break;
                case BuildTarget.Android:
                    buildOutput = Path.Join(buildOutput, $"{Application.productName}.apk");
                    break;
                default:
                    Debug.LogError($"[ucom] Build failed: {buildTarget} build target not supported.");
                    EditorApplication.Exit(1);
                    return;
            }

            var buildPlayerOptions = new BuildPlayerOptions
            {
                scenes = GetScenePaths(),
                locationPathName = buildOutput,
                target = EditorUserBuildSettings.activeBuildTarget,
                options = BuildOptions.None
            };

            var report = BuildPipeline.BuildPlayer(buildPlayerOptions);
            var summary = report.summary;

            switch (summary.result)
            {
                case BuildResult.Succeeded:
                    Debug.Log($"[ucom] Build succeeded: {summary.totalSize} bytes");
                    break;
                case BuildResult.Failed:
                    Debug.Log("[ucom] Build failed");
                    EditorApplication.Exit(1);
                    return;
            }
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