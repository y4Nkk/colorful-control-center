using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace ColorfulControlCenter.Installer
{
    internal sealed class InstallationRequest
    {
        private const int MaxDirectoryLength = 220;
        private const uint AddFile = 0x0002;
        private const uint AddSubdirectory = 0x0004;
        private const uint BackupSemantics = 0x02000000;

        internal string Directory { get; private set; }
        internal bool ReplacesInstallation { get; private set; }

        internal static InstallationRequest Create(string value, InstallationRequest previous = null,
            bool requireInstalled = false)
        {
            string path = Normalize(value);
            bool replacing;
            try { replacing = CheckTarget(path, previous, requireInstalled); }
            catch (UnauthorizedAccessException error)
            {
                throw new ArgumentException("无法访问此安装位置，请选择可访问的文件夹。", error);
            }
            catch (IOException error)
            {
                throw new ArgumentException("无法检查此安装位置，请确认磁盘和目录仍可用。", error);
            }
            return new InstallationRequest { Directory = path, ReplacesInstallation = replacing };
        }

        private static string Normalize(string value)
        {
            if (value != null && value.Length > MaxDirectoryLength)
                throw new ArgumentException("安装路径最多 " + MaxDirectoryLength + " 个字符，请缩短路径。");
            char driveLetter = String.IsNullOrEmpty(value) ? '\0' : Char.ToUpperInvariant(value[0]);
            if (value == null || value.Length < 3 || driveLetter < 'A' || driveLetter > 'Z' || value[1] != ':'
                || (value[2] != '\\' && value[2] != '/'))
                throw new ArgumentException("请选择本地磁盘中的完整文件夹路径，例如 D:\\Apps\\ColorfulControlCenter。");
            foreach (char character in value)
                if (Char.IsControl(character) || character == '"')
                    throw new ArgumentException("安装路径不能包含引号或控制字符。");
            if (value.IndexOfAny(new[] { '*', '?', '<', '>', '|' }) >= 0 || value.Substring(2).Contains(":"))
                throw new ArgumentException("安装路径包含无效字符，请选择有效文件夹。");
            foreach (string segment in value.Replace('/', '\\').Substring(3).Split('\\'))
                if (segment.EndsWith(" ") || segment.EndsWith("."))
                    throw new ArgumentException("文件夹名称不能以空格或句点结尾。");
            string path = Path.GetFullPath(value).TrimEnd('\\', '/');
            if (path.Length <= 2)
                throw new ArgumentException("请选择独立的安装文件夹，不能直接安装到磁盘根目录。");
            return path;
        }

        private static bool CheckTarget(string path, InstallationRequest previous, bool requireInstalled)
        {
            var drive = new DriveInfo(Path.GetPathRoot(path));
            if (drive.DriveType == DriveType.Network || drive.DriveType == DriveType.CDRom)
                throw new ArgumentException("请选择可写的本地磁盘。");
            if (!drive.IsReady)
                throw new ArgumentException("安装磁盘不可用，请连接磁盘或选择其他本地磁盘。");

            string folder = path;
            FileAttributes? attributes = ExistingAttributes(folder);
            while (!attributes.HasValue)
            {
                folder = Path.GetDirectoryName(folder);
                if (folder == null) throw new ArgumentException("安装磁盘不可用，请选择其他本地磁盘。");
                attributes = ExistingAttributes(folder);
            }
            if ((attributes.Value & FileAttributes.Directory) == 0)
                throw new ArgumentException("安装路径中包含文件，请选择文件夹。");

            bool exists = String.Equals(folder, path, StringComparison.OrdinalIgnoreCase);
            bool replacing = exists && File.Exists(Path.Combine(path, Product.ApplicationFileStem + ".exe"))
                && File.Exists(Path.Combine(path, "uninstall.exe"));
            bool retry = previous != null && String.Equals(previous.Directory, path, StringComparison.OrdinalIgnoreCase);
            if (requireInstalled && (!exists || (!replacing && !retry)))
                throw new ArgumentException("原目录中找不到完整的安装文件。");
            if (exists && !replacing && !retry)
                using (var entries = System.IO.Directory.EnumerateFileSystemEntries(path).GetEnumerator())
                    if (entries.MoveNext())
                        throw new ArgumentException("该文件夹包含其他文件。请为控制台选择独立的空文件夹。");
            CheckWriteAccess(folder, exists);
            return replacing;
        }

        private static FileAttributes? ExistingAttributes(string path)
        {
            try { return File.GetAttributes(path); }
            catch (FileNotFoundException) { return null; }
            catch (DirectoryNotFoundException) { return null; }
        }

        private static void CheckWriteAccess(string folder, bool exists)
        {
            // An existing directory handle tests creation rights without writing probe files.
            using (var handle = CreateFile(folder, exists ? AddFile : AddSubdirectory,
                FileShare.Read | FileShare.Write | FileShare.Delete, IntPtr.Zero, FileMode.Open, BackupSemantics, IntPtr.Zero))
            {
                if (handle.IsInvalid)
                {
                    var error = new Win32Exception(Marshal.GetLastWin32Error());
                    throw new ArgumentException(exists ? "无法写入此文件夹，请选择可写的安装位置。"
                        : "无法在上级目录新建文件夹，请选择可写的安装位置。", error);
                }
            }
        }

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFile(string path, uint access, FileShare shareMode,
            IntPtr security, FileMode creationDisposition, uint flags, IntPtr templateFile);

        internal string EngineArguments()
        {
            // NSIS /D consumes the rest of the command line, including spaces, without quotes.
            return "/S /D=" + Directory;
        }
    }
}
