using System;
using System.ComponentModel;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Security.Principal;

namespace ColorfulControlCenter.Installer
{
    internal static partial class ControllerShutdown
    {
        internal const string ExitCommand = "--request-controller-exit";
        private const int AccessDenied = 5, InvalidParameter = 87, Cancelled = 1223, NotFound = 1168;

        private static bool RequestElevatedExit(ProcessIdentity target, string directory)
        {
            string installer = Assembly.GetEntryAssembly().Location;
            string user;
            using (var identity = WindowsIdentity.GetCurrent()) user = identity.User.Value;
            var launch = new ProcessStartInfo(installer) {
                UseShellExecute = true, Verb = "runas", WorkingDirectory = Environment.SystemDirectory,
                Arguments = String.Format(CultureInfo.InvariantCulture, "{0} {1} {2} {3} \"{4}\"",
                    ExitCommand, target.Id, CreatedAt(target), user, directory),
            };

            Process helper;
            try
            {
                using (File.Open(installer, FileMode.Open, FileAccess.Read, FileShare.Read))
                    helper = Process.Start(launch);
            }
            catch (Win32Exception error)
            {
                if (error.NativeErrorCode == Cancelled) return false;
                throw;
            }
            if (helper == null) throw new IOException("未能启动管理员退出请求。");
            using (helper)
            {
                helper.WaitForExit();
                if (helper.ExitCode != 0) throw NativeError(helper.ExitCode, "以管理员权限请求控制台退出");
            }
            return true;
        }

        internal static int RunExitCommand(string[] args)
        {
            uint pid;
            ulong started;
            if (args.Length != 5 || args[0] != ExitCommand
                || !UInt32.TryParse(args[1], NumberStyles.None, CultureInfo.InvariantCulture, out pid) || pid == 0
                || !UInt64.TryParse(args[2], NumberStyles.None, CultureInfo.InvariantCulture, out started) || started == 0)
                return InvalidParameter;

            try
            {
                using (var identity = WindowsIdentity.GetCurrent())
                {
                    if (identity.User.Value != args[3]
                        || !new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator)) return AccessDenied;
                }
                var directory = InstallationRequest.Create(args[4], requireInstalled: true).Directory;
                string executable = Path.Combine(directory, Product.ApplicationFileStem + ".exe");
                var verified = FindController(executable);
                // The original installer owns the file-release wait, including workers left after the main window exits.
                if (!verified.HasValue) return 0;
                if (verified.Value.Id != pid || CreatedAt(verified.Value) != started) return NotFound;

                using (var shutdown = new RestartManagerSession())
                {
                    shutdown.RegisterController(verified.Value);
                    shutdown.Shutdown();
                }
                return 0;
            }
            catch (Win32Exception error) { return error.NativeErrorCode; }
            catch (ArgumentException) { return InvalidParameter; }
            catch (Exception) { return 1; }
        }

        private static ulong CreatedAt(ProcessIdentity identity)
        {
            return ((ulong)identity.CreatedHigh << 32) | identity.CreatedLow;
        }
    }
}
