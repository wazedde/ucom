// This file is part of the ucom command line tool (https://github.com/wazedde/ucom).
// Copyright 2022-2026 Jakko van Hunen
// SPDX-License-Identifier: Apache-2.0

#if UNITY_2021_3_OR_NEWER
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.Build;
using UnityEngine;

// ReSharper disable once CheckNamespace
namespace Ucom
{
    /// <summary>
    /// The Ucom settings in the Unity Editor.
    /// </summary>
    internal static class MenuSettings
    {
        private const string DefineSymbol = "DISABLE_UCOM_MENU";

        [SettingsProvider]
        public static SettingsProvider CreateUcomSettings() => new("Project/Ucom", SettingsScope.Project)
        {
            label = "Ucom",
            guiHandler = _ =>
            {
                var targetGroup = BuildPipeline.GetBuildTargetGroup(EditorUserBuildSettings.activeBuildTarget);
                var isEnabled = !ScriptingDefines.HasDefine(DefineSymbol, targetGroup);
                var newIsEnabled = EditorGUILayout.Toggle("Enable Ucom Menu", isEnabled);

                EditorGUILayout.LabelField(
                    $"Disabling the menu adds the {DefineSymbol} compiler symbol to the current build target ({targetGroup}).",
                    EditorStyles.wordWrappedLabel);

                if (newIsEnabled != isEnabled)
                    ScriptingDefines.SetDefine(DefineSymbol, !newIsEnabled, targetGroup);
            },
            keywords = new HashSet<string>(new[] { "ucom", "menu", "build" }),
        };
    }

    public static class ScriptingDefines
    {
        /// <summary>
        /// Returns true if the given scripting define symbol is defined for the build target group.
        /// </summary>
        public static bool HasDefine(string define, BuildTargetGroup buildTargetGroup) =>
            GetDefines(buildTargetGroup).Contains(define);

        /// <summary>
        /// Adds (<paramref name="enabled"/> is true) or removes the scripting define symbol for the build target group.
        /// </summary>
        public static void SetDefine(string define, bool enabled, BuildTargetGroup buildTargetGroup)
        {
            var defines = GetDefines(buildTargetGroup);
            if (defines.Contains(define) == enabled)
                return;

            var newDefines = enabled ? defines.Append(define) : defines.Where(s => s != define);
            PlayerSettings.SetScriptingDefineSymbols(NamedBuildTarget.FromBuildTargetGroup(buildTargetGroup),
                string.Join(";", newDefines));
        }

        private static string[] GetDefines(BuildTargetGroup buildTargetGroup) => PlayerSettings
            .GetScriptingDefineSymbols(NamedBuildTarget.FromBuildTargetGroup(buildTargetGroup))
            .Split(';')
            .Where(s => s.Length > 0)
            .ToArray();
    }

    /// <summary>
    /// Constants for the menu.
    /// </summary>
    public static class MenuConstants
    {
        public const string MenuName = "Builder";

#if UNITY_IOS
        public const string PlatformName = "iOS";
#elif UNITY_ANDROID
        public const string PlatformName = "Android";
#elif UNITY_STANDALONE_OSX
        public const string PlatformName = "macOS";
#elif UNITY_STANDALONE_WIN
        public const string PlatformName = "Windows";
#elif UNITY_STANDALONE_LINUX
        public const string PlatformName = "Linux";
#elif UNITY_WEBGL
        public const string PlatformName = "WebGL";
#else
        public const string PlatformName = "Unsupported";
#endif
    }

#if !DISABLE_UCOM_MENU
    /// <summary>
    /// Menu options for building the project.
    /// Also serves as example usage of the <see cref="UnityBuilder"/> class for building with your own settings.
    /// E.g. you can create menu options for building specific scenes only or a debug build.
    /// </summary>
    public static class EditorMenu
    {
        private const string PlatformMenu = MenuConstants.MenuName + "/Platform: " + MenuConstants.PlatformName;
        private const string ReleaseMenu = MenuConstants.MenuName + "/Release/";
        private const string DebugMenu = MenuConstants.MenuName + "/Debug/";

        private const BuildOptions DebugOptions = BuildOptions.AutoRunPlayer
                                                  | BuildOptions.Development
                                                  | BuildOptions.WaitForPlayerConnection
                                                  | BuildOptions.ConnectToHost;

