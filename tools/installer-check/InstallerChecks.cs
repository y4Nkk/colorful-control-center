using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using ColorfulControlCenter.Installer;

internal static partial class InstallerChecks
{
    private static string root;
    private static int assertions;

    [STAThread]
    private static int Main(string[] arguments)
    {
        root = Path.GetFullPath(arguments[0]);
        Environment.SetEnvironmentVariable("CCC_INSTALLER_CHECK_ROOT", root);
        string temporary = Path.Combine(root, "temp");
        Directory.CreateDirectory(temporary);
        Environment.SetEnvironmentVariable("TEMP", temporary);
        Environment.SetEnvironmentVariable("TMP", temporary);
        var app = new Application { ShutdownMode = ShutdownMode.OnExplicitShutdown };
        System.Threading.SynchronizationContext.SetSynchronizationContext(new DispatcherSynchronizationContext());
        try
        {
            CheckTransitions();
            if (arguments.Length > 1 && arguments[1] == "render")
            {
                CheckLayout();
                File.WriteAllText(Path.Combine(root, "result.json"), "{\"passed\":true,\"renderOnly\":true}");
                return 0;
            }
            CheckPaths();
            CheckWizard();
            CheckFailure("failure");
            CheckFailure("missing");
            CheckFailure("tampered");
            CheckRunningGuard();
            CheckNativeUninstall();
            CheckFixtureBoundary();
            File.WriteAllText(Path.Combine(root, "result.json"),
                "{\"passed\":true,\"assertions\":" + assertions + ",\"engine\":\"isolated NSIS\",\"realInstallation\":false}");
            return 0;
        }
        catch (Exception error)
        {
            File.WriteAllText(Path.Combine(root, "failure.txt"), error.ToString());
            return 1;
        }
        finally { app.Shutdown(); }
    }

    private static void CheckLayout()
    {
        var completion = new System.Threading.Tasks.TaskCompletionSource<bool>();
        var window = Window(null, path => { throw new Exception("Preview must not launch"); }, (request, progress) => {
            progress.Report(new InstallProgress { Phase = "正在安装程序与运行组件…" });
            return completion.Task;
        });
        Capture(window, "01-welcome");
        Click(window, "primary");
        Field<TextBox>(window, "directoryBox").Text = Path.Combine(root, "中文安装位置-用于验证长路径换行");
        Capture(window, "02-location");
        window.Width = 760;
        window.Height = 500;
        Capture(window, "03-small", 1.5);
        window.Width = 1040;
        window.Height = 640;
        Click(window, "primary");
        Capture(window, "04-installing");
        completion.SetResult(true);
        Wait(window);
        Capture(window, "05-complete");
        window.Close();
        var failure = Window(null, path => { throw new Exception("Preview must not launch"); }, (request, progress) => {
            var task = new System.Threading.Tasks.TaskCompletionSource<bool>();
            task.SetException(new InstallFailure(new IOException("安装被中断，请确认目录权限后重试。"),
                Path.Combine(root, "preview-install.log")));
            return task.Task;
        });
        Click(failure, "primary");
        Field<TextBox>(failure, "directoryBox").Text = Path.Combine(root, "failure-preview");
        Click(failure, "primary");
        Capture(failure, "06-failure");
        failure.Close();
    }

    private static void CheckPaths()
    {
        foreach (string invalid in new[] { "", "relative", @"C:relative", @"C:\", "C:\\bad\nfolder",
            "C:\\quoted\"folder", @"C:\bad.\folder", @"C:\bad:folder", @"\\server\share\folder" })
        {
            bool rejected = false;
            try { InstallationRequest.Create(invalid); }
            catch (ArgumentException) { rejected = true; }
            Assert(rejected, "Invalid path rejected: " + invalid);
        }
        string occupied = Path.Combine(root, "unrelated");
        Directory.CreateDirectory(occupied);
        File.WriteAllText(Path.Combine(occupied, "keep.txt"), "untouched");
        bool refused = false;
        try { InstallationRequest.Create(occupied); } catch (ArgumentException) { refused = true; }
        Assert(refused && File.ReadAllText(Path.Combine(occupied, "keep.txt")) == "untouched", "Unrelated folder preserved");
        var request = InstallationRequest.Create(Path.Combine(root, "中文 安装路径"));
        Assert(request.EngineArguments() == "/S /D=" + request.Directory, "NSIS directory is last and unquoted");
    }

    private static InstallerWindow Window(string existing, Action<string> launch,
        Func<InstallationRequest, IProgress<InstallProgress>, System.Threading.Tasks.Task> run = null)
    {
        var window = new InstallerWindow(existing, run ?? new InstallerEngine().Run, launch) {
            ShowActivated = false, ShowInTaskbar = false, Left = -20000, Top = -20000,
            WindowStartupLocation = WindowStartupLocation.Manual,
        };
        window.Show();
        Pump();
        return window;
    }

