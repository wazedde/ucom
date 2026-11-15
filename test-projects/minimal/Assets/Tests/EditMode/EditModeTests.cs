using NUnit.Framework;

/// <summary>
/// One passing, one skipped and one failing test, so ucom sees each kind of result.
/// The failing test is in the "Failing" category: run with --categories "!Failing" to leave it out,
/// or --categories Failing to run only it.
/// </summary>
public class EditModeTests
{
    [Test]
    public void Passes() => Assert.AreEqual(4, 2 + 2);

    [Test]
    [Ignore("Always skipped, to give ucom a skipped result")]
    public void IsSkipped() => Assert.Fail("Never runs");

    [Test]
    [Category("Failing")]
    public void FailsOnPurpose() => Assert.Fail("Failing on purpose");
}
