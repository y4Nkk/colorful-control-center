using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Threading;
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
            CheckCompactLocation();
            if (arguments.Length > 1 && arguments[1] == "render")
            {
                CheckLayout();
                File.WriteAllText(Path.Combine(root, "result.json"), "{\"passed\":true,\"renderOnly\":true}");
                return 0;
            }
            CheckPaths();
            CheckWizard();
            CheckUnavailableLocation();
            CheckRecoveredLocation();
            CheckFailure("failure");
            CheckFailure("check-failed");
            CheckFailure("missing");
            CheckFailure("tampered");
            CheckControllerShutdown();
            CheckControllerVeto();
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
        var window = Window(null, path => { throw new Exception("Preview must not launch"); }, (request, progress, confirm) => {
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
        var failure = Window(null, path => { throw new Exception("Preview must not launch"); }, (request, progress, confirm) => {
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
        Assert(!Directory.Exists(request.Directory), "Confirming a new path does not create it");
        RejectPath(Path.Combine(occupied, "keep.txt", "child"), "包含文件");
        RejectPath(Path.Combine(root, new string('x', 220 - root.Length)), "220 个字符");
        var drives = DriveInfo.GetDrives();
        var unused = Enumerable.Range('D', 'Z' - 'D' + 1).Select(letter => (char)letter)
            .FirstOrDefault(letter => !drives.Any(drive => Char.ToUpperInvariant(drive.Name[0]) == letter));
        if (unused != '\0') RejectPath(unused + @":\ColorfulControlCenter", "安装磁盘不可用");
        CheckWriteAccess();
    }

    private static void CheckWriteAccess()
    {
        var directory = Directory.CreateDirectory(Path.Combine(root, "read-only-target"));
        var restricted = directory.GetAccessControl(AccessControlSections.Access);
        string original = restricted.GetSecurityDescriptorSddlForm(AccessControlSections.Access);
        using (var identity = WindowsIdentity.GetCurrent())
            restricted.AddAccessRule(new FileSystemAccessRule(identity.User,
                FileSystemRights.CreateFiles | FileSystemRights.CreateDirectories, AccessControlType.Deny));
        directory.SetAccessControl(restricted);
        try
        {
            RejectPath(directory.FullName, "无法写入此文件夹");
            RejectPath(Path.Combine(directory.FullName, "new", "installation"), "无法在上级目录新建");
            Assert(!directory.EnumerateFileSystemInfos().Any(), "Permission checks leave the target untouched");
        }
        finally
        {
            restricted.SetSecurityDescriptorSddlForm(original, AccessControlSections.Access);
            directory.SetAccessControl(restricted);
        }
        Assert(InstallationRequest.Create(directory.FullName).Directory == directory.FullName,
            "The same directory is accepted after write access is restored");
    }

    private static void RejectPath(string path, string message)
    {
        try { InstallationRequest.Create(path); }
        catch (ArgumentException error)
        {
            Assert(error.Message.Contains(message), "Path error identifies the problem: " + error.Message);
            return;
        }
        Assert(false, "Path unexpectedly passed validation: " + path);
    }

    private static InstallerWindow Window(string existing, Action<string> launch,
        Func<InstallationRequest, IProgress<InstallProgress>, Func<bool>, System.Threading.Tasks.Task<bool>> run = null)
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
        Assert(Field<int>(window, "step") == 1 && Field<TextBlock>(window, "locationError").Visibility == Visibility.Visible,
            "Path failure is displayed on the editable location page");
        Capture(window, "02-invalid-path");
        Field<TextBox>(window, "directoryBox").Text = target;
        Assert(Field<TextBlock>(window, "locationError").Visibility == Visibility.Collapsed, "Editing clears stale path feedback");
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

    private static void CheckUnavailableLocation()
    {
        string old = Directory.CreateDirectory(Path.Combine(root, "unavailable-location")).FullName;
        string keep = Path.Combine(old, "keep.txt");
        File.WriteAllText(keep, "unrelated");
        var window = Window(old, path => { throw new Exception("Must not launch"); });
        Click(window, "primary");
        var box = Field<TextBox>(window, "directoryBox");
        Assert(!box.IsReadOnly && box.Text.Length == 0 && FindButton((DependencyObject)window.Content, "选择安装位置…") != null,
            "An unavailable registered location offers a different directory");
        Capture(window, "location-recovery");
        box.Text = old;
        Click(window, "primary");
        Assert(Field<int>(window, "step") == 1 && Field<TextBlock>(window, "locationError").Text.Contains("空文件夹"),
            "Recovery will not reuse a directory containing unrelated files");
        string target = Path.Combine(root, "恢复 中文目录");
        box.Text = target;
        Click(window, "primary");
        Wait(window);
        Assert(window.ExitCode == 0 && File.Exists(Path.Combine(target, Product.ApplicationFileStem + ".exe"))
            && File.ReadAllText(keep) == "unrelated", "Recovery installs into the confirmed location and preserves old files");
        window.Close();
    }

    private static void CheckRecoveredLocation()
    {
        string old = Path.Combine(root, "restored-location");
        SeedInstalledFiles(old);
        int calls = 0;
        var window = Window(old, path => { throw new Exception("Must not launch"); }, (request, progress, confirm) => {
            calls++;
            Assert(request.Directory == old, "Restored installation receives an in-place update");
            return System.Threading.Tasks.Task.FromResult(false);
        });
        Click(window, "primary");
        Assert(Field<TextBox>(window, "directoryBox").IsReadOnly, "Usable registered location is read-only");
        File.Delete(Path.Combine(old, Product.ApplicationFileStem + ".exe"));
        Click(window, "primary");
        Assert(calls == 0 && !Field<TextBox>(window, "directoryBox").IsReadOnly,
            "A location lost before confirmation becomes editable without submitting");
        SeedInstalledFiles(old);
        Field<TextBox>(window, "directoryBox").Text = Path.Combine(root, "alternative");
        Click(window, "primary");
        Assert(calls == 0 && Field<TextBox>(window, "directoryBox").Text == old
            && Field<TextBox>(window, "directoryBox").IsReadOnly, "Recovery rechecks the original before accepting a replacement");
        Click(window, "primary");
        Assert(calls == 1, "Updating the restored location requires explicit confirmation");
        window.Close();
    }

    private static void SeedInstalledFiles(string folder)
    {
        Directory.CreateDirectory(folder);
        File.WriteAllText(Path.Combine(folder, Product.ApplicationFileStem + ".exe"), "installed controller fixture");
        File.WriteAllText(Path.Combine(folder, "uninstall.exe"), "installed uninstaller fixture");
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
        if (mode == "failure") Assert(error.EngineExitCode == 23, "Unclassified engine failure retains its exit code");
        if (mode == "check-failed")
        {
            Assert(error.EngineExitCode == Product.ProcessCheckFailedExitCode, "Check failures retain their dedicated code");
            Assert(Field<TextBlock>(window, "heading").Text == "安装未完成", "Check failure differs from a running controller");
        }
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
        CaptureContent(window, name, scale);
        Assert(Field<Button>(window, "primary").ActualHeight >= 40, "Action remains visible: " + name);
    }

    private static void CaptureContent(Window window, string name, double scale)
    {
        window.UpdateLayout();
        var content = (FrameworkElement)window.Content;
        var bitmap = new RenderTargetBitmap((int)(content.ActualWidth * scale), (int)(content.ActualHeight * scale),
            96 * scale, 96 * scale, PixelFormats.Pbgra32);
        bitmap.Render(content);
        var encoder = new PngBitmapEncoder();
        encoder.Frames.Add(BitmapFrame.Create(bitmap));
        using (var file = File.Create(Path.Combine(root, name + ".png"))) encoder.Save(file);
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