    private static void CheckWizard()
    {
        int launches = 0;
        var window = Window(null, path => {
            launches++;
            if (launches == 1) throw new IOException("模拟启动失败");
        });
        Capture(window, "01-welcome");
        Click(window, "primary");
        string target = Path.Combine(root, "中文 安装位置");
        Field<TextBox>(window, "directoryBox").Text = target;
        Click(window, "back");
        Click(window, "primary");
        Assert(Field<TextBox>(window, "directoryBox").Text == target, "Back preserves directory draft");
        Field<TextBox>(window, "directoryBox").Text = "relative";
        Click(window, "primary");
        Assert(Field<int>(window, "step") == 1 && Field<Exception>(window, "failure") != null, "Path failure remains editable");
        Capture(window, "02-invalid-path");
        Field<TextBox>(window, "directoryBox").Text = target;
        Capture(window, "03-location");
        Environment.SetEnvironmentVariable("CCC_INSTALLER_CHECK_GATE", "wait");
        Click(window, "primary");
        Assert(Field<bool>(window, "busy") && !Field<Button>(window, "primary").IsEnabled,
            "Install locks duplicate action; state=" + Field<int>(window, "step") + "; " + Field<Exception>(window, "failure"));
        window.Close();
        Assert(window.IsVisible, "Closing is blocked while engine is active");
        Capture(window, "04-installing");
        File.WriteAllText(Path.Combine(root, "release-engine"), "continue isolated engine");
        Environment.SetEnvironmentVariable("CCC_INSTALLER_CHECK_GATE", null);
        Wait(window);
        Assert(Field<int>(window, "step") == 3 && window.ExitCode == 0, "Real fixture process reaches completion");
        Assert(launches == 0, "Completion never launches automatically");
        Assert(File.Exists(Path.Combine(target, "colorful-control-center.exe")), "Native NSIS honors /D with spaces and Chinese");
        Capture(window, "05-complete");
        Click(window, "primary");
        Assert(launches == 1 && window.IsVisible && window.ExitCode == 0, "Launch failure preserves installed success and retry");
        Capture(window, "06-launch-failed");
        Click(window, "primary");
        Assert(launches == 2 && !window.IsVisible, "Explicit launch can retry");
        var update = Window(target, path => { throw new Exception("Must not launch"); });
        Click(update, "primary");
        Assert(Field<TextBox>(update, "directoryBox").IsReadOnly, "Existing installation keeps its location");
        update.Width = 760;
        update.Height = 500;
        Capture(update, "07-existing-small", 1.5);
        update.Close();
        File.WriteAllText(Path.Combine(target, "colorful-control-center.exe"), "changed");
        bool mismatch = false;
        try { InstallerEngine.VerifyInstallation(target); } catch (IOException) { mismatch = true; }
        Assert(mismatch, "Stale or changed executable cannot pass completion verification");
    }

    private static void CheckFailure(string mode)
    {
        Environment.SetEnvironmentVariable("CCC_INSTALLER_CHECK_RESULT", mode);
        var window = Window(null, path => { throw new Exception("Must not launch"); });
        Click(window, "primary");
        Field<TextBox>(window, "directoryBox").Text = Path.Combine(root, mode);
        Click(window, "primary");
        Wait(window);
        Assert(Field<int>(window, "step") == 2 && window.ExitCode != 0, mode + " is never completed");
        var error = Field<Exception>(window, "failure") as InstallFailure;
        Assert(error != null && File.Exists(error.LogPath), mode + " preserves structured log location");
        if (mode == "tampered")
        {
            string log = File.ReadAllText(error.LogPath);
            Assert(log.Contains("ExpectedSHA256") && log.Contains("ActualSHA256") && log.Contains("ApplicationPath"),
                "Checksum failure retains exact expected/actual digest and target path");
            string diagnostic = InstallerDiagnostics.Describe(error);
            Assert(diagnostic.Contains("ExpectedSHA256") && diagnostic.Contains("ActualSHA256"),
                "Copied diagnostic retains structured details without parsing UI copy");
        }
        Assert(Directory.GetFiles(Path.Combine(root, "temp"), "install.log", SearchOption.AllDirectories).Length > 0,
            mode + " writes diagnostic log");
        Capture(window, "08-" + mode);
        Environment.SetEnvironmentVariable("CCC_INSTALLER_CHECK_RESULT", null);
        if (mode == "missing")
        {
            Click(window, "back");
            Assert(Field<TextBox>(window, "directoryBox").Text == Path.Combine(root, mode), "Return keeps partial-install target");
        }
        Click(window, "primary");
        Wait(window);
        Assert(Field<int>(window, "step") == 3 && window.ExitCode == 0, mode + " retries successfully");
        Click(window, "secondary");
        Assert(!window.IsVisible, "Finish closes without application launch");
    }

    private static void Wait(InstallerWindow window)
    {
        var time = Stopwatch.StartNew();
        while (Field<bool>(window, "busy") && time.ElapsedMilliseconds < 90000) Pump();
        Assert(!Field<bool>(window, "busy"), "Engine completed within fixture timeout");
    }

