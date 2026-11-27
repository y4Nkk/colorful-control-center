using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;
using System.Threading;
using Microsoft.Win32.SafeHandles;

namespace ColorfulControlCenter.Installer
{
    internal static partial class ControllerShutdown
    {
        private const uint QueryLimitedInformation = 0x1000;
        private const uint TokenQuery = 0x0008;

        internal static bool ReleaseExecutable(string directory, Action<string> log)
        {
            string executable = Path.Combine(directory, Product.ApplicationFileStem + ".exe");
            if (!IsExecutableInUse(executable)) return true;

            var controller = FindController(executable);
            if (controller.HasValue)
            {
                log("Requesting normal controller exit. PID: " + controller.Value.Id);
                bool elevate;
                using (var shutdown = new RestartManagerSession())
                {
                    shutdown.RegisterController(controller.Value);
                    elevate = shutdown.PermissionDenied();
                    if (!elevate) shutdown.Shutdown();
                }
                if (elevate)
                {
                    log("Restart Manager reports insufficient shutdown permission. Requesting administrator authorization.");
                    if (!RequestElevatedExit(controller.Value, directory)) return false;
                }
            }
            else log("No matching controller window. Waiting for the executable to be released.");

            var wait = Stopwatch.StartNew();
            while (IsExecutableInUse(executable))
            {
                if (wait.Elapsed >= TimeSpan.FromSeconds(30))
                    throw new TimeoutException("等待控制台安装文件解除占用超时。");
                Thread.Sleep(500);
            }
            log("Controller executable released.");
            return true;
        }

        private static bool IsExecutableInUse(string executable)
        {
            // A session can retain its original lock list after a recovery worker exits.
            using (var files = new RestartManagerSession())
            {
                files.RegisterFile(executable);
                return files.InUse();
            }
        }

        private static ProcessIdentity? FindController(string executable)
        {
            var ids = new HashSet<int>();
            var title = new StringBuilder(Product.ControllerWindowTitle.Length + 2);
            if (!EnumWindows((window, parameter) => {
                if (GetWindowText(window, title, title.Capacity) > 0 && title.ToString() == Product.ControllerWindowTitle)
                {
                    uint id;
                    if (GetWindowThreadProcessId(window, out id) != 0) ids.Add((int)id);
                }
                return true;
            }, IntPtr.Zero)) throw new Win32Exception();

            int session;
            using (var current = Process.GetCurrentProcess()) session = current.SessionId;
            ProcessIdentity? match = null;
            using (var identity = WindowsIdentity.GetCurrent())
            {
                foreach (int id in ids)
                {
                    var candidate = FindProcess(id, session, executable, identity.User);
                    if (!candidate.HasValue) continue;
                    if (match.HasValue) throw new IOException("安装位置匹配多个控制台主进程。");
                    match = candidate;
                }
            }
            return match;
        }

        private static ProcessIdentity? FindProcess(int id, int session, string executable, SecurityIdentifier user)
        {
            try
            {
                using (var process = Process.GetProcessById(id))
                {
                    if (process.SessionId != session || !String.Equals(process.ProcessName,
                        Product.ApplicationFileStem, StringComparison.OrdinalIgnoreCase)) return null;
                    long started = process.StartTime.ToFileTimeUtc();
                    if (!MatchesImageAndUser(id, executable, user)) return null;
                    return new ProcessIdentity { Id = (uint)id, CreatedLow = (uint)started, CreatedHigh = (uint)(started >> 32) };
                }
            }
            catch (ArgumentException) { return null; }
            catch (InvalidOperationException) { return null; }
        }

