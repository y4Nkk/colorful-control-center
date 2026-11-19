using System;
using System.IO;
using System.Threading;
using System.Windows.Forms;

internal static class ControllerFixture
{
    private static int Main(string[] args)
    {
        string root = Environment.GetEnvironmentVariable("CCC_INSTALLER_CHECK_ROOT");
        if (String.IsNullOrEmpty(root)) return 90;
        if (args.Length == 2 && args[0] == "--hold")
            using (var release = EventWaitHandle.OpenExisting(args[1]))
                return release.WaitOne(120000) ? 0 : 91;
        if (args.Length == 3 && (args[0] == "--controller" || args[0] == "--refuse-exit"))
        {
            string marker = Path.GetFullPath(args[2]);
            if (!marker.StartsWith(Path.GetFullPath(root) + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)) return 92;
            var window = new ShutdownWindow(args[1], marker, args[0] == "--refuse-exit");
            File.WriteAllText(marker + ".ready", "window available");
            Application.Run();
            window.DestroyHandle();
        }
        return 0;
    }

    private sealed class ShutdownWindow : NativeWindow
    {
        private readonly string marker;
        private readonly bool refuse;

        internal ShutdownWindow(string title, string marker, bool refuse)
        {
            this.marker = marker;
            this.refuse = refuse;
            CreateHandle(new CreateParams { Caption = title });
        }

        protected override void WndProc(ref Message message)
        {
            if (message.Msg == 0x11) // WM_QUERYENDSESSION
            {
                File.WriteAllText(marker + ".queried", "Windows requested shutdown");
                message.Result = refuse ? IntPtr.Zero : new IntPtr(1);
                return;
            }
            if (message.Msg == 0x16 && message.WParam != IntPtr.Zero) // WM_ENDSESSION
            {
                Thread.Sleep(600);
                File.WriteAllText(marker + ".cleaned", "normal cleanup finished");
                Application.ExitThread();
                message.Result = IntPtr.Zero;
                return;
            }
            base.WndProc(ref message);
        }
    }
}
