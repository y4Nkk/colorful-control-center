using System;
using System.ComponentModel;
using System.IO;
using System.Reflection;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Automation.Peers;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Shell;

namespace ColorfulControlCenter.Installer
{
    internal enum InstallerStep { Confirm, Install, Complete }

    internal sealed partial class InstallerWindow : Window
    {
        private const double ShadowMargin = 16;
        private readonly Func<InstallationRequest, IProgress<InstallProgress>, Func<bool>, Task<bool>> install;
        private readonly Action<string> launch;
        private readonly string registeredDirectory;
        private string selectedDirectory;
        private string locationEditStart;
        private string recoveryReason;
        private string replacementDirectory;
        private InstallationRequest request;
        private Exception failure;
        private bool busy, editingLocation;
        private InstallerStep step;
        private StackPanel body;
        private TextBox directoryBox;
        private TextBlock locationError;
        private TextBlock heading;
        private TextBlock phaseStatus;
        private ProgressBar progressBar;
        private Button primary, secondary, changeLocation, close;

        internal InstallerWindow() : this(InstalledApplication.FindDirectory(),
            new InstallerEngine().Run, InstalledApplication.Launch) { }

        internal InstallerWindow(string installedDirectory,
            Func<InstallationRequest, IProgress<InstallProgress>, Func<bool>, Task<bool>> install, Action<string> launch,
            Size? workArea = null)
        {
            this.install = install;
            this.launch = launch;
            registeredDirectory = installedDirectory;
            selectedDirectory = installedDirectory ?? InstalledApplication.DefaultDirectory();
            ExitCode = 1;
            Resources.MergedDictionaries.Add(LoadDictionary("CCC.Palette"));
            Resources.MergedDictionaries.Add(LoadDictionary("CCC.Theme"));
            if (SystemParameters.HighContrast) ApplyHighContrastPalette();
            CheckRegisteredTarget();
            editingLocation = recoveryReason != null;
            FontFamily = new FontFamily("Microsoft YaHei UI, Segoe UI");
            Size available = workArea ?? SystemParameters.WorkArea.Size;
            SizeToContent = SizeToContent.Manual;
            Width = Math.Min(960 + 2 * ShadowMargin, available.Width);
            Height = Math.Min(520 + 2 * ShadowMargin, available.Height);
            MinWidth = MaxWidth = Width;
            MinHeight = MaxHeight = Height;
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
                if (args.Key == Key.Escape && !args.Handled)
                {
                    args.Handled = true;
                    if (busy) return;
                    if (step == InstallerStep.Confirm && editingLocation && recoveryReason == null)
                        ShowLocationSummary();
                    else Close();
                }
            };
            Loaded += delegate { heading.Focus(); };
        }

        internal int ExitCode { get; private set; }
        private bool Updating { get { return registeredDirectory != null && recoveryReason == null; } }

        private static ResourceDictionary LoadDictionary(string name)
        {
            using (var input = Assembly.GetExecutingAssembly().GetManifestResourceStream(name))
            {
                if (input == null) throw new InvalidDataException("缺少安装界面资源：" + name);
                var context = new ParserContext { XamlTypeMapper = new XamlTypeMapper(new string[0]) };
                context.XamlTypeMapper.AddMappingProcessingInstruction("urn:ccc-installer",
                    typeof(ButtonCorners).Namespace, typeof(ButtonCorners).Assembly.FullName);
                return (ResourceDictionary)XamlReader.Load(input, context);
            }
        }

        private void ApplyHighContrastPalette()
        {
            Resources["Canvas"] = Resources["Surface"] = SystemColors.WindowBrush;
            Resources["Ink"] = Resources["Muted"] = Resources["Error"] = SystemColors.WindowTextBrush;
            Resources["MutedSurface"] = SystemColors.ControlBrush;
            Resources["ControlHover"] = Resources["ControlPress"] = SystemColors.ControlBrush;
            Resources["Hairline"] = Resources["FieldEdge"] = SystemColors.WindowTextBrush;
            Resources["ActionInk"] = Resources["FocusInk"] = SystemColors.WindowTextBrush;
            Resources["ActionFill"] = SystemColors.HighlightBrush;
            Resources["OnAction"] = SystemColors.HighlightTextBrush;
        }

        private async void OnPrimary(object sender, RoutedEventArgs args)
        {
            if (busy) return;
            if (step == InstallerStep.Complete) { Launch(); return; }
            bool wasUpdating = Updating;
            var registeredTarget = Updating || request == null ? CheckRegisteredTarget() : null;
            if (wasUpdating != Updating)
            {
                failure = null;
                replacementDirectory = null;
                editingLocation = !Updating;
                ShowStep(InstallerStep.Confirm);
                return;
            }

            InstallationRequest target;
            try { target = registeredTarget ?? InstallationRequest.Create(selectedDirectory, request); }
            catch (ArgumentException error) { ShowLocationError(error.Message); return; }
            selectedDirectory = target.Directory;
            if (!Updating && target.ReplacesInstallation && !SameDirectory(request == null ? null : request.Directory, target.Directory)
                && !SameDirectory(replacementDirectory, target.Directory))
            {
                replacementDirectory = target.Directory;
                editingLocation = false;
                ShowStep(InstallerStep.Confirm);
                return;
            }
            request = target;
            replacementDirectory = null;
            await Install();
        }

