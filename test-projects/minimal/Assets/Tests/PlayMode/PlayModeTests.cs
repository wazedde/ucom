using System.Collections;
using NUnit.Framework;
using UnityEngine;
using UnityEngine.TestTools;

public class PlayModeTests
{
    [UnityTest]
    public IEnumerator FramesAdvance()
    {
        var start = Time.frameCount;
        yield return null;
        Assert.Greater(Time.frameCount, start);
    }
}
