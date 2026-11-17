using System;
using System.Diagnostics;
using System.IO;
using Microsoft.Win32;

namespace ColorfulControlCenter.Installer
{
    internal static class InstalledApplication
    {
        internal static string FindDirectory()
        {
            using (var hive = RegistryKey.OpenBaseKey(RegistryHive.CurrentUser, RegistryView.Registry64))
            using (var key = hive.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\" + Product.ProductName))
            {
                string location = key == null ? null : key.GetValue("InstallLocation") as string;
                if (!String.IsNullOrWhiteSpace(location)) return location.Trim('"');
            }
            return null;
        }

        internal static string DefaultDirectory()
        {
            return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                Product.ProductName);
        }

        internal static void Launch(string directory)
        {
            using (var process = Process.Start(new ProcessStartInfo(
                Path.Combine(directory, Product.ApplicationFileStem + ".exe")) {
                UseShellExecute = true, WorkingDirectory = directory,
            }))
                if (process == null) throw new IOException("未能启动控制台，请稍后从开始菜单打开。");
        }
    }
}