        private static bool SameDirectory(string left, string right)
        {
            return String.Equals(left, right, StringComparison.OrdinalIgnoreCase);
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
            if (step != InstallerStep.Confirm || !editingLocation)
            {
                failure = null;
                replacementDirectory = null;
                editingLocation = true;
                locationEditStart = selectedDirectory;
                ShowStep(InstallerStep.Confirm);
            }
            locationError.Text = message;
            locationError.Visibility = Visibility.Visible;
            directoryBox.Focus();
            locationError.BringIntoView();
            Announce(locationError);
        }

        private async Task Install()
        {
            busy = true;
            failure = null;
            ShowStep(InstallerStep.Install);
            bool installed = false;
            try { installed = await install(request, new Progress<InstallProgress>(UpdateProgress), ConfirmControllerClose); }
            catch (Exception error) { failure = error; }
            busy = false;
            if (installed) ExitCode = 0;
            editingLocation = recoveryReason != null;
            ShowStep(failure != null ? InstallerStep.Install
                : installed ? InstallerStep.Complete : InstallerStep.Confirm);
        }

        private void Launch()
        {
            try
            {
                InstallerEngine.VerifyInstallation(request.Directory);
                launch(request.Directory);
                Close();
            }
            catch (Exception error)
            {
                failure = error;
                Render();
                primary.Focus();
            }
        }

        private void EditLocation()
        {
            if (busy || Updating) return;
            locationEditStart = selectedDirectory;
            replacementDirectory = null;
            editingLocation = true;
            failure = null;
            ShowStep(InstallerStep.Confirm);
            directoryBox.SelectAll();
        }

        private void ShowLocationSummary()
        {
            selectedDirectory = locationEditStart;
            editingLocation = false;
            Render();
            changeLocation.Focus();
        }

        private void ShowStep(InstallerStep next)
        {
            InstallerStep previous = step;
            step = next;
            Render();
            bodyScroll.ScrollToTop();
            AnimateStep(previous);
            heading.Focus();
        }

        private void OnClosing(object sender, CancelEventArgs args)
        {
            args.Cancel = busy;
        }

        private void Render()
        {
            Title = Product.ControllerWindowTitle + (Updating ? " · 更新" : " · 安装");
            primary.Content = step == InstallerStep.Confirm ? ConfirmAction
                : step == InstallerStep.Complete ? (failure == null ? "打开控制台" : "重试打开")
                : Updating ? "重试更新" : "重试安装";
            AutomationProperties.SetName(primary, primary.Content.ToString());
            primary.IsEnabled = !busy;
            secondary.Content = step == InstallerStep.Confirm ? "取消" : "关闭";
            AutomationProperties.SetName(secondary, secondary.Content.ToString());
            secondary.IsEnabled = !busy;
            close.IsEnabled = !busy;
            actions.Visibility = busy ? Visibility.Collapsed : Visibility.Visible;
            RenderBody();
        }

        private string ConfirmAction
        {
            get
            {
                if (Updating) return "更新";
                if (replacementDirectory != null) return "替换并安装";
                return recoveryReason == null ? "安装" : "重新安装";
            }
        }

        private void Browse(object sender, RoutedEventArgs args)
        {
            if (Updating)
            {
                if (CheckRegisteredTarget() == null)
                {
                    editingLocation = true;
                    ShowStep(InstallerStep.Confirm);
                    return;
                }
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
                try { InstallationRequest.Create(directoryBox.Text, request); }
                catch (ArgumentException error) { ShowLocationError(error.Message); }
            }
        }

        private void UpdateProgress(InstallProgress progress)
        {
            if (!busy || phaseStatus == null) return;
            bool preparing = progress.Fraction.HasValue;
            phaseStatus.Text = preparing ? progress.Phase.TrimEnd('…') + " " + progress.Fraction.Value.ToString("P0")
                : progress.Phase;
            if (progressBar.IsIndeterminate == preparing)
            {
                progressBar.IsIndeterminate = !preparing;
                AutomationProperties.SetName(progressBar, preparing ? "准备安装文件进度" : "安装进行中");
                Announce(phaseStatus);
            }
            else if (!preparing) Announce(phaseStatus);
            progressBar.Value = progress.Fraction ?? 0;
        }

        private static void Announce(TextBlock text)
        {
            var peer = UIElementAutomationPeer.CreatePeerForElement(text);
            if (peer != null) peer.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
        }

        private sealed class FolderDialogOwner : System.Windows.Forms.IWin32Window
        {
            private readonly Window owner;
            internal FolderDialogOwner(Window owner) { this.owner = owner; }
            public IntPtr Handle { get { return new System.Windows.Interop.WindowInteropHelper(owner).Handle; } }
        }
    }
}