        private const BuildOptions ProfilingOptions = DebugOptions | BuildOptions.ConnectWithProfiler;

        [MenuItem(PlatformMenu + "/Build Settings...", false, 0)]
        public static void BuildSettings() =>
            EditorWindow.GetWindow(System.Type.GetType("UnityEditor.BuildPlayerWindow,UnityEditor"));

        [MenuItem(PlatformMenu + "/Player Settings...", false, 0)]
        public static void PlatformSettings() => SettingsService.OpenProjectSettings("Project/Player");

        /// <summary>
        /// Builds the project to the Builds directory in the project root and shows the build in the file browser.
        /// </summary>
        [MenuItem(ReleaseMenu + "Build", false, 1)]
        public static void Build()
        {
            if (BuildTo(OutputType.Release, BuildOptions.None, "Release", out var locationPath))
                EditorUtility.RevealInFinder(locationPath);
        }

        /// <summary>
        /// Builds the project to the Builds directory in the project root and runs it.
        /// </summary>
        [MenuItem(ReleaseMenu + "Build and Run", false, 1)]
        public static void BuildAndRun() => BuildTo(OutputType.Release, BuildOptions.AutoRunPlayer, "Release", out _);

        /// <summary>
        /// Builds a development build for script debugging.
        /// </summary>
        [MenuItem(DebugMenu + "Build and Run", false, 2)]
        public static void DebugBuild() =>
            BuildTo(OutputType.Debug, DebugOptions | BuildOptions.AllowDebugging, "Debug", out _);

        /// <summary>
        /// Builds a debug build for profiling.
        /// </summary>
        [MenuItem(DebugMenu + "Build and Run (Profiling)", false, 2)]
        public static void ProfilingBuild() => BuildTo(OutputType.Debug, ProfilingOptions, "Debug (Profiling)", out _);

        /// <summary>
        /// Builds a debug build for deep profiling.
        /// </summary>
        [MenuItem(DebugMenu + "Build and Run (Deep Profiling)", false, 2)]
        public static void DeepProfilingBuild() => BuildTo(OutputType.Debug,
            ProfilingOptions | BuildOptions.EnableDeepProfilingSupport,
            "Debug (Deep Profiling)",
            out _);

        /// <summary>
        /// Opens the Builds directory in the project root.
        /// </summary>
        [MenuItem(MenuConstants.MenuName + "/Open Builds Directory")]
        public static void OpenBuildDirectory() => EditorUtility.OpenWithDefaultApp(UnityBuilder.GetBuildsDirectoryPath());

        [MenuItem(MenuConstants.MenuName + "/Open Builds Directory", true)]
        public static bool ValidateOpenBuildDirectory() => Directory.Exists(UnityBuilder.GetBuildsDirectoryPath());

        /// <summary>
        /// Builds the active scenes for the active build target to <c>Builds/&lt;outputType&gt;/&lt;target&gt;</c>.
        /// </summary>
        /// <param name="label">The name of the build in the log message.</param>
        /// <param name="locationPath">The path of the build output.</param>
        /// <returns><c>true</c> if the build succeeded; <c>false</c> otherwise.</returns>
        private static bool BuildTo(OutputType outputType, BuildOptions options, string label, out string locationPath)
        {
            var target = EditorUserBuildSettings.activeBuildTarget;
            if (!UnityBuilder.TryGetDefaultBuildOutputPath(out var outputPath, outputType))
            {
                UnityBuilder.Log($"[Builder] Error: Unsupported build target {target}", LogType.Error);
                locationPath = null;
                return false;
            }

            // Cannot fail: TryGetDefaultBuildOutputPath has already checked that the target is supported.
            UnityBuilder.TryGetBuildLocationPath(outputPath, Application.productName, target, out locationPath);

            var scenes = UnityBuilder.GetActiveScenes();
            if (scenes.Length == 0)
            {
                EditorUtility.DisplayDialog("No Active Scenes to Build",
                    "Add at least one active scene to the Build Settings.",
                    "Ok");
                return false;
            }

            // On failure, UnityBuilder.Build has already logged the errors.
            if (!UnityBuilder.Build(locationPath, scenes, options))
                return false;

            Debug.Log($"[Builder] Finished {label} build");
            return true;
        }
    }
#endif // !DISABLE_UCOM_MENU
}
#endif // UNITY_2021_3_OR_NEWER
