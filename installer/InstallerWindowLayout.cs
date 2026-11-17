using System;
using System.Reflection;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
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
            var layout = new Grid { Background = Brush("Canvas"), ClipToBounds = true };
            layout.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(0.55, GridUnitType.Star) });
            layout.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(0.45, GridUnitType.Star) });
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
            return layout;
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
            var brand = new StackPanel { Margin = new Thickness(32), VerticalAlignment = VerticalAlignment.Top,
                Orientation = Orientation.Horizontal };
            brand.Children.Add(new Image { Source = Image("CCC.Icon"), Width = 32, Height = 32,
                Margin = new Thickness(0, 0, 12, 0) });
            var name = Text("Colorful Control Center", 19);
            name.FontWeight = FontWeights.SemiBold;
            name.VerticalAlignment = VerticalAlignment.Center;
            brand.Children.Add(name);
            panel.Children.Add(brand);
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

        private Grid BuildFooter()
        {
            var footer = new Grid { Margin = new Thickness(24, 12, 24, 28) };
            footer.ColumnDefinitions.Add(new ColumnDefinition());
            footer.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            footer.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            back = Action("上一步", OnBack, "上一步");
            back.HorizontalAlignment = HorizontalAlignment.Left;
            footer.Children.Add(back);
            secondary = Action("取消", delegate { Close(); }, "结束或取消安装");
            Grid.SetColumn(secondary, 1);
            footer.Children.Add(secondary);
            primary = Action("", OnPrimary, "继续安装向导");
            primary.Style = (Style)FindResource("Primary");
            primary.Margin = new Thickness(12, 0, 0, 0);
            Grid.SetColumn(primary, 2);
            footer.Children.Add(primary);
            return footer;
        }

        private void RenderBody()
        {
            body.Children.Clear();
            directoryBox = null;
            body.VerticalAlignment = failure == null ? VerticalAlignment.Center : VerticalAlignment.Top;
            string[] titles = { "欢迎使用", "安装位置",
                failure == null ? "正在安装" : "安装未完成", "安装完成" };
            heading = Text(titles[step], 32);
            heading.FontWeight = FontWeights.SemiBold;
            heading.LineHeight = 44;
            body.Children.Add(heading);
            if (step == 0) BuildWelcome();
            else if (step == 1) BuildLocation();
            else if (step == 2) BuildProgress();
            else BuildComplete();
            if (failure != null) BuildError();
        }

        private void BuildWelcome()
        {
            body.Children.Add(Text("你的游戏本控制台。", 15, "Muted", new Thickness(0, 12, 0, 0)));
        }

        private void BuildLocation()
        {
            body.Children.Add(Text(existing ? "更新至原位置，保留你的设置。"
                : "仅为当前用户安装。", 14, "Muted", new Thickness(0, 12, 0, 20)));
            directoryBox = new TextBox { Text = selectedDirectory, IsReadOnly = existing,
                TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 8, 0, 8), MinHeight = 48 };
            AutomationProperties.SetName(directoryBox, "安装文件夹");
            body.Children.Add(directoryBox);
            var browse = Action(existing ? "打开安装目录" : "选择安装位置…", Browse,
                existing ? "打开当前安装目录" : "选择安装位置");
            browse.HorizontalAlignment = HorizontalAlignment.Left;
            body.Children.Add(browse);
            body.Children.Add(Text("需要时将联网安装 WebView2。", 12, "Muted", new Thickness(0, 20, 0, 0)));
        }

        private void BuildProgress()
        {
            body.Children.Add(Text(busy ? phaseText : "请处理下方问题后重试。",
                14, "Muted", new Thickness(0, 16, 0, 24)));
            if (busy)
            {
                var progress = new ProgressBar { Height = 6, Minimum = 0, Maximum = 1,
                    IsIndeterminate = !fraction.HasValue, Value = fraction ?? 0,
                    Foreground = Brush("Action"), Background = Brush("MutedSurface") };
                AutomationProperties.SetName(progress, fraction.HasValue ? "准备文件进度" : "安装进行中，等待引擎完成");
                body.Children.Add(progress);
                if (fraction.HasValue) body.Children.Add(Text(Math.Round(fraction.Value * 100) + "%",
                    13, "Muted", new Thickness(0, 12, 0, 24)));
            }
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
