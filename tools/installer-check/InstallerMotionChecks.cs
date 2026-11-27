using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
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
        var window = Window(null, path => { throw new Exception("Animation fixture must not launch"); }, (request, progress, confirm) => {
            report = progress;
            return completion.Task;
        });
        window.Resources["MotionMilliseconds"] = 600d;
        var size = new Size(window.ActualWidth, window.ActualHeight);
        CheckWorkspace(window, true);
        bool motion = SystemParameters.ClientAreaAnimation && !SystemParameters.HighContrast;
        var content = Field<StackPanel>(window, "body");
        var offset = Field<TranslateTransform>(window, "bodyOffset");
        string draft = Path.Combine(root, "motion-draft");
        SelectLocation(window, draft);
        Assert(FindButton((DependencyObject)window.Content, "浏览…") != null, "Fresh install exposes directory picker");
        Click(window, "primary");
        if (motion)
        {
            Assert(content.HasAnimatedProperties && offset.X > 0, "Forward step fades and moves into place");
            Capture(window, "motion-forward", 1, false);
        }
        completion.SetResult(false);
        Wait(window);
        Pump();
        if (motion) Assert(offset.X < 0, "Canceled install replaces forward motion");
        Assert(Stage(window) == InstallerStep.Confirm && Field<string>(window, "selectedDirectory") == draft,
            "Canceling installation preserves the confirmed location");
        Settle(window);
        CheckFixedSize(window, size);
        Assert(content.Opacity == 1 && offset.X == 0, "Transition settles at its base values");
        completion = new System.Threading.Tasks.TaskCompletionSource<bool>();
        Click(window, "primary");
        int serial = Field<int>(window, "transitionSerial");
        var bar = Field<ProgressBar>(window, "progressBar");
        report.Report(new InstallProgress { Phase = "isolated progress refresh", Fraction = 0.5 });
        Pump();
        Assert(Field<int>(window, "transitionSerial") == serial && ReferenceEquals(bar, Field<ProgressBar>(window, "progressBar"))
            && !bar.IsIndeterminate && bar.Value == 0.5, "Progress updates in place without replaying step transition");
        report.Report(new InstallProgress { Phase = "正在安装程序与运行组件…" });
        Pump();
        Assert(bar.IsIndeterminate && !Field<TextBlock>(window, "phaseStatus").Text.Contains("%"),
            "Unknown total clears the file preparation percentage");
        completion.SetResult(true);
        Wait(window);
        Settle(window);
        CheckFixedSize(window, size);
        Assert(Stage(window) == InstallerStep.Complete && content.Opacity == 1 && offset.X == 0, "Completion settles after rapid state updates");
        window.Close();
        CheckReducedMotion();
    }

    private static void CheckReducedMotion()
    {
        string installed = Path.Combine(root, "工作空间", "离线安装与升级验证", "Colorful Control Center");
        SeedInstalledFiles(installed);
        var completion = new System.Threading.Tasks.TaskCompletionSource<bool>();
        var window = Window(installed, path => { throw new Exception("Must not launch"); }, (request, progress, confirm) => completion.Task);
        window.Resources["MotionMilliseconds"] = 0d;
        Assert(Field<TextBox>(window, "directoryBox") == null, "Update preserves registered install location");
        Assert(FindButton((DependencyObject)window.Content, "打开目录") != null, "Update exposes current directory action");
        Capture(window, "update-location");
        var summary = Field<StackPanel>(window, "body").Children.OfType<Grid>().Single()
            .Children.OfType<TextBlock>().Single();
        Assert(summary.Text == installed && summary.TextWrapping == TextWrapping.NoWrap
            && summary.TextTrimming == TextTrimming.CharacterEllipsis,
            "The update summary trims the displayed line while keeping the registered path");
        Click(window, "primary");
        var content = Field<StackPanel>(window, "body");
        Assert(!content.HasAnimatedProperties && Field<TranslateTransform>(window, "bodyOffset").X == 0,
            "Reduced motion shows the new step immediately");
        completion.SetResult(false);
        Wait(window);
        window.Close();
    }

    private static void CheckCompactLocation()
    {
        var window = Window(null, path => { throw new Exception("Must not launch"); }, workArea: new Size(650, 395));
        var size = new Size(window.ActualWidth, window.ActualHeight);
        Capture(window, "compact-location");
        CheckWorkspace(window, false);
        CheckFooter(window);
        Assert(Field<ScrollViewer>(window, "bodyScroll").ScrollableHeight == 0,
            "Compact confirmation fits the folder and runtime notice without scrolling");
        string directory = @"D:\应用软件\硬件调校\笔记本性能管理\测试环境\用于验证安装向导单行省略的目录\用户安装目录\Colorful Control Center 2.0.0";
        SelectLocation(window, directory);
        Capture(window, "compact-editor");
        CheckFixedSize(window, size);
        Assert(Field<ScrollViewer>(window, "bodyScroll").ScrollableHeight == 0,
            "A long directory keeps the runtime notice in view in the compact editor");
        var editor = Field<TextBox>(window, "directoryBox");
        var preview = (TextBlock)editor.Template.FindName("PathPreview", editor);
        var host = (ScrollViewer)editor.Template.FindName("PART_ContentHost", editor);
        Assert(preview.IsVisible && host.Opacity == 0 && preview.Text == directory
            && preview.TextTrimming == TextTrimming.CharacterEllipsis && preview.TextWrapping == TextWrapping.NoWrap,
            "Opening the editor shows a single trimmed directory before the field is focused");
        editor.ScrollToHorizontalOffset(host.ScrollableWidth);
        Pump();
        Assert(editor.LineCount == 1 && host.HorizontalOffset > 0 && editor.Text == directory
            && Field<string>(window, "selectedDirectory") == directory, "The complete path is editable on one line");
        var browse = FindButton((DependencyObject)window.Content, "浏览…");
        DependencyObject viewport = VisualTreeHelper.GetParent(browse);
        while (viewport != null && !(viewport is ScrollContentPresenter))
            viewport = VisualTreeHelper.GetParent(viewport);
        Assert(viewport != null && new Rect(((FrameworkElement)viewport).RenderSize)
            .Contains(Bounds(browse, (Visual)viewport)), "The directory picker is visible at 650 by 395 without scrolling");
        window.Close();
        var narrow = Window(null, path => { throw new Exception("Must not launch"); }, workArea: new Size(334, 520));
        CheckFooter(narrow);
        narrow.Close();
    }

    private static void CheckFooter(InstallerWindow window)
    {
        var panel = Field<Border>(window, "workspacePanel");
        var cancel = Bounds(Field<Button>(window, "secondary"), panel);
        var confirm = Bounds(Field<Button>(window, "primary"), panel);
        var area = new Rect(panel.RenderSize);
        Assert(area.Contains(cancel) && area.Contains(confirm), "Footer actions fit inside the workspace");
        Assert(!cancel.IntersectsWith(confirm), "Footer actions do not overlap in a compact window");
    }

    private static Rect Bounds(FrameworkElement element, Visual ancestor)
    {
        return element.TransformToAncestor(ancestor).TransformBounds(new Rect(element.RenderSize));
    }

    private static void CheckWorkspace(InstallerWindow window, bool wide)
    {
        window.UpdateLayout();
        var panel = Field<Border>(window, "workspacePanel");
        var layout = (Grid)panel.Parent;
        double radius = (double)window.FindResource("PanelRadius");
        double origin = panel.TranslatePoint(new Point(), layout).X;
        if (!wide || SystemParameters.HighContrast)
        {
            Assert(origin == 0 && layout.ColumnDefinitions[0].ActualWidth == 0 && panel.Clip == null,
                "Compact workspace hides the artwork and uses the full width");
            return;
        }
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
        return Field<StackPanel>(window, "body").HasAnimatedProperties
            || Field<TranslateTransform>(window, "bodyOffset").HasAnimatedProperties
            || Field<FrameworkElement>(window, "actions").HasAnimatedProperties;
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
