using System;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using ColorfulControlCenter.Installer;

internal static partial class InstallerChecks
{
    private static void CheckTransitions()
    {
        var completion = new System.Threading.Tasks.TaskCompletionSource<bool>();
        IProgress<InstallProgress> report = null;
        var window = Window(null, path => { throw new Exception("Animation fixture must not launch"); }, (request, progress) => {
            report = progress;
            return completion.Task;
        });
        window.Resources["MotionMilliseconds"] = 600d;
        CheckOverlay(window);
        Click(window, "primary");
        Assert(FindButton((DependencyObject)window.Content, "选择安装位置…") != null, "Fresh install exposes directory picker");
        bool motion = SystemParameters.ClientAreaAnimation && !SystemParameters.HighContrast;
        var content = Field<StackPanel>(window, "body");
        var offset = Field<TranslateTransform>(window, "bodyOffset");
        if (motion)
        {
            Assert(content.HasAnimatedProperties && offset.X > 0, "Forward step fades and moves into place");
            Capture(window, "motion-forward", 1, false);
        }
        string draft = Path.Combine(root, "motion-draft");
        Field<TextBox>(window, "directoryBox").Text = draft;
        Click(window, "back");
        if (motion) Assert(offset.X < 0, "Rapid backward step replaces forward motion");
        Click(window, "primary");
        Assert(Field<TextBox>(window, "directoryBox").Text == draft, "Rapid transitions preserve directory draft");
        Settle(window);
        Assert(content.Opacity == 1 && offset.X == 0, "Transition settles at its base values");
        window.Width = 760;
        window.Height = 500;
        window.UpdateLayout();
        CheckOverlay(window);
        Click(window, "primary");
        int serial = Field<int>(window, "transitionSerial");
        report.Report(new InstallProgress { Phase = "isolated progress refresh", Fraction = 0.5 });
        Pump();
        Assert(Field<int>(window, "transitionSerial") == serial, "Progress refresh does not replay step transition");
        completion.SetResult(true);
        Wait(window);
        Settle(window);
        Assert(Field<int>(window, "step") == 3 && content.Opacity == 1 && offset.X == 0, "Completion settles after rapid state updates");
        window.Close();
        CheckReducedMotion();
    }

    private static void CheckReducedMotion()
    {
        var window = Window(Path.Combine(root, "existing-install"), path => { throw new Exception("Must not launch"); });
        window.Resources["MotionMilliseconds"] = 0d;
        Click(window, "primary");
        var content = Field<StackPanel>(window, "body");
        Assert(!content.HasAnimatedProperties && Field<TranslateTransform>(window, "bodyOffset").X == 0,
            "Reduced motion shows the new step immediately");
        Assert(Field<TextBox>(window, "directoryBox").IsReadOnly, "Update preserves registered install location");
        Assert(FindButton((DependencyObject)window.Content, "打开安装目录") != null, "Update exposes current directory action");
        Capture(window, "update-location");
        window.Close();
    }

    private static void CheckOverlay(InstallerWindow window)
    {
        window.UpdateLayout();
        var panel = Field<Border>(window, "workspacePanel");
        var layout = (Grid)window.Content;
        double radius = (double)window.FindResource("PanelRadius");
        double origin = panel.TranslatePoint(new Point(), layout).X;
        Assert(Math.Abs(origin - (layout.ColumnDefinitions[0].ActualWidth - radius)) < 0.1,
            "White panel overlaps the gray pane by its corner radius");
        Assert(!panel.Clip.FillContains(new Point(1, 1))
            && !panel.Clip.FillContains(new Point(1, panel.ActualHeight - 1))
            && panel.Clip.FillContains(new Point(radius + 1, 1)), "Left corners clip correctly after resize");
    }

    private static void Settle(InstallerWindow window)
    {
        var time = Stopwatch.StartNew();
        while (HasAnimations(window) && time.ElapsedMilliseconds < 3000) Pump();
        Assert(!HasAnimations(window),
            "Completed animation clocks are released");
    }

    private static bool HasAnimations(InstallerWindow window)
    {
        if (Field<StackPanel>(window, "body").HasAnimatedProperties
            || Field<TranslateTransform>(window, "bodyOffset").HasAnimatedProperties
            || Field<FrameworkElement>(window, "actions").HasAnimatedProperties) return true;
        foreach (var marker in Field<Label[]>(window, "markers"))
            if (marker.HasAnimatedProperties || ((SolidColorBrush)marker.Background).HasAnimatedProperties) return true;
        return false;
    }

    private static Button FindButton(DependencyObject root, string label)
    {
        var button = root as Button;
        if (button != null && Object.Equals(button.Content, label)) return button;
        for (int index = 0; index < VisualTreeHelper.GetChildrenCount(root); index++)
        {
            var match = FindButton(VisualTreeHelper.GetChild(root, index), label);
            if (match != null) return match;
        }
        return null;
    }
}
