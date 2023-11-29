/*
 * This file contains is part of the ucom command line tool (https://github.com/jakkovanhunen/ucom).
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

#if UNITY_2018_3_OR_NEWER
using System.Collections.Generic;
using System.Linq;
using UnityEditor;
using UnityEngine;

// ReSharper disable once CheckNamespace
namespace Ucom
{
    /// <summary>
    /// The Ucom settings in the Unity Editor.
    /// </summary>
    internal static class MenuSettings
    {
        /// <summary>
        /// Scripting Define Symbols to add to the target Player Settings.
        /// </summary>
        private const string AddDefinesArg = "--ucom-add-defines";

        private const string DefineSymbol = "DISABLE_UCOM_MENU";

        [SettingsProvider]
        public static SettingsProvider CreateUcomSettings()
        {
            var provider = new SettingsProvider("Project/Ucom", SettingsScope.Project)
            {
                label = "Ucom",
                guiHandler = _ =>
                {
                    var activeBuildTargetGroup = BuildPipeline.GetBuildTargetGroup(EditorUserBuildSettings.activeBuildTarget);
                    var isEnabled = !ScriptingDefines.HasDefine(DefineSymbol, activeBuildTargetGroup);
                    var newIsEnabled = EditorGUILayout.Toggle("Enable Ucom Menu", isEnabled);

                    EditorGUILayout.LabelField(
                        $"Disabling the menu adds the {DefineSymbol} compiler symbol to the current build target ({activeBuildTargetGroup}).",
                        EditorStyles.wordWrappedLabel);

                    if (newIsEnabled != isEnabled)
                        ScriptingDefines.SetDefine(DefineSymbol, !newIsEnabled, activeBuildTargetGroup);
                },

                keywords = new HashSet<string>(new[] { "ucom", "menu", "build" }),
            };

            return provider;
        }
    }

    public static class ScriptingDefines
    {
        /// <summary>
        /// Returns true if the given scripting define symbol is defined in the build settings.
        /// </summary>
        /// <param name="define">The scripting define symbol to check.</param>
        /// <param name="buildTargetGroup">The <see cref="BuildTargetGroup"/> to check.</param>
        /// <returns>True if the scripting define symbol is defined; False otherwise.</returns>
        public static bool HasDefine(string define, BuildTargetGroup buildTargetGroup)
        {
            var symbols = PlayerSettings.GetScriptingDefineSymbolsForGroup(buildTargetGroup);
            return symbols.Contains(define);
        }

        /// <summary>
        /// Adds or removes the given scripting define symbol from the build settings.
        /// </summary>
        /// <param name="define">The scripting define symbol to add or remove.</param>
        /// <param name="enabled">If true, the scripting define symbol will be added; if false, the symbol will be removed.</param>
        /// <param name="buildTargetGroup"></param>
        public static void SetDefine(string define, bool enabled, BuildTargetGroup buildTargetGroup)
        {
            var symbols = PlayerSettings.GetScriptingDefineSymbolsForGroup(buildTargetGroup);
            var symbolList = symbols.Split(';');

            switch (enabled)
            {
                case true when !symbolList.Contains(define):
                    symbols += ";" + define;
                    break;
                case false when symbolList.Contains(define):
                    symbols = string.Join(";", symbolList.Where(s => s != define));
                    break;
                default:
                    return;
            }

            PlayerSettings.SetScriptingDefineSymbolsForGroup(buildTargetGroup, symbols);
        }
    }
}

#if !DISABLE_UCOM_MENU
namespace Ucom
{
    using System.IO;

    /// <summary>
    /// Menu options for building the project.
    /// Also serves as example usage of the <see cref="UnityBuilder"/> class for building with your own settings.
    /// E.g. you can create menu options for building specific scenes only or a debug build.
    /// </summary>
    public static class EditorMenu
    {
        public const string MenuName = "Builder";

        /// <summary>
        /// Builds the project to the Builds directory in the project root.
        /// </summary>
        [MenuItem(MenuName + "/Release/Build", false, 1)]
        public static void Build()
        {
            if (!TryGetOutputDirectory(out var outputDirectory, OutputType.Release) || !ValidateActiveScenes())
                return;

            if (UnityBuilder.Build(outputDirectory, UnityBuilder.GetActiveScenes()))
                EditorUtility.OpenWithDefaultApp(outputDirectory);

            Debug.Log("[Builder] Finished Release build");
        }

        /// <summary>
        /// Builds the project to the Builds directory in the project root and runs it.
        /// </summary>
        [MenuItem(MenuName + "/Release/Build and Run", false, 1)]
        public static void BuildAndRun()
        {
            if (!TryGetOutputDirectory(out var outputDirectory, OutputType.Release) || !ValidateActiveScenes())
                return;

            UnityBuilder.Build(outputDirectory, UnityBuilder.GetActiveScenes(), BuildOptions.AutoRunPlayer);

            Debug.Log("[Builder] Finished Release build");
        }

#if UNITY_2019_1_OR_NEWER
        /// <summary>
        /// Builds a development for script debugging.
        /// </summary>
        [MenuItem(MenuName + "/Debug/Build and Run", false, 2)]
        public static void DebugBuild()
        {
            if (!TryGetOutputDirectory(out var outputDirectory, OutputType.Debug) || !ValidateActiveScenes())
                return;

            UnityBuilder.Build(outputDirectory, UnityBuilder.GetActiveScenes(),
                BuildOptions.AutoRunPlayer |
                BuildOptions.Development |
                BuildOptions.AllowDebugging |
                BuildOptions.WaitForPlayerConnection |
                BuildOptions.ConnectToHost);

            Debug.Log("[Builder] Finished Debug build");
        }

        /// <summary>
        /// Builds a debug build for profiling.
        /// </summary>
        [MenuItem(MenuName + "/Debug/Build and Run (Profiling)", false, 2)]
        public static void ProfilingBuild()
        {
            if (!TryGetOutputDirectory(out var outputDirectory, OutputType.Debug) || !ValidateActiveScenes())
                return;

            UnityBuilder.Build(outputDirectory, UnityBuilder.GetActiveScenes(),
                BuildOptions.AutoRunPlayer |
                BuildOptions.Development |
                BuildOptions.ConnectWithProfiler |
                BuildOptions.WaitForPlayerConnection |
                BuildOptions.ConnectToHost);

            Debug.Log("[Builder] Finished Debug (Profiling) build");
        }
#endif // UNITY_2019_1_OR_NEWER

#if UNITY_2019_3_OR_NEWER
        /// <summary>
        /// Builds a debug build for deep profiling.
        /// </summary>
        [MenuItem(MenuName + "/Debug/Build and Run (Deep Profiling)", false, 2)]
        public static void DeepProfilingBuild()
        {
            if (!TryGetOutputDirectory(out var outputDirectory, OutputType.Debug) || !ValidateActiveScenes())
                return;

            UnityBuilder.Build(outputDirectory, UnityBuilder.GetActiveScenes(),
                BuildOptions.AutoRunPlayer |
                BuildOptions.Development |
                BuildOptions.ConnectWithProfiler |
                BuildOptions.EnableDeepProfilingSupport |
                BuildOptions.WaitForPlayerConnection |
                BuildOptions.ConnectToHost);

            Debug.Log("[Builder] Finished Debug (Deep Profiling) build");
        }
#endif // UNITY_2019_3_OR_NEWER

        /// <summary>
        /// Opens the Builds directory in the project root.
        /// </summary>
        [MenuItem(MenuName + "/Open Builds Directory")]
        public static void OpenBuildDirectory()
        {
            EditorUtility.OpenWithDefaultApp(UnityBuilder.GetBuildsDirectoryPath());
        }

        [MenuItem(MenuName + "/Open Builds Directory", true)]
        public static bool ValidateOpenBuildDirectory() => Directory.Exists(UnityBuilder.GetBuildsDirectoryPath());

        private static bool TryGetOutputDirectory(out string outputDirectory, OutputType outputTypeType)
        {
            if (UnityBuilder.TryGetDefaultBuildOutputPath(out outputDirectory, outputTypeType))
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
#endif // !DISABLE_UCOM_MENU

#endif // UNITY_2018_3_OR_NEWER