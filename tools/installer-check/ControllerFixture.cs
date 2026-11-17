using System;
using System.Threading;

internal static class ControllerFixture
{
    private static int Main(string[] args)
    {
        if (String.IsNullOrEmpty(Environment.GetEnvironmentVariable("CCC_INSTALLER_CHECK_ROOT"))) return 90;
        if (args.Length == 1 && args[0] == "--hold") Thread.Sleep(120000);
        return 0;
    }
}
