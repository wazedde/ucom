using System;
using UnityEngine;

/// <summary>
/// The pre-build hook that ucom runs before each build, with the --build-args string.
/// Pass --build-args fail to make it throw, which must fail the build.
/// </summary>
/// <remarks>Needs the ucom builder script (`ucom add builder`), which defines the attribute.</remarks>
public static class BuildHook
{
    [Ucom.UcomPreProcessBuild]
    public static void PreProcessBuild(string args)
    {
        Debug.Log($"[MinimalProject] Pre-build hook ran with args: '{args}'");
        if (args == "fail")
            throw new InvalidOperationException("The pre-build hook was asked to fail");
    }
}
