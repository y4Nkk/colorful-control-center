using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using ColorfulControlCenter.Installer;

internal static partial class InstallerChecks
{
    private static void CheckControllerShutdown()
    {
        string target = Path.Combine(root, "running-target");
        string executable = SeedController(target);
        string marker = Path.Combine(root, "controller");
        string sameName = Path.Combine(Directory.CreateDirectory(Path.Combine(root, "other-controller")).FullName,
            Path.GetFileName(executable));
        string otherMarker = Path.Combine(root, "other");
        File.Copy(executable, sameName);
        string sentinel = Path.Combine(target, "partial-file.txt");
        File.WriteAllText(sentinel, "original installation");
        string releaseName = "CCC.InstallerCheck." + Guid.NewGuid().ToString("N");
        using (var release = new EventWaitHandle(false, EventResetMode.ManualReset, releaseName))
        using (var holder = StartFixture(executable, "--hold " + releaseName))
        using (var controller = StartFixture(executable, ControllerArguments("--controller", marker)))
        using (var other = StartFixture(sameName, ControllerArguments("--controller", otherMarker)))
        {
            try
            {
                WaitForCondition(() => File.Exists(marker + ".ready") && File.Exists(otherMarker + ".ready"),
                    "Both hidden controller windows became available");
                Assert(CheckNativeGuard() == Product.ControllerRunningExitCode, "NSIS still blocks occupied files before authorization");
                var update = Window(target, path => { throw new Exception("Must not launch"); });
                Click(update, "primary");
                RespondToClosePrompt(update, false);
                Wait(update);
                Assert(Field<int>(update, "step") == 1 && Field<Exception>(update, "failure") == null,
                    "Cancel returns to the verified update location without an error");
                Assert(Field<TextBox>(update, "directoryBox").Text == target, "Cancel preserves update location");
                Assert(!controller.HasExited && !File.Exists(marker + ".queried"), "Cancel never requests controller exit");
                Assert(File.ReadAllText(sentinel) == "original installation", "Cancel leaves the installation intact");

                RespondToClosePrompt(update, true, "09-close-and-install");
                WaitForCondition(() => controller.HasExited, "Windows shuts down the verified controller");
                Assert(controller.ExitCode == 0 && File.Exists(marker + ".cleaned"), "The controller completes its normal exit path");
                Assert(!other.HasExited && !File.Exists(otherMarker + ".queried"), "Same-named app in another directory is never asked to exit");
                Assert(!holder.HasExited && Field<bool>(update, "busy"), "The recovery worker retains its process while the wizard waits");
                Assert(File.ReadAllText(sentinel) == "original installation", "Installation waits for the final executable lock");
                release.Set();
                Wait(update);
                Assert(holder.WaitForExit(3000) && holder.ExitCode == 0, "The recovery worker exits on its own signal");
                Assert(CheckNativeGuard() == 0, "NSIS removes the write block only after all processes release the file");
                Assert(Field<int>(update, "step") == 3 && update.ExitCode == 0, "Confirmed shutdown automatically continues to a complete install");
                Assert(File.ReadAllText(sentinel) == "isolated NSIS output", "NSIS actually ran after the lock cleared");
                Click(update, "secondary");
            }
            finally
            {
                release.Set();
                StopFixture(controller);
                StopFixture(other);
            }
        }
    }

    private static void CheckControllerVeto()
    {
        string target = Path.Combine(root, "refusing-controller");
        string executable = SeedController(target);
        string marker = Path.Combine(root, "refuses");
        using (var controller = StartFixture(executable, ControllerArguments("--refuse-exit", marker)))
        {
            try
            {
                WaitForCondition(() => File.Exists(marker + ".ready"), "Controller that refuses shutdown is running");
                var update = Window(target, path => { throw new Exception("Must not launch"); });
                Click(update, "primary");
                RespondToClosePrompt(update, true);
                Wait(update);
                var failure = Field<Exception>(update, "failure") as InstallFailure;
                Assert(failure != null && failure.Message.Contains("系统托盘"), "Refusal shows manual exit guidance");
                Assert(!controller.HasExited && File.Exists(marker + ".queried") && !File.Exists(marker + ".cleaned"),
                    "The wizard respects a refused shutdown");
                string log = File.ReadAllText(failure.LogPath);
                Assert(log.Contains("NSIS exit code: " + Product.ControllerRunningExitCode) && log.Contains("WindowsErrorCode"),
                    "The failure log retains the guard result and native exit error");
                Capture(update, "10-close-refused");
                Click(update, "secondary");
            }
            finally { StopFixture(controller); }
        }
    }

