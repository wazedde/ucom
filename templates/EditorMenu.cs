/*
 * This file is part of the ucom command line tool (https://github.com/jakkovanhunen/ucom).
 *
 * Copyright 2022-2024 Jakko van Hunen
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
                    bool isEnabled = !ScriptingDefines.HasDefine(DefineSymbol, activeBuildTargetGroup);
                    bool newIsEnabled = EditorGUILayout.Toggle("Enable Ucom Menu", isEnabled);

                    EditorGUILayout.LabelField(
                        $"Disabling the menu adds the {DefineSymbol} compiler symbol to the current build target ({activeBuildTargetGroup}).",
                        EditorStyles.wordWrappedLabel
                    );

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
            string[] symbols = PlayerSettings.GetScriptingDefineSymbolsForGroup(buildTargetGroup).Split(';');
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
            string symbols = PlayerSettings.GetScriptingDefineSymbolsForGroup(buildTargetGroup);
            string[] symbolList = symbols.Split(';');

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
#endif
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
        private const string PlatformMenu = MenuConstants.MenuName + "/Platform: " + MenuConstants.PlatformName;

        [MenuItem(PlatformMenu + "/Build Settings...", false, 0)]
        public static void BuildSettings()
        {
            EditorWindow.GetWindow(System.Type.GetType("UnityEditor.BuildPlayerWindow,UnityEditor"));
        }

        [MenuItem(PlatformMenu + "/Player Settings...", false, 0)]
        public static void PlatformSettings()
        {
            SettingsService.OpenProjectSettings("Project/Player");
        }

        /// <summary>
        /// Builds the project to the Builds directory in the project root.
        /// </summary>
        [MenuItem(MenuConstants.MenuName + "/Release/Build", false, 1)]
        public static void Build()
        {
            if (!TryGetOutputPath(out string outputDirectory, OutputType.Release) || !ValidateActiveScenes())
                return;

            if (UnityBuilder.Build(outputDirectory, UnityBuilder.GetActiveScenes()))
                EditorUtility.OpenWithDefaultApp(outputDirectory);

            Debug.Log("[Builder] Finished Release build");
        }

        /// <summary>
        /// Builds the project to the Builds directory in the project root and runs it.
        /// </summary>
        [MenuItem(MenuConstants.MenuName + "/Release/Build and Run", false, 1)]
        public static void BuildAndRun()
        {
            if (!TryGetOutputPath(out string outputDirectory, OutputType.Release) || !ValidateActiveScenes())
                return;

            UnityBuilder.Build(outputDirectory, UnityBuilder.GetActiveScenes(), BuildOptions.AutoRunPlayer);

            Debug.Log("[Builder] Finished Release build");
        }

#if UNITY_2019_3_OR_NEWER
        /// <summary>
        /// Builds a debug build for deep profiling.
        /// </summary>
        [MenuItem(MenuConstants.MenuName + "/Debug/Build and Run (Deep Profiling)", false, 2)]
        public static void DeepProfilingBuild()
        {
            if (!TryGetOutputPath(out string outputDirectory, OutputType.Debug) || !ValidateActiveScenes())
                return;

            UnityBuilder.Build(outputDirectory,
                UnityBuilder.GetActiveScenes(),
                BuildOptions.AutoRunPlayer |
                BuildOptions.Development |
                BuildOptions.ConnectWithProfiler |
                BuildOptions.EnableDeepProfilingSupport |
                BuildOptions.WaitForPlayerConnection |
                BuildOptions.ConnectToHost
            );

            Debug.Log("[Builder] Finished Debug (Deep Profiling) build");
        }
#endif // UNITY_2019_3_OR_NEWER

        /// <summary>
        /// Opens the Builds directory in the project root.
        /// </summary>
        [MenuItem(MenuConstants.MenuName + "/Open Builds Directory")]
        public static void OpenBuildDirectory()
        {
            EditorUtility.OpenWithDefaultApp(UnityBuilder.GetBuildsDirectoryPath());
        }

        [MenuItem(MenuConstants.MenuName + "/Open Builds Directory", true)]
        public static bool ValidateOpenBuildDirectory() => Directory.Exists(UnityBuilder.GetBuildsDirectoryPath());

        private static bool TryGetOutputPath(out string outputDirectory, OutputType outputType)
        {
            if (UnityBuilder.TryGetDefaultBuildOutputPath(out string outputPath, outputType)
                && UnityBuilder.TryGetBuildLocationPath(outputPath,
                    Application.productName,
                    EditorUserBuildSettings.activeBuildTarget,
                    out outputDirectory
                ))
            {
                return true;
            }

            UnityBuilder.Log($"[Builder] Unsupported build target{EditorUserBuildSettings.activeBuildTarget}", LogType.Error);
            outputDirectory = null;
            return false;
        }

        private static bool ValidateActiveScenes()
        {
            if (UnityBuilder.GetActiveScenes().Length > 0)
                return true;

            EditorUtility.DisplayDialog("No Active Scenes to Build", "Add at least one active scene to the Build Settings.", "Ok");
            return false;
        }

#if UNITY_2019_1_OR_NEWER
        /// <summary>
        /// Builds a development for script debugging.
        /// </summary>
        [MenuItem(MenuConstants.MenuName + "/Debug/Build and Run", false, 2)]
        public static void DebugBuild()
        {
            if (!TryGetOutputPath(out string outputDirectory, OutputType.Debug) || !ValidateActiveScenes())
                return;

            UnityBuilder.Build(outputDirectory,
                UnityBuilder.GetActiveScenes(),
                BuildOptions.AutoRunPlayer |
                BuildOptions.Development |
                BuildOptions.AllowDebugging |
                BuildOptions.WaitForPlayerConnection |
                BuildOptions.ConnectToHost
            );

            Debug.Log("[Builder] Finished Debug build");
        }

        /// <summary>
        /// Builds a debug build for profiling.
        /// </summary>
        [MenuItem(MenuConstants.MenuName + "/Debug/Build and Run (Profiling)", false, 2)]
        public static void ProfilingBuild()
        {
            if (!TryGetOutputPath(out string outputDirectory, OutputType.Debug) || !ValidateActiveScenes())
                return;

            UnityBuilder.Build(outputDirectory,
                UnityBuilder.GetActiveScenes(),
                BuildOptions.AutoRunPlayer |
                BuildOptions.Development |
                BuildOptions.ConnectWithProfiler |
                BuildOptions.WaitForPlayerConnection |
                BuildOptions.ConnectToHost
            );

            Debug.Log("[Builder] Finished Debug (Profiling) build");
        }
#endif // UNITY_2019_1_OR_NEWER
    }
}
#endif // !DISABLE_UCOM_MENU

#endif // UNITY_2018_3_OR_NEWER