        private static bool MatchesImageAndUser(int id, string executable, SecurityIdentifier user)
        {
            using (var handle = OpenProcess(QueryLimitedInformation, false, id))
            {
                if (handle.IsInvalid) throw new Win32Exception();
                var path = new StringBuilder(32768);
                int length = path.Capacity;
                if (!QueryFullProcessImageName(handle, 0, path, ref length)) throw new Win32Exception();
                if (!String.Equals(path.ToString(), executable, StringComparison.OrdinalIgnoreCase)) return false;
                SafeAccessTokenHandle token;
                if (!OpenProcessToken(handle, TokenQuery, out token)) throw new Win32Exception();
                using (token)
                using (var identity = new WindowsIdentity(token.DangerousGetHandle()))
                    return user.Equals(identity.User);
            }
        }

        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern SafeProcessHandle OpenProcess(uint access, bool inheritHandle, int id);

        [DllImport("advapi32.dll", SetLastError = true)]
        private static extern bool OpenProcessToken(SafeProcessHandle process, uint access, out SafeAccessTokenHandle token);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern bool QueryFullProcessImageName(SafeProcessHandle process, int flags, StringBuilder path,
            ref int length);

        [StructLayout(LayoutKind.Sequential)]
        private struct ProcessIdentity
        {
            // RM_UNIQUE_PROCESS contains a DWORD process ID followed by a FILETIME.
            internal uint Id, CreatedLow, CreatedHigh;
        }

        private delegate bool WindowCallback(IntPtr window, IntPtr parameter);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool EnumWindows(WindowCallback callback, IntPtr parameter);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowText(IntPtr window, StringBuilder text, int maxCount);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

        private sealed class RestartManagerSession : IDisposable
        {
            private const int ErrorMoreData = 234;
            private const uint RebootReasonPermissionDenied = 0x1;
            private readonly uint session;

            internal RestartManagerSession()
            {
                Check(RmStartSession(out session, 0, new StringBuilder(33)), "创建退出检查会话");
            }

            internal void RegisterFile(string executable)
            {
                Check(RmRegisterResources(session, 1, new[] { executable }, 0, null, 0, IntPtr.Zero), "检查安装文件占用");
            }

            internal bool PermissionDenied()
            {
                uint required, reasons, available = 0;
                int code = RmGetList(session, out required, ref available, IntPtr.Zero, out reasons);
                if (code != ErrorMoreData) Check(code, "检查控制台退出权限");
                return (reasons & RebootReasonPermissionDenied) != 0;
            }

            internal void RegisterController(ProcessIdentity controller)
            {
                // The file can also be held by recovery workers, which must exit on their own.
                Check(RmRegisterResources(session, 0, null, 1, new[] { controller }, 0, IntPtr.Zero), "选择要退出的控制台");
            }

            internal void Shutdown()
            {
                Check(RmShutdown(session, 0, IntPtr.Zero), "请求控制台正常退出");
            }

            internal bool InUse()
            {
                uint required, reasons, available = 0;
                int code = RmGetList(session, out required, ref available, IntPtr.Zero, out reasons);
                if (code == ErrorMoreData) return true;
                Check(code, "读取安装文件占用状态");
                return false;
            }

            public void Dispose() { RmEndSession(session); }

            private static void Check(int code, string operation)
            {
                if (code != 0) throw NativeError(code, operation);
            }

            [DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
            private static extern int RmStartSession(out uint session, uint flags, StringBuilder key);

            [DllImport("rstrtmgr.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
            private static extern int RmRegisterResources(uint session, uint fileCount,
                [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPWStr)] string[] files,
                uint processCount, ProcessIdentity[] processes, uint serviceCount, IntPtr serviceNames);

            [DllImport("rstrtmgr.dll")]
            private static extern int RmGetList(uint session, out uint needed, ref uint count, IntPtr entries, out uint rebootReasons);

            [DllImport("rstrtmgr.dll")]
            private static extern int RmShutdown(uint session, uint flags, IntPtr callback);

            [DllImport("rstrtmgr.dll")]
            private static extern int RmEndSession(uint session);
        }

        private static Win32Exception NativeError(int code, string operation)
        {
            var error = new Win32Exception(code);
            error.Data["Operation"] = operation;
            error.Data["WindowsErrorCode"] = code;
            return error;
        }
    }
}
