using System;
using System.ComponentModel;
using System.IO;
using System.Reflection;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Shell;

namespace ColorfulControlCenter.Installer
{
    internal sealed partial class InstallerWindow : Window
    {
        private readonly Func<InstallationRequest, IProgress<InstallProgress>, Task> install;
        private readonly Action<string> launch;
        private readonly bool existing;
        private string selectedDirectory;
        private InstallationRequest request;
        private Exception failure;
        private string phaseText;
        private double? fraction;
        private bool busy;
        private int step;
        private StackPanel body;
        private TextBox directoryBox;
        private TextBlock heading;
        private Button primary, secondary, back, close;
        private readonly Label[] markers = new Label[4];
        private int renderedStep = -1;

        internal InstallerWindow() : this(InstalledApplication.FindDirectory(),
            new InstallerEngine().Run, InstalledApplication.Launch) { }

        internal InstallerWindow(string installedDirectory,
            Func<InstallationRequest, IProgress<InstallProgress>, Task> install, Action<string> launch)
        {
            this.install = install;
            this.launch = launch;
            existing = installedDirectory != null;
            selectedDirectory = installedDirectory ?? InstalledApplication.DefaultDirectory();
            ExitCode = 1;
            Resources.MergedDictionaries.Add(LoadDictionary("CCC.Palette"));
            Resources.MergedDictionaries.Add(LoadDictionary("CCC.Theme"));
            Title = "Colorful Control Center · 安装";
            FontFamily = new FontFamily("Microsoft YaHei UI, Segoe UI");
            Width = Math.Min(1040, SystemParameters.WorkArea.Width - 32);
            Height = Math.Min(640, SystemParameters.WorkArea.Height - 32);
            WindowStartupLocation = WindowStartupLocation.CenterScreen;
            WindowStyle = WindowStyle.None;
            ResizeMode = ResizeMode.CanMinimize;
            Background = Brush("Canvas");
            UseLayoutRounding = true;
            WindowChrome.SetWindowChrome(this, new WindowChrome {
                CaptionHeight = 48, ResizeBorderThickness = new Thickness(0),
                GlassFrameThickness = new Thickness(0), CornerRadius = new CornerRadius(0),
            });
            Content = BuildLayout();
            Render();
            Closing += OnClosing;
            PreviewKeyDown += delegate(object sender, KeyEventArgs args) {
                if (args.Key == Key.Escape) { args.Handled = true; Close(); }
            };
            Loaded += delegate { primary.Focus(); };
        }

        internal int ExitCode { get; private set; }

        private static ResourceDictionary LoadDictionary(string name)
        {
            using (var input = Assembly.GetExecutingAssembly().GetManifestResourceStream(name))
            {
                if (input == null) throw new InvalidDataException("缺少安装界面资源：" + name);
                return (ResourceDictionary)XamlReader.Load(input);
            }
        }

        private async void OnPrimary(object sender, RoutedEventArgs args)
        {
            if (busy) return;
            if (step == 0) { step = 1; Render(); return; }
            if (step == 3) { Launch(); return; }
            if (step == 2 && request != null) { await Install(); return; }
            if (directoryBox != null) selectedDirectory = directoryBox.Text;
            try { request = InstallationRequest.Create(selectedDirectory, request); }
            catch (Exception error) { failure = error; step = 1; Render(); return; }
            await Install();
        }

        private async Task Install()
        {
            busy = true;
            failure = null;
            step = 2;
            phaseText = "正在检查安装条件…";
            fraction = null;
            Render();
            try
            {
                await install(request, new Progress<InstallProgress>(progress => {
                    if (!busy || step != 2) return;
                    phaseText = progress.Phase;
                    fraction = progress.Fraction;
                    RenderBody();
                }));
                step = 3;
                ExitCode = 0;
            }
            catch (Exception error)
            {
                failure = error;
            }
            finally { busy = false; Render(); }
        }

        private void Launch()
        {
            try
            {
                InstallerEngine.VerifyInstallation(request.Directory);
                launch(request.Directory);
                Close();
            }
            catch (Exception error) { failure = new IOException("安装已完成，启动未成功。" + error.Message, error); Render(); }
        }

        private void OnBack(object sender, RoutedEventArgs args)
        {
            if (busy) return;
            if (directoryBox != null) selectedDirectory = directoryBox.Text;
            step = step == 2 ? 1 : 0;
            failure = null;
            Render();
        }

        private void OnClosing(object sender, CancelEventArgs args)
        {
            if (!busy) return;
            args.Cancel = true;
            phaseText = "安装正在进行，请等待完成后关闭。";
            RenderBody();
        }

        private void Render()
        {
            int previousStep = renderedStep;
            renderedStep = step;
            for (int index = 0; index < 4; index++)
            {
                bool current = index == step;
                bool completed = index < step;
                UpdateMarker(markers[index], current && failure != null ? "Error"
                    : current || completed ? "Action" : "Hairline", completed ? 0.35 : 1);
                string name = new[] { "欢迎", "位置", "安装", "完成" }[index];
                string status = completed ? "已完成" : current ? (failure == null ? "当前步骤" : "需要处理") : "等待中";
                System.Windows.Automation.AutomationProperties.SetName(markers[index], name + "，" + status);
            }
            primary.Content = step == 0 ? "开始设置" : step == 1 ? (existing ? "确认并更新" : "确认并安装")
                : step == 2 ? (busy ? "正在安装…" : "重试安装") : failure == null ? "启动控制台" : "重试启动";
            primary.IsEnabled = !busy;
            System.Windows.Automation.AutomationProperties.SetName(primary, primary.Content.ToString());
            primary.IsDefault = step != 3;
            secondary.Content = step == 3 ? "完成" : "取消";
            System.Windows.Automation.AutomationProperties.SetName(secondary, secondary.Content.ToString());
            secondary.IsDefault = step == 3;
            secondary.IsEnabled = !busy;
            close.IsEnabled = !busy;
            back.Visibility = step == 1 || (step == 2 && !busy) ? Visibility.Visible : Visibility.Collapsed;
            RenderBody();
            AnimateStep(previousStep);
        }

        private void Browse(object sender, RoutedEventArgs args)
        {
            if (existing)
            {
                try { using (System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(selectedDirectory) {
                    UseShellExecute = true,
                })) { } }
                catch (Exception error) { failure = error; Render(); }
                return;
            }
            using (var dialog = new System.Windows.Forms.FolderBrowserDialog())
            {
                dialog.Description = "选择 Colorful Control Center 的独立安装文件夹";
                if (Directory.Exists(directoryBox.Text)) dialog.SelectedPath = directoryBox.Text;
                if (dialog.ShowDialog(new FolderDialogOwner(this)) == System.Windows.Forms.DialogResult.OK)
                    directoryBox.Text = dialog.SelectedPath;
            }
        }

        private sealed class FolderDialogOwner : System.Windows.Forms.IWin32Window
        {
            private readonly Window owner;
            internal FolderDialogOwner(Window owner) { this.owner = owner; }
            public IntPtr Handle { get { return new System.Windows.Interop.WindowInteropHelper(owner).Handle; } }
        }
    }
}
