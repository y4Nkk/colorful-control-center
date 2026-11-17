using System;
using System.Threading;
using System.Windows;

namespace ColorfulControlCenter.Installer
{
    internal static class Program
    {
        [STAThread]
        private static int Main(string[] args)
        {
            try
            {
                if (args.Length != 0) throw new ArgumentException("请直接打开安装向导，不需要命令行参数。");
                bool created;
                using (var mutex = new Mutex(true, @"Local\ColorfulControlCenter.Setup", out created))
                {
                    if (!created) throw new InvalidOperationException("安装向导已打开，请完成当前安装。");
                    var app = new Application();
                    var window = new InstallerWindow();
                    app.Run(window);
                    return window.ExitCode;
                }
            }
            catch (Exception error)
            {
                MessageBox.Show(error.ToString(), "Colorful Control Center · 安装", MessageBoxButton.OK,
                    MessageBoxImage.Error);
                return 2;
            }
        }
    }
}