    private static string SeedController(string target)
    {
        var initial = Window(null, path => { throw new Exception("Must not launch"); });
        Click(initial, "primary");
        Field<TextBox>(initial, "directoryBox").Text = target;
        Click(initial, "primary");
        Wait(initial);
        Assert(initial.ExitCode == 0, "Existing fixture first installs successfully");
        Click(initial, "secondary");
        return Path.Combine(target, Product.ApplicationFileStem + ".exe");
    }

    private static void RespondToClosePrompt(InstallerWindow update, bool confirm, string capture = null)
    {
        bool answered = false;
        Exception error = null;
        var watcher = new DispatcherTimer(DispatcherPriority.Background) { Interval = TimeSpan.FromMilliseconds(25) };
        watcher.Tick += delegate {
            var dialog = update.OwnedWindows.Cast<Window>().SingleOrDefault(window => window.IsVisible);
            if (dialog == null) return;
            watcher.Stop();
            try
            {
                var frame = (Border)dialog.Content;
                var content = (StackPanel)frame.Child;
                var buttons = (StackPanel)content.Children[content.Children.Count - 1];
                var cancel = (Button)buttons.Children[0];
                var closeAndInstall = (Button)buttons.Children[1];
                Assert(cancel.IsCancel && (string)closeAndInstall.Content == "关闭并安装", "Modal offers cancel and explicit shutdown");
                Assert(Field<bool>(update, "busy") && !Field<Button>(update, "primary").IsEnabled, "Modal cannot repeat the install action");
                if (capture != null)
                {
                    CaptureContent(dialog, capture, 1.5);
                    var corner = closeAndInstall.TranslatePoint(new Point(closeAndInstall.ActualWidth, closeAndInstall.ActualHeight), frame);
                    Assert(closeAndInstall.ActualHeight >= 40 && corner.X <= frame.ActualWidth && corner.Y <= frame.ActualHeight,
                        "Modal actions fit within the client area");
                }
                (confirm ? closeAndInstall : cancel).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
                answered = true;
            }
            catch (Exception failure) { error = failure; dialog.Close(); }
        };
        watcher.Start();
        try
        {
            Click(update, "primary");
            WaitForCondition(() => answered || error != null || !Field<bool>(update, "busy"),
                "The window responds to the occupied installer state");
        }
        finally { watcher.Stop(); }
        if (error != null) throw error;
        Assert(answered, "Running controller opens the shutdown confirmation");
    }

    private static string ControllerArguments(string command, string marker)
    {
        return command + " \"" + Product.ControllerWindowTitle + "\" \"" + marker + "\"";
    }

    private static Process StartFixture(string executable, string arguments)
    {
        return Process.Start(new ProcessStartInfo(executable, arguments) { UseShellExecute = false, CreateNoWindow = true });
    }

    private static int CheckNativeGuard()
    {
        using (var guard = StartFixture(Path.Combine(root, "running-guard.exe"), ""))
        {
            if (!guard.WaitForExit(30000)) throw new Exception("NSIS guard timed out");
            return guard.ExitCode;
        }
    }

    private static void WaitForCondition(Func<bool> condition, string description)
    {
        var elapsed = Stopwatch.StartNew();
        while (!condition() && elapsed.Elapsed < TimeSpan.FromSeconds(45)) Pump();
        Assert(condition(), description);
    }

    private static void StopFixture(Process fixture)
    {
        if (!fixture.HasExited) { fixture.Kill(); fixture.WaitForExit(3000); }
    }
}
