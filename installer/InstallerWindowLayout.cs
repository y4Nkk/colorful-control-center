using System;
using System.Reflection;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Effects;
using System.Windows.Media.Imaging;
using System.Windows.Shell;

namespace ColorfulControlCenter.Installer
{
    internal sealed partial class InstallerWindow
    {
        private Brush Brush(string key) { return (Brush)FindResource(key); }
        private double Measure(string key) { return (double)FindResource(key); }

        private Grid BuildLayout()
        {
            var inset = new Thickness(ShadowMargin);
            var layout = new Grid { Background = Brush("Canvas"), ClipToBounds = true, Margin = inset };
            layout.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(0.55, GridUnitType.Star) });
            var workspaceColumn = new ColumnDefinition { Width = new GridLength(0.45, GridUnitType.Star) };
            layout.ColumnDefinitions.Add(workspaceColumn);
            layout.Children.Add(BuildArtwork());
            double radius = Measure("PanelRadius");
            var workspace = new Grid { Margin = new Thickness(radius, 8, 12, 8) };
            workspace.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            workspace.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            workspace.RowDefinitions.Add(new RowDefinition());
            workspace.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            workspace.Children.Add(BuildHeader());
            var navigation = BuildSteps();
            Grid.SetRow(navigation, 1);
            workspace.Children.Add(navigation);
            body = new StackPanel { Margin = new Thickness(24, 32, 24, 24), RenderTransform = bodyOffset };
            var scroll = new ScrollViewer { Content = body, VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled, Focusable = false };
            Grid.SetRow(scroll, 2);
            workspace.Children.Add(scroll);
            actions = BuildFooter();
            Grid.SetRow(actions, 3);
            workspace.Children.Add(actions);
            SizeChanged += delegate(object sender, SizeChangedEventArgs args) {
                workspaceColumn.MinWidth = Math.Min(360, Math.Max(0, args.NewSize.Width - 2 * ShadowMargin));
                bool compact = args.NewSize.Height - 2 * ShadowMargin < 440;
                navigation.Margin = compact ? new Thickness(16, 12, 16, 0) : new Thickness(24, 24, 24, 0);
                body.Margin = compact ? new Thickness(16, 8, 16, 12) : new Thickness(24, 32, 24, 24);
                actions.Margin = compact ? new Thickness(16, 8, 16, 12) : new Thickness(24, 12, 24, 20);
            };
            workspacePanel = new Border { Background = Brush("Surface"),
                CornerRadius = new CornerRadius(radius, 0, 0, radius),
                Margin = new Thickness(-radius, 0, 0, 0), Child = workspace };
            workspacePanel.SizeChanged += delegate {
                workspacePanel.Clip = new RectangleGeometry(new Rect(0, 0,
                    workspacePanel.ActualWidth + radius, workspacePanel.ActualHeight), radius, radius);
            };
            Grid.SetColumn(workspacePanel, 1);
            Panel.SetZIndex(workspacePanel, 1);
            layout.Children.Add(workspacePanel);
            var shadow = new Border { Background = Brush("Canvas"), Margin = inset, IsHitTestVisible = false,
                Effect = new DropShadowEffect {
                    Color = ((SolidColorBrush)Brush("Ink")).Color, BlurRadius = 14,
                    ShadowDepth = 2, Direction = 270, Opacity = 0.20,
                } };
            return new Grid { Children = { shadow, layout } };
        }

        private Grid BuildArtwork()
        {
            var panel = new Grid { ClipToBounds = true, Background = Brush("Canvas") };
            panel.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            panel.RowDefinitions.Add(new RowDefinition());
            panel.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var artwork = new Border { IsHitTestVisible = false, Margin = new Thickness(16, 0, Measure("PanelRadius"), 0),
                Background = new ImageBrush(Image("CCC.Artwork")) {
                Stretch = Stretch.Uniform,
            } };
            Grid.SetRow(artwork, 1);
            panel.Children.Add(artwork);
            var brand = new StackPanel { Orientation = Orientation.Horizontal };
            brand.Children.Add(new Image { Source = Image("CCC.Icon"), Width = 32, Height = 32,
                Margin = new Thickness(0, 0, 12, 0) });
            var name = Text("Colorful Control Center", 19);
            name.FontWeight = FontWeights.SemiBold;
            name.VerticalAlignment = VerticalAlignment.Center;
            brand.Children.Add(name);
            panel.Children.Add(new Viewbox { Child = brand, StretchDirection = StretchDirection.DownOnly,
                Margin = new Thickness(32), HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Top });
            var version = Text("版本 " + Product.Version, 12, "Muted", new Thickness(32, 0, 20, 24));
            version.VerticalAlignment = VerticalAlignment.Bottom;
            Grid.SetRow(version, 2);
            panel.Children.Add(version);
            return panel;
        }

        private Grid BuildHeader()
        {
            var header = new Grid { Height = 44, Margin = new Thickness(24, 0, 0, 0) };
            header.ColumnDefinitions.Add(new ColumnDefinition());
            header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
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

        private StackPanel BuildSteps()
        {
            var panel = new StackPanel { Orientation = Orientation.Horizontal,
                HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(24, 24, 24, 0) };
            for (int index = 0; index < 4; index++)
            {
                markers[index] = new Label { Style = (Style)FindResource("StepDot"),
                    Margin = new Thickness(0, 0, index < 3 ? 8 : 0, 0) };
                panel.Children.Add(markers[index]);
            }
            return panel;
        }

        private WrapPanel BuildFooter()
        {
            var footer = new WrapPanel { HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(24, 12, 24, 20) };
            back = Action("上一步", OnBack, "上一步");
            secondary = Action("取消", delegate { Close(); }, "结束或取消安装");
            primary = Action("", OnPrimary, "继续安装向导");
            primary.Style = (Style)FindResource("Primary");
            foreach (var button in new[] { back, secondary, primary })
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
            body.VerticalAlignment = failure == null ? VerticalAlignment.Center : VerticalAlignment.Top;
            string[] titles = { "欢迎使用", "安装位置", failure == null ? "正在安装" : "安装未完成", "安装完成" };
            heading = Text(titles[step], 32);
            heading.FontWeight = FontWeights.SemiBold;
            heading.LineHeight = 44;
            body.Children.Add(heading);
            if (step == 0) BuildWelcome();
            else if (step == 1) BuildLocation();
            else if (step == 2) { if (busy) BuildProgress(); }
            else BuildComplete();
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
            var title = Text("关闭控制台后安装", 20);
            title.FontWeight = FontWeights.SemiBold;
            content.Children.Add(title);
            content.Children.Add(Text("控制台正在运行。关闭后，安装会自动继续。", 14, "Muted", new Thickness(0, 10, 0, 22)));
            var actions = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right };
            var cancel = Action("取消", delegate { dialog.DialogResult = false; }, "取消");
            cancel.IsCancel = true;
            var confirm = Action("关闭并安装", delegate { dialog.DialogResult = true; }, "关闭并安装");
            confirm.Style = (Style)FindResource("Primary");
            confirm.Margin = new Thickness(10, 0, 0, 0);
            actions.Children.Add(cancel);
            actions.Children.Add(confirm);
            content.Children.Add(actions);
            dialog.Content = new Border { Padding = new Thickness(24), Background = Brush("Surface"), Child = content };
            dialog.Loaded += delegate { cancel.Focus(); };
            return dialog.ShowDialog() == true;
        }

        private void BuildWelcome()
        {
            body.Children.Add(Text("你的游戏本控制台。", 15, "Muted", new Thickness(0, 12, 0, 0)));
        }

        private void BuildLocation()
        {
            string description = Updating ? "更新至原位置，保留你的设置。" : "仅为当前用户安装。";
            if (recoveryReason != null) description = "原安装位置不可用，请选择目录重新安装。";
            body.Children.Add(Text(description, 14, "Muted", new Thickness(0, 8, 0, 8)));
            directoryBox = new TextBox { Text = selectedDirectory, IsReadOnly = Updating,
                TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 0, 0, 8), MinHeight = 48 };
            AutomationProperties.SetName(directoryBox, "安装文件夹");
            body.Children.Add(directoryBox);
            var browse = Action(Updating ? "打开安装目录" : "选择安装位置…", Browse,
                Updating ? "打开当前安装目录" : "选择安装位置");
            browse.HorizontalAlignment = HorizontalAlignment.Left;
            body.Children.Add(browse);
            var feedback = Text("", 13, "Error", new Thickness(0, 8, 0, 0));
            feedback.Visibility = Visibility.Collapsed;
            locationError = feedback;
            directoryBox.TextChanged += delegate { feedback.Visibility = Visibility.Collapsed; };
            body.Children.Add(feedback);
            if (recoveryReason != null) body.Children.Add(Text(recoveryReason, 12, "Muted", new Thickness(0, 8, 0, 0)));
            if (!Updating) body.Children.Add(Text("请选择空文件夹，或输入新目录的完整路径。", 12, "Muted", new Thickness(0, 8, 0, 0)));
            body.Children.Add(Text("需要时将联网安装 WebView2。", 12, "Muted", new Thickness(0, 8, 0, 0)));
        }

        private void BuildProgress()
        {
            body.Children.Add(Text(phaseText, 14, "Muted", new Thickness(0, 16, 0, 24)));
            var progress = new ProgressBar { Height = 6, Minimum = 0, Maximum = 1,
                IsIndeterminate = !fraction.HasValue, Value = fraction ?? 0,
                Foreground = Brush("Action"), Background = Brush("MutedSurface") };
            AutomationProperties.SetName(progress, fraction.HasValue ? "准备文件进度" : "安装进行中，等待引擎完成");
            body.Children.Add(progress);
            if (fraction.HasValue) body.Children.Add(Text(Math.Round(fraction.Value * 100) + "%",
                13, "Muted", new Thickness(0, 12, 0, 24)));
        }

        private void BuildComplete()
        {
            body.Children.Add(Text("可以打开控制台了。", 15, "Muted", new Thickness(0, 12, 0, 0)));
        }

        private void BuildError()
        {
            body.Children.Add(Text(failure.Message, 13, "Error", new Thickness(0, 20, 0, 12)));
            var installFailure = failure as InstallFailure;
            if (installFailure != null) body.Children.Add(new Expander { Header = "诊断日志",
                Content = Text(installFailure.LogPath, 12, "Muted"), Margin = new Thickness(0, 0, 0, 12) });
            var copy = Action("复制诊断", delegate {
                try { Clipboard.SetText(InstallerDiagnostics.Describe(failure)); }
                catch (Exception error) { MessageBox.Show(this, error.Message, "未能复制诊断"); }
            }, "复制安装诊断");
            copy.HorizontalAlignment = HorizontalAlignment.Left;
            body.Children.Add(copy);
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