    private static void CheckRunningGuard()
    {
        string controller = Path.Combine(root, "colorful-control-center.exe");
        File.Copy(Path.Combine(root, "controller-fixture.exe"), controller);
        using (var process = Process.Start(new ProcessStartInfo(controller, "--hold") {
            UseShellExecute = false, CreateNoWindow = true,
        }))
        {
            try
            {
                System.Threading.Thread.Sleep(100);
                using (var guard = Process.Start(new ProcessStartInfo(Path.Combine(root, "running-guard.exe")) {
                    UseShellExecute = false, CreateNoWindow = true,
                }))
                {
                    if (!guard.WaitForExit(30000)) throw new Exception("NSIS guard timed out");
                    Assert(guard.ExitCode == 2, "NSIS rejects running file instead of force shutdown");
                }
                Assert(!process.HasExited, "Running controller fixture remains alive");
            }
            finally { if (!process.HasExited) { process.Kill(); process.WaitForExit(); } }
        }
        using (var guard = Process.Start(new ProcessStartInfo(Path.Combine(root, "running-guard.exe")) {
            UseShellExecute = false, CreateNoWindow = true,
        }))
        {
            if (!guard.WaitForExit(30000)) throw new Exception("Stopped NSIS guard timed out");
            Assert(guard.ExitCode == 0, "NSIS proceeds after normal controller exit");
        }
    }

    private static void Pump()
    {
        var frame = new DispatcherFrame();
        var timer = new DispatcherTimer(DispatcherPriority.Background) { Interval = TimeSpan.FromMilliseconds(20) };
        timer.Tick += delegate { timer.Stop(); frame.Continue = false; };
        timer.Start();
        Dispatcher.PushFrame(frame);
    }

    private static void CheckNativeUninstall()
    {
        string target = Path.Combine(root, "原生卸载 路径");
        var window = Window(null, path => { throw new Exception("Must not launch"); });
        Click(window, "primary");
        Field<TextBox>(window, "directoryBox").Text = target;
        Click(window, "primary");
        Wait(window);
        Assert(window.ExitCode == 0, "Native uninstall fixture was installed and verified");
        Click(window, "secondary");
        string keep = Path.Combine(target, "keep.txt");
        File.WriteAllText(keep, "unrelated content");
        string uninstaller = Path.Combine(target, "uninstall.exe");
        using (var process = Process.Start(new ProcessStartInfo(uninstaller, "/S _?=" + target) {
            UseShellExecute = false, CreateNoWindow = true,
        }))
        {
            if (!process.WaitForExit(30000)) throw new Exception("Native uninstall fixture timed out");
            Assert(process.ExitCode == 0, "Real NSIS uninstaller completed");
        }
        Assert(!File.Exists(Path.Combine(target, "colorful-control-center.exe")), "Native uninstaller removed its application");
        Assert(File.ReadAllText(keep) == "unrelated content", "Native uninstall preserved unrelated data");
        File.Delete(uninstaller);
    }

    private static void CheckFixtureBoundary()
    {
        string outside = Path.Combine(Path.GetDirectoryName(root), "rejected-" + Guid.NewGuid().ToString("N"));
        using (var process = Process.Start(new ProcessStartInfo(Path.Combine(root, "engine-fixture.exe"), "/S /D=" + outside) {
            UseShellExecute = false, CreateNoWindow = true,
        }))
        {
            if (!process.WaitForExit(30000)) throw new Exception("Fixture boundary check timed out");
            Assert(process.ExitCode == 92, "NSIS fixture rejected a target outside its private root");
        }
        Assert(!Directory.Exists(outside), "Rejected target was never created");
    }

    private static void Capture(InstallerWindow window, string name, double scale = 1, bool settle = true)
    {
        if (settle) Settle(window);
        Pump();
        window.UpdateLayout();
        var content = (FrameworkElement)window.Content;
        var bitmap = new RenderTargetBitmap((int)(content.ActualWidth * scale), (int)(content.ActualHeight * scale),
            96 * scale, 96 * scale, PixelFormats.Pbgra32);
        bitmap.Render(content);
        var encoder = new PngBitmapEncoder();
        encoder.Frames.Add(BitmapFrame.Create(bitmap));
        using (var file = File.Create(Path.Combine(root, name + ".png"))) encoder.Save(file);
        Assert(Field<Button>(window, "primary").ActualHeight >= 40, "Action remains visible: " + name);
    }

    private static T Field<T>(object instance, string name)
    {
        return (T)instance.GetType().GetField(name, BindingFlags.Instance | BindingFlags.NonPublic).GetValue(instance);
    }

    private static void Click(InstallerWindow window, string name)
    {
        Field<Button>(window, name).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        Pump();
    }

    private static void Assert(bool passed, string description)
    {
        if (!passed) throw new Exception(description);
        assertions++;
    }
}
