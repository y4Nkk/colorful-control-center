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
        private const double ShadowMargin = 16;
        private readonly Func<InstallationRequest, IProgress<InstallProgress>, Func<bool>, Task<bool>> install;
        private readonly Action<string> launch;
        private readonly string registeredDirectory;
        private string selectedDirectory;
        private string recoveryReason;
        private InstallationRequest request;
        private Exception failure;
        private string phaseText;
        private double? fraction;
        private bool busy;
        private int step;
        private StackPanel body;
        private TextBox directoryBox;
        private TextBlock locationError;
        private TextBlock heading;
        private Button primary, secondary, back, close;
        private readonly Label[] markers = new Label[4];
        private int renderedStep = -1;

        internal InstallerWindow() : this(InstalledApplication.FindDirectory(),
            new InstallerEngine().Run, InstalledApplication.Launch) { }

        internal InstallerWindow(string installedDirectory,
            Func<InstallationRequest, IProgress<InstallProgress>, Func<bool>, Task<bool>> install, Action<string> launch)
        {
            this.install = install;
            this.launch = launch;
            registeredDirectory = installedDirectory;
            selectedDirectory = installedDirectory ?? InstalledApplication.DefaultDirectory();
            ExitCode = 1;
            Resources.MergedDictionaries.Add(LoadDictionary("CCC.Palette"));
            Resources.MergedDictionaries.Add(LoadDictionary("CCC.Theme"));
            Title = "Colorful Control Center · 安装";
            FontFamily = new FontFamily("Microsoft YaHei UI, Segoe UI");
            Width = Math.Min(1040 + 2 * ShadowMargin, SystemParameters.WorkArea.Width);
            Height = Math.Min(640 + 2 * ShadowMargin, SystemParameters.WorkArea.Height);
            WindowStartupLocation = WindowStartupLocation.CenterScreen;
            WindowStyle = WindowStyle.None;
            AllowsTransparency = true;
            ResizeMode = ResizeMode.CanMinimize;
            Background = Brushes.Transparent;
            UseLayoutRounding = true;
            WindowChrome.SetWindowChrome(this, new WindowChrome {
                CaptionHeight = 48 + ShadowMargin, ResizeBorderThickness = new Thickness(0),
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
        private bool Updating { get { return registeredDirectory != null && recoveryReason == null; } }

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
            if (step == 0)
            {
                if (Updating || request == null) CheckRegisteredTarget();
                step = 1;
                Render();
                return;
            }
            if (step == 3) { Launch(); return; }
            if (directoryBox != null) selectedDirectory = directoryBox.Text;
            bool wasUpdating = Updating;
            var registeredTarget = Updating || request == null ? CheckRegisteredTarget() : null;
            if (wasUpdating != Updating)
            {
                step = 1;
                failure = null;
                Render();
                return;
            }

            try { request = registeredTarget ?? InstallationRequest.Create(selectedDirectory, request); }
            catch (ArgumentException error) { ShowLocationError(error.Message); return; }
            selectedDirectory = request.Directory;
            await Install();
        }

        private InstallationRequest CheckRegisteredTarget()
        {
            if (registeredDirectory == null) return null;
            try
            {
                var target = InstallationRequest.Create(registeredDirectory, request, requireInstalled: true);
                recoveryReason = null;
                selectedDirectory = target.Directory;
                return target;
            }
            catch (ArgumentException error)
            {
                if (Updating) selectedDirectory = String.Empty;
                recoveryReason = error.Message;
                return null;
            }
        }

        private void ShowLocationError(string message)
        {
            if (step != 1)
            {
                step = 1;
                failure = null;
                Render();
            }
            locationError.Text = message;
            locationError.Visibility = Visibility.Visible;
            locationError.BringIntoView();
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
                bool installed = await install(request, new Progress<InstallProgress>(progress => {
                    if (!busy || step != 2) return;
                    phaseText = progress.Phase;
                    fraction = progress.Fraction;
                    RenderBody();
                }), ConfirmControllerClose);
                step = installed ? 3 : 1;
                if (installed) ExitCode = 0;
            }
            catch (Exception error)
            {
                failure = error;
            }
            finally { busy = false; Render(); }
            if (step == 1) primary.Focus();
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
            primary.Content = step == 0 ? "开始设置" : step == 1 ? (Updating ? "确认并更新" : "确认并安装")
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
            if (Updating)
            {
                if (CheckRegisteredTarget() == null) { Render(); return; }
                try { using (System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(selectedDirectory) {
                    UseShellExecute = true,
                })) { } }
                catch (Exception error) { failure = error; Render(); }
                return;
            }
            using (var dialog = new System.Windows.Forms.FolderBrowserDialog())
            {
                dialog.Description = "请选择独立的空文件夹，也可以新建一个文件夹。";
                if (Directory.Exists(directoryBox.Text)) dialog.SelectedPath = directoryBox.Text;
                if (dialog.ShowDialog(new FolderDialogOwner(this)) != System.Windows.Forms.DialogResult.OK) return;
                directoryBox.Text = dialog.SelectedPath;
                try { InstallationRequest.Create(directoryBox.Text, request); locationError.Visibility = Visibility.Collapsed; }
                catch (ArgumentException error) { ShowLocationError(error.Message); }
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
