using System;
using System.Reflection;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shell;

namespace ColorfulControlCenter.Installer
{
    internal sealed partial class InstallerWindow
    {
        private ScrollViewer bodyScroll;
        private Border workspacePanel;

        private Brush Brush(string key) { return (Brush)FindResource(key); }
        private double Measure(string key) { return (double)FindResource(key); }

        private Grid BuildLayout()
        {
            var inset = new Thickness(ShadowMargin);
            bool compact = Width - 2 * ShadowMargin < 760 || Height - 2 * ShadowMargin < 440
                || SystemParameters.HighContrast;
            var layout = new Grid { Background = Brush(compact ? "Surface" : "Canvas"), ClipToBounds = true, Margin = inset };
            double radius = Measure("PanelRadius");
            layout.ColumnDefinitions.Add(new ColumnDefinition {
                Width = compact ? new GridLength(0) : new GridLength(0.45, GridUnitType.Star),
            });
            layout.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(0.55, GridUnitType.Star) });
            if (!compact) layout.Children.Add(BuildArtwork(radius));
            var workspace = new Grid {
                Margin = compact ? new Thickness(22, 62, 12, 10) : new Thickness(radius + 36, 96, 30, 24),
                MaxWidth = compact ? 700 : Double.PositiveInfinity,
            };
            workspace.RowDefinitions.Add(new RowDefinition());
            workspace.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            body = new StackPanel { RenderTransform = bodyOffset };
            bodyScroll = new ScrollViewer { Content = body, VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled, Focusable = false,
                Padding = new Thickness(0, 0, 10, 0) };
            workspace.Children.Add(bodyScroll);
            actions = BuildFooter();
            Grid.SetRow(actions, 1);
            workspace.Children.Add(actions);
            workspacePanel = new Border {
                Background = Brush("Surface"), Child = workspace,
                Margin = new Thickness(compact ? 0 : -radius, 0, 0, 0),
                CornerRadius = compact ? new CornerRadius(0) : new CornerRadius(radius, 0, 0, radius),
            };
            if (!compact)
                workspacePanel.SizeChanged += delegate {
                    workspacePanel.Clip = new RectangleGeometry(new Rect(0, 0,
                        workspacePanel.ActualWidth + radius, workspacePanel.ActualHeight), radius, radius);
                };
            Grid.SetColumn(workspacePanel, 1);
            layout.Children.Add(workspacePanel);
            var header = BuildHeader();
            header.Margin = compact ? new Thickness(22, 4, 10, 0) : new Thickness(30, 12, 16, 0);
            Grid.SetColumnSpan(header, 2);
            layout.Children.Add(header);
            var edge = new Border { BorderBrush = Brush("Hairline"), BorderThickness = new Thickness(1),
                Margin = inset, IsHitTestVisible = false };
            return new Grid { Children = { layout, edge } };
        }

        private Border BuildArtwork(double radius)
        {
            return new Border { IsHitTestVisible = false, Margin = new Thickness(26, 68, radius + 26, 42),
                Background = new ImageBrush(Image("CCC.Artwork")) { Stretch = Stretch.Uniform } };
        }

        private Grid BuildHeader()
        {
            var header = new Grid { Height = 44, VerticalAlignment = VerticalAlignment.Top };
            header.ColumnDefinitions.Add(new ColumnDefinition());
            header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var brand = new StackPanel { Orientation = Orientation.Horizontal };
            brand.Children.Add(new Image { Source = Image("CCC.Icon"), Width = 28, Height = 28,
                Margin = new Thickness(0, 0, 10, 0) });
            var name = Text(Product.ControllerWindowTitle, 17);
            name.FontWeight = FontWeights.SemiBold;
            name.VerticalAlignment = VerticalAlignment.Center;
            brand.Children.Add(name);
            header.Children.Add(new Viewbox { Child = brand, StretchDirection = StretchDirection.DownOnly,
                HorizontalAlignment = HorizontalAlignment.Left });
            header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var minimize = Action("−", delegate { WindowState = WindowState.Minimized; }, "最小化");
            minimize.Style = (Style)FindResource("Chrome");
            Grid.SetColumn(minimize, 1);
            header.Children.Add(minimize);
            close = Action("×", delegate { Close(); }, "关闭安装向导");
            close.Style = (Style)FindResource("Chrome");
            Grid.SetColumn(close, 2);
            header.Children.Add(close);
            return header;
        }

        private WrapPanel BuildFooter()
        {
            var footer = new WrapPanel { HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 12, 10, 0) };
            secondary = Action("取消", delegate { Close(); }, "取消");
            primary = Action("", OnPrimary, "");
            primary.Style = (Style)FindResource("Primary");
            foreach (var button in new[] { secondary, primary })
            {
                button.Margin = new Thickness(8, 0, 0, 8);
                footer.Children.Add(button);
            }
            return footer;
        }

        private void RenderBody()
        {
            body.Children.Clear();
            directoryBox = null;
            changeLocation = null;
            phaseStatus = null;
            progressBar = null;
            string action = Updating ? "更新" : "安装";
            string title = step == InstallerStep.Confirm ? action + " " + Product.ControllerWindowTitle
                : step == InstallerStep.Complete ? action + "完成"
                : busy ? "正在" + action : action + "未完成";
            if (step == InstallerStep.Confirm && recoveryReason != null) title = "重新安装控制台";
            heading = Text(title, 28);
            heading.FontWeight = FontWeights.SemiBold;
            heading.LineHeight = 36;
            heading.Focusable = true;
            KeyboardNavigation.SetIsTabStop(heading, false);
            AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level1);
            body.Children.Add(heading);
            if (step == InstallerStep.Confirm) BuildLocation();
            else if (busy) BuildProgress();
            else if (step == InstallerStep.Complete && failure == null) BuildComplete();
            if (failure != null) BuildError();
        }

        private bool ConfirmControllerClose()
        {
            var dialog = new Window { Owner = this, Title = Product.ControllerWindowTitle,
                ShowInTaskbar = false, WindowStartupLocation = WindowStartupLocation.CenterOwner,
                Width = Math.Min(440, SystemParameters.WorkArea.Width - 32), SizeToContent = SizeToContent.Height,
                ResizeMode = ResizeMode.NoResize, Background = Brush("Surface"), FontFamily = FontFamily };
            dialog.Resources.MergedDictionaries.Add(Resources);
            var content = new StackPanel();
            string action = Updating ? "更新" : "安装";
            var title = Text("关闭控制台后" + action, 21);
            title.FontWeight = FontWeights.SemiBold;
            content.Children.Add(title);
            content.Children.Add(Text("控制台关闭后会自动继续。" +
                "如果控制台以管理员身份运行，Windows 会请求授权。", 14, "Muted", new Thickness(0, 10, 0, 22)));
            var actions = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right };
            var cancel = Action("取消", delegate { dialog.DialogResult = false; }, "取消");
            cancel.IsCancel = true;
            var confirm = Action("关闭并" + action, delegate { dialog.DialogResult = true; }, "关闭并" + action);
            confirm.Style = (Style)FindResource("Primary");
            confirm.Margin = new Thickness(10, 0, 0, 0);
            actions.Children.Add(cancel);
            actions.Children.Add(confirm);
            content.Children.Add(actions);
            dialog.Content = new Border { Padding = new Thickness(24), Background = Brush("Surface"), Child = content };
            dialog.Loaded += delegate { cancel.Focus(); };
            return dialog.ShowDialog() == true;
        }

        private void BuildLocation()
        {
            string description = Updating ? "将更新到 " + Product.Version + "，保留你的设置。"
                : "版本 " + Product.Version + " · 仅为当前用户安装";
            body.Children.Add(Text(description, 14, "Muted", new Thickness(0, 8, 0, 12)));
            if (recoveryReason != null)
                body.Children.Add(Text("原安装位置不可用。" + recoveryReason, 14, "Error", new Thickness(0, 0, 0, 8)));
            var label = Text("安装位置", 14);
            label.FontWeight = FontWeights.SemiBold;
            body.Children.Add(label);
            if (editingLocation) BuildLocationEditor(label);
            else BuildLocationSummary();
            if (replacementDirectory != null)
                body.Children.Add(Text("此目录已有控制台。继续会替换原程序文件。", 14, "Muted", new Thickness(0, 4, 0, 0)));
            body.Children.Add(Text("如缺少 WebView2，安装时需要联网。", 14, "Muted", new Thickness(0, 6, 0, 0)));
        }

        private void BuildLocationSummary()
        {
            var row = new Grid { Margin = new Thickness(0, 0, 0, 2) };
            row.ColumnDefinitions.Add(new ColumnDefinition());
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var path = Text(selectedDirectory, 14, "Ink", new Thickness(0, 4, 12, 4));
            path.VerticalAlignment = VerticalAlignment.Center;
            path.TextWrapping = TextWrapping.NoWrap;
            path.TextTrimming = TextTrimming.CharacterEllipsis;
            row.Children.Add(path);
            changeLocation = Updating ? QuietAction("打开目录", Browse) :
                QuietAction("更改…", delegate { EditLocation(); });
            AutomationProperties.SetName(changeLocation, Updating ? "打开安装目录" : "更改安装位置");
            Grid.SetColumn(changeLocation, 1);
            row.Children.Add(changeLocation);
            body.Children.Add(row);
        }

        private void BuildLocationEditor(TextBlock label)
        {
            var row = new Grid { Margin = new Thickness(0, 4, 0, 0) };
            row.ColumnDefinitions.Add(new ColumnDefinition());
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            directoryBox = new TextBox { Text = selectedDirectory, TextWrapping = TextWrapping.NoWrap,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
                Margin = new Thickness(0, 0, 8, 0), MinHeight = 42 };
            AutomationProperties.SetLabeledBy(directoryBox, label);
            row.Children.Add(directoryBox);
            var browse = QuietAction("浏览…", Browse);
            AutomationProperties.SetName(browse, "浏览安装文件夹");
            Grid.SetColumn(browse, 1);
            browse.VerticalAlignment = VerticalAlignment.Center;
            row.Children.Add(browse);
            body.Children.Add(row);
            locationError = Text("", 14, "Error", new Thickness(0, 4, 0, 0));
            locationError.Visibility = Visibility.Collapsed;
            AutomationProperties.SetLiveSetting(locationError, AutomationLiveSetting.Polite);
            body.Children.Add(locationError);
            directoryBox.TextChanged += delegate {
                selectedDirectory = directoryBox.Text;
                locationError.Visibility = Visibility.Collapsed;
            };
            body.Children.Add(Text("可选择空文件夹，或输入新目录的完整路径。", 14, "Muted", new Thickness(0, 6, 0, 0)));
        }

        private void BuildProgress()
        {
            phaseStatus = Text("正在检查安装条件…", 14, "Muted", new Thickness(0, 12, 0, 14));
            AutomationProperties.SetLiveSetting(phaseStatus, AutomationLiveSetting.Polite);
            body.Children.Add(phaseStatus);
            progressBar = new ProgressBar { Height = 6, Minimum = 0, Maximum = 1,
                IsIndeterminate = true, BorderThickness = new Thickness(0),
                Foreground = Brush("ActionFill"), Background = Brush("MutedSurface") };
            AutomationProperties.SetName(progressBar, "安装进行中");
            body.Children.Add(progressBar);
        }

        private void BuildComplete()
        {
            body.Children.Add(Text("已" + (Updating ? "更新至" : "安装") + "版本 " + Product.Version + "。",
                14, "Muted", new Thickness(0, 8, 0, 0)));
        }

        private void BuildError()
        {
            string message = step == InstallerStep.Complete ? "控制台未能打开。可重试，或稍后从开始菜单打开。"
                : failure.Message;
            body.Children.Add(Text(message, 14, "Error", new Thickness(0, 8, 0, 12)));
            if (step == InstallerStep.Install && !Updating)
                body.Children.Add(QuietAction("更改安装位置…", delegate { EditLocation(); }));
            var details = new StackPanel { Margin = new Thickness(16, 6, 0, 0) };
            var installFailure = failure as InstallFailure;
            if (installFailure != null)
                details.Children.Add(Text(installFailure.LogPath, 14, "Muted"));
            var feedback = Text("", 14, "Muted");
            feedback.Visibility = Visibility.Collapsed;
            AutomationProperties.SetLiveSetting(feedback, AutomationLiveSetting.Polite);
            var copy = QuietAction("复制诊断", delegate {
                try { Clipboard.SetText(InstallerDiagnostics.Describe(failure)); feedback.Text = "已复制诊断。"; }
                catch (Exception) { feedback.Text = "无法复制诊断，请重试。"; }
                feedback.Visibility = Visibility.Visible;
                Announce(feedback);
            });
            details.Children.Add(copy);
            details.Children.Add(feedback);
            body.Children.Add(new Expander { Header = "诊断详情", Content = details, Margin = new Thickness(0, 4, 0, 0) });
        }

        private Button QuietAction(string label, RoutedEventHandler handler)
        {
            var button = Action(label, handler, label);
            button.Style = (Style)FindResource("Quiet");
            button.HorizontalAlignment = HorizontalAlignment.Left;
            return button;
        }

        private TextBlock Text(string text, double size, string color = "Ink", Thickness? margin = null)
        {
            return new TextBlock { Text = text, FontSize = size, Foreground = Brush(color),
                Margin = margin ?? new Thickness(0), TextWrapping = TextWrapping.Wrap };
        }

        private Button Action(string label, RoutedEventHandler handler, string name)
        {
            var button = new Button { Content = label };
            AutomationProperties.SetName(button, name);
            WindowChrome.SetIsHitTestVisibleInChrome(button, true);
            button.Click += handler;
            return button;
        }

        private static BitmapImage Image(string name)
        {
            using (var input = Assembly.GetExecutingAssembly().GetManifestResourceStream(name))
            {
                var bitmap = new BitmapImage();
                bitmap.BeginInit();
                bitmap.CacheOption = BitmapCacheOption.OnLoad;
                bitmap.StreamSource = input;
                bitmap.EndInit();
                bitmap.Freeze();
                return bitmap;
            }
        }
    }

}
