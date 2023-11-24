/*
 * This file contains the UnityBuilder class which handles build capabilities for Unity projects and is part of the
 * ucom command line tool (https://github.com/jakkovanhunen/ucom).
 *
 * Copyright 2022-2023 Jakko van Hunen
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

using System;
using System.Collections.Generic;
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

            if (!args.TryGetArgValue(PreBuildArgs, out var preBuildArgs))
                preBuildArgs = "";

            var buildFailed = invalidArgs || !Build(outputDirectory, GetActiveScenes(), options, preBuildArgs);

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
        /// <param name="preBuildArgs">The pre-build arguments that are passed to methods with the <see cref="UcomPreProcessBuildAttribute"/>.</param>
        /// <returns><c>true</c> if the build succeeded; <c>false</c> otherwise.</returns>
        public static bool Build(string outputDirectory, string[] scenes, BuildOptions options = BuildOptions.None, string preBuildArgs = "")
        {
            if (scenes.Length == 0)
            {
                Log("[Builder] Error: no scenes to build specified.", LogType.Error);
                return false;
            }

            if (!TryGetBuildLocationPath(outputDirectory,
                    Application.productName,
                    EditorUserBuildSettings.activeBuildTarget,
                    out var applicationPath))
            {
                return false;
            }

            if (!RunPreProcessBuildMethods(preBuildArgs))
                return false;

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

            if (Environment.GetCommandLineArgs().TryGetArgValue("-logFile", out var logFile))
                sb.AppendLine($"    Log file:     {logFile}");

            // End of build report.
            sb.AppendLine();

            switch (summary.result)
            {
                case BuildResult.Succeeded:
                    Log("[Builder] Build succeeded.");
                    Log(sb.ToString());
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
        /// Logs a message to the Unity console without the stack trace.
        /// </summary>
        /// <param name="message">The message.</param>
        /// <param name="logType">The <see cref="LogType"/>.</param>
        public static void Log(string message, LogType logType = LogType.Log)
        {
            Debug.LogFormat(logType, LogOption.NoStacktrace, null, message);
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

        private static bool RunPreProcessBuildMethods(string arg)
        {
            return AppDomain.CurrentDomain
                            .GetAssemblies()
                            .SelectMany(assembly => assembly.GetTypes())
                            .SelectMany(GetPreProcessBuildMethods)
                            .All(method => InvokeMethod(method, arg));
        }

        private static IEnumerable<MethodInfo> GetPreProcessBuildMethods(Type type)
        {
            return type.GetMethods(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static)
                       .Where(m => m.GetCustomAttributes(typeof(UcomPreProcessBuildAttribute), false).Any());
        }

        private static bool InvokeMethod(MethodBase method, string message)
        {
            var parameters = method.GetParameters();
            if (parameters.Length == 1 && parameters[0].ParameterType == typeof(string))
            {
                method.Invoke(null, new object[] { message });
                return true;
            }

            Log($"Invalid method signature for UcomPreProcessBuildAttribute: {method.ReflectedType.FullName}.{method.Name}", LogType.Error);
            return false;
        }
    }

    /// <summary>
    /// Add this attribute to a method to get a notification just before building the player.
    /// The method must be static with a single string parameter.
    /// The string parameter will be the argument passed in from the command line.
    /// </summary>
    [AttributeUsage(AttributeTargets.Method)]
    public class UcomPreProcessBuildAttribute : Attribute { }


    /// <summary>
    /// The Ucom preferences in the Unity Editor.
    /// </summary>
    public static class UcomPreferences
    {
        private const string Symbol = "UCOM_MENU";

        [PreferenceItem("Ucom")]
        public static void PreferencesGUI()
        {
            var hasSymbol = HasCompilerSymbol(Symbol);
            var newHasSymbol = EditorGUILayout.Toggle("Enable Ucom Menu", hasSymbol);
            EditorGUILayout.LabelField($"Enabling the menu adds the {Symbol} compiler symbol to the current build target ({EditorUserBuildSettings.selectedBuildTargetGroup}).", EditorStyles.wordWrappedLabel);

            if (newHasSymbol != hasSymbol)
                SetCompilerSymbol(Symbol, newHasSymbol);
        }

        private static bool HasCompilerSymbol(string symbol)
        {
            var symbols = PlayerSettings.GetScriptingDefineSymbolsForGroup(EditorUserBuildSettings.selectedBuildTargetGroup);
            return symbols.Contains(symbol);
        }

        private static void SetCompilerSymbol(string symbol, bool enabled)
        {
            var symbols = PlayerSettings.GetScriptingDefineSymbolsForGroup(EditorUserBuildSettings.selectedBuildTargetGroup);
            var symbolList = symbols.Split(';');

            switch (enabled)
            {
                case true when !symbolList.Contains(symbol):
                    symbols += ";" + symbol;
                    break;
                case false when symbolList.Contains(symbol):
                    symbols = string.Join(";", symbolList.Where(s => s != symbol));
                    break;
                default:
                    return;
            }

            PlayerSettings.SetScriptingDefineSymbolsForGroup(EditorUserBuildSettings.selectedBuildTargetGroup, symbols);
        }
    }

#if UCOM_MENU
    namespace Ucom
    {
        /// <summary>
        /// Menu options for building the project.
        /// Also serves as example usage of the <see cref="UnityBuilder"/> class for building with your own settings.
        /// E.g. you can create menu options for building specific scenes only or a debug build.
        /// </summary>
        public static class UcomMenu
        {
            public const string MenuName = "Ucom";

            /// <summary>
            /// Builds the project to the Builds directory in the project root.
            /// </summary>
            [MenuItem(MenuName + "/Build to Builds Directory")]
            public static void Build()
            {
                if (!GetOutputDirectory(out var outputDirectory) || !ValidateActiveScenes())
                    return;

                if (UnityBuilder.Build(outputDirectory, UnityBuilder.GetActiveScenes()))
                    EditorUtility.OpenWithDefaultApp(outputDirectory);
            }

            /// <summary>
            /// Builds the project to the Builds directory in the project root and runs it.
            /// </summary>
            [MenuItem(MenuName + "/Build to Builds Directory and Run")]
            public static void BuildAndRun()
            {
                if (GetOutputDirectory(out var outputDirectory) && ValidateActiveScenes())
                    UnityBuilder.Build(outputDirectory, UnityBuilder.GetActiveScenes(), BuildOptions.AutoRunPlayer);
            }

            /// <summary>
            /// Opens the Builds directory in the project root.
            /// </summary>
            [MenuItem(MenuName + "/Open Build's Directory")]
            public static void OpenBuildDirectory()
            {
                if (GetOutputDirectory(out var outputDirectory))
                    EditorUtility.OpenWithDefaultApp(outputDirectory);
            }

            [MenuItem(MenuName + "/Open Build's Directory", true)]
            public static bool ValidateOpenBuildDirectory() =>
                GetOutputDirectory(out var outputDirectory) && Directory.Exists(outputDirectory);

            private static bool GetOutputDirectory(out string outputDirectory)
            {
                if (UnityBuilder.TryGetDefaultBuildOutputPath(out outputDirectory))
                    return true;

                UnityBuilder.Log($"[Builder] Unsupported build target{EditorUserBuildSettings.activeBuildTarget}", LogType.Error);
                return false;
            }

            private static bool ValidateActiveScenes()
            {
                if (UnityBuilder.GetActiveScenes().Length > 0)
                    return true;

                EditorUtility.DisplayDialog("No Active Scenes to Build", "Add at least one active scene to the Build Settings.", "Ok");
                return false;
            }
        }
    }
#endif
